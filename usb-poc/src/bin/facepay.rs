//! SuiCash payment terminal host daemon.
//!
//! Built on usb-poc card (FeliCa reads) + oracle (challenge/settle/attest),
//! it talks to the terminal (Hi-CARA WebView) over WebSocket and performs
//! on-chain payments from the IDi-derived wallet (via sui-pay.mjs).
//!
//!   [Suica] ⇄ RC-S634 ⇄ facepay ⇄ oracle (IDi auth)
//!                          │
//!                          ├─ sui-pay.mjs: IDi-derived wallet balance / transfer
//!                          └─ WebSocket(:8899) ⇄ Hi-CARA WebView (face auth, gate LCD)
//!
//! The terminal connects to ws://localhost:8899 via adb reverse tcp:8899.
//! host → terminal: card / mode / paymentResult / cardRemoved
//! terminal → host: faceOk / faceNg / enrolled / cancel
//!
//! On startup every feature (FeliCa reads, oracle auth, WS broadcast, payments)
//! is enabled, and the Hi-CARA terminal client (WebView) is auto-launched via adb.
//! The GUI version (facepay-admin) starts this daemon as a child process.
//!
//! Environment variables:
//!   FACEPAY_ORACLE    oracle URL (default https://felica-oracle.serken.tech)
//!   FACEPAY_WS_PORT   WebSocket port (default 8899)
//!   FACEPAY_MERCHANT  merchant address (payment recipient; payments disabled if unset)
//!   FACEPAY_SUI_HELPER path to sui-pay.mjs (resolved automatically by default)
//!   SUI_RPC           fullnode RPC (passed through to sui-pay.mjs)
//!   FACEPAY_AUTOLAUNCH  terminal auto-launch (default 1; 0 disables)
//!   FACEPAY_UI_PORT     UI server port the terminal loads (default 5173)
//!   FACEPAY_TERMINAL_URL   URL the terminal opens (default built from UI_PORT/WS_PORT)
//!   FACEPAY_TERMINAL_COMPONENT  launch component of the terminal app
//!   ADB               path to the adb binary (default adb)

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
    /// Pending payment amount (MIST). None = balance inquiry mode
    pay_mode: Option<u64>,
    /// IDi of the card currently shown on the terminal
    current_idi: Option<String>,
    /// Senders to every connected client (terminal + admin GUI). Broadcast to all.
    clients: Vec<(u64, Sender<String>)>,
    next_client_id: u64,
    /// Latest card event JSON. Resent on connect so the terminal still reacts
    /// if it starts/reloads while a card is already on the reader.
    last_card_json: Option<String>,
    /// Latest payment result JSON (for the admin GUI)
    last_payment_json: Option<String>,
    /// Payment-in-progress flag. Prevents double payments/races from repeated faceOk
    paying: bool,
}
type State = Arc<Mutex<Shared>>;

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Resolves the location of sui-pay.mjs independently of cwd.
/// Env var wins; otherwise search cwd / repo layout / relative to the executable.
fn resolve_helper() -> String {
    if let Ok(p) = std::env::var("FACEPAY_SUI_HELPER") {
        return p;
    }
    let mut candidates: Vec<std::path::PathBuf> = vec![
        "facepay/sui-pay.mjs".into(),
        "usb-poc/facepay/sui-pay.mjs".into(),
    ];
    // ../../facepay/sui-pay.mjs as seen from the executable (usb-poc/target/debug/facepay)
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
        if cfg.merchant.is_empty() { "(unset: payments disabled)" } else { &cfg.merchant },
        cfg.sui_helper,
    );

    {
        let (state, cfg) = (state.clone(), cfg.clone());
        thread::spawn(move || ws_server(state, cfg));
    }
    // Terminal auto-launch (adb reverse + am start), once the WS listener is up.
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

// ----------------------------------------------------------------- send to terminal

/// Broadcast to every connected client (terminal, admin GUI)
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

/// Status event for the admin GUI
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
    eprintln!("WS listening on {addr} (terminal connects via adb reverse tcp:{})", cfg.ws_port);
    for stream in listener.incoming().flatten() {
        let (state, cfg) = (state.clone(), cfg.clone());
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
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(100)))?;
    eprintln!("client connected");

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
        // If a card is already present, re-notify right after connecting
        if let Some(card) = card {
            let _ = ws.send(tungstenite::Message::Text(card));
        }
        if let Some(payment) = payment {
            let _ = ws.send(tungstenite::Message::Text(payment));
        }
    }
    // Connection count changed, so broadcast status to the other clients too
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

    // Disconnected: remove this client
    {
        let mut s = state.lock().unwrap();
        s.clients.retain(|(cid, _)| *cid != id);
    }
    broadcast_status(&state);
    result
}

/// Handles reports from the terminal (faceOk in payment mode triggers a transfer)
fn handle_report(text: &str, state: &State, cfg: &Arc<Config>) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    eprintln!("terminal → host: {t}");
    match t {
        "faceOk" => {
            // Don't start again while paying (prevents double payment / coin races from repeated faceOk)
            let go = {
                let mut s = state.lock().unwrap();
                if s.paying {
                    None
                } else if let (Some(amount), Some(idi)) = (s.pay_mode, s.current_idi.clone()) {
                    s.paying = true;
                    Some((amount, idi))
                } else {
                    None
                }
            };
            if let Some((amount, idi)) = go {
                let (state, cfg) = (state.clone(), cfg.clone());
                thread::spawn(move || run_payment(&state, &cfg, &idi, amount));
            }
        }
        // Operator commands from the admin GUI {"type":"op","cmd":"pay"|"idle"|"status","amount":<MIST>}
        "op" => {
            let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
            match cmd {
                "pay" => {
                    // amount accepts MIST (number) or SUI (the "sui" field)
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

/// Sets the pending payment amount and broadcasts mode and status to terminal/GUI
fn set_pay_mode(state: &State, mist: Option<u64>) {
    state.lock().unwrap().pay_mode = mist;
    send_terminal(state, mode_json(mist));
    broadcast_status(state);
}

/// Executes the payment (transfer) and returns paymentResult to the terminal
fn run_payment(state: &State, cfg: &Arc<Config>, idi: &str, amount: u64) {
    if cfg.merchant.is_empty() {
        emit_payment(state, false, r#"{"type":"paymentResult","ok":false,"amount":"0","balanceAfter":"0","error":"Merchant address not set"}"#.to_string());
        return;
    }
    // Check the balance before transferring; if short, report "Insufficient balance".
    // Sui charges gas even for failed txs, so if the balance can't cover amount + gas
    // reserve, reject without sending (avoids burning gas: failed yet balance drops).
    const GAS_RESERVE_MIST: u128 = 20_000_000; // matches the gas budget in sui-pay.mjs
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance: u128 = bal
        .get("balance")
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if balance < amount as u128 + GAS_RESERVE_MIST {
        emit_payment(
            state,
            false,
            format!(
                r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"{balance}","error":"Insufficient balance"}}"#
            ),
        );
        return;
    }

    let res = sui_helper(cfg, &["pay", idi, &amount.to_string(), &cfg.merchant]);
    let ok = res.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
    let msg = if ok {
        let after = res.get("balanceAfter").and_then(|x| x.as_str()).unwrap_or("0");
        let digest = res.get("digest").and_then(|x| x.as_str()).unwrap_or("");
        format!(r#"{{"type":"paymentResult","ok":true,"amount":"{amount}","balanceAfter":"{after}","digest":"{digest}"}}"#)
    } else {
        // Map every transfer failure to a user-facing message; gas/balance shortfall -> "Insufficient balance"
        let raw = res.get("error").and_then(|x| x.as_str()).unwrap_or("");
        eprintln!("payment helper failed (raw error): {raw}");
        let jp = if raw.to_lowercase().contains("insufficient")
            || raw.to_lowercase().contains("gas")
            || raw.to_lowercase().contains("balance")
        {
            "Insufficient balance"
        } else {
            "Payment failed"
        };
        format!(
            r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"{balance}","error":"{jp}"}}"#
        )
    };
    emit_payment(state, ok, msg);
}

/// Broadcasts the payment result and clears the in-progress flag.
/// On success, consumes the pending payment (must be re-armed for the next customer) to prevent double payment.
fn emit_payment(state: &State, ok: bool, json: String) {
    {
        let mut s = state.lock().unwrap();
        s.last_payment_json = Some(json.clone());
        s.paying = false;
        if ok {
            s.pay_mode = None;
        }
    }
    send_terminal(state, json);
    if ok {
        // On success, return to balance inquiry mode (both terminal and GUI)
        send_terminal(state, mode_json(None));
    }
    broadcast_status(state);
}

// ------------------------------------------------------------- sui-pay.mjs calls

fn sui_helper(cfg: &Config, args: &[&str]) -> serde_json::Value {
    match Command::new("node").arg(&cfg.sui_helper).args(args).output() {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str(s.trim().lines().last().unwrap_or("{}"))
                .unwrap_or_else(|_| serde_json::json!({"ok":false,"error":"helper parse error"}))
        }
        Err(e) => serde_json::json!({"ok": false, "error": format!("failed to run node: {e}")}),
    }
}

// ----------------------------------------------------- terminal (Hi-CARA) auto-launch

/// Auto-launches the terminal client via adb (best effort).
/// On failure it only warns; the daemon itself keeps running.
fn launch_terminal(cfg: &Config) {
    let adb = env("ADB", "adb");
    let ui_port: u16 = env("FACEPAY_UI_PORT", "5173").parse().unwrap_or(5173);
    let component = env(
        "FACEPAY_TERMINAL_COMPONENT",
        "jp.serkenn.hicara.suicashui/.MainActivity",
    );
    // Bridge the terminal's localhost:{ws_port} to the host (WS); same for the UI server.
    let ws_arg = format!("tcp:{}", cfg.ws_port);
    let ui_arg = format!("tcp:{ui_port}");
    // URL the terminal opens. ?ws= is URL-encoded (terminal.ts decodes it).
    let default_url = format!(
        "http://localhost:{ui_port}/?ws=ws%3A%2F%2Flocalhost%3A{}",
        cfg.ws_port
    );
    let url = env("FACEPAY_TERMINAL_URL", &default_url);

    // Check that a terminal is connected
    let devices = Command::new(&adb).arg("devices").output();
    match &devices {
        Ok(o) if String::from_utf8_lossy(&o.stdout).lines().skip(1).any(|l| l.contains("device")) => {}
        Ok(_) => {
            eprintln!("terminal auto-launch: no adb device found (if connecting manually, restart after adb)");
            return;
        }
        Err(e) => {
            eprintln!("terminal auto-launch: cannot run adb ({e}) — skipping");
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
            Ok(o) if o.status.success() => eprintln!("terminal auto-launch: {label} OK"),
            Ok(o) => eprintln!(
                "terminal auto-launch: {label} failed {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => eprintln!("terminal auto-launch: {label} cannot run ({e})"),
        }
    }
    eprintln!("terminal auto-launch: done ({url})");
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
                    println!("→ awaiting payment {sui} SUI ({mist} MIST)");
                } else {
                    println!("usage: pay <SUI>  e.g. pay 0.3");
                }
            }
            Some("idle") => {
                set_pay_mode(&state, None);
                println!("→ balance inquiry mode");
            }
            Some("status") => {
                let s = state.lock().unwrap();
                println!(
                    "mode={} card={} clients={} (helper={})",
                    match s.pay_mode {
                        Some(m) => format!("payment {m} MIST"),
                        None => "balance".into(),
                    },
                    s.current_idi.clone().unwrap_or_else(|| "(none)".into()),
                    s.clients.len(),
                    cfg.sui_helper,
                );
            }
            Some("quit") | Some("exit") => std::process::exit(0),
            Some(other) => println!("unknown command: {other}"),
            None => {}
        }
    }
}

// ------------------------------------------------------------- FeliCa read loop

fn card_loop(state: State, cfg: Arc<Config>) {
    let oracle = match Oracle::new(cfg.oracle.clone()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("oracle init failed: {e:#}");
            return;
        }
    };
    match oracle.ping() {
        Ok(p) => eprintln!("oracle ping: {p:?}"),
        Err(e) => eprintln!("oracle ping failed (continuing): {e:#}"),
    }
    eprintln!("waiting for card… (RC-S634)");

    let mut last_idm = String::new();
    loop {
        let t0 = std::time::Instant::now();
        match Card::poll(ReaderPreference::ForcePort100, SYSTEM_CODE, REQUEST_CODE, TIME_SLOTS, POLL_WAIT_SECS)
        {
            Ok(mut card) => {
                let idm = hex::encode(card.idm);
                if idm == last_idm {
                    // Same card still on the reader; reopen less often
                    thread::sleep(Duration::from_millis(800));
                    continue;
                }
                // Notify "authenticating" the moment a tap is detected (oracle auth takes seconds)
                send_terminal(&state, r#"{"type":"detecting"}"#.to_string());
                match attest_with_card(&mut card, &oracle) {
                    Ok(idi) => {
                        last_idm = idm;
                        on_card(&state, &cfg, &idi);
                    }
                    Err(e) => {
                        eprintln!("auth failed: {e:#}");
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
            Err(_) => {
                // Distinguish the kind of Err by elapsed time:
                //  - fast failure (< 700ms): transient reopen failure (flicker) -> not a removal
                //  - slow failure (polling timed out): reader opened but no card -> removal
                // This handles "not present = removed immediately" and "lift and re-tap the
                // same card" quickly and reliably (the same card can be read repeatedly).
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

/// Runs challenge→Auth1→settle→Auth2→attest on a polled card to obtain the IDi
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

/// On card confirmation: look up balance / registration and send a card event to the terminal
fn on_card(state: &State, cfg: &Arc<Config>, idi: &str) {
    eprintln!("card: IDi={idi}");
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance = bal.get("balance").and_then(|x| x.as_str()).unwrap_or("0").to_string();
    // On-chain registration is approximated by balance > 0 (charged via regist-web = registered)
    let registered = balance.parse::<u128>().map(|n| n > 0).unwrap_or(false);
    let json = format!(r#"{{"type":"card","idi":"{idi}","registered":{registered},"balance":"{balance}"}}"#);
    {
        let mut s = state.lock().unwrap();
        s.current_idi = Some(idi.to_string());
        s.last_card_json = Some(json.clone());
    }
    send_terminal(state, json);
}
