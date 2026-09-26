//! `usb-poc` — attest a real Suica IDi over USB and verify the proof.
//!
//! # What this demonstrates
//!
//! The oracle holds the FeliCa master keys; this process holds a card. Neither
//! ever hands the other the long-term material, and the proof that comes out
//! the far end is verified locally against the oracle's published verifying
//! key. Concretely, one run establishes:
//!
//! 1. **The card accepted our C1A.** C1A is `3DES(alpha, L, R1)` and the card
//!    recomputes `L = k_group XOR IDm` from its own keys and its own IDm. So the
//!    card accepting C1A *is* the check that the oracle's `gsk`/`usk` are the
//!    real ones for this card. This is why the PoC needs no key material of its
//!    own — the card is the verifier.
//! 2. **The oracle accepted the card's C1B**, which it can only do by deriving
//!    the same `L`/`beta`. Agreement is bilateral.
//! 3. **The card accepted our C2B**, completing mutual authentication.
//! 4. **The AUTH2 frame carries a valid MAC** under the session's `R2`, and the
//!    IDi lives inside it. The oracle checks the MAC; this process cannot even
//!    see the plaintext.
//! 5. **A Groth16 proof verifies** against the oracle's verifying key, binding
//!    `pi0 = idi` to that transcript inside the R1CS.
//!
//! # What it does not demonstrate
//!
//! Step 5 is a proof that *someone* ran a correct session, not that a card was
//! physically present. Issue #1 in the prover is open: `gsk`/`usk`/`idm` are
//! unconstrained witnesses, so anyone holding the public proving key can mint
//! an accepted attestation for an arbitrary IDi without a card. The PoC prints
//! this caveat next to the verdict on purpose — a demo that hides it would be
//! claiming something the circuit does not prove.
//!
//! Note that steps 1–3 are *not* subject to that gap: they are checks the
//! physical card performs. A forgery cannot make a real Suica accept a C1A.

mod card;
mod oracle;

use anyhow::{bail, Context, Result};
use card::{Block, Card};
use felica::felica_standard::ServiceCode;
use felica::ReaderPreference;
use oracle::Oracle;
use std::process::ExitCode;

/// The Suica service codes worth reporting key versions for, from the node
/// table in `soltia48/suica-viewer`. Probing these is key-free and shows which
/// of them this particular card actually carries.
const SUICA_SERVICES: &[(u16, &str)] = &[
    (0x0048, "transit common data"),
    (0x004A, "issuance info"),
    (0x008B, "attributes / balance"),
    (0x0816, "misc"),
    (0x08CA, "last top-up"),
    (0x090F, "transaction history"),
    (0x104A, "commuter pass"),
    (0x108F, "gate entry/exit"),
    (0x10CB, "SF gate entry"),
    (0x184B, "paid ticket"),
];

/// The key version the oracle is provisioned for: `0000` is the DES key set.
const DES_KEY_VERSION: u16 = 0x0000;

const DEFAULT_ORACLE: &str = "https://felica-oracle.ouchiserver.aokiapp.com";

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1).collect()) {
        Ok(Some(args)) => args,
        Ok(None) => return ExitCode::SUCCESS, // --help
        Err(err) => {
            eprintln!("error: {err}\n");
            eprintln!("{}", Args::usage());
            return ExitCode::from(2);
        }
    };

    match run(&args) {
        Ok(Outcome::Verified) => ExitCode::SUCCESS,
        Ok(Outcome::ProofRejected) => ExitCode::from(3),
        Err(err) => {
            eprintln!("\n\x1b[31mFAILED\x1b[0m: {err:#}");
            ExitCode::from(1)
        }
    }
}

enum Outcome {
    Verified,
    ProofRejected,
}

fn run(args: &Args) -> Result<Outcome> {
    let oracle = Oracle::new(args.oracle.clone())?;
    println!("Oracle: {}", oracle.base());

    // --- 0. The oracle is up and has a key loaded -------------------------
    step("Oracle liveness");
    let pong = oracle.ping().context("oracle did not answer ping")?;
    println!("  ping -> {pong:?}");
    let vk_hex = oracle.verifying_key().context("oracle did not return a verifying key")?;
    let vk_bytes = hex::decode(&vk_hex).context("verifying key is not valid hex")?;
    // 264 bytes of curve points plus one 32-byte scalar per public input.
    println!(
        "  verifying key: {} bytes ({} public inputs)",
        vk_bytes.len(),
        vk_bytes.len().saturating_sub(264) / 32
    );

    // --- 1. Find the card -------------------------------------------------
    step("Card");
    let mut card = Card::poll(
        args.reader,
        args.system_code,
        args.request_code,
        args.time_slots,
        args.wait,
    )?;
    println!("  bitrate:       {}", card.bitrate);
    println!("  IDm:           {}", hex::encode_upper(card.idm));
    println!("  PMm:           {}", hex::encode_upper(card.pmm));
    if !card.polling_optional.is_empty() {
        println!(
            "  poll optional: {}",
            hex::encode_upper(&card.polling_optional)
        );
    }
    let idm = card.idm;

    // --- 2. Key-free interrogation ----------------------------------------
    step("Card interrogation (no keys involved)");
    let system_codes = card.request_system_codes(500).unwrap_or_default();
    if system_codes.is_empty() {
        println!("  system codes:  <unavailable>");
    } else {
        let list: Vec<String> = system_codes.iter().map(|c| format!("{c:04X}")).collect();
        println!("  system codes:  {}", list.join(", "));
    }

    let probe: Vec<u16> = SUICA_SERVICES.iter().map(|(c, _)| *c).collect();
    match card.request_service(&probe, 500) {
        Ok(info) => {
            println!("  key versions by service:");
            for ((code, name), version) in SUICA_SERVICES.iter().zip(info.key_versions.iter()) {
                let marker = if *version == DES_KEY_VERSION {
                    "\x1b[32m<=\x1b[0m"
                } else {
                    " "
                };
                println!("    {code:04X}  {version:04X} {marker}  {name}");
            }
            let matching = info
                .key_versions
                .iter()
                .filter(|v| **v == DES_KEY_VERSION)
                .count();
            println!(
                "  {matching}/{} use the DES key set ({DES_KEY_VERSION:04X}), which is what the oracle holds",
                info.key_versions.len()
            );
        }
        // Not fatal: these are informational, and some cards refuse a node the
        // reader is not entitled to ask about.
        Err(err) => println!("  key versions: unavailable ({err:#})"),
    }

    match card.request_code_list(0x0000, 0x0000, 500) {
        Ok(list) => {
            println!("  code list (continue flag {:02X}):", list.continue_flag);
            for (area, end) in &list.areas {
                println!("    area {area:04X} .. {end:04X}");
            }
            let svcs: Vec<String> = list.services.iter().map(|c| format!("{c:04X}")).collect();
            println!("    services: {}", svcs.join(", "));
        }
        Err(err) => println!("  code list: unavailable ({err:#})"),
    }

    match card.request_block_information(&probe, 500) {
        Ok(counts) => {
            let list: Vec<String> = SUICA_SERVICES
                .iter()
                .zip(counts.iter())
                .map(|((code, name), blocks)| format!("{code:04X}={blocks} ({name})"))
                .collect();
            println!("  blocks per service: {}", list.join(", "));
        }
        Err(err) => println!("  block information: unavailable ({err:#})"),
    }

    // --- 3. Start mutual authentication -----------------------------------
    step("Challenge (oracle derives C1A)");
    let r1: Block = {
        use rand::RngCore;
        let mut b = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut b);
        b
    };
    println!("  R1 (ours):  {}", hex::encode_upper(r1));
    let challenge = oracle
        .challenge(&hex::encode(idm), &hex::encode(r1))
        .context("oracle challenge failed")?;
    println!("  C1A:        {}", challenge.c1a.to_uppercase());
    println!("  system code: {:04X}", challenge.system_code);
    println!(
        "  node path:   areas [{}] services [{}]",
        join_codes(&challenge.areas),
        join_codes(&challenge.services)
    );
    if challenge.system_code != args.system_code {
        bail!(
            "the oracle is provisioned for system {:04X} but the card was polled as {:04X}; \
             these keys are not for this card",
            challenge.system_code,
            args.system_code
        );
    }
    if challenge.areas.is_empty() && challenge.services.is_empty() {
        bail!("the oracle returned an empty node path, so Authentication1 cannot be addressed");
    }

    let services: Vec<ServiceCode> = challenge
        .services
        .iter()
        .map(|c| ServiceCode::new(*c))
        .collect();

    // The node path the oracle is keyed for, checked against the card.
    let oracle_nodes: Vec<u16> = challenge
        .areas
        .iter()
        .chain(challenge.services.iter())
        .copied()
        .collect();
    if let Ok(info) = card.request_service(&oracle_nodes, 500) {
        let versions: Vec<String> = info
            .key_versions
            .iter()
            .map(|v| format!("{v:04X}"))
            .collect();
        println!("  key versions on that path: {}", versions.join(", "));
    }

    step("Authentication1 (card checks our C1A)");
    println!("  >>> the card now recomputes L = k_group XOR IDm from its own keys.");
    println!("  >>> If C1A is wrong it will simply not answer.");
    let c1a: Block = hex::decode(&challenge.c1a)
        .context("C1A is not valid hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("C1A must be 8 bytes"))?;
    let (c1b, c2a) = card.authentication1(&challenge.areas, &services, &c1a, args.auth1_timeout)?;
    println!("  card answered.");
    println!("  C1B: {}", hex::encode_upper(c1b));
    println!("  C2A: {}", hex::encode_upper(c2a));

    step("Settle (oracle checks the card's C1B, returns C2B)");
    let settle = oracle
        .settle(
            &hex::encode(idm),
            &hex::encode(r1),
            &hex::encode(c1b),
            &hex::encode(c2a),
        )
        .context("oracle settle failed — the card and oracle disagree on the key schedule")?;
    let c2b: Block = hex::decode(&settle.c2b)
        .context("C2B is not valid hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("C2B must be 8 bytes"))?;
    println!("  C2B: {}", hex::encode_upper(c2b));
    println!("  C2B is 3DES(alpha, L, R2), where the oracle recovered R2 by");
    println!("  decrypting C2A. The card will only accept it if the two agree.");

    step("Authentication2 (card checks our C2B, returns AUTH2)");
    let auth2 = card.authentication2(&c2b, args.auth2_timeout)?;
    println!("  AUTH2 ciphertext: {} bytes", auth2.len());
    println!("  {}", hex::encode_upper(&auth2));
    println!("  The IDi is sealed inside this. This process cannot read it;");
    println!("  the oracle can, because it knows R2.");

    step("Attest (oracle produces the Groth16 proof)");
    let attest = oracle
        .attest(
            &hex::encode(idm),
            &hex::encode(c1b),
            &hex::encode(c2a),
            &hex::encode(&auth2),
        )
        .context("oracle attest failed")?;
    println!("  claimed IDi: {}", attest.idi.to_uppercase());
    println!("  attested_at: {} ({})", attest.attested_at, rfc3339(attest.attested_at));
    println!("  alg:         {}", attest.proof.alg);
    for (name, value) in [
        ("a.x", &attest.proof.a.0),
        ("a.y", &attest.proof.a.1),
        ("b.x.0", &attest.proof.b.0 .0),
        ("b.x.1", &attest.proof.b.0 .1),
        ("b.y.0", &attest.proof.b.1 .0),
        ("b.y.1", &attest.proof.b.1 .1),
        ("c.x", &attest.proof.c.0),
        ("c.y", &attest.proof.c.1),
    ] {
        println!("  {name:>6}: {}", value);
    }

    // --- 4. Verify ---------------------------------------------------------
    step("Verify the proof locally");
    let vk = prover::load_verifying_key(&vk_bytes)
        .map_err(|e| anyhow::anyhow!("verifying key rejected by the prover: {e}"))?;

    let idi: [u8; 8] = hex::decode(&attest.idi)
        .context("attested IDi is not valid hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("attested IDi must be 8 bytes"))?;

    let envelope = prover::Attestation {
        idi,
        attested_at: attest.attested_at,
        proof: prover::Groth16Proof {
            alg: attest.proof.alg.clone(),
            a: attest.proof.a.clone(),
            b: attest.proof.b.clone(),
            c: attest.proof.c.clone(),
            public_inputs: attest.proof.public_inputs.clone(),
        },
    };

    match prover::verify_attestation_detailed(&vk, &envelope) {
        Ok(()) => println!("  \x1b[32mGroth16 pairing: PASS\x1b[0m"),
        Err(err) => {
            println!("  \x1b[31mGroth16 pairing: FAIL\x1b[0m ({err})");
            println!("\n\x1b[31mVERDICT: REJECTED\x1b[0m — the proof does not verify.");
            return Ok(Outcome::ProofRejected);
        }
    }

    // --- 5. Decode the statement and cross-check --------------------------
    step("Statement");
    if envelope.proof.public_inputs.len() != 3 {
        bail!(
            "expected 3 public inputs, got {}",
            envelope.proof.public_inputs.len()
        );
    }
    let mut limbs: Vec<[u8; 8]> = Vec::new();
    for (i, scalar) in envelope.proof.public_inputs.iter().enumerate() {
        let bytes = hex::decode(scalar).with_context(|| format!("public input {i} is not hex"))?;
        if bytes.len() != 32 {
            bail!("public input {i} is {} bytes, expected 32", bytes.len());
        }
        if bytes[8..].iter().any(|b| *b != 0) {
            bail!("public input {i} is not a canonical 8-byte scalar (upper 24 bytes set)");
        }
        let limb: [u8; 8] = bytes[..8].try_into().expect("8 bytes");
        println!("  pi{i} = {}  ->  {}", scalar, hex::encode_upper(limb));
        limbs.push(limb);
    }

    let pi_idi = limbs[0];
    let pi_r1 = limbs[1];
    let pi_at = u64::from_le_bytes(limbs[2]);

    println!();
    let mut ok = true;
    ok &= check("pi0 == the IDi the oracle reported", pi_idi == idi);
    ok &= check("pi1 == the R1 this process generated", pi_r1 == r1);
    ok &= check("pi2 == the attested_at the oracle reported", pi_at == attest.attested_at);
    // The interesting one: the proof is bound to *our* challenge, so it cannot
    // be replayed against a different session.
    ok &= check("proof is bound to this session's R1", pi_r1 == r1);
    // attested_at が現在時刻から ±1 日以内か(オラクルの時計のドリフト信号)。
    // epoch 秒なのでタイムゾーンには依存しない。旧実装は now を使わず
    // `attested_at - 86400 >= attested_at` を評価しており常に false だった。
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let drift = now.abs_diff(pi_at);
    ok &= check(
        "attested_at is a plausible timestamp",
        pi_at != 0 && drift <= 86_400,
    );

    // The full IDi chain: IDm was read in cleartext, the proof attests to IDi,
    // and the two are different values by design.
    println!();
    println!("  IDm (cleartext, from polling)      {}", hex::encode_upper(idm));
    println!("  IDi (attested, from inside AUTH2)  {}", hex::encode_upper(idi));
    println!(
        "  {}",
        if idm == idi {
            "note: IDm == IDi on this card; not required, they are distinct fields"
        } else {
            "note: IDm and IDi are distinct fields, as expected"
        }
    );

    verdict(ok, &idi);
    Ok(if ok {
        Outcome::Verified
    } else {
        Outcome::ProofRejected
    })
}

fn verdict(ok: bool, idi: &[u8; 8]) {
    step("Verdict");
    let mark = if ok { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
    println!("  {mark} The Groth16 proof verifies against the oracle's verifying key.");
    println!("  {mark} Its public input pi0 commits to IDi {}.", hex::encode_upper(idi));
    println!("  {mark} pi1 binds it to the challenge this process generated, so it is not replayable.");
    println!();
    println!("  Chain of evidence, strongest first:");
    println!("    1. The card accepted C1A.");
    println!("       -> the oracle's gsk/usk are this card's real keys, and the IDm");
    println!("          we polled is the card's real IDm. Not forgeable: it required");
    println!("          a physical card to say yes.");
    println!("    2. The oracle accepted C1B and the card accepted C2B.");
    println!("       -> both sides derived the same L, alpha and beta. Mutual");
    println!("          authentication completed.");
    println!("    3. The AUTH2 MAC verified under R2.");
    println!("       -> the IDi really was inside a MAC-valid frame from this session.");
    println!("    4. The proof verifies.");
    println!("       -> pi0 == IDi is constrained in the R1CS against that transcript.");
    println!();
    println!("  \x1b[33mCaveat, and it matters:\x1b[0m steps 1-3 are checks a physical");
    println!("  card performed. Step 4 is not. Issue #1 in the prover is open: gsk/usk/idm");
    println!("  are unconstrained witnesses, so a holder of the *public* proving key can");
    println!("  mint an accepted proof for any IDi with no card at all. A verified");
    println!("  attestation is therefore NOT evidence that a card was present. See");
    println!("  prover/README.md and prover/tests/forgery_authority.rs.");
}

/// A checked statement, printed as it is evaluated.
fn check(what: &str, ok: bool) -> bool {
    let mark = if ok { "\x1b[32m✓\x1b[0m" } else { "\x1b[31m✗\x1b[0m" };
    println!("  {mark} {what}");
    ok
}

fn step(title: &str) {
    println!("\n\x1b[1m== {title} ==\x1b[0m");
}

fn join_codes(codes: &[u16]) -> String {
    if codes.is_empty() {
        return "<none>".into();
    }
    codes
        .iter()
        .map(|c| format!("{c:04X}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `attested_at` is prover-chosen and is not time — see the caveat in `verdict`.
/// This is only here so the demo output is readable.
fn rfc3339(secs: u64) -> String {
    // Avoid a date dependency: print the raw value alongside a coarse label.
    if secs == 0 {
        return "unset".into();
    }
    let days = secs / 86_400;
    let rem = secs % 86_400;
    format!(
        "{}d {:02}:{:02}:{:02} since epoch",
        days,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

struct Args {
    oracle: String,
    reader: ReaderPreference,
    system_code: u16,
    request_code: u8,
    time_slots: u8,
    wait: u64,
    auth1_timeout: u16,
    auth2_timeout: u16,
}

impl Args {
    fn usage() -> String {
        format!(
            "\
usb-poc — attest a Suica IDi over USB against the FeliCa oracle

USAGE:
    usb-poc [OPTIONS]
    cargo run --release --manifest-path usb-poc/Cargo.toml -- [OPTIONS]

OPTIONS:
    --oracle <URL>          Oracle base URL
                             [env: ORACLE_URL] [default: {DEFAULT_ORACLE}]
    --reader <KIND>         auto | port100 | port400 | rcs320 | rcs956
                             [default: port100]
                             RC-S380 is a Port-100 reader (USB 054c:06c1/06c3).
                             The RC-S320 is a different driver (054c:01bb).
    --system-code <HEX>     Polling system code [default: 0003]
    --request-code <HEX>    Polling request code [default: 00]
    --time-slots <HEX>      Polling time slots [default: 00]
    --wait <SECS>           Seconds to keep polling for a card [default: 30]
    --auth1-timeout <MS>    Authentication1 timeout [default: 2000]
    --auth2-timeout <MS>    Authentication2 timeout [default: 1000]
    -h, --help              Show this help

EXIT CODES:
    0  proof verified and every cross-check passed
    1  operational failure (no card, RPC error, reader error)
    2  bad arguments
    3  proof rejected, or a cross-check failed

NOTES:
    This tool takes no key material. The oracle holds gsk/usk; the card verifies
    the oracle by accepting C1A. Nothing secret is read from or written to disk.
"
        )
    }

    /// Returns `Ok(None)` when the user asked for help.
    fn parse(argv: Vec<String>) -> Result<Option<Self>> {
        let mut args = Args {
            oracle: std::env::var("ORACLE_URL").unwrap_or_else(|_| DEFAULT_ORACLE.into()),
            reader: ReaderPreference::ForcePort100,
            system_code: 0x0003,
            request_code: 0x00,
            time_slots: 0x00,
            wait: 30,
            auth1_timeout: 2000,
            auth2_timeout: 1000,
        };

        let mut i = 0;
        while i < argv.len() {
            let flag = argv[i].clone();
            if flag == "-h" || flag == "--help" {
                println!("{}", Args::usage());
                return Ok(None);
            }
            // Pull the value for `flag`, or fail with the flag's own name.
            let mut value = || -> Result<String> {
                i += 1;
                argv.get(i)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))
            };
            match flag.as_str() {
                "--oracle" => args.oracle = value()?,
                "--reader" => {
                    args.reader = match value()?.as_str() {
                        "auto" => ReaderPreference::Auto,
                        "port100" => ReaderPreference::ForcePort100,
                        "port400" => ReaderPreference::ForcePort400,
                        "rcs320" => ReaderPreference::ForceRcs320,
                        "rcs956" => ReaderPreference::ForceRcs956,
                        other => bail!("unknown reader kind {other:?}"),
                    }
                }
                "--system-code" => args.system_code = parse_hex_u16(&value()?, "system code")?,
                "--request-code" => args.request_code = parse_hex_u8(&value()?, "request code")?,
                "--time-slots" => args.time_slots = parse_hex_u8(&value()?, "time slots")?,
                "--wait" => {
                    args.wait = value()?
                        .parse()
                        .context("--wait must be a whole number of seconds")?
                }
                "--auth1-timeout" => {
                    args.auth1_timeout = value()?
                        .parse()
                        .context("--auth1-timeout must be milliseconds")?
                }
                "--auth2-timeout" => {
                    args.auth2_timeout = value()?
                        .parse()
                        .context("--auth2-timeout must be milliseconds")?
                }
                other => bail!("unknown flag {other:?}"),
            }
            i += 1;
        }
        Ok(Some(args))
    }
}

fn parse_hex_u16(text: &str, what: &str) -> Result<u16> {
    let trimmed = text.trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(trimmed, 16).with_context(|| format!("{what} {text:?} is not hex"))
}

fn parse_hex_u8(text: &str, what: &str) -> Result<u8> {
    let trimmed = text.trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(trimmed, 16).with_context(|| format!("{what} {text:?} is not hex"))
}
