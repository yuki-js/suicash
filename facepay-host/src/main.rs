//! SuiCash 決済端末の母艦デーモン。
//!
//! 役割:
//!  1. FeliCa(Suica/PASMO)を RC-S634 で読み、オラクルで IDi を認証
//!  2. IDi 導出ウォレットの残高照会・オンチェーン送金(sui-pay.mjs 経由)
//!  3. 端末(Hi-CARA の WebView)と WebSocket で連携:
//!       母艦→端末: card / mode / paymentResult / cardRemoved
//!       端末→母艦: faceOk / faceNg / enrolled / cancel
//!  4. オペレータ CLI: 残高照会モード / 決済待機(額指定)の切替
//!
//! 端末は adb reverse tcp:<port> 経由で ws://localhost:<port> に接続する。
//!
//! 環境変数:
//!   FACEPAY_ORACLE   オラクル JSON-RPC(既定 felica-oracle.ouchiserver...)
//!   FACEPAY_WS_PORT  WebSocket ポート(既定 8899)
//!   FACEPAY_MERCHANT 店舗アドレス(決済の送金先。必須)
//!   FACEPAY_SUI_HELPER  sui-pay.mjs のパス(既定 ./sui-pay.mjs)
//!   SUI_RPC          fullnode RPC(sui-pay.mjs に引き継ぐ)

use std::io::BufRead;
use std::process::Command;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use felica::felica_standard::{FelicaDriver, FelicaStandard, ServiceCode};
use felica::{open_reader, ReaderPreference};

const R1_HEX: &str = "0011223344556677";
const SYSTEM_CODE: u16 = 0x0003;

struct Config {
    oracle: String,
    ws_port: u16,
    merchant: String,
    sui_helper: String,
}

#[derive(Default)]
struct Shared {
    /// 決済待機額(MIST)。None = 残高照会モード
    pay_mode: Option<u64>,
    /// いま端末に出しているカードの IDi
    current_idi: Option<String>,
    /// 端末への送信口(WS 接続中のみ Some)
    term_tx: Option<Sender<String>>,
}

type State = Arc<Mutex<Shared>>;

fn main() {
    let cfg = Arc::new(Config {
        oracle: env("FACEPAY_ORACLE", "https://felica-oracle.ouchiserver.aokiapp.com/"),
        ws_port: env("FACEPAY_WS_PORT", "8899").parse().unwrap_or(8899),
        merchant: env("FACEPAY_MERCHANT", ""),
        sui_helper: env("FACEPAY_SUI_HELPER", "sui-pay.mjs"),
    });
    let state: State = Arc::new(Mutex::new(Shared::default()));

    eprintln!("facepay-host: oracle={} ws=:{} merchant={}", cfg.oracle, cfg.ws_port,
        if cfg.merchant.is_empty() { "(未設定: 決済不可)" } else { &cfg.merchant });

    // WebSocket サーバ(端末連携)
    {
        let state = state.clone();
        let cfg = cfg.clone();
        thread::spawn(move || ws_server(state, cfg));
    }
    // CLI(オペレータ操作)
    {
        let state = state.clone();
        let cfg = cfg.clone();
        thread::spawn(move || cli_loop(state, cfg));
    }
    // FeliCa 読取ループ(メインスレッド)
    felica_loop(state, cfg);
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

// ----------------------------------------------------------------- 端末へ送信

fn send_terminal(state: &State, json: String) {
    let tx = { state.lock().unwrap().term_tx.clone() };
    if let Some(tx) = tx {
        let _ = tx.send(json);
    }
}

// ------------------------------------------------------------- WebSocket サーバ

fn ws_server(state: State, cfg: Arc<Config>) {
    let addr = format!("127.0.0.1:{}", cfg.ws_port);
    let listener = match std::net::TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("WS bind 失敗 {addr}: {e}");
            return;
        }
    };
    eprintln!("WS 待受 {addr}(端末は adb reverse tcp:{0} で接続)", cfg.ws_port);
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let state = state.clone();
        let cfg = cfg.clone();
        thread::spawn(move || {
            if let Err(e) = ws_handle(stream, state, cfg) {
                eprintln!("WS 接続終了: {e}");
            }
        });
    }
}

fn ws_handle(
    stream: std::net::TcpStream,
    state: State,
    cfg: Arc<Config>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ws = tungstenite::accept(stream)?;
    // 読み取りにタイムアウトを設け、送受信を1ループで回す
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(100)))?;
    eprintln!("端末が接続しました");

    let (tx, rx) = channel::<String>();
    {
        let mut s = state.lock().unwrap();
        s.term_tx = Some(tx);
        // 接続直後に現在モードを通知
        let mode = mode_json(s.pay_mode);
        drop(s);
        let _ = ws.send(tungstenite::Message::Text(mode));
    }

    loop {
        // 送信キューを掃く
        while let Ok(msg) = rx.try_recv() {
            ws.send(tungstenite::Message::Text(msg))?;
        }
        // 受信(タイムアウトあり)
        match ws.read() {
            Ok(tungstenite::Message::Text(t)) => handle_report(&t, &state, &cfg),
            Ok(tungstenite::Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.into()),
        }
    }
    // 切断: 送信口を外す
    let mut s = state.lock().unwrap();
    s.term_tx = None;
    Ok(())
}

fn mode_json(pay_mode: Option<u64>) -> String {
    match pay_mode {
        Some(a) => format!(r#"{{"type":"mode","payment":{{"amount":"{a}"}}}}"#),
        None => r#"{"type":"mode","payment":null}"#.to_string(),
    }
}

/// 端末からのレポート処理
fn handle_report(text: &str, state: &State, cfg: &Arc<Config>) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    eprintln!("端末→母艦: {t}");
    match t {
        "faceOk" => {
            let (pay_mode, idi) = {
                let s = state.lock().unwrap();
                (s.pay_mode, s.current_idi.clone())
            };
            if let (Some(amount), Some(idi)) = (pay_mode, idi) {
                // 決済モード: 送金を別スレッドで実行
                let state = state.clone();
                let cfg = cfg.clone();
                thread::spawn(move || run_payment(&state, &cfg, &idi, amount));
            }
            // 残高照会モードは端末側が残高を表示済み。母艦は何もしない
        }
        _ => {}
    }
}

/// 決済(送金)を実行し paymentResult を端末へ返す
fn run_payment(state: &State, cfg: &Arc<Config>, idi: &str, amount: u64) {
    if cfg.merchant.is_empty() {
        send_terminal(
            state,
            r#"{"type":"paymentResult","ok":false,"amount":"0","balanceAfter":"0","error":"店舗アドレス未設定"}"#.to_string(),
        );
        return;
    }
    let res = sui_helper(cfg, &["pay", idi, &amount.to_string(), &cfg.merchant]);
    let ok = res.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
    let msg = if ok {
        let after = res.get("balanceAfter").and_then(|x| x.as_str()).unwrap_or("0");
        let digest = res.get("digest").and_then(|x| x.as_str()).unwrap_or("");
        format!(
            r#"{{"type":"paymentResult","ok":true,"amount":"{amount}","balanceAfter":"{after}","digest":"{digest}"}}"#
        )
    } else {
        let err = res.get("error").and_then(|x| x.as_str()).unwrap_or("送金失敗");
        format!(
            r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"0","error":{}}}"#,
            serde_json::to_string(err).unwrap()
        )
    };
    send_terminal(state, msg);
}

// ------------------------------------------------------------- sui-pay.mjs 呼出

fn sui_helper(cfg: &Config, args: &[&str]) -> serde_json::Value {
    let out = Command::new("node").arg(&cfg.sui_helper).args(args).output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(s.trim().lines().last().unwrap_or("{}"))
                .unwrap_or_else(|_| serde_json::json!({"ok":false,"error":"helper parse error"}))
        }
        Err(e) => serde_json::json!({"ok": false, "error": format!("node 実行失敗: {e}")}),
    }
}

// ------------------------------------------------------------------- CLI

fn cli_loop(state: State, cfg_for_cli: Arc<Config>) {
    eprintln!("CLI: pay <SUI> | idle | testcard <idi> | remove | status | quit");
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        let mut it = line.split_whitespace();
        match it.next() {
            Some("pay") => {
                if let Some(sui) = it.next().and_then(|x| x.parse::<f64>().ok()) {
                    let mist = (sui * 1e9).round() as u64;
                    {
                        let mut s = state.lock().unwrap();
                        s.pay_mode = Some(mist);
                    }
                    send_terminal(&state, mode_json(Some(mist)));
                    println!("→ 決済待機 {sui} SUI ({mist} MIST)");
                } else {
                    println!("使い方: pay <SUI>  例) pay 0.3");
                }
            }
            Some("idle") => {
                {
                    state.lock().unwrap().pay_mode = None;
                }
                send_terminal(&state, mode_json(None));
                println!("→ 残高照会モード");
            }
            Some("status") => {
                let s = state.lock().unwrap();
                println!(
                    "mode={} card={} terminal={}",
                    match s.pay_mode {
                        Some(m) => format!("決済 {} MIST", m),
                        None => "残高照会".into(),
                    },
                    s.current_idi.clone().unwrap_or_else(|| "(なし)".into()),
                    if s.term_tx.is_some() { "接続中" } else { "未接続" }
                );
            }
            Some("testcard") => {
                // リーダー無しで検証・デモするための擬似カード投入
                if let Some(idi) = it.next() {
                    let idi = idi.to_string();
                    println!("→ 擬似カード投入 idi={idi}");
                    let state2 = state.clone();
                    let cfg2 = cfg_for_cli.clone();
                    thread::spawn(move || on_card(&state2, &cfg2, &idi));
                } else {
                    println!("使い方: testcard <idiHex>  例) testcard 05d5807e28260205");
                }
            }
            Some("remove") => {
                {
                    state.lock().unwrap().current_idi = None;
                }
                send_terminal(&state, r#"{"type":"cardRemoved"}"#.to_string());
                println!("→ カード離脱(擬似)");
            }
            Some("quit") | Some("exit") => std::process::exit(0),
            Some(other) => println!("不明なコマンド: {other}"),
            None => {}
        }
    }
}

// ------------------------------------------------------------- FeliCa 読取ループ

fn felica_loop(state: State, cfg: Arc<Config>) {
    let mut reader = match open_reader(ReaderPreference::Auto) {
        Ok(r) => r,
        Err(e) => {
            // リーダーが無くても WS/CLI は動かし続ける(testcard で検証・デモ可)
            eprintln!("リーダーを開けません(testcard で擬似投入は可能): {e:?}");
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
        }
    };
    eprintln!("FeliCa リーダー準備完了。カード待ち…");
    let mut last_idm = String::new();
    loop {
        let driver = reader.driver_mut();
        match read_card_idi(driver, &cfg) {
            Ok(Some((idm, idi))) => {
                if idm == last_idm {
                    // 同じカードが載りっぱなし。離れるまで待つ
                    thread::sleep(Duration::from_millis(400));
                    continue;
                }
                last_idm = idm;
                on_card(&state, &cfg, &idi);
            }
            Ok(None) => {
                // カードなし。直前まで載っていたら「離れた」通知
                if !last_idm.is_empty() {
                    last_idm.clear();
                    let mut s = state.lock().unwrap();
                    s.current_idi = None;
                    drop(s);
                    send_terminal(&state, r#"{"type":"cardRemoved"}"#.to_string());
                }
                thread::sleep(Duration::from_millis(300));
            }
            Err(e) => {
                eprintln!("読取エラー: {e}");
                thread::sleep(Duration::from_millis(300));
            }
        }
    }
}

/// カード発見時: 残高照会・登録判定して card イベントを端末へ
fn on_card(state: &State, cfg: &Arc<Config>, idi: &str) {
    eprintln!("カード: IDi={idi}");
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance = bal.get("balance").and_then(|x| x.as_str()).unwrap_or("0").to_string();
    // オンチェーン登録の判定は残高 > 0(regist-web でチャージ済み=登録済み)を代用
    let registered = balance.parse::<u128>().map(|n| n > 0).unwrap_or(false);
    {
        let mut s = state.lock().unwrap();
        s.current_idi = Some(idi.to_string());
    }
    let ev = format!(
        r#"{{"type":"card","idi":"{idi}","registered":{registered},"balance":"{balance}"}}"#
    );
    send_terminal(state, ev);
}

/// 1回ポーリングし、カードがあればオラクルで IDi を得る。無ければ None
fn read_card_idi<D: FelicaDriver + ?Sized>(
    driver: &mut D,
    cfg: &Config,
) -> Result<Option<(String, String)>, Box<dyn std::error::Error>> {
    let (mut felica, _) =
        match FelicaStandard::polling_multi(driver, &["212F", "424F"], SYSTEM_CODE, 0x00, 0x00) {
            Ok(f) => f,
            Err(_) => return Ok(None), // カードなし
        };
    let idm_hex = hex::encode(felica.idm());

    let ch = rpc(cfg, "challenge", serde_json::json!({"idm": idm_hex, "r1": R1_HEX}))?;
    let c1a = hex8(&ch["c1a"])?;
    let areas = u16_list(&ch["areas"]);
    let services: Vec<ServiceCode> = u16_list(&ch["services"]).into_iter().map(ServiceCode::new).collect();

    let (c1b, c2a) = felica.authentication1(&areas, &services, &c1a)?;
    let st = rpc(
        cfg,
        "settle",
        serde_json::json!({"idm": idm_hex, "r1": R1_HEX, "c1b": hex::encode(c1b), "c2a": hex::encode(c2a)}),
    )?;
    let c2b = hex8(&st["c2b"])?;
    let resp = felica.authentication2(&c2b)?;
    let ct = extract_ciphertext(&format!("{resp:?}")).ok_or("auth2 抽出失敗")?;
    let at = rpc(
        cfg,
        "attest",
        serde_json::json!({"idm": idm_hex, "c1b": hex::encode(c1b), "c2a": hex::encode(c2a), "auth2": hex::encode(&ct)}),
    )?;
    let idi = at["idi"].as_str().ok_or("idi なし")?.to_string();
    Ok(Some((idm_hex, idi)))
}

// ------------------------------------------------------------- オラクル RPC / util

fn rpc(cfg: &Config, method: &str, params: serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    let resp: serde_json::Value = ureq::post(&cfg.oracle)
        .timeout(Duration::from_secs(30))
        .send_json(body)?
        .into_json()?;
    if let Some(err) = resp.get("error") {
        return Err(format!("oracle {method}: {err}").into());
    }
    resp.get("result").cloned().ok_or_else(|| "no result".into())
}

fn hex8(v: &serde_json::Value) -> Result<[u8; 8], Box<dyn std::error::Error>> {
    let s = v.as_str().ok_or("hex string 期待")?;
    hex::decode(s)?.try_into().map_err(|_| "not 8 bytes".into())
}

fn u16_list(v: &serde_json::Value) -> Vec<u16> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u16)).collect())
        .unwrap_or_default()
}

/// Authentication2Response の暗号文(private)を Debug 表示から取り出す
fn extract_ciphertext(dbg: &str) -> Option<Vec<u8>> {
    let start = dbg.find('[')?;
    let end = dbg[start..].find(']')? + start;
    let bytes: Vec<u8> = dbg[start + 1..end]
        .split(',')
        .filter_map(|t| t.trim().parse::<u8>().ok())
        .collect();
    if bytes.len() == 32 {
        Some(bytes)
    } else {
        None
    }
}
