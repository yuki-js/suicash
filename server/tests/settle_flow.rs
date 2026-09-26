mod common;

use common::{auth1_flow, auth2_flow, settle_req, setup, R1_HEX};
use oracle::oracle::fixture;

/// Full holder flow through the real RPC-layer methods:
/// challenge → card Auth1 → settle → card Auth2.
///
/// `settle` no longer emits an encrypted Read command. The reference
/// implementation returned `ecmd` here and verified the 40-byte read response
/// under the session key; at IDi level `auth2` already carries `idi`, so there
/// is no read to issue and no session key to publish.
#[tokio::test]
async fn settle_flow_completes_mutual_auth_via_emulator() {
    use felica::felica_standard::{FelicaStandardCommand, FelicaStandardResponse};

    let (oracle, mut card, _, _) = setup();
    let (c1b, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;

    let st = oracle.settle(settle_req(c1b, c2a)).await.expect("settle");
    let c2b: [u8; 8] = hex::decode(&st.c2b).unwrap().try_into().unwrap();

    // Card accepts C2B and answers with AUTH2.
    let frame = card
        .handle_command(FelicaStandardCommand::Authentication2 {
            idm: fixture::IDM,
            challenge_2b: c2b,
        })
        .expect("card answers authentication2");
    assert!(matches!(
        FelicaStandardResponse::from_bytes(&frame).expect("parse"),
        FelicaStandardResponse::Authentication2(_)
    ));
    assert_eq!(&frame[..2], &[34, 0x13], "AUTH2 frame: len + opcode");

    // And that ciphertext attests cleanly, closing the loop.
    let auth2_ct = auth2_flow(&mut card, c2b);
    let resp = oracle
        .attest(common::attest_req(c1b, c2a, auth2_ct))
        .await
        .expect("attest");
    assert_eq!(resp.idi, hex::encode(fixture::IDI));
}

/// `settle` returns `c2b` and nothing else.
#[tokio::test]
async fn settle_returns_c2b_only() {
    let (oracle, mut card, _, _) = setup();
    let (c1b, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    let st = oracle.settle(settle_req(c1b, c2a)).await.expect("settle");
    assert_eq!(st.c2b.len(), 16, "8-byte hex");
    let json = serde_json::to_value(&st).expect("serializes");
    let obj = json.as_object().expect("object");
    assert_eq!(
        obj.keys().collect::<Vec<_>>(),
        ["c2b"],
        "no ecmd, no read_spec, nothing else on the wire"
    );
}

#[tokio::test]
async fn settle_rejects_forged_c1b() {
    let (oracle, mut card, _, _) = setup();
    let (_, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    let bad_c1b: [u8; 8] = hex::decode("aabbccddeeff0011").unwrap().try_into().unwrap();
    let err = oracle
        .settle(settle_req(bad_c1b, c2a))
        .await
        .expect_err("forged c1b rejected");
    assert_eq!(err.code(), -32012);
}

/// The C1B check is only meaningful because `settle` requires the true `r1`.
/// Without it the check is recover-then-re-encrypt, which always matches.
#[tokio::test]
async fn settle_rejects_c1b_from_a_different_r1() {
    let (oracle, mut card, _, _) = setup();
    let (_, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    let (c1b_other, _) = auth1_flow(&oracle, &mut card, common::R1B_HEX).await;
    let err = oracle
        .settle(settle_req(c1b_other, c2a))
        .await
        .expect_err("c1b from another session rejected");
    assert_eq!(err.code(), -32012);
}

/// `settle` is unauthenticated at the transport layer by design — the spec
/// assigns client authentication and rate limiting to a gateway (§11), and
/// nothing in this crate enforces either.
///
/// Pinned as a decision on record rather than an oversight, because it matters
/// more here than it looks: the oracle holds the FeliCa master keys and
/// derives per-card session keys for any `idm` it is asked about, so this
/// endpoint is a card-key-derivation oracle. Put a gateway in front of it
/// before exposing it.
#[tokio::test]
async fn settle_has_no_transport_authentication() {
    let (oracle, mut card, _, _) = setup();
    let (c1b, c2a) = auth1_flow(&oracle, &mut card, R1_HEX).await;
    // No credential of any kind is presented, and it succeeds.
    let st = oracle.settle(settle_req(c1b, c2a)).await.expect("settle");
    assert_eq!(st.c2b.len(), 16);
}
