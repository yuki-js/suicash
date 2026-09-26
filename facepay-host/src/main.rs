//! Host PC daemon for the SuiCash payment terminal.
//!
//! Responsibilities:
//!  1. Read FeliCa (Suica/PASMO) with the RC-S634 and authenticate the IDi via the oracle
//!  2. Balance query and on-chain transfer for the IDi-derived wallet (via sui-pay.mjs)
//!  3. Talk to the terminal (Hi-CARA WebView) over WebSocket:
//!       host → terminal: card / mode / paymentResult / cardRemoved
//!       terminal → host: faceOk / faceNg / enrolled / cancel
//!  4. Operator CLI: switch between balance mode / awaiting payment (with amount)
//!
//! The terminal connects to ws://localhost:<port> via adb reverse tcp:<port>.
//!
//! Environment:
//!   FACEPAY_ORACLE   oracle JSON-RPC (default felica-oracle.ouchiserver...)
//!   FACEPAY_WS_PORT  WebSocket port (default 8899)
//!   FACEPAY_MERCHANT merchant address (payment recipient; required)
//!   FACEPAY_SUI_HELPER  path to sui-pay.mjs (default ./sui-pay.mjs)
//!   SUI_RPC          fullnode RPC (passed to sui-pay.mjs)

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
    /// Pending payment amount (MIST). None = balance mode
    pay_mode: Option<u64>,
    /// IDi of the card currently shown on the terminal
    current_idi: Option<String>,
    /// Sender to the terminal (Some only while WS is connected)
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
        if cfg.merchant.is_empty() { "(unset: payments disabled)" } else { &cfg.merchant });

    // WebSocket server (terminal link)
    {
        let state = state.clone();
        let cfg = cfg.clone();
        thread::spawn(move || ws_server(state, cfg));
    }
    // CLI (operator control)
    {
        let state = state.clone();
        let cfg = cfg.clone();
        thread::spawn(move || cli_loop(state, cfg));
    }
    // FeliCa read loop (main thread)
    felica_loop(state, cfg);
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

// ----------------------------------------------------------------- Send to terminal

fn send_terminal(state: &State, json: String) {
    let tx = { state.lock().unwrap().term_tx.clone() };
    if let Some(tx) = tx {
        let _ = tx.send(json);
    }
}

// ------------------------------------------------------------- WebSocket server

fn ws_server(state: State, cfg: Arc<Config>) {
    let addr = format!("127.0.0.1:{}", cfg.ws_port);
    let listener = match std::net::TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("WS bind failed {addr}: {e}");
            return;
        }
    };
    eprintln!("WS listening on {addr} (terminal connects via adb reverse tcp:{0})", cfg.ws_port);
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let state = state.clone();
        let cfg = cfg.clone();
        thread::spawn(move || {
            if let Err(e) = ws_handle(stream, state, cfg) {
                eprintln!("WS connection closed: {e}");
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
    // Put a timeout on reads and handle send/receive in one loop
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(100)))?;
    eprintln!("Terminal connected");

    let (tx, rx) = channel::<String>();
    {
        let mut s = state.lock().unwrap();
        s.term_tx = Some(tx);
        // Announce the current mode right after connecting
        let mode = mode_json(s.pay_mode);
        drop(s);
        let _ = ws.send(tungstenite::Message::Text(mode));
    }

    loop {
        // Drain the send queue
        while let Ok(msg) = rx.try_recv() {
            ws.send(tungstenite::Message::Text(msg))?;
        }
        // Receive (with timeout)
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
    // Disconnected: drop the sender
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

/// Handle reports from the terminal
fn handle_report(text: &str, state: &State, cfg: &Arc<Config>) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    eprintln!("terminal → host: {t}");
    match t {
        "faceOk" => {
            let (pay_mode, idi) = {
                let s = state.lock().unwrap();
                (s.pay_mode, s.current_idi.clone())
            };
            if let (Some(amount), Some(idi)) = (pay_mode, idi) {
                // Payment mode: run the transfer on a separate thread
                let state = state.clone();
                let cfg = cfg.clone();
                thread::spawn(move || run_payment(&state, &cfg, &idi, amount));
            }
            // In balance mode the terminal already shows the balance. Nothing to do on the host
        }
        _ => {}
    }
}

/// Execute the payment (transfer) and send paymentResult back to the terminal
fn run_payment(state: &State, cfg: &Arc<Config>, idi: &str, amount: u64) {
    if cfg.merchant.is_empty() {
        send_terminal(
            state,
            r#"{"type":"paymentResult","ok":false,"amount":"0","balanceAfter":"0","error":"merchant address not set"}"#.to_string(),
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
        let err = res.get("error").and_then(|x| x.as_str()).unwrap_or("transfer failed");
        format!(
            r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"0","error":{}}}"#,
            serde_json::to_string(err).unwrap()
        )
    };
    send_terminal(state, msg);
}

// ------------------------------------------------------------- sui-pay.mjs calls

fn sui_helper(cfg: &Config, args: &[&str]) -> serde_json::Value {
    let out = Command::new("node").arg(&cfg.sui_helper).args(args).output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(s.trim().lines().last().unwrap_or("{}"))
                .unwrap_or_else(|_| serde_json::json!({"ok":false,"error":"helper parse error"}))
        }
        Err(e) => serde_json::json!({"ok": false, "error": format!("failed to run node: {e}")}),
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
                    println!("→ Awaiting payment {sui} SUI ({mist} MIST)");
                } else {
                    println!("Usage: pay <SUI>  e.g. pay 0.3");
                }
            }
            Some("idle") => {
                {
                    state.lock().unwrap().pay_mode = None;
                }
                send_terminal(&state, mode_json(None));
                println!("→ Balance mode");
            }
            Some("status") => {
                let s = state.lock().unwrap();
                println!(
                    "mode={} card={} terminal={}",
                    match s.pay_mode {
                        Some(m) => format!("pay {} MIST", m),
                        None => "balance".into(),
                    },
                    s.current_idi.clone().unwrap_or_else(|| "(none)".into()),
                    if s.term_tx.is_some() { "connected" } else { "disconnected" }
                );
            }
            Some("testcard") => {
                // Simulated card insertion for testing/demos without a reader
                if let Some(idi) = it.next() {
                    let idi = idi.to_string();
                    println!("→ Simulated card inserted idi={idi}");
                    let state2 = state.clone();
                    let cfg2 = cfg_for_cli.clone();
                    thread::spawn(move || on_card(&state2, &cfg2, &idi));
                } else {
                    println!("Usage: testcard <idiHex>  e.g. testcard 05d5807e28260205");
                }
            }
            Some("remove") => {
                {
                    state.lock().unwrap().current_idi = None;
                }
                send_terminal(&state, r#"{"type":"cardRemoved"}"#.to_string());
                println!("→ Card removed (simulated)");
            }
            Some("quit") | Some("exit") => std::process::exit(0),
            Some(other) => println!("Unknown command: {other}"),
            None => {}
        }
    }
}

// ------------------------------------------------------------- FeliCa read loop

fn felica_loop(state: State, cfg: Arc<Config>) {
    let mut reader = match open_reader(ReaderPreference::Auto) {
        Ok(r) => r,
        Err(e) => {
            // Keep WS/CLI running without a reader (testcard still works for testing/demos)
            eprintln!("Cannot open reader (simulated insertion via testcard still available): {e:?}");
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
        }
    };
    eprintln!("FeliCa reader ready. Waiting for a card…");
    let mut last_idm = String::new();
    loop {
        let driver = reader.driver_mut();
        match read_card_idi(driver, &cfg) {
            Ok(Some((idm, idi))) => {
                if idm == last_idm {
                    // Same card still present. Wait until it is removed
                    thread::sleep(Duration::from_millis(400));
                    continue;
                }
                last_idm = idm;
                on_card(&state, &cfg, &idi);
            }
            Ok(None) => {
                // No card. If one was present, send a "removed" notice
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
                eprintln!("Read error: {e}");
                thread::sleep(Duration::from_millis(300));
            }
        }
    }
}

/// On card detection: query balance, check registration, and send a card event to the terminal
fn on_card(state: &State, cfg: &Arc<Config>, idi: &str) {
    eprintln!("Card: IDi={idi}");
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance = bal.get("balance").and_then(|x| x.as_str()).unwrap_or("0").to_string();
    // On-chain registration is approximated by balance > 0 (topped up via regist-web = registered)
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

/// Poll once; if a card is present, get its IDi from the oracle. Otherwise None
fn read_card_idi<D: FelicaDriver + ?Sized>(
    driver: &mut D,
    cfg: &Config,
) -> Result<Option<(String, String)>, Box<dyn std::error::Error>> {
    let (mut felica, _) =
        match FelicaStandard::polling_multi(driver, &["212F", "424F"], SYSTEM_CODE, 0x00, 0x00) {
            Ok(f) => f,
            Err(_) => return Ok(None), // no card
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
    let ct = extract_ciphertext(&format!("{resp:?}")).ok_or("failed to extract auth2")?;
    let at = rpc(
        cfg,
        "attest",
        serde_json::json!({"idm": idm_hex, "c1b": hex::encode(c1b), "c2a": hex::encode(c2a), "auth2": hex::encode(&ct)}),
    )?;
    let idi = at["idi"].as_str().ok_or("missing idi")?.to_string();
    Ok(Some((idm_hex, idi)))
}

// ------------------------------------------------------------- Oracle RPC / util

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
    let s = v.as_str().ok_or("expected hex string")?;
    hex::decode(s)?.try_into().map_err(|_| "not 8 bytes".into())
}

fn u16_list(v: &serde_json::Value) -> Vec<u16> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u16)).collect())
        .unwrap_or_default()
}

/// Extract the (private) ciphertext of Authentication2Response from its Debug output
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
