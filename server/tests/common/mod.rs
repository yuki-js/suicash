// Shared helpers across integration test binaries; each binary uses a
// subset, so unused warnings are noise.
#![allow(dead_code)]

use oracle::api::types::{AttestRequest, ChallengeRequest, SettleRequest};
use oracle::api::OracleImpl;
use oracle::oracle::fixture;
use oracle::params::ProvingKeyBytes;

pub const R1_HEX: &str = "0011223344556677";
pub const R1B_HEX: &str = "aabbccddeeff0011";

pub type Emulator = felica::felica_standard::FelicaStandardEmulator;

/// Test proving key: real ceremony output, loaded once per test binary.
///
/// `FELICA_TEST_PK` must point at a proving-key file (`felica-setup` output).
/// Generating one in-process is not an option: the circuit is ~149k
/// constraints and a single setup takes ~15s in release, far more per test
/// binary in debug.
///
/// The previous fixture passed `vec![0u8; 32]`, which cannot deserialize as a
/// Groth16 `ProvingKey`. Every RPC test that reached `attest` therefore failed
/// with `PROVE_FAILED` -- and nothing noticed, because CI ran no tests at all
/// (fixed in the `ci: gate image publish on fmt, clippy and both test suites`
/// commit). A fixture that cannot work is worse than no fixture: it makes the
/// suite look covered.
///
/// Panics with an actionable message rather than a deserialize error deep in
/// a test, so the cause is obvious.
pub fn test_proving_key_bytes() -> Vec<u8> {
    static BYTES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    BYTES
        .get_or_init(|| {
            let path = std::env::var("FELICA_TEST_PK").unwrap_or_else(|_| {
                panic!(
                    "FELICA_TEST_PK must point at a proving-key file. Generate one with:\n  \
                     cargo run --release --manifest-path ../prover/Cargo.toml \
                     --bin felica-setup -- ../prover/assets --force\n  \
                     then: FELICA_TEST_PK=../prover/assets/proving_key.bin cargo test"
                )
            });
            let bytes =
                std::fs::read(&path).unwrap_or_else(|e| panic!("read proving key at {path}: {e}"));
            // Fail here rather than inside a request handler.
            prover::load_proving_key(&bytes)
                .unwrap_or_else(|_| panic!("{path} is not a valid proving key"));
            bytes
        })
        .clone()
}

/// Test oracle wired to the shared fixture card. Returns
/// `(oracle, card, gsk, usk)`; the card IDm is always [`fixture::IDM`].
pub fn setup() -> (OracleImpl, Emulator, [u8; 8], [u8; 8]) {
    let f = fixture::setup();
    let config = fixture::app_config(&f);
    // In-memory key bytes (no fs in the request path): deserialization stays
    // in the prover crate and is cached by the handler.
    let key =
        ProvingKeyBytes::from_bytes(config.proving_key_path.clone(), test_proving_key_bytes());
    let oracle = OracleImpl::new(config, key);
    let fixture::Fixture { card, gsk, usk } = f;
    (oracle, card, gsk, usk)
}

/// challenge → card Authentication1, returning genuine `(c1b, c2a)` for `r1_hex`.
pub async fn auth1_flow(
    oracle: &OracleImpl,
    card: &mut Emulator,
    r1_hex: &str,
) -> ([u8; 8], [u8; 8]) {
    use felica::felica_standard::{FelicaStandardCommand, FelicaStandardResponse};

    let ch = oracle
        .challenge(ChallengeRequest {
            idm: fixture::IDM_HEX.to_string(),
            r1: r1_hex.to_string(),
        })
        .await
        .expect("challenge");
    assert_eq!(ch.system_code, 0x0003, "challenge carries the node path");
    assert_eq!(ch.areas, vec![fixture::AREA]);
    assert_eq!(ch.services, vec![fixture::SERVICE]);
    let c1a: [u8; 8] = hex::decode(&ch.c1a).unwrap().try_into().unwrap();
    let frame = card
        .handle_command(FelicaStandardCommand::Authentication1 {
            idm: fixture::IDM,
            areas: vec![fixture::AREA],
            services: vec![fixture::SERVICE],
            challenge_1a: c1a,
        })
        .expect("card answers authentication1");
    match FelicaStandardResponse::from_bytes(&frame).expect("parse") {
        FelicaStandardResponse::Authentication1 {
            challenge_1b,
            challenge_2a,
            ..
        } => (challenge_1b, challenge_2a),
        other => panic!("unexpected response: {other:?}"),
    }
}

pub fn settle_req(c1b: [u8; 8], c2a: [u8; 8]) -> SettleRequest {
    SettleRequest {
        idm: fixture::IDM_HEX.to_string(),
        r1: R1_HEX.to_string(),
        c1b: hex::encode(c1b),
        c2a: hex::encode(c2a),
    }
}

pub fn attest_req(c1b: [u8; 8], c2a: [u8; 8], auth2_ct: [u8; 32]) -> AttestRequest {
    AttestRequest {
        idm: fixture::IDM_HEX.to_string(),
        c1b: hex::encode(c1b),
        c2a: hex::encode(c2a),
        auth2: hex::encode(auth2_ct),
    }
}

/// Card Authentication2 → raw 32-byte AUTH2 ciphertext.
pub fn auth2_flow(card: &mut Emulator, c2b: [u8; 8]) -> [u8; 32] {
    use felica::felica_standard::{FelicaStandardCommand, FelicaStandardResponse};

    let frame = card
        .handle_command(FelicaStandardCommand::Authentication2 {
            idm: fixture::IDM,
            challenge_2b: c2b,
        })
        .expect("card answers authentication2");
    match FelicaStandardResponse::from_bytes(&frame).expect("parse") {
        FelicaStandardResponse::Authentication2(_) => {}
        other => panic!("unexpected response: {other:?}"),
    }
    frame[2..34].try_into().expect("32B ciphertext")
}
