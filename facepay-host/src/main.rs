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
//!   SUICASH_GATE_PKG / SUICASH_GATE_OBJ
//!                    on-chain ZK gate (felica_oracle package and shared Gate
//!                    object), passed to sui-pay.mjs. If unset, gate.json next
//!                    to sui-pay.mjs is used.
//!
//! Payment flow: on card tap the oracle issues a Groth16 proof; sui-pay.mjs
//! verifies it on-chain (felica_oracle::suicash_gate::verify) at the head of the
//! payment PTB. The proof is bound to a fresh r1 and burned on-chain, so it is
//! single-use and the next payment requires a re-tap.

use std::io::BufRead;
use std::process::Command;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use felica::felica_standard::{FelicaDriver, FelicaStandard, ServiceCode};
use felica::{open_reader, ReaderPreference};
use rand::RngCore;

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
    /// Attestation JSON of the card currently on the terminal (passed to sui-pay.mjs).
    /// Single-use because the on-chain gate burns r1: it is taken when a payment
    /// starts, and the next payment requires a card re-tap (re-attest).
    current_att: Option<String>,
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
            // Take the attestation out (single-use: the on-chain gate burns r1).
            // Leave it in place when not entering a payment.
            let job = {
                let mut s = state.lock().unwrap();
                match (s.pay_mode, s.current_idi.clone()) {
                    (Some(amount), Some(idi)) => Some((amount, idi, s.current_att.take())),
                    _ => None,
                }
            };
            if let Some((amount, idi, att)) = job {
                // Payment mode: run the transfer on a separate thread
                let state = state.clone();
                let cfg = cfg.clone();
                thread::spawn(move || run_payment(&state, &cfg, &idi, amount, att));
            }
            // In balance mode the terminal already shows the balance. Nothing to do on the host
        }
        _ => {}
    }
}

/// Execute the payment (transfer) and send paymentResult back to the terminal.
///
/// The attestation's Groth16 proof is verified on-chain at the head of the
/// transfer PTB (felica_oracle::suicash_gate::verify). If the proof fails the
/// whole transfer aborts, so no payment path bypasses ZK verification.
fn run_payment(state: &State, cfg: &Arc<Config>, idi: &str, amount: u64, att: Option<String>) {
    if cfg.merchant.is_empty() {
        send_terminal(
            state,
            r#"{"type":"paymentResult","ok":false,"amount":"0","balanceAfter":"0","error":"merchant address not set"}"#.to_string(),
        );
        return;
    }
    let Some(att) = att else {
        send_terminal(
            state,
            format!(
                r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"0","error":"No attestation. Please tap your card again"}}"#
            ),
        );
        return;
    };
    // Check the balance before transferring; if short, report "Insufficient balance"
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance: u128 = bal
        .get("balance")
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if balance < amount as u128 {
        send_terminal(
            state,
            format!(
                r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"{balance}","error":"Insufficient balance"}}"#
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
        format!(
            r#"{{"type":"paymentResult","ok":true,"amount":"{amount}","balanceAfter":"{after}","digest":"{digest}"}}"#
        )
    } else {
        // Pass the helper's user-facing message through (it maps Move aborts to
        // readable text); fall back to "Insufficient balance" on gas/funds errors.
        let raw = res.get("error").and_then(|x| x.as_str()).unwrap_or("");
        eprintln!("payment helper failed (raw error): {raw}");
        let user = if raw.to_lowercase().contains("insufficient")
            || raw.to_lowercase().contains("gas")
            || raw.to_lowercase().contains("balance")
        {
            "Insufficient balance".to_string()
        } else if raw.is_empty() {
            "Payment failed".to_string()
        } else {
            raw.to_string()
        };
        format!(
            r#"{{"type":"paymentResult","ok":false,"amount":"{amount}","balanceAfter":"{balance}","error":{}}}"#,
            serde_json::to_string(&user).unwrap()
        )
    };
    send_terminal(state, msg);
}

// ------------------------------------------------------------- sui-pay.mjs calls

fn sui_helper(cfg: &Config, args: &[&str]) -> serde_json::Value {
    sui_helper_env(cfg, args, &[])
}

/// Calls sui-pay.mjs with extra environment variables. The attestation is passed
/// via env because as an argument it would be exposed in ps and hit length limits.
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
                    // A simulated card has no oracle attestation, so payments are
                    // refused until a real card is tapped (balance mode still works).
                    thread::spawn(move || on_card(&state2, &cfg2, &idi, None));
                } else {
                    println!("Usage: testcard <idiHex>  e.g. testcard 05d5807e28260205");
                }
            }
            Some("remove") => {
                {
                    let mut s = state.lock().unwrap();
                    s.current_idi = None;
                    s.current_att = None;
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
            Ok(Some((idm, idi, att_json))) => {
                if idm == last_idm {
                    // Same card still present. Wait until it is removed
                    thread::sleep(Duration::from_millis(400));
                    continue;
                }
                last_idm = idm;
                on_card(&state, &cfg, &idi, Some(att_json));
            }
            Ok(None) => {
                // No card. If one was present, send a "removed" notice
                if !last_idm.is_empty() {
                    last_idm.clear();
                    let mut s = state.lock().unwrap();
                    s.current_idi = None;
                    s.current_att = None;
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

/// On card detection: query balance, check registration, and send a card event to the terminal.
/// `att_json` is the on-chain ZK attestation (None for a simulated card).
fn on_card(state: &State, cfg: &Arc<Config>, idi: &str, att_json: Option<String>) {
    eprintln!(
        "Card: IDi={idi}{}",
        if att_json.is_some() { " (ZK attestation obtained)" } else { "" }
    );
    let bal = sui_helper(cfg, &["balance", idi]);
    let balance = bal.get("balance").and_then(|x| x.as_str()).unwrap_or("0").to_string();
    // On-chain registration is approximated by balance > 0 (topped up via regist-web = registered)
    let registered = balance.parse::<u128>().map(|n| n > 0).unwrap_or(false);
    {
        let mut s = state.lock().unwrap();
        s.current_idi = Some(idi.to_string());
        s.current_att = att_json;
    }
    let ev = format!(
        r#"{{"type":"card","idi":"{idi}","registered":{registered},"balance":"{balance}"}}"#
    );
    send_terminal(state, ev);
}

/// Poll once; if a card is present, complete oracle auth and return
/// `(idm, idi, attestation_json)`. Otherwise None.
///
/// A fresh `r1` is drawn per session: the on-chain gate burns it, so a constant
/// challenge would make every payment after the first abort with `gate::EReplay`.
fn read_card_idi<D: FelicaDriver + ?Sized>(
    driver: &mut D,
    cfg: &Config,
) -> Result<Option<(String, String, String)>, Box<dyn std::error::Error>> {
    let (mut felica, _) =
        match FelicaStandard::polling_multi(driver, &["212F", "424F"], SYSTEM_CODE, 0x00, 0x00) {
            Ok(f) => f,
            Err(_) => return Ok(None), // no card
        };
    let idm_hex = hex::encode(felica.idm());

    // Fresh holder challenge for this session (bound into the proof as pi1).
    let mut r1 = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut r1);
    let r1_hex = hex::encode(r1);

    let ch = rpc(cfg, "challenge", serde_json::json!({"idm": idm_hex, "r1": r1_hex}))?;
    let c1a = hex8(&ch["c1a"])?;
    let areas = u16_list(&ch["areas"]);
    let services: Vec<ServiceCode> = u16_list(&ch["services"]).into_iter().map(ServiceCode::new).collect();

    let (c1b, c2a) = felica.authentication1(&areas, &services, &c1a)?;
    let st = rpc(
        cfg,
        "settle",
        serde_json::json!({"idm": idm_hex, "r1": r1_hex, "c1b": hex::encode(c1b), "c2a": hex::encode(c2a)}),
    )?;
    let c2b = hex8(&st["c2b"])?;
    let resp = felica.authentication2(&c2b)?;
    let ct = extract_ciphertext(&format!("{resp:?}")).ok_or("failed to extract auth2")?;
    let at_json = rpc(
        cfg,
        "attest",
        serde_json::json!({"idm": idm_hex, "c1b": hex::encode(c1b), "c2a": hex::encode(c2a), "auth2": hex::encode(&ct)}),
    )?;
    let at: AttestResult = serde_json::from_value(at_json)?;
    let att_json = attestation_json(&at, &r1)?;
    Ok(Some((idm_hex, at.idi.to_lowercase(), att_json)))
}

/// `attest` result: claimed identity plus its Groth16 proof (coordinate form).
#[derive(Debug, Clone, serde::Deserialize)]
struct AttestResult {
    idi: String,
    attested_at: u64,
    proof: WireProof,
}

/// The oracle's `a`/`b`/`c` are hex coordinate strings; `b` is two Fq2 pairs.
#[derive(Debug, Clone, serde::Deserialize)]
struct WireProof {
    alg: String,
    a: (String, String),
    b: ((String, String), (String, String)),
    c: (String, String),
    /// Exactly three 32-byte little-endian scalars, hex-encoded: idi, r1, attested_at.
    public_inputs: Vec<String>,
}

/// Converts the oracle's coordinate-form Groth16 proof into the Arkworks
/// compressed bytes accepted by `sui::groth16`, and builds the attestation JSON
/// passed to sui-pay.mjs.
///
/// `r1` is chosen by this process's CSPRNG and the proof is bound to it. The
/// on-chain gate requires the same `r1` and burns it, so this JSON is single-use.
fn attestation_json(att: &AttestResult, r1: &[u8; 8]) -> Result<String, Box<dyn std::error::Error>> {
    let wire = prover::Groth16Proof {
        alg: att.proof.alg.clone(),
        a: att.proof.a.clone(),
        b: att.proof.b.clone(),
        c: att.proof.c.clone(),
        public_inputs: att.proof.public_inputs.clone(),
    };
    let proof = prover::proof_compressed_bytes(&wire)
        .map_err(|e| format!("failed to compress proof: {e}"))?;
    let pis = prover::public_inputs_bytes(&wire)
        .map_err(|e| format!("failed to encode public inputs: {e}"))?;
    Ok(serde_json::json!({
        "idi": att.idi.to_lowercase(),
        "attested_at": att.attested_at,
        "r1": hex::encode(r1),
        "proof": hex::encode(proof),
        "public_inputs": hex::encode(pis),
    })
    .to_string())
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
