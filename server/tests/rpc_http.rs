//! JSON-RPC over HTTP: spec `docs/spec.md` §8 surface test.
//!
//! The direct-call unit tests bypass serialization; this binary drives the real
//! HTTP server (same `ServerConfig` as `src/main.rs`, batch disabled) with
//! spec-shaped object params and asserts wire shapes, error codes, and the
//! prover connection (`attest` returns a verifying Groth16 attestation).
//!
//! NOTE: `settle` keeps the documented `r1` extension
//! (`src/api/types.rs::SettleRequest`): params are
//! `{idm, r1, c1b, c2a}`. `challenge` likewise returns the node
//! path (`system_code/areas/services`) alongside `c1a`.

mod common;

use std::net::SocketAddr;

use oracle::api::{OracleApiServer, OracleImpl};
use oracle::oracle::fixture;
use oracle::params::ProvingKeyBytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const R1_HEX: &str = "0011223344556677";

async fn start_server() -> (
    jsonrpsee::server::ServerHandle,
    SocketAddr,
    fixture::Fixture,
) {
    let f = fixture::setup();
    let config = fixture::app_config(&f);
    // Real ceremony key, same source as the other suites -- see
    // `common::test_proving_key_bytes` for why a placeholder cannot work.
    let key = ProvingKeyBytes::from_bytes(
        config.proving_key_path.clone(),
        common::test_proving_key_bytes(),
    );
    let module = OracleImpl::new(config, key).into_rpc();
    // Same config as `src/main.rs`: spec §8 forbids batch requests.
    let cfg = jsonrpsee::server::ServerConfig::builder()
        .set_batch_request_config(jsonrpsee::server::BatchRequestConfig::Disabled)
        .build();
    let server = jsonrpsee::server::Server::builder()
        .set_config(cfg)
        .build("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let handle = server.start(module);
    (handle, addr, f)
}

/// Raw HTTP/1.1 POST (no extra client deps); returns the parsed JSON body.
async fn rpc(addr: SocketAddr, body: serde_json::Value) -> serde_json::Value {
    let raw = serde_json::to_vec(&body).unwrap();
    let head = format!(
        "POST / HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        raw.len()
    );
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
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

/// challenge (HTTP, spec §8.1 object params) → card Authentication1.
async fn http_auth1(
    addr: SocketAddr,
    card: &mut felica::felica_standard::FelicaStandardEmulator,
    r1_hex: &str,
) -> ([u8; 8], [u8; 8]) {
    let resp = rpc(
        addr,
        call(
            "challenge",
            1,
            serde_json::json!({"idm": fixture::IDM_HEX, "r1": r1_hex}),
        ),
    )
    .await;
    let result = resp.get("result").expect("challenge result");
    assert_eq!(result["system_code"], fixture::SYSTEM_CODE);
    assert_eq!(result["areas"], serde_json::json!([fixture::AREA]));
    assert_eq!(result["services"], serde_json::json!([fixture::SERVICE]));
    let c1a: [u8; 8] = hex::decode(result["c1a"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let frame = card
        .handle_command(
            felica::felica_standard::FelicaStandardCommand::Authentication1 {
                idm: fixture::IDM,
                areas: vec![fixture::AREA],
                services: vec![fixture::SERVICE],
                challenge_1a: c1a,
            },
        )
        .expect("card answers authentication1");
    match felica::felica_standard::FelicaStandardResponse::from_bytes(&frame).expect("parse") {
        felica::felica_standard::FelicaStandardResponse::Authentication1 {
            challenge_1b,
            challenge_2a,
            ..
        } => (challenge_1b, challenge_2a),
        other => panic!("unexpected response: {other:?}"),
    }
}

/// Card Authentication2 → raw 32-byte AUTH2 ciphertext.
fn card_auth2(
    card: &mut felica::felica_standard::FelicaStandardEmulator,
    c2b: [u8; 8],
) -> [u8; 32] {
    let frame = card
        .handle_command(
            felica::felica_standard::FelicaStandardCommand::Authentication2 {
                idm: fixture::IDM,
                challenge_2b: c2b,
            },
        )
        .expect("card answers authentication2");
    frame[2..34].try_into().expect("32B ciphertext")
}

#[tokio::test]
async fn http_challenge_settle_flow_and_errors() {
    let (handle, addr, f) = start_server().await;
    let fixture::Fixture { mut card, gsk, usk } = f;

    // challenge → card Auth1 → settle → card Auth2.
    let (c1b, c2a) = http_auth1(addr, &mut card, R1_HEX).await;
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
    let result = resp.get("result").expect("settle result");
    let c2b: [u8; 8] = hex::decode(result["c2b"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    // `settle` returns `c2b` and nothing else. The reference implementation
    // also emitted `ecmd`, a 26-byte encrypted Read command; at IDi level
    // there is no read, so the field is gone rather than null.
    let obj = result.as_object().expect("object");
    assert_eq!(
        obj.keys().collect::<Vec<_>>(),
        ["c2b"],
        "settle payload is c2b only"
    );
    let auth2_ct = card_auth2(&mut card, c2b);
    assert_eq!(auth2_ct.len(), 32);
    // Sanity: the keys are wired (unused now that the read leg is gone).
    let _ = oracle::oracle::OracleKeys::new(gsk, usk);

    // Errors: bad hex → -32602; forged c1b → -32012.
    let resp = rpc(
        addr,
        call(
            "challenge",
            4,
            serde_json::json!({"idm": "zzzz", "r1": R1_HEX}),
        ),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602);
    let resp = rpc(
        addr,
        call(
            "settle",
            5,
            serde_json::json!({
                "idm": fixture::IDM_HEX,
                "r1": R1_HEX,
                "c1b": "aabbccddeeff0011",
                "c2a": hex::encode(c2a),
            }),
        ),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32012);
    // Protocol break: a stale client still sending `read_spec` gets a *valid*
    // response, because jsonrpsee extracts named params individually and
    // silently ignores ones the method does not declare. `#[serde(
    // deny_unknown_fields)]` on `SettleRequest` cannot help — the struct is
    // assembled by hand after extraction, so it never sees the params object.
    //
    // Pinned here because it is a real hazard, not a hypothetical: the caller
    // asked for a read and receives an attestation with no read in it. The
    // response shape is unchanged, so nothing errors. See README "Protocol
    // break vs the reference".
    let resp = rpc(
        addr,
        call(
            "settle",
            6,
            serde_json::json!({
                "idm": fixture::IDM_HEX,
                "r1": R1_HEX,
                "c1b": hex::encode(c1b),
                "c2a": hex::encode(c2a),
                "read_spec": {"service": 0x0999, "block": 0},
            }),
        ),
    )
    .await;
    assert_eq!(
        resp["result"]["c2b"].as_str().map(str::len),
        Some(16),
        "stale read_spec is ignored, not honoured: {resp}"
    );

    // Batch is not supported (spec §8): the server must not answer with
    // per-call results.
    let batch = serde_json::json!([
        {"jsonrpc": "2.0", "id": 7, "method": "ping", "params": []},
        {"jsonrpc": "2.0", "id": 8, "method": "ping", "params": []},
    ]);
    let resp = rpc(addr, batch).await;
    let ok = resp.get("error").is_some()
        || resp
            .as_array()
            .is_some_and(|a| a.iter().any(|e| e.get("error").is_some()));
    assert!(ok, "batch must be rejected: {resp}");

    handle.stop().unwrap();
}

/// Full holder flow over HTTP through `attest`: the response carries the
/// Groth16 attestation binding the verified session (prover connection).
/// Heavy: one real proof (~130k constraints).
#[tokio::test]
async fn http_attest_proves_session() {
    let (handle, addr, f) = start_server().await;
    let fixture::Fixture { mut card, .. } = f;

    let (c1b, c2a) = http_auth1(addr, &mut card, R1_HEX).await;
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
    let c2b: [u8; 8] = hex::decode(resp["result"]["c2b"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let auth2_ct = card_auth2(&mut card, c2b);

    let resp = rpc(
        addr,
        call(
            "attest",
            3,
            serde_json::json!({
                "idm": fixture::IDM_HEX,
                "c1b": hex::encode(c1b),
                "c2a": hex::encode(c2a),
                "auth2": hex::encode(auth2_ct),
            }),
        ),
    )
    .await;
    let result = resp.get("result").expect("attest result").clone();
    assert_eq!(result["idi"], hex::encode(fixture::IDI));
    assert_eq!(result["proof"]["alg"], "groth16-bn254");
    assert_eq!(
        result["proof"]["public_inputs"].as_array().unwrap().len(),
        3,
        "IDi-level packing"
    );
    assert!(result["attested_at"].as_u64().unwrap() > 0);

    // The envelope is IDi-only: no cm_out, no r2.
    let obj = result.as_object().expect("object");
    assert!(!obj.contains_key("cm_out"), "no commitment on the wire");
    assert!(!obj.contains_key("r2"), "no session key on the wire");

    // The three public inputs pin idi and r1, little-endian.
    let pi: Vec<Vec<u8>> = result["proof"]["public_inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| hex::decode(s.as_str().unwrap()).unwrap())
        .collect();
    for (i, raw) in pi.iter().enumerate() {
        assert_eq!(raw.len(), 32, "limb {i} is one 32-byte LE field element");
    }
    assert_eq!(&pi[0][..8], &fixture::IDI, "pi0 = idi");
    let r1: [u8; 8] = hex::decode(R1_HEX).unwrap().try_into().unwrap();
    assert_eq!(&pi[1][..8], &r1, "pi1 = r1");
    assert_eq!(
        u64::from_le_bytes(pi[2][..8].try_into().unwrap()),
        result["attested_at"].as_u64().unwrap(),
        "pi2 = attested_at"
    );

    // A stale `cm` is ignored for the same reason as `read_spec` above, and
    // this is the sharper edge of the two: a reference-era client sends `cm`,
    // gets a valid proof, and the commitment it asked to be bound is simply
    // absent. It does fail — but on the client, when it looks for `cm_out` in
    // a response that no longer has the field. Pinned so the behaviour is a
    // decision on record rather than a surprise.
    let resp = rpc(
        addr,
        call(
            "attest",
            40,
            serde_json::json!({
                "idm": fixture::IDM_HEX,
                "c1b": hex::encode(c1b),
                "c2a": hex::encode(c2a),
                "auth2": hex::encode(auth2_ct),
                "cm": "00".repeat(32),
            }),
        ),
    )
    .await;
    assert!(
        resp.get("result").is_some(),
        "stale cm is accepted and ignored: {resp}"
    );
    assert!(
        resp["result"].get("cm_out").is_none(),
        "and the response carries no commitment at all: {resp}"
    );

    // The `get_proving_key` RPC was removed (issue #1 hygiene): the proving key
    // is not a secret, but serving it advertised the attack surface.
    let resp = rpc(addr, call("get_proving_key", 41, serde_json::json!([]))).await;
    assert!(
        resp["error"].is_object(),
        "get_proving_key must not exist: {resp}"
    );

    // Tampered AUTH2 → MAC_MISMATCH (-32010).
    let mut bad = auth2_ct;
    bad[0] ^= 0xFF;
    let resp = rpc(
        addr,
        call(
            "attest",
            5,
            serde_json::json!({
                "idm": fixture::IDM_HEX,
                "c1b": hex::encode(c1b),
                "c2a": hex::encode(c2a),
                "auth2": hex::encode(bad),
            }),
        ),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32010);

    handle.stop().unwrap();
}
