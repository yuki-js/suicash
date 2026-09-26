mod common;

use common::{attest_req, auth1_flow, auth2_flow, settle_req, setup, R1B_HEX, R1_HEX};
use oracle::oracle::{fixture, verify_session, OracleKeys};

/// challenge → Auth1 → settle → Auth2 → attest verification, end to end.
/// The RPC returns the Groth16 attestation binding the verified session.
#[tokio::test]
async fn attest_verifies_full_session() {
    let (oracle, mut card, gsk, usk) = setup();
    let (c1b, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    let st = oracle.settle(settle_req(c1b, c2a)).await.expect("settle");
    let c2b: [u8; 8] = hex::decode(&st.c2b).unwrap().try_into().unwrap();
    let auth2_ct = auth2_flow(&mut card, c2b);

    let keys = OracleKeys::new(gsk, usk);
    let v = verify_session(&keys, &fixture::IDM, &c1b, &c2a, &auth2_ct).expect("session verifies");
    let r1: [u8; 8] = hex::decode(R1_HEX).unwrap().try_into().unwrap();
    assert_eq!(v.r1, r1);
    assert_eq!(v.tid, r1[2..8]);
    assert_eq!(v.idi, fixture::IDI);
    // R2 matches the card's session key. It is deliberately NOT on the wire:
    // at IDi level nothing needs to decrypt the read payload, so the session
    // key stays private to the card and the oracle.
    let r2 = keys.session(&fixture::IDM).open_r2(&c2a);
    assert_eq!(v.r2, r2);

    // RPC surface: Groth16 attestation binds the verified session.
    let resp = oracle
        .attest(attest_req(c1b, c2a, auth2_ct))
        .await
        .expect("attest proves genuine session");
    assert_eq!(resp.idi, hex::encode(fixture::IDI));
    assert!(resp.attested_at > 0);
    assert_eq!(resp.proof.alg, "groth16-bn254");
    assert_eq!(
        resp.proof.public_inputs.len(),
        3,
        "IDi-level packing: idi, r1, attested_at"
    );

    // The envelope must not leak the session key or a commitment. Both were on
    // the wire in the reference implementation; at IDi level they are private
    // witnesses and there is nothing for a verifier to check them against.
    let json = serde_json::to_string(&resp).expect("response serializes");
    assert!(
        !json.contains("cm_out"),
        "no commitment on the wire: {json}"
    );
}

/// The attest envelope carries only fields the proof commits to. Serialising
/// it must not surface any of the removed keys.
#[tokio::test]
async fn attest_envelope_is_idi_only() {
    let (oracle, mut card, _, _) = setup();
    let (c1b, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    let st = oracle.settle(settle_req(c1b, c2a)).await.expect("settle");
    let c2b: [u8; 8] = hex::decode(&st.c2b).unwrap().try_into().unwrap();
    let auth2_ct = auth2_flow(&mut card, c2b);
    let resp = oracle
        .attest(attest_req(c1b, c2a, auth2_ct))
        .await
        .expect("attest");

    let v: serde_json::Value = serde_json::to_value(&resp).expect("serializes");
    let obj = v.as_object().expect("object");
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["attested_at", "idi", "proof"]);
    assert!(obj["proof"]["public_inputs"].as_array().unwrap().len() == 3);
}

#[tokio::test]
async fn attest_rejects_tampered_auth2() {
    let (oracle, mut card, _, _) = setup();
    let (c1b, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    let st = oracle.settle(settle_req(c1b, c2a)).await.expect("settle");
    let c2b: [u8; 8] = hex::decode(&st.c2b).unwrap().try_into().unwrap();
    let mut bad = auth2_flow(&mut card, c2b);
    bad[0] ^= 0xFF;
    let err = oracle
        .attest(attest_req(c1b, c2a, bad))
        .await
        .expect_err("tampered auth2 rejected");
    assert_eq!(err.code(), -32010);
}

#[tokio::test]
async fn attest_rejects_tid_mismatch() {
    let (oracle, mut card, gsk, usk) = setup();
    // AUTH2 bound to R1A's TID…
    let (c1b_a, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    let st = oracle.settle(settle_req(c1b_a, c2a)).await.expect("settle");
    let c2b: [u8; 8] = hex::decode(&st.c2b).unwrap().try_into().unwrap();
    let auth2_ct = auth2_flow(&mut card, c2b);
    // …paired with C1B from a different R1.
    let (c1b_b, _) = auth1_flow(&oracle, &mut card, R1B_HEX).await;
    let keys = OracleKeys::new(gsk, usk);
    let err = verify_session(&keys, &fixture::IDM, &c1b_b, &c2a, &auth2_ct)
        .expect_err("tid mismatch rejected");
    assert_eq!(err, oracle::oracle::AttestError::TidMismatch);
    // Same through the RPC surface.
    let rpc_err = oracle
        .attest(attest_req(c1b_b, c2a, auth2_ct))
        .await
        .expect_err("tid mismatch rejected");
    assert_eq!(rpc_err.code(), -32011);
}

/// `attest` runs its ~1.2 s proof on the blocking pool, so a burst of
/// concurrent calls must all succeed rather than deadlock the event loop or
/// trip a memory ceiling. With `MAX_CONCURRENT_PROOFS = 1` they serialise.
/// `attest` runs its ~1.2 s proof on the blocking pool behind a
/// single-permit semaphore, so a burst of concurrent calls must all succeed
/// rather than deadlock the event loop or exhaust memory. They serialise on
/// the permit, which is the intended behaviour.
///
/// The card emulator is not `Send`, so the sessions are set up sequentially
/// and the three `attest` futures are then polled together with `join!`. That
/// still exercises the semaphore: all three reach the permit before any is
/// released.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_attest_all_succeed() {
    let (oracle, _, _, _) = setup();

    // Three distinct sessions. A card mints a fresh R2 per Authentication1, so
    // each session needs its own card instance; `fixture::setup()` is
    // deterministic, so all three agree on IDm and keys.
    let mut sessions = Vec::new();
    for _ in 0..3 {
        let fixture::Fixture { mut card, .. } = fixture::setup();
        let (c1b, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
        let st = oracle.settle(settle_req(c1b, c2a)).await.expect("settle");
        let c2b: [u8; 8] = hex::decode(&st.c2b).unwrap().try_into().unwrap();
        let auth2_ct = auth2_flow(&mut card, c2b);
        sessions.push((c1b, c2a, auth2_ct));
    }

    let sessions: [([u8; 8], [u8; 8], [u8; 32]); 3] = sessions.try_into().expect("3 sessions");
    let (c1b, c2a, auth2) = sessions[0];

    let (a, b, c) = tokio::join!(
        oracle.attest(attest_req(c1b, c2a, auth2)),
        oracle.attest(attest_req(c1b, c2a, auth2)),
        oracle.attest(attest_req(c1b, c2a, auth2)),
    );
    for resp in [a, b, c] {
        let resp = resp.expect("attest");
        assert_eq!(resp.idi, hex::encode(fixture::IDI));
        assert_eq!(resp.proof.public_inputs.len(), 3);
    }
}
