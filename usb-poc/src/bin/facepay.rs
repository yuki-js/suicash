//! SuiCash 決済端末の母艦デーモン。
//!
//! usb-poc の card(FeliCa 読取)+ oracle(challenge/settle/attest)を土台に、
//! 端末(Hi-CARA の WebView)と WebSocket で連携し、IDi 導出ウォレットの
//! オンチェーン決済(sui-pay.mjs 経由)を行う。
//!
//!   [Suica] ⇄ RC-S634 ⇄ facepay ⇄ oracle(IDi 認証)
//!                          │
//!                          ├─ sui-pay.mjs: IDi導出ウォレットの残高照会・送金
//!                          └─ WebSocket(:8899) ⇄ Hi-CARA WebView(顔認証・改札LCD)
//!
//! 端末は adb reverse tcp:8899 経由で ws://localhost:8899 に接続する。
//! 母艦→端末: card / mode / paymentResult / cardRemoved
//! 端末→母艦: faceOk / faceNg / enrolled / cancel
//!
//! 環境変数:
//!   FACEPAY_ORACLE    オラクル URL(既定 https://felica-oracle.serken.tech)
//!   FACEPAY_WS_PORT   WebSocket ポート(既定 8899)
//!   FACEPAY_MERCHANT  店舗アドレス(決済の送金先。未設定だと決済不可)
//!   FACEPAY_SUI_HELPER sui-pay.mjs のパス(既定 facepay/sui-pay.mjs)
//!   SUI_RPC           fullnode RPC(sui-pay.mjs へ引継)

use std::io::BufRead;
use std::process::Command;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use felica::felica_standard::ServiceCode;
use felica::ReaderPreference;
use rand::RngCore;
use usb_poc::card::{Block, Card};
use usb_poc::oracle::Oracle;

const SYSTEM_CODE: u16 = 0x0003;
const REQUEST_CODE: u8 = 0x00;
const TIME_SLOTS: u8 = 0x00;
const POLL_WAIT_SECS: u64 = 1;
const AUTH1_TIMEOUT: u16 = 2000;
const AUTH2_TIMEOUT: u16 = 1000;

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
    /// 直近の card イベント JSON。WS 接続時に再送し、カードが載ったまま
    /// 端末が起動/リロードしても反応するようにする。
    last_card_json: Option<String>,
}
type State = Arc<Mutex<Shared>>;

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn main() {
    let cfg = Arc::new(Config {
        oracle: env("FACEPAY_ORACLE", "https://felica-oracle.serken.tech"),
        ws_port: env("FACEPAY_WS_PORT", "8899").parse().unwrap_or(8899),
        merchant: env("FACEPAY_MERCHANT", ""),
        sui_helper: env("FACEPAY_SUI_HELPER", "facepay/sui-pay.mjs"),
    });
    let state: State = Arc::new(Mutex::new(Shared::default()));

    eprintln!(
        "facepay: oracle={} ws=:{} merchant={}",
        cfg.oracle,
        cfg.ws_port,
        if cfg.merchant.is_empty() { "(未設定: 決済不可)" } else { &cfg.merchant }
    );

    {
        let (state, cfg) = (state.clone(), cfg.clone());
        thread::spawn(move || ws_server(state, cfg));
    }
    {
        let (state, cfg) = (state.clone(), cfg.clone());
        thread::spawn(move || cli_loop(state, cfg));
    }
    card_loop(state, cfg);
}

// ----------------------------------------------------------------- 端末へ送信

fn send_terminal(state: &State, json: String) {
    let tx = { state.lock().unwrap().term_tx.clone() };
    if let Some(tx) = tx {
        let _ = tx.send(json);
    }
}

fn mode_json(pay_mode: Option<u64>) -> String {
    match pay_mode {
        Some(a) => format!(r#"{{"type":"mode","payment":{{"amount":"{a}"}}}}"#),
        None => r#"{"type":"mode","payment":null}"#.to_string(),
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
    eprintln!("WS 待受 {addr}(端末は adb reverse tcp:{} で接続)", cfg.ws_port);
    for stream in listener.incoming().flatten() {
        let (state, cfg) = (state.clone(), cfg.clone());
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
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(100)))?;
    eprintln!("端末が接続しました");

    let (tx, rx) = channel::<String>();
    {
        let mut s = state.lock().unwrap();
        s.term_tx = Some(tx);
        let mode = mode_json(s.pay_mode);
        let card = s.last_card_json.clone();
        drop(s);
        let _ = ws.send(tungstenite::Message::Text(mode));
        // カードが既に載っているなら接続直後に再通知
        if let Some(card) = card {
            let _ = ws.send(tungstenite::Message::Text(card));
        }
    }

    loop {
        while let Ok(msg) = rx.try_recv() {
            ws.send(tungstenite::Message::Text(msg))?;
        }
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
    state.lock().unwrap().term_tx = None;
    Ok(())
}

/// 端末からのレポート処理(faceOk で決済モードなら送金)
fn handle_report(text: &str, state: &State, cfg: &Arc<Config>) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    eprintln!("端末→母艦: {t}");
    if t == "faceOk" {
        let (pay_mode, idi) = {
            let s = state.lock().unwrap();
            (s.pay_mode, s.current_idi.clone())
        };
        if let (Some(amount), Some(idi)) = (pay_mode, idi) {
            let (state, cfg) = (state.clone(), cfg.clone());
            thread::spawn(move || run_payment(&state, &cfg, &idi, amount));
        }
    }
}

/// 決済(送金)を実行し paymentResult を端末へ返す
fn run_payment(state: &State, cfg: &Arc<Config>, idi: &str, amount: u64) {
    if cfg.merchant.is_empty() {
        send_terminal(state, r#"{"type":"paymentResult","ok":false,"amount":"0","balanceAfter":"0","error":"店舗アドレス未設定"}"#.to_string());
        return;
    }
    let res = sui_helper(cfg, &["pay", idi, &amount.to_string(), &cfg.merchant]);
    let ok = res.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
    let msg = if ok {
        let after = res.get("balanceAfter").and_then(|x| x.as_str()).unwrap_or("0");
        let digest = res.get("digest").and_then(|x| x.as_str()).unwrap_or("");
        format!(r#"{{"type":"paymentResult","ok":true,"amount":"{amount}","balanceAfter":"{after}","digest":"{digest}"}}"#)
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
    match Command::new("node").arg(&cfg.sui_helper).args(args).output() {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(s.trim().lines().last().unwrap_or("{}"))
                .unwrap_or_else(|_| serde_json::json!({"ok":false,"error":"helper parse error"}))
        }
        Err(e) => serde_json::json!({"ok": false, "error": format!("node 実行失敗: {e}")}),
    }
}

// ------------------------------------------------------------------- CLI

fn cli_loop(state: State, cfg: Arc<Config>) {
    eprintln!("CLI: pay <SUI> | idle | status | quit");
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("pay") => {
                if let Some(sui) = it.next().and_then(|x| x.parse::<f64>().ok()) {
                    let mist = (sui * 1e9).round() as u64;
                    state.lock().unwrap().pay_mode = Some(mist);
                    send_terminal(&state, mode_json(Some(mist)));
                    println!("→ 決済待機 {sui} SUI ({mist} MIST)");
                } else {
                    println!("使い方: pay <SUI>  例) pay 0.3");
                }
            }
            Some("idle") => {
                state.lock().unwrap().pay_mode = None;
                send_terminal(&state, mode_json(None));
                println!("→ 残高照会モード");
            }
            Some("status") => {
                let s = state.lock().unwrap();
                println!(
                    "mode={} card={} terminal={} (helper={})",
                    match s.pay_mode {
                        Some(m) => format!("決済 {m} MIST"),
                        None => "残高照会".into(),
                    },
                    s.current_idi.clone().unwrap_or_else(|| "(なし)".into()),
                    if s.term_tx.is_some() { "接続中" } else { "未接続" },
                    cfg.sui_helper,
                );
            }
            Some("quit") | Some("exit") => std::process::exit(0),
            Some(other) => println!("不明なコマンド: {other}"),
            None => {}
        }
    }
}

// ------------------------------------------------------------- FeliCa 読取ループ

fn card_loop(state: State, cfg: Arc<Config>) {
    let oracle = match Oracle::new(cfg.oracle.clone()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("オラクル初期化失敗: {e:#}");
            return;
        }
    };
    match oracle.ping() {
        Ok(p) => eprintln!("オラクル ping: {p:?}"),
        Err(e) => eprintln!("オラクル ping 失敗(続行): {e:#}"),
    }
    eprintln!("カード待ち…(RC-S634)");

    let mut last_idm = String::new();
    loop {
        let t0 = std::time::Instant::now();
        match Card::poll(ReaderPreference::ForcePort100, SYSTEM_CODE, REQUEST_CODE, TIME_SLOTS, POLL_WAIT_SECS)
        {
            Ok(mut card) => {
                let idm = hex::encode(card.idm);
                if idm == last_idm {
                    // 同じカードが載りっぱなし。再オープン頻度を下げる
                    thread::sleep(Duration::from_millis(800));
                    continue;
                }
                // タップを検出した瞬間に「認証中」を即通知(オラクル認証は数秒かかる)
                send_terminal(&state, r#"{"type":"detecting"}"#.to_string());
                match attest_with_card(&mut card, &oracle) {
                    Ok(idi) => {
                        last_idm = idm;
                        on_card(&state, &cfg, &idi);
                    }
                    Err(e) => {
                        eprintln!("認証失敗: {e:#}");
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
            Err(_) => {
                // Err の種類を経過時間で区別する:
                //  - 速い失敗(< 700ms): 再オープンの一過性失敗(チラつき)→ 離脱としない
                //  - 遅い失敗(ポーリング満了): リーダーは開けたがカード無し → 離脱
                // これで「載っていない=即離脱」「同じカードのリフト→再タッチ」を
                // 速く確実に扱える(同じカードを連続で読める)。
                let slow = t0.elapsed() >= Duration::from_millis(700);
                if slow && !last_idm.is_empty() {
                    last_idm.clear();
                    {
                        let mut s = state.lock().unwrap();
                        s.current_idi = None;
                        s.last_card_json = None;
                    }
                    send_terminal(&state, r#"{"type":"cardRemoved"}"#.to_string());
                }
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

/// ポーリング済みカードに対し challenge→Auth1→settle→Auth2→attest を行い IDi を得る
fn attest_with_card(card: &mut Card, oracle: &Oracle) -> anyhow::Result<String> {
    let idm = card.idm;
    let mut r1: Block = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut r1);

    let ch = oracle.challenge(&hex::encode(idm), &hex::encode(r1))?;
    let services: Vec<ServiceCode> = ch.services.iter().map(|c| ServiceCode::new(*c)).collect();
    let c1a: Block = hex::decode(&ch.c1a)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| anyhow::anyhow!("bad c1a"))?;
    let (c1b, c2a) = card.authentication1(&ch.areas, &services, &c1a, AUTH1_TIMEOUT)?;
    let settle = oracle.settle(&hex::encode(idm), &hex::encode(r1), &hex::encode(c1b), &hex::encode(c2a))?;
    let c2b: Block = hex::decode(&settle.c2b)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| anyhow::anyhow!("bad c2b"))?;
    let auth2 = card.authentication2(&c2b, AUTH2_TIMEOUT)?;
    let attest = oracle.attest(&hex::encode(idm), &hex::encode(c1b), &hex::encode(c2a), &hex::encode(&auth2))?;
    Ok(attest.idi.to_lowercase())
}

/// カード確定時: 残高照会・登録判定して card イベントを端末へ
fn on_card(state: &State, cfg: &Arc<Config>, idi: &str) {
    eprintln!("カード: IDi={idi}");
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance = bal.get("balance").and_then(|x| x.as_str()).unwrap_or("0").to_string();
    // オンチェーン登録の判定は残高 > 0(regist-web でチャージ済み=登録済み)を代用
    let registered = balance.parse::<u128>().map(|n| n > 0).unwrap_or(false);
    let json = format!(r#"{{"type":"card","idi":"{idi}","registered":{registered},"balance":"{balance}"}}"#);
    {
        let mut s = state.lock().unwrap();
        s.current_idi = Some(idi.to_string());
        s.last_card_json = Some(json.clone());
    }
    send_terminal(state, json);
}
