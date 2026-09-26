//! Emit one genuine attestation as the `SUICASH_ATTESTATION` JSON the
//! payment helper (`usb-poc/facepay/sui-pay.mjs`) consumes — an end-to-end
//! smoke probe for the *deployed* on-chain gate, no card reader or oracle
//! required.
//!
//! Run:
//!
//! ```text
//! cargo run --release --bin attest-json -- assets/proving_key.bin
//! SUICASH_ATTESTATION="$(cargo run --release --bin attest-json -- assets/proving_key.bin)" \
//!     node usb-poc/facepay/sui-pay.mjs verify 1020304050607080
//! ```
//!
//! The session is the emulated fixture card `felica-fixture` uses, but with a
//! *random* `r1` (so successive probes do not collide in the gate's dedup
//! store) and `attested_at = now` (so the on-chain drift check passes). The
//! keys are the emulator's derived test keys, not production master keys —
//! which suffices precisely because the circuit's statement is key-schedule
//! consistency, not possession of any particular key set (issue #1). What
//! the probe exercises is everything downstream of proving: the compressed
//! serialization, the PTB argument packing, `sui::groth16` on the pinned
//! ceremony key, the envelope cross-checks, the drift bound and the `r1`
//! burn.

use felica::felica_standard::{
    generate_service_keys_des, EmulatedArea as EmuArea, EmulatedService as EmuSvc,
    EmulatedSystem as EmuSys, FelicaStandardCommand, FelicaStandardEmulator,
    FelicaStandardResponse, ServiceCode,
};
use hex_literal::hex;
use prover::des::{des_encrypt, tdes_decrypt, tdes_encrypt};
use prover::{
    load_proving_key, proof_compressed_bytes, prove_compressed, public_inputs_bytes, ProveRequest,
};
use rand::RngCore;

/// Fixture card, identical to `felica-fixture` and `tests/common`.
const IDM: [u8; 8] = hex!("0102030405060708");
const IDI: [u8; 8] = hex!("1020304050607080");
const PMI: [u8; 8] = hex!("a0a1a2a3a4a5a6a7");
const SYSTEM_KEY: [u8; 8] = hex!("1122334455667788");
const AREA_KEY: [u8; 8] = hex!("21436587a9cbed0f");
const SERVICE_KEY: [u8; 8] = hex!("0102030405060708");
const SYSTEM_CODE: u16 = 0x0003;
const AREA: u16 = 0x0040;
const SERVICE: u16 = 0x0048;
const BLOCK: [u8; 16] = hex!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

fn main() {
    let pk_path = std::env::args()
        .nth(1)
        .expect("usage: attest-json <proving_key.bin>");
    let pk_bytes =
        std::fs::read(&pk_path).unwrap_or_else(|e| panic!("read proving key at {pk_path}: {e}"));
    let pk = load_proving_key(&pk_bytes).expect("deserialize proving key");

    let mut r1 = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut r1);
    let attested_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();

    let (c1b, c2a, auth2, gsk, usk) = mint(&r1);
    let (att, _) = prove_compressed(
        &pk,
        &ProveRequest {
            idm: IDM,
            c1b,
            c2a,
            auth2,
            gsk,
            usk,
            attested_at,
        },
    )
    .expect("genuine session proves");
    assert_eq!(att.idi, IDI, "fixture IDi drifted");

    // Round-trip through the wire form on purpose: this is the same path
    // facepay takes with a real oracle reply, so the probe covers it.
    let proof = proof_compressed_bytes(&att.proof).expect("wire proof reserializes");
    let pis = public_inputs_bytes(&att.proof).expect("wire public inputs repack");

    println!(
        "{}",
        serde_json::json!({
            "idi": hex::encode(att.idi),
            "attested_at": att.attested_at,
            "r1": hex::encode(r1),
            "proof": hex::encode(proof),
            "public_inputs": hex::encode(pis),
        })
    );
}

/// One full mutual authentication against the emulated card, with the given
/// holder challenge. Mirrors `felica-fixture::mint_fixed` but parameterised
/// over `r1`.
fn mint(r1: &[u8; 8]) -> ([u8; 8], [u8; 8], [u8; 32], [u8; 8], [u8; 8]) {
    let (gsk, usk) = generate_service_keys_des(&SYSTEM_KEY, &[AREA_KEY], &[SERVICE_KEY]);
    let mut emusys = EmuSys::new(SYSTEM_CODE, IDM, PMI).expect("system");
    emusys.set_system_key(SYSTEM_KEY);
    emusys.set_idi_pmi(IDI, PMI);
    let mut emuarea = EmuArea::new(AREA, 0x00FF).expect("area");
    emuarea.set_key(AREA_KEY);
    let mut emusvc = EmuSvc::with_blocks(ServiceCode::new(SERVICE), 0x0000, vec![BLOCK]);
    emusvc.set_key(SERVICE_KEY);
    emuarea.add_service(emusvc).expect("service fits");
    emusys.add_area(emuarea).expect("area fits");
    let mut card = FelicaStandardEmulator::new();
    card.add_system(emusys);

    let l: [u8; 8] = std::array::from_fn(|i| gsk[i] ^ IDM[i]);
    let alpha = des_encrypt(&usk, &l);
    let beta = des_encrypt(&l, &alpha);

    let c1a = tdes_encrypt(r1, &alpha, &l);
    let frame = card
        .handle_command(FelicaStandardCommand::Authentication1 {
            idm: IDM,
            areas: vec![AREA],
            services: vec![SERVICE],
            challenge_1a: c1a,
        })
        .expect("card answers authentication1");
    let (c1b, c2a) = match FelicaStandardResponse::from_bytes(&frame).expect("parse") {
        FelicaStandardResponse::Authentication1 {
            challenge_1b,
            challenge_2a,
            ..
        } => (challenge_1b, challenge_2a),
        other => panic!("unexpected response: {other:?}"),
    };
    let r2 = tdes_decrypt(&c2a, &l, &beta);
    let c2b = tdes_encrypt(&r2, &alpha, &l);
    let frame = card
        .handle_command(FelicaStandardCommand::Authentication2 {
            idm: IDM,
            challenge_2b: c2b,
        })
        .expect("card answers authentication2");
    assert_eq!(&frame[..2], &[34, 0x13], "AUTH2 frame");
    let auth2: [u8; 32] = frame[2..34].try_into().expect("32B ciphertext");
    (c1b, c2a, auth2, gsk, usk)
}
