//! End-to-end over real HTTP: `challenge` → `settle` → `attest`, then local
//! verification of the returned proof against the published verifying key.
//!
//! Run it against a server started with the *same* key material, which the
//! fixture derives deterministically. This example prints the exact
//! `FELICA_KEYS_JSON` and `FELICA_PROVING_KEY_PATH` it needs:
//!
//! ```sh
//! # terminal 1 — the lines printed below
//! export FELICA_KEYS_JSON='<printed>'
//! export FELICA_PROVING_KEY_PATH=<printed>
//! export FELICA_BIND_ADDR=127.0.0.1:3000
//! cargo run --release
//!
//! # terminal 2
//! RPC_ADDR=127.0.0.1:3000 cargo run --release --example rpc_attest
//! ```
//!
//! `RPC_ADDR` defaults to `127.0.0.1:3000`.
//!
//! The card is the shared emulator fixture, so the server's keys must be the
//! fixture's derived GSK/USK. Any other key material makes the card reject
//! C1A, and the first `challenge` call fails with a JSON-RPC error.

use std::net::SocketAddr;

use felica::felica_standard::{FelicaStandardCommand, FelicaStandardResponse};
use oracle::oracle::fixture;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const R1_HEX: &str = "0011223344556677";

async fn rpc(addr: SocketAddr, body: serde_json::Value) -> serde_json::Value {
    let raw = serde_json::to_vec(&body).unwrap();
    let head = format!(
        "POST / HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        raw.len()
    );
    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "cannot reach the oracle at {addr}: {e}\n\
             start one with the FELICA_KEYS_JSON printed below"
            )
        });
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(&raw).await.unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let start = text.find("\r\n\r\n").unwrap() + 4;
    serde_json::from_str(&text[start..]).unwrap()
}

fn call(method: &str, id: u64, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// Unwrap a JSON-RPC `result`, surfacing `error` with its code and message.
///
/// The bare `.get("result").unwrap()` this replaces panicked with
/// `Option::unwrap() on None`, hiding the fact that the server had returned a
/// structured error — which is exactly what you get when the server's keys do
/// not match the card's.
fn result(v: &serde_json::Value) -> serde_json::Value {
    if let Some(err) = v.get("error") {
        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("(no message)");
        panic!("JSON-RPC error {code}: {msg}\nfull response: {v}");
    }
    v.get("result")
        .unwrap_or_else(|| panic!("response has neither result nor error: {v}"))
        .clone()
}

fn hex8(v: &serde_json::Value) -> [u8; 8] {
    hex::decode(v.as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap()
}

/// Build an `Attestation` purely from the RPC response.
///
/// `cm_out` is read from the response, never filled in from the request: the
/// previous version hardcoded `[0u8; 32]` and the caller patched it by hand
/// after the fact, which is exactly the envelope-consistency bug this example
/// should have been catching. If the server omits or corrupts `cm_out`, the
/// verification below now fails instead of being papered over.
fn to_attestation(result: &serde_json::Value) -> prover::Attestation {
    let proof = &result["proof"];
    let pair = |v: &serde_json::Value| {
        let a = v.as_array().unwrap();
        (
            a[0].as_str().unwrap().to_string(),
            a[1].as_str().unwrap().to_string(),
        )
    };
    let b = proof["b"].as_array().unwrap();
    prover::Attestation {
        idi: hex8(&result["idi"]),
        attested_at: result["attested_at"].as_u64().unwrap(),
        proof: prover::Groth16Proof {
            alg: proof["alg"].as_str().unwrap().to_string(),
            a: pair(&proof["a"]),
            b: (pair(&b[0]), pair(&b[1])),
            c: pair(&proof["c"]),
            public_inputs: proof["public_inputs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_string())
                .collect(),
        },
    }
}

#[tokio::main]
async fn main() {
    let addr: SocketAddr = std::env::var("RPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()
        .unwrap();
    let fixture::Fixture { mut card, gsk, usk } = fixture::setup();
    println!("start the server with exactly these keys:\n");
    println!(
        "  FELICA_KEYS_JSON='{{\"gsk\":\"{}\",\"usk\":\"{}\",\"system_code\":{},\"areas\":[{}],\"services\":[{}]}}'",
        hex::encode(gsk),
        hex::encode(usk),
        fixture::SYSTEM_CODE,
        fixture::AREA,
        fixture::SERVICE
    );
    // Absolute, so the line above is copy-pasteable from any working
    // directory. A CWD-relative `../prover/...` only resolves when the server
    // is launched from this package directory, which is a sharp edge to hand
    // someone.
    let key_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../prover/assets/proving_key.bin");
    let key_path = key_path.canonicalize().unwrap_or(key_path);
    println!("  FELICA_PROVING_KEY_PATH={}", key_path.display());
    println!("  FELICA_BIND_ADDR={addr}\n");

    // --- auth-only attest ---
    let resp = rpc(
        addr,
        call(
            "challenge",
            1,
            serde_json::json!({"idm": fixture::IDM_HEX, "r1": R1_HEX}),
        ),
    )
    .await;
    let c1a: [u8; 8] = hex8(&resp["result"]["c1a"]);

    let frame = card
        .handle_command(FelicaStandardCommand::Authentication1 {
            idm: fixture::IDM,
            areas: vec![fixture::AREA],
            services: vec![fixture::SERVICE],
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

    let resp = rpc(
        addr,
        call(
            "settle",
            2,
            serde_json::json!({
                "idm": fixture::IDM_HEX,
                "r1": R1_HEX,
                "c1b": hex::encode(c1b),
                "c2a": hex::encode(c2a),
            }),
        ),
    )
    .await;
    let c2b: [u8; 8] = hex8(&resp["result"]["c2b"]);

    let frame = card
        .handle_command(FelicaStandardCommand::Authentication2 {
            idm: fixture::IDM,
            challenge_2b: c2b,
        })
        .expect("card answers authentication2");
    let auth2_ct = hex::encode(&frame[2..34]);

    let resp = rpc(
        addr,
        call(
            "attest",
            3,
            serde_json::json!({
                "idm": fixture::IDM_HEX,
                "c1b": hex::encode(c1b),
                "c2a": hex::encode(c2a),
                "auth2": auth2_ct,
            }),
        ),
    )
    .await;
    let result = result(&resp);
    println!("auth-only idi: {}", result["idi"]);
    assert_eq!(
        result["idi"],
        serde_json::Value::String(hex::encode(fixture::IDI))
    );
    // --- local verification against the oracle-published vk ---
    //
    // NOTE: this fetches the verifying key from the same party whose attestation
    // it then checks, so it proves *plumbing*, not trust. A malicious oracle
    // would serve its own `vk` and this would still pass. Real verification
    // means pinning the 360-byte vk out of band — from the ceremony output or
    // a package constant — which is what an on-chain verifier does.
    let resp = rpc(addr, call("get_verifying_key", 4, serde_json::json!([]))).await;
    let vk_bytes = hex::decode(resp["result"].as_str().unwrap()).unwrap();
    println!("vk: {} bytes", vk_bytes.len());
    assert_eq!(
        vk_bytes.len(),
        360,
        "BN254 Groth16 vk for 3 public inputs is 264 + 3*32 = 360 bytes"
    );
    let vk = prover::load_verifying_key(&vk_bytes).expect("vk loads");
    let att = to_attestation(&result);
    assert!(
        prover::verify_attestation(&vk, &att),
        "proof verifies against the RPC-served vk"
    );
    assert_eq!(att.idi, fixture::IDI, "attested IDi matches the card");
    assert_eq!(att.proof.public_inputs.len(), 3, "IDi-level packing");

    println!("\nIDi attestation verified: {}", hex::encode(att.idi));
    println!("(this proves transcript consistency + key-schedule binding, NOT card presence)");
    println!("see ../prover/README.md — the P0 remains open");
}
