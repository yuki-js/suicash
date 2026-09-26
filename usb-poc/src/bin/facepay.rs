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
//! 起動すると全機能(FeliCa読取・オラクル認証・WS配信・決済)が有効化され、
//! さらに adb 経由で Hi-CARA 端末クライアント(WebView)を自動起動する。
//! GUI 版(facepay-admin)はこのデーモンを子プロセスとして起動する。
//!
//! 環境変数:
//!   FACEPAY_ORACLE    オラクル URL(既定 https://felica-oracle.serken.tech)
//!   FACEPAY_WS_PORT   WebSocket ポート(既定 8899)
//!   FACEPAY_MERCHANT  店舗アドレス(決済の送金先。未設定だと決済不可)
//!   FACEPAY_SUI_HELPER sui-pay.mjs のパス(既定は自動解決)
//!   SUI_RPC           fullnode RPC(sui-pay.mjs へ引継)
//!   SUICASH_GATE_PKG / SUICASH_GATE_OBJ
//!                     オンチェーン ZK ゲート(felica_oracle パッケージと共有
//!                     Gate オブジェクト)。sui-pay.mjs へ引継。未設定なら
//!                     sui-pay.mjs 隣の gate.json が使われる
//!
//! 決済フロー: カードタップ時にオラクルが Groth16 証明を発行し、決済 PTB の
//! 先頭で suicash_gate::verify がそれをオンチェーン検証する。検証に失敗すると
//! 送金ごとアボートする(証明なしの決済経路はない)。証明は r1 に束縛され
//! オンチェーンで burn されるため一度きり。次の決済は再タッチが必要。
//!   FACEPAY_AUTOLAUNCH  端末オートローンチ(既定 1。0 で無効)
//!   FACEPAY_UI_PORT     端末が読む UI 配信ポート(既定 5173)
//!   FACEPAY_TERMINAL_URL   端末に開かせる URL(既定は UI_PORT/WS_PORT から生成)
//!   FACEPAY_TERMINAL_COMPONENT  端末アプリの起動コンポーネント
//!   ADB               adb バイナリのパス(既定 adb)

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
    /// いま端末に出しているカードの attestation JSON(sui-pay.mjs へ渡す)。
    /// オンチェーンの gate が r1 を burn するため一度きり: 決済開始時に
    /// take され、次の決済にはカードの再タッチ(再 attest)が必要。
    current_att: Option<String>,
    /// 接続中の全クライアント(端末 + 管理GUI)への送信口。全員に配信する。
    clients: Vec<(u64, Sender<String>)>,
    next_client_id: u64,
    /// 直近の card イベント JSON。接続時に再送し、カードが載ったまま
    /// 端末が起動/リロードしても反応するようにする。
    last_card_json: Option<String>,
    /// 直近の決済結果 JSON(管理GUI 表示用)
    last_payment_json: Option<String>,
}
type State = Arc<Mutex<Shared>>;

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// sui-pay.mjs の場所を cwd に依存せず解決する。
/// 環境変数優先。なければ cwd / リポジトリ配置 / 実行ファイル相対で探す。
fn resolve_helper() -> String {
    if let Ok(p) = std::env::var("FACEPAY_SUI_HELPER") {
        return p;
    }
    let mut candidates: Vec<std::path::PathBuf> = vec![
        "facepay/sui-pay.mjs".into(),
        "usb-poc/facepay/sui-pay.mjs".into(),
    ];
    // 実行ファイル(usb-poc/target/debug/facepay)から見た ../../facepay/sui-pay.mjs
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("../../facepay/sui-pay.mjs"));
            candidates.push(dir.join("../../../usb-poc/facepay/sui-pay.mjs"));
        }
    }
    for c in &candidates {
        if c.exists() {
            return c.to_string_lossy().into_owned();
        }
    }
    "facepay/sui-pay.mjs".to_string()
}

fn main() {
    let cfg = Arc::new(Config {
        oracle: env("FACEPAY_ORACLE", "https://felica-oracle.serken.tech"),
        ws_port: env("FACEPAY_WS_PORT", "8899").parse().unwrap_or(8899),
        merchant: env("FACEPAY_MERCHANT", ""),
        sui_helper: resolve_helper(),
    });
    let state: State = Arc::new(Mutex::new(Shared::default()));

    eprintln!(
        "facepay: oracle={} ws=:{} merchant={} helper={}",
        cfg.oracle,
        cfg.ws_port,
        if cfg.merchant.is_empty() { "(未設定: 決済不可)" } else { &cfg.merchant },
        cfg.sui_helper,
    );

    {
        let (state, cfg) = (state.clone(), cfg.clone());
        thread::spawn(move || ws_server(state, cfg));
    }
    // 端末オートローンチ(adb reverse + am start)。WS 待受が立ってから。
    if env("FACEPAY_AUTOLAUNCH", "1") != "0" {
        let cfg = cfg.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(700));
            launch_terminal(&cfg);
        });
    }
    {
        let (state, cfg) = (state.clone(), cfg.clone());
        thread::spawn(move || cli_loop(state, cfg));
    }
    card_loop(state, cfg);
}

// ----------------------------------------------------------------- 端末へ送信

/// 接続中の全クライアント(端末・管理GUI)へ配信
fn send_terminal(state: &State, json: String) {
    let txs: Vec<Sender<String>> = {
        let s = state.lock().unwrap();
        s.clients.iter().map(|(_, tx)| tx.clone()).collect()
    };
    for tx in txs {
        let _ = tx.send(json.clone());
    }
}

fn mode_json(pay_mode: Option<u64>) -> String {
    match pay_mode {
        Some(a) => format!(r#"{{"type":"mode","payment":{{"amount":"{a}"}}}}"#),
        None => r#"{"type":"mode","payment":null}"#.to_string(),
    }
}

/// 管理GUI 向けの状態イベント
fn status_json(s: &Shared) -> String {
    let mode = match s.pay_mode {
        Some(a) => format!(r#""payment","amount":"{a}""#),
        None => r#""idle","amount":null"#.to_string(),
    };
    let idi = s
        .current_idi
        .as_deref()
        .map(|x| format!("\"{x}\""))
        .unwrap_or_else(|| "null".to_string());
    format!(
        r#"{{"type":"status","mode":{mode},"idi":{idi},"clients":{}}}"#,
        s.clients.len()
    )
}

fn broadcast_status(state: &State) {
    let json = { status_json(&state.lock().unwrap()) };
    send_terminal(state, json);
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
    eprintln!("クライアント接続");

    let (tx, rx) = channel::<String>();
    let id;
    {
        let mut s = state.lock().unwrap();
        id = s.next_client_id;
        s.next_client_id += 1;
        s.clients.push((id, tx));
        let mode = mode_json(s.pay_mode);
        let card = s.last_card_json.clone();
        let payment = s.last_payment_json.clone();
        let status = status_json(&s);
        drop(s);
        let _ = ws.send(tungstenite::Message::Text(mode));
        let _ = ws.send(tungstenite::Message::Text(status));
        // カードが既に載っているなら接続直後に再通知
        if let Some(card) = card {
            let _ = ws.send(tungstenite::Message::Text(card));
        }
        if let Some(payment) = payment {
            let _ = ws.send(tungstenite::Message::Text(payment));
        }
    }
    // 接続数が変わったので他クライアントにも status を配信
    broadcast_status(&state);

    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
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
        Ok(())
    })();

    // 切断: このクライアントを外す
    {
        let mut s = state.lock().unwrap();
        s.clients.retain(|(cid, _)| *cid != id);
    }
    broadcast_status(&state);
    result
}

/// 端末からのレポート処理(faceOk で決済モードなら送金)
fn handle_report(text: &str, state: &State, cfg: &Arc<Config>) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    eprintln!("端末→母艦: {t}");
    match t {
        "faceOk" => {
            // attestation は take で取り出す(オンチェーンで r1 が burn される
            // ため一度きり)。決済に入らないときは残しておく。
            let job = {
                let mut s = state.lock().unwrap();
                match (s.pay_mode, s.current_idi.clone()) {
                    (Some(amount), Some(idi)) => Some((amount, idi, s.current_att.take())),
                    _ => None,
                }
            };
            if let Some((amount, idi, att)) = job {
                let (state, cfg) = (state.clone(), cfg.clone());
                thread::spawn(move || run_payment(&state, &cfg, &idi, amount, att));
            }
        }
        // 管理GUI からの操作コマンド {"type":"op","cmd":"pay"|"idle"|"status","amount":<MIST>}
        "op" => {
            let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
            match cmd {
                "pay" => {
                    // amount は MIST(数値) か SUI(文字列 sui) を許容
                    let mist = v
                        .get("amount")
                        .and_then(|x| x.as_u64())
                        .or_else(|| v.get("sui").and_then(|x| x.as_f64()).map(|s| (s * 1e9).round() as u64));
                    if let Some(mist) = mist {
                        set_pay_mode(state, Some(mist));
                    }
                }
                "idle" => set_pay_mode(state, None),
                "status" => broadcast_status(state),
                _ => {}
            }
        }
        _ => {}
    }
}

/// 決済待機額を設定して端末・GUI に mode と status を配信
fn set_pay_mode(state: &State, mist: Option<u64>) {
    state.lock().unwrap().pay_mode = mist;
    send_terminal(state, mode_json(mist));
    broadcast_status(state);
}

/// 決済(送金)を実行し paymentResult を端末へ返す。
///
/// 送金 PTB の先頭で attestation の Groth16 証明をオンチェーン検証する
/// (felica_oracle::suicash_gate::verify)。証明が通らなければ送金ごと
/// アボートするので、ZK 検証を通らない決済経路は存在しない。
fn run_payment(state: &State, cfg: &Arc<Config>, idi: &str, amount: u64, att: Option<String>) {
    if cfg.merchant.is_empty() {
        emit_payment(state, r#"{"type":"paymentResult","ok":false,"amount":"0","balanceAfter":"0","error":"店舗アドレス未設定"}"#.to_string());
        return;
    }
    let Some(att) = att else {
        emit_payment(state, format!(
            r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"0","error":"認証情報がありません。カードを再タッチしてください"}}"#
        ));
        return;
    };
    // 送金前に残高を確認。足りなければ日本語で「残高がありません」
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance: u128 = bal
        .get("balance")
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if balance < amount as u128 {
        emit_payment(
            state,
            format!(
                r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"{balance}","error":"残高がありません"}}"#
            ),
        );
        return;
    }

    let res = sui_helper_env(
        cfg,
        &["pay", idi, &amount.to_string(), &cfg.merchant],
        &[("SUICASH_ATTESTATION", &att)],
    );
    let ok = res.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
    let msg = if ok {
        let after = res.get("balanceAfter").and_then(|x| x.as_str()).unwrap_or("0");
        let digest = res.get("digest").and_then(|x| x.as_str()).unwrap_or("");
        format!(r#"{{"type":"paymentResult","ok":true,"amount":"{amount}","balanceAfter":"{after}","digest":"{digest}"}}"#)
    } else {
        // 送金失敗はすべて日本語に。ガス/残高不足は「残高がありません」
        let raw = res.get("error").and_then(|x| x.as_str()).unwrap_or("");
        let jp = if raw.to_lowercase().contains("insufficient")
            || raw.to_lowercase().contains("gas")
            || raw.to_lowercase().contains("balance")
        {
            "残高がありません"
        } else {
            "決済に失敗しました"
        };
        format!(
            r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"{balance}","error":"{jp}"}}"#
        )
    };
    emit_payment(state, msg);
}

/// 決済結果を配信し、管理GUI 再表示用に保持する
fn emit_payment(state: &State, json: String) {
    state.lock().unwrap().last_payment_json = Some(json.clone());
    send_terminal(state, json);
}

// ------------------------------------------------------------- sui-pay.mjs 呼出

fn sui_helper(cfg: &Config, args: &[&str]) -> serde_json::Value {
    sui_helper_env(cfg, args, &[])
}

/// 追加の環境変数つきで sui-pay.mjs を呼ぶ。attestation は引数だと ps に
/// 露出し長さ制限も踏むので、環境変数で渡す。
fn sui_helper_env(cfg: &Config, args: &[&str], envs: &[(&str, &str)]) -> serde_json::Value {
    let mut cmd = Command::new("node");
    cmd.arg(&cfg.sui_helper).args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    match cmd.output() {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(s.trim().lines().last().unwrap_or("{}"))
                .unwrap_or_else(|_| serde_json::json!({"ok":false,"error":"helper parse error"}))
        }
        Err(e) => serde_json::json!({"ok": false, "error": format!("node 実行失敗: {e}")}),
    }
}

// ----------------------------------------------------- 端末(Hi-CARA)オートローンチ

/// adb 経由で端末クライアントを自動起動する(ベストエフォート)。
/// 失敗しても警告だけ出してデーモン本体は継続する。
fn launch_terminal(cfg: &Config) {
    let adb = env("ADB", "adb");
    let ui_port: u16 = env("FACEPAY_UI_PORT", "5173").parse().unwrap_or(5173);
    let component = env(
        "FACEPAY_TERMINAL_COMPONENT",
        "jp.serkenn.hicara.suicashui/.MainActivity",
    );
    // 端末の localhost:{ws_port} を母艦へ橋渡し(WS)。UI 配信も同様に橋渡し。
    let ws_arg = format!("tcp:{}", cfg.ws_port);
    let ui_arg = format!("tcp:{ui_port}");
    // 端末が開く URL。?ws= は URL エンコードして渡す(terminal.ts が復号する)。
    let default_url = format!(
        "http://localhost:{ui_port}/?ws=ws%3A%2F%2Flocalhost%3A{}",
        cfg.ws_port
    );
    let url = env("FACEPAY_TERMINAL_URL", &default_url);

    // 端末が接続されているか確認
    let devices = Command::new(&adb).arg("devices").output();
    match &devices {
        Ok(o) if String::from_utf8_lossy(&o.stdout).lines().skip(1).any(|l| l.contains("device")) => {}
        Ok(_) => {
            eprintln!("端末オートローンチ: adb デバイス未検出(手動接続時は adb 後に再起動)");
            return;
        }
        Err(e) => {
            eprintln!("端末オートローンチ: adb 実行不可({e}) — スキップ");
            return;
        }
    }

    let steps: [(&str, Vec<&str>); 3] = [
        ("reverse WS", vec!["reverse", &ws_arg, &ws_arg]),
        ("reverse UI", vec!["reverse", &ui_arg, &ui_arg]),
        (
            "am start",
            vec!["shell", "am", "start", "-n", &component, "-d", &url],
        ),
    ];
    for (label, args) in steps {
        match Command::new(&adb).args(&args).output() {
            Ok(o) if o.status.success() => eprintln!("端末オートローンチ: {label} OK"),
            Ok(o) => eprintln!(
                "端末オートローンチ: {label} 失敗 {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => eprintln!("端末オートローンチ: {label} 実行不可({e})"),
        }
    }
    eprintln!("端末オートローンチ: 完了({url})");
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
                    set_pay_mode(&state, Some(mist));
                    println!("→ 決済待機 {sui} SUI ({mist} MIST)");
                } else {
                    println!("使い方: pay <SUI>  例) pay 0.3");
                }
            }
            Some("idle") => {
                set_pay_mode(&state, None);
                println!("→ 残高照会モード");
            }
            Some("status") => {
                let s = state.lock().unwrap();
                println!(
                    "mode={} card={} clients={} (helper={})",
                    match s.pay_mode {
                        Some(m) => format!("決済 {m} MIST"),
                        None => "残高照会".into(),
                    },
                    s.current_idi.clone().unwrap_or_else(|| "(なし)".into()),
                    s.clients.len(),
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
                    Ok((idi, att_json)) => {
                        last_idm = idm;
                        on_card(&state, &cfg, &idi, att_json);
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
                        s.current_att = None;
                        s.last_card_json = None;
                    }
                    send_terminal(&state, r#"{"type":"cardRemoved"}"#.to_string());
                }
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

/// ポーリング済みカードに対し challenge→Auth1→settle→Auth2→attest を行い、
/// IDi と、オンチェーン ZK 検証に必要な attestation JSON を得る
fn attest_with_card(card: &mut Card, oracle: &Oracle) -> anyhow::Result<(String, String)> {
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
    let att_json = attestation_json(&attest, &r1)?;
    Ok((attest.idi.to_lowercase(), att_json))
}

/// オラクルの座標形式 Groth16 証明を `sui::groth16` が受ける Arkworks 圧縮
/// バイト列へ変換し、sui-pay.mjs へ渡す attestation JSON を組み立てる。
///
/// r1 はこのプロセスの CSPRNG が選んだ値で、証明はそれに束縛されている。
/// オンチェーンの gate が同じ r1 を要求して burn するので、この JSON は
/// 一度しか使えない。
fn attestation_json(att: &usb_poc::oracle::AttestResult, r1: &Block) -> anyhow::Result<String> {
    let wire = prover::Groth16Proof {
        alg: att.proof.alg.clone(),
        a: att.proof.a.clone(),
        b: att.proof.b.clone(),
        c: att.proof.c.clone(),
        public_inputs: att.proof.public_inputs.clone(),
    };
    let proof = prover::proof_compressed_bytes(&wire)
        .map_err(|e| anyhow::anyhow!("proof の圧縮変換に失敗: {e}"))?;
    let pis = prover::public_inputs_bytes(&wire)
        .map_err(|e| anyhow::anyhow!("public inputs の変換に失敗: {e}"))?;
    Ok(serde_json::json!({
        "idi": att.idi.to_lowercase(),
        "attested_at": att.attested_at,
        "r1": hex::encode(r1),
        "proof": hex::encode(proof),
        "public_inputs": hex::encode(pis),
    })
    .to_string())
}

/// カード確定時: 残高照会・登録判定して card イベントを端末へ
fn on_card(state: &State, cfg: &Arc<Config>, idi: &str, att_json: String) {
    eprintln!("カード: IDi={idi}(ZK attestation 取得済み)");
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance = bal.get("balance").and_then(|x| x.as_str()).unwrap_or("0").to_string();
    // オンチェーン登録の判定は残高 > 0(regist-web でチャージ済み=登録済み)を代用
    let registered = balance.parse::<u128>().map(|n| n > 0).unwrap_or(false);
    let json = format!(r#"{{"type":"card","idi":"{idi}","registered":{registered},"balance":"{balance}"}}"#);
    {
        let mut s = state.lock().unwrap();
        s.current_idi = Some(idi.to_string());
        s.current_att = Some(att_json);
        s.last_card_json = Some(json.clone());
    }
    send_terminal(state, json);
}
