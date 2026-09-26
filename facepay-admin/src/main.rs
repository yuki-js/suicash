//! SuiCash admin GUI (standalone native app for the host PC).
//!
//! Like the official Hi-CARA admin software, it runs standalone on the host PC to
//! operate the payment terminal. No browser: it connects to the facepay daemon's
//! WebSocket (:8899) as a client (receiving the same broadcast as the terminal).
//!
//!   facepay daemon (:8899)
//!     ├─ Hi-CARA terminal (WebView) … face auth, gate LCD
//!     └─ this admin GUI             … status display, payment/balance mode switch
//!
//! Receives: mode / status / card / paymentResult / detecting / cardRemoved
//! Sends: {"type":"op","cmd":"pay","sui":<f64>} / {"type":"op","cmd":"idle"}
//!
//! The endpoint can be changed with FACEPAY_ADMIN_WS (default ws://127.0.0.1:8899).

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui;

const DEFAULT_WS: &str = "ws://127.0.0.1:8899";

/// State shared between the GUI and WS threads
#[derive(Default)]
struct Shared {
    connected: bool,
    /// "payment" or "idle" (from daemon status)
    mode: String,
    amount_mist: Option<u64>,
    idi: Option<String>,
    /// Number of connections (terminals + GUI)
    clients: usize,
    registered: Option<bool>,
    balance_mist: Option<u128>,
    /// Terminal-side phase (detecting/card/removed etc., for logging)
    last_event: String,
    last_payment: Option<PaymentView>,
    log: Vec<String>,
}

#[derive(Clone)]
struct PaymentView {
    ok: bool,
    amount_mist: u64,
    balance_after_mist: u128,
    digest: Option<String>,
    error: Option<String>,
}

type State = Arc<Mutex<Shared>>;

fn mist_to_sui(mist: u128) -> String {
    format!("{:.4}", mist as f64 / 1e9)
}

fn push_log(s: &State, line: impl Into<String>) {
    let mut g = s.lock().unwrap();
    let line = line.into();
    g.log.push(line);
    let len = g.log.len();
    if len > 200 {
        g.log.drain(0..len - 200);
    }
}

// -------------------------------------------------------------- WS client

fn ws_thread(url: String, state: State, rx: Receiver<String>, ctx: egui::Context) {
    loop {
        push_log(&state, format!("Connecting… {url}"));
        match tungstenite::connect(&url) {
            Ok((mut ws, _)) => {
                if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_ref() {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(100)));
                }
                {
                    let mut g = state.lock().unwrap();
                    g.connected = true;
                }
                push_log(&state, "Connected to daemon");
                ctx.request_repaint();

                loop {
                    // Flush the send queue
                    let mut send_err = false;
                    while let Ok(msg) = rx.try_recv() {
                        if ws.send(tungstenite::Message::Text(msg)).is_err() {
                            send_err = true;
                            break;
                        }
                    }
                    if send_err {
                        break;
                    }
                    match ws.read() {
                        Ok(tungstenite::Message::Text(t)) => {
                            handle_message(&state, &t);
                            ctx.request_repaint();
                        }
                        Ok(tungstenite::Message::Close(_)) => break,
                        Ok(_) => {}
                        Err(tungstenite::Error::Io(ref e))
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut => {}
                        Err(e) => {
                            push_log(&state, format!("Receive error: {e}"));
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                push_log(&state, format!("Connection failed: {e}"));
            }
        }
        {
            let mut g = state.lock().unwrap();
            g.connected = false;
        }
        ctx.request_repaint();
        // Wait a bit, discarding queued sends while disconnected so the queue doesn't build up
        while rx.try_recv().is_ok() {}
        thread::sleep(Duration::from_secs(2));
    }
}

fn get_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn get_u128(v: &serde_json::Value, key: &str) -> Option<u128> {
    v.get(key).and_then(|x| match x {
        serde_json::Value::String(s) => s.parse().ok(),
        serde_json::Value::Number(n) => n.as_u64().map(|n| n as u128),
        _ => None,
    })
}

fn handle_message(state: &State, text: &str) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
    match t {
        "status" => {
            let mut g = state.lock().unwrap();
            g.mode = get_str(&v, "mode").unwrap_or_default();
            g.amount_mist = get_u128(&v, "amount").map(|n| n as u64);
            g.idi = get_str(&v, "idi");
            g.clients = v.get("clients").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        }
        "mode" => {
            let mut g = state.lock().unwrap();
            match v.get("payment") {
                Some(serde_json::Value::Object(o)) => {
                    g.mode = "payment".into();
                    g.amount_mist = o
                        .get("amount")
                        .and_then(|x| x.as_str())
                        .and_then(|s| s.parse().ok());
                }
                _ => {
                    g.mode = "idle".into();
                    g.amount_mist = None;
                }
            }
        }
        "card" => {
            {
                let mut g = state.lock().unwrap();
                g.idi = get_str(&v, "idi");
                g.registered = v.get("registered").and_then(|x| x.as_bool());
                g.balance_mist = get_u128(&v, "balance");
                g.last_event = "Card detected".into();
            }
            let idi = get_str(&v, "idi").unwrap_or_default();
            let reg = v.get("registered").and_then(|x| x.as_bool()).unwrap_or(false);
            push_log(
                state,
                format!("Card IDi={idi} registered={}", if reg { "yes" } else { "no" }),
            );
        }
        "detecting" => {
            state.lock().unwrap().last_event = "Authenticating…".into();
            push_log(state, "Terminal: authenticating…");
        }
        "cardRemoved" => {
            let mut g = state.lock().unwrap();
            g.idi = None;
            g.registered = None;
            g.balance_mist = None;
            g.last_event = "No card".into();
        }
        "paymentResult" => {
            let pv = PaymentView {
                ok: v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
                amount_mist: get_u128(&v, "amount").unwrap_or(0) as u64,
                balance_after_mist: get_u128(&v, "balanceAfter").unwrap_or(0),
                digest: get_str(&v, "digest").filter(|s| !s.is_empty()),
                error: get_str(&v, "error").filter(|s| !s.is_empty()),
            };
            let line = if pv.ok {
                format!(
                    "Payment succeeded {} SUI  digest={}",
                    mist_to_sui(pv.amount_mist as u128),
                    pv.digest.clone().unwrap_or_default()
                )
            } else {
                format!(
                    "Payment failed {} SUI  {}",
                    mist_to_sui(pv.amount_mist as u128),
                    pv.error.clone().unwrap_or_default()
                )
            };
            state.lock().unwrap().last_payment = Some(pv);
            push_log(state, line);
        }
        _ => {}
    }
}

// -------------------------------------------------------- Daemon auto-start

/// Whether a facepay daemon is already listening on :port
fn daemon_running(host: &str, port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("{host}:{port}")
            .parse()
            .unwrap_or_else(|_| ([127, 0, 0, 1], port).into()),
        Duration::from_millis(300),
    )
    .is_ok()
}

/// Resolve the location of the facepay binary
fn find_facepay_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FACEPAY_BIN") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    let mut candidates: Vec<PathBuf> = vec![
        "usb-poc/target/release/facepay".into(),
        "usb-poc/target/debug/facepay".into(),
        "../usb-poc/target/debug/facepay".into(),
    ];
    // Relative to the admin GUI executable (facepay-admin/target/debug/facepay-admin)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("../../../usb-poc/target/debug/facepay"));
            candidates.push(dir.join("../../../usb-poc/target/release/facepay"));
        }
    }
    // The child is spawned with current_dir set, so always normalize to an absolute path
    // (a relative program path would resolve against the new cwd and fail with ENOENT)
    candidates
        .into_iter()
        .find(|c| c.exists())
        .and_then(|c| c.canonicalize().ok())
}

/// Guard that reliably kills the child process on exit
struct DaemonGuard(Child);
impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Spawn the facepay daemon as a child process and forward its stderr to the log
fn spawn_daemon(state: &State, bin: &PathBuf) -> Option<DaemonGuard> {
    // Use the repo root as cwd so the daemon can resolve sui-pay.mjs too
    let cwd = bin
        .ancestors()
        .nth(3) // usb-poc/target/debug/facepay -> repo root
        .map(|p| p.to_path_buf());
    let mut cmd = Command::new(bin);
    if let Some(cwd) = &cwd {
        cmd.current_dir(cwd);
    }
    // Close stdin (the GUI operates via WS, so the daemon's stdin CLI is not needed)
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    // Make sure the daemon (child) dies if the GUI (parent) dies. A safety net on top of the kill in Drop.
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong, 0, 0, 0);
            Ok(())
        });
    }
    match cmd.spawn() {
        Ok(mut child) => {
            push_log(state, format!("Daemon started: {}", bin.display()));
            if let Some(err) = child.stderr.take() {
                let state = state.clone();
                thread::spawn(move || {
                    for line in BufReader::new(err).lines().map_while(Result::ok) {
                        push_log(&state, format!("daemon: {line}"));
                    }
                });
            }
            Some(DaemonGuard(child))
        }
        Err(e) => {
            push_log(state, format!("Failed to start daemon: {e}"));
            None
        }
    }
}

// ----------------------------------------------------------------------- GUI

struct App {
    state: State,
    tx: Sender<String>,
    sui_input: String,
    /// Daemon spawned by the GUI (None if started externally). Killed on Drop.
    _daemon: Option<DaemonGuard>,
}

impl App {
    fn send_op(&self, json: String) {
        let _ = self.tx.send(json);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The WS thread calls request_repaint, but repaint periodically as a fallback
        ctx.request_repaint_after(Duration::from_millis(500));

        let snap = {
            let g = self.state.lock().unwrap();
            (
                g.connected,
                g.mode.clone(),
                g.amount_mist,
                g.idi.clone(),
                g.clients,
                g.registered,
                g.balance_mist,
                g.last_event.clone(),
                g.last_payment.clone(),
            )
        };
        let (connected, mode, amount_mist, idi, clients, registered, balance, last_event, last_pay) =
            snap;
        let terminals = clients.saturating_sub(1); // approximate terminal count, excluding this GUI

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("SuiCash Payment Terminal Admin");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (col, label) = if connected {
                        (egui::Color32::from_rgb(0x3B, 0xD1, 0x6F), "Daemon connected")
                    } else {
                        (egui::Color32::from_rgb(0xE0, 0x5B, 0x5B), "Daemon disconnected")
                    };
                    ui.colored_label(col, format!("● {label}"));
                });
            });
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // ---- Current operating mode ----
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Operating mode").strong());
                ui.add_space(4.0);
                if mode == "payment" {
                    let sui = mist_to_sui(amount_mist.unwrap_or(0) as u128);
                    ui.colored_label(
                        egui::Color32::from_rgb(0xFF, 0xB3, 0x4D),
                        egui::RichText::new(format!("Awaiting payment — {sui} SUI")).size(22.0),
                    );
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(0x7F, 0xC8, 0xFF),
                        egui::RichText::new("Balance mode").size(22.0),
                    );
                }
                ui.add_space(2.0);
                ui.label(format!("Connected terminals: {terminals}"));
            });

            ui.add_space(10.0);

            // ---- Current card ----
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Current card").strong());
                ui.add_space(4.0);
                match &idi {
                    Some(idi) => {
                        ui.monospace(format!("IDi: {idi}"));
                        if let Some(b) = balance {
                            ui.label(format!("Balance: {} SUI", mist_to_sui(b)));
                        }
                        match registered {
                            Some(true) => ui.colored_label(
                                egui::Color32::from_rgb(0x3B, 0xD1, 0x6F),
                                "Registered",
                            ),
                            Some(false) => ui.colored_label(
                                egui::Color32::from_rgb(0xE0, 0x9B, 0x3B),
                                "Not registered (top-up required)",
                            ),
                            None => ui.label(""),
                        };
                    }
                    None => {
                        ui.weak(if last_event.is_empty() {
                            "No card detected".to_string()
                        } else {
                            last_event.clone()
                        });
                    }
                }
            });

            ui.add_space(10.0);

            // ---- Controls ----
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Controls").strong());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Amount (SUI):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.sui_input)
                            .desired_width(90.0)
                            .hint_text("0.1"),
                    );
                    let can_pay = connected && self.sui_input.trim().parse::<f64>().map(|v| v > 0.0).unwrap_or(false);
                    if ui
                        .add_enabled(can_pay, egui::Button::new("Await payment"))
                        .clicked()
                    {
                        if let Ok(sui) = self.sui_input.trim().parse::<f64>() {
                            self.send_op(format!(
                                r#"{{"type":"op","cmd":"pay","sui":{sui}}}"#
                            ));
                        }
                    }
                });
                ui.add_space(6.0);
                if ui
                    .add_enabled(connected, egui::Button::new("Back to balance mode"))
                    .clicked()
                {
                    self.send_op(r#"{"type":"op","cmd":"idle"}"#.to_string());
                }
            });

            ui.add_space(10.0);

            // ---- Latest payment result ----
            if let Some(p) = &last_pay {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new("Latest payment").strong());
                    ui.add_space(4.0);
                    if p.ok {
                        ui.colored_label(
                            egui::Color32::from_rgb(0x3B, 0xD1, 0x6F),
                            format!("Succeeded: {} SUI", mist_to_sui(p.amount_mist as u128)),
                        );
                        ui.label(format!("Balance after payment: {} SUI", mist_to_sui(p.balance_after_mist)));
                        if let Some(d) = &p.digest {
                            ui.monospace(format!("digest: {d}"));
                        }
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(0xE0, 0x5B, 0x5B),
                            format!(
                                "Failed: {}",
                                p.error.clone().unwrap_or_else(|| "Error".into())
                            ),
                        );
                    }
                });
                ui.add_space(10.0);
            }

            // ---- Log ----
            ui.label(egui::RichText::new("Log").strong());
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let g = self.state.lock().unwrap();
                    for line in &g.log {
                        ui.monospace(line);
                    }
                });
        });
    }
}

fn install_japanese_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJKjp-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
        "/usr/share/fonts/truetype/vlgothic/VL-Gothic-Regular.ttf",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    ];
    let Some(bytes) = CANDIDATES.iter().find_map(|p| std::fs::read(p).ok()) else {
        eprintln!("Japanese font not found (CJK text may not render)");
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("jp".to_owned(), egui::FontData::from_owned(bytes));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "jp".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    let url = std::env::var("FACEPAY_ADMIN_WS").unwrap_or_else(|_| DEFAULT_WS.to_string());
    // Extract the port from the URL (to check whether the daemon is running)
    let (ws_host, ws_port) = url::Url::parse(&url)
        .ok()
        .map(|u| {
            (
                u.host_str().unwrap_or("127.0.0.1").to_string(),
                u.port().unwrap_or(8899),
            )
        })
        .unwrap_or_else(|| ("127.0.0.1".to_string(), 8899));

    let state: State = Arc::new(Mutex::new(Shared::default()));
    let (tx, rx) = channel::<String>();

    // Start the daemon ourselves if it isn't running, so launching enables everything.
    // Disable with FACEPAY_ADMIN_NO_SPAWN=1 (for setups running facepay externally).
    let no_spawn = std::env::var("FACEPAY_ADMIN_NO_SPAWN").ok().as_deref() == Some("1");
    let daemon = if !no_spawn && !daemon_running(&ws_host, ws_port) {
        match find_facepay_bin() {
            Some(bin) => spawn_daemon(&state, &bin),
            None => {
                push_log(
                    &state,
                    "facepay binary not found (set FACEPAY_BIN). Trying to connect to an external daemon.",
                );
                None
            }
        }
    } else {
        push_log(&state, "Connecting to existing daemon");
        None
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 720.0])
            .with_min_inner_size([380.0, 520.0])
            .with_title("SuiCash Admin"),
        ..Default::default()
    };

    let state_for_app = state.clone();
    eframe::run_native(
        "SuiCash Admin",
        options,
        Box::new(move |cc| {
            install_japanese_font(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();
            {
                let state = state.clone();
                thread::spawn(move || ws_thread(url, state, rx, ctx));
            }
            Ok(Box::new(App {
                state: state_for_app,
                tx,
                sui_input: "0.1".to_string(),
                _daemon: daemon,
            }))
        }),
    )
}
