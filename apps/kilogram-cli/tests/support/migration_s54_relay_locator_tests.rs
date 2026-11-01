//! Real protocol builders over signed synthetic public data; no enrollment or process.
use super::*;
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, DeviceCapability, DeviceCertificate,
    DeviceEncryptionIdentity, DeviceIdentity, EncryptionPublicKey,
};
use kilogram_protocol::ordered_dialog_v2 as v2;
use serde::Serialize;

#[derive(Serialize)]
struct Signed<T> {
    content: T,
    signature: Vec<u8>,
}
#[derive(Serialize)]
struct Certificate {
    version: u8,
    account_id: AccountId,
    device_id: kilogram_identity::DeviceId,
    encryption_public_key: EncryptionPublicKey,
    authority_sequence: u64,
    capabilities: Vec<DeviceCapability>,
}
#[derive(Serialize)]
struct Authority {
    version: u8,
    account_id: AccountId,
    revision: u64,
    revocations: Vec<kilogram_identity::DeviceRevocation>,
}
fn signed<T: Serialize>(content: T, root: &DeviceIdentity, tag: &[u8]) -> Result<Vec<u8>> {
    let mut message = tag.to_vec();
    message.extend(postcard::to_allocvec(&content)?);
    let signature = root.sign(&message).to_vec();
    Ok(postcard::to_allocvec(&Signed { content, signature })?)
}
fn peers() -> Result<([DeviceIdentity; 2], [v2::RootVerifiedCredential; 2])> {
    let devices = [
        DeviceIdentity::from_secret_bytes([31; 32]),
        DeviceIdentity::from_secret_bytes([52; 32]),
    ];
    let mut credentials = Vec::new();
    for (role, device) in devices.iter().enumerate() {
        let root = DeviceIdentity::from_secret_bytes([91 + role as u8; 32]);
        let account = AccountId::from_bytes(*root.device_id().as_bytes());
        let raw = signed(
            Certificate {
                version: 2,
                account_id: account,
                device_id: device.device_id(),
                encryption_public_key: DeviceEncryptionIdentity::from_secret_bytes(
                    [61 + role as u8; 32],
                )
                .public_key(),
                authority_sequence: 1,
                capabilities: DeviceCapability::MESSAGING.to_vec(),
            },
            &root,
            b"kilogram:device-certificate-signature:v2\0",
        )?;
        let cert = DeviceCertificate::decode_and_verify(&raw)?;
        let raw = signed(
            Authority {
                version: 1,
                account_id: account,
                revision: 2,
                revocations: vec![],
            },
            &root,
            b"kilogram:account-authority-snapshot-signature:v1\0",
        )?;
        credentials.push(v2::RootVerifiedCredential::from_parts(
            cert,
            AccountAuthoritySnapshot::decode_and_verify(&raw)?,
        )?);
    }
    Ok((
        devices,
        credentials
            .try_into()
            .map_err(|_| anyhow::anyhow!("G3 test credential pair"))?,
    ))
}
const URL: &str = "http://127.0.0.1:40123/";
fn fields(peers: &[v2::RootVerifiedCredential; 2], role: usize) -> v2::RuntimeLocatorFields2 {
    v2::RuntimeLocatorFields2 {
        c: [17; 32],
        cert: peers[role].cert_ref(),
        peer_cert: peers[1 - role].cert_ref(),
        endpoint: [7 + role as u8; 32],
        ips: vec![],
        relay: Some(URL.into()),
        revision: 2,
        not_before: 100,
        expires: 200,
    }
}
#[test]
fn s54_p10b_signed_two_context_locators_require_current_scope_role_and_signature() -> Result<()> {
    let (devices, peers) = peers()?;
    for context in [[17; 32], [18; 32]] {
        for (owner, device) in devices.iter().enumerate() {
            let mut f = fields(&peers, owner);
            f.c = context;
            let signed = v2::ParsedRuntimeLocator2::sign(f, device)?;
            locators::verify(signed.raw(), context, &peers, owner, 150, Some(URL), None)?;
            ensure!(
                locators::verify(signed.raw(), [99; 32], &peers, owner, 150, Some(URL), None)
                    .is_err()
                    && locators::verify(
                        signed.raw(),
                        context,
                        &peers,
                        1 - owner,
                        150,
                        Some(URL),
                        None
                    )
                    .is_err(),
                "wrong actual context/role cannot import"
            );
            let mut bad = signed.raw().to_vec();
            let last = bad.last_mut().context("signed bytes")?;
            *last ^= 1;
            ensure!(
                locators::verify(&bad, context, &peers, owner, 150, Some(URL), None).is_err(),
                "invalid signature"
            );
        }
    }
    Ok(())
}
#[test]
fn s54_p10b_signed_url_only_locators_reject_ip_other_url_expiry_and_cert_substitution() -> Result<()>
{
    let (devices, peers) = peers()?;
    for mutation in 0..5 {
        let mut f = fields(&peers, 0);
        match mutation {
            0 => f.ips.push("127.0.0.1:12345".parse()?),
            1 => f.relay = Some("http://127.0.0.1:40124/".into()),
            2 => f.cert = [99; 32],
            3 => f.peer_cert = [99; 32],
            _ => {
                f.relay = None;
                f.ips.push("127.0.0.1:12345".parse()?);
            }
        }
        let signed = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?;
        ensure!(
            locators::verify(signed.raw(), [17; 32], &peers, 0, 150, Some(URL), None).is_err(),
            "signed but wrong relay publication {mutation}"
        );
    }
    let signed = v2::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    for now in [99, 200] {
        ensure!(
            locators::verify(signed.raw(), [17; 32], &peers, 0, now, Some(URL), None).is_err(),
            "not current interval"
        );
    }
    Ok(())
}
#[test]
fn s54_p10b_route_flip_requires_strict_signed_revision_not_endpoint_rotation() -> Result<()> {
    let (devices, peers) = peers()?;
    let mut before = fields(&peers, 0);
    before.revision = 1;
    before.relay = None;
    before.ips = vec!["127.0.0.1:12345".parse()?];
    let before = v2::ParsedRuntimeLocator2::sign(before, &devices[0])?;
    let prior = locators::verify(before.raw(), [17; 32], &peers, 0, 150, None, None)?;
    let after = v2::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    let actual = locators::verify(
        after.raw(),
        [17; 32],
        &peers,
        0,
        150,
        Some(URL),
        Some(&prior),
    )?;
    ensure!(
        actual.endpoint == prior.endpoint && actual.revision == 2,
        "same endpoint is valid after actual route content/revision change"
    );
    let mut stale = fields(&peers, 0);
    stale.revision = 1;
    let stale = v2::ParsedRuntimeLocator2::sign(stale, &devices[0])?;
    ensure!(
        locators::verify(
            stale.raw(),
            [17; 32],
            &peers,
            0,
            150,
            Some(URL),
            Some(&prior)
        )
        .is_err()
            && locators::verify(
                before.raw(),
                [17; 32],
                &peers,
                0,
                150,
                Some(URL),
                Some(&prior)
            )
            .is_err(),
        "stale revision/direct substitute"
    );
    Ok(())
}
fn admitted_row() -> Value {
    json!({"slot":0,"mode":"Active","policy_error":null,"handoff_policy_error":null,"lease_current":true,"lease_family":2,"lease_generation":7,"publication_ready":true,"publication_error":null,"existing_origin_ready":true,"origin_publication_error":null,"own_locator_revision":2,"peer_locator_revision":2})
}
#[test]
fn s54_p10b_actual_relay_admission_rejects_config_only_stale_head_revision_and_ready_slot()
-> Result<()> {
    let (devices, peers) = peers()?;
    let own = v2::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    let peer = v2::ParsedRuntimeLocator2::sign(fields(&peers, 1), &devices[1])?;
    let own = locators::verify(own.raw(), [17; 32], &peers, 0, 150, Some(URL), None)?;
    let peer = locators::verify(peer.raw(), [17; 32], &peers, 1, 150, Some(URL), None)?;
    let row = admitted_row();
    let route = json!({"slot":0,"policy":"relay-only","selected_path":"relay"});
    let head = Some(([20; 32], 7, 640));
    ensure!(
        locators::admitted(&row, &route, &own, &peer, head, true)?,
        "actual current admission joined to signed publications"
    );
    let mut config_only = route.clone();
    config_only["selected_path"] = Value::Null;
    ensure!(
        !locators::admitted(&row, &config_only, &own, &peer, head, true)?,
        "configured relay alone cannot authorize release"
    );
    for mutation in 0..4 {
        let mut bad = row.clone();
        match mutation {
            0 => bad["lease_current"] = false.into(),
            1 => bad["own_locator_revision"] = 1.into(),
            2 => bad["peer_locator_revision"] = 1.into(),
            _ => bad["publication_ready"] = false.into(),
        }
        ensure!(
            !locators::admitted(&bad, &route, &own, &peer, head, true)?,
            "incomplete/stale admission {mutation}"
        );
    }
    ensure!(
        !locators::admitted(&row, &route, &own, &peer, Some(([20; 32], 8, 640)), true)?,
        "stale Head"
    );
    let mut wrong = route.clone();
    wrong["selected_path"] = "direct".into();
    ensure!(
        locators::admitted(&row, &wrong, &own, &peer, head, true).is_err(),
        "wrong actual selected path"
    );
    let mut wrong = route;
    wrong["slot"] = 1.into();
    ensure!(
        locators::admitted(&row, &wrong, &own, &peer, head, true).is_err(),
        "wrong Ready physical slot"
    );
    Ok(())
}

#[test]
fn s54_p11a2_same_route_reuses_exact_signed_bytes_for_both_contexts_and_owners() -> Result<()> {
    let (devices, peers) = peers()?;
    for context in [[17; 32], [18; 32]] {
        for (owner, device) in devices.iter().enumerate() {
            let mut f = fields(&peers, owner);
            f.c = context;
            let signed = v2::ParsedRuntimeLocator2::sign(f, device)?;
            let prior =
                locators::verify(signed.raw(), context, &peers, owner, 150, Some(URL), None)?;
            // Reuse was a publisher decision earlier, not a new 60s verifier margin.
            let current = locators::verify_same_route(
                signed.raw(),
                context,
                &peers,
                owner,
                199,
                URL,
                &prior,
            )?;
            ensure!(
                current.revision == prior.revision
                    && current.endpoint == prior.endpoint
                    && current.raw == prior.raw,
                "same authenticated publication, even one second before expiry"
            );
            ensure!(
                locators::verify(
                    signed.raw(),
                    context,
                    &peers,
                    owner,
                    199,
                    Some(URL),
                    Some(&prior)
                )
                .is_err(),
                "P10 strict route-change path still rejects unchanged revision/bytes"
            );
        }
    }
    Ok(())
}
#[test]
fn s54_p11a2_newer_same_route_accepts_unchanged_or_rotated_endpoint() -> Result<()> {
    let (devices, peers) = peers()?;
    let prior = v2::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    let prior = locators::verify(prior.raw(), [17; 32], &peers, 0, 150, Some(URL), None)?;
    for endpoint in [prior.endpoint, [99; 32]] {
        let mut f = fields(&peers, 0);
        f.revision = 3;
        f.endpoint = endpoint;
        f.not_before = 140;
        f.expires = 240;
        let signed = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?;
        let current =
            locators::verify_same_route(signed.raw(), [17; 32], &peers, 0, 150, URL, &prior)?;
        ensure!(
            current.revision == 3 && current.endpoint == endpoint && current.raw == signed.raw(),
            "actual valid new revision under same relay, no endpoint rotation assumption"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a2_lower_or_equal_revision_changed_signed_content_is_rejected() -> Result<()> {
    let (devices, peers) = peers()?;
    let prior = v2::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    let prior = locators::verify(prior.raw(), [17; 32], &peers, 0, 150, Some(URL), None)?;
    for mutation in 0..3 {
        let mut f = fields(&peers, 0);
        match mutation {
            0 => f.revision = 1,
            1 => f.endpoint = [99; 32],
            _ => {
                f.not_before = 110;
                f.expires = 210;
            }
        }
        let signed = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?;
        ensure!(
            locators::verify_same_route(signed.raw(), [17; 32], &peers, 0, 150, URL, &prior)
                .is_err(),
            "valid signature cannot authorize lower/equal-conflicting publication {mutation}"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a2_expired_prior_allows_new_current_but_not_expired_equal_reuse() -> Result<()> {
    let (devices, peers) = peers()?;
    let mut f = fields(&peers, 0);
    f.not_before = 10;
    f.expires = 100;
    let old = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?;
    let prior = locators::verify(old.raw(), [17; 32], &peers, 0, 99, Some(URL), None)?;
    let mut f = fields(&peers, 0);
    f.revision = 3;
    f.not_before = 140;
    f.expires = 160;
    let new = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?;
    locators::verify_same_route(new.raw(), [17; 32], &peers, 0, 150, URL, &prior)?;
    for now in [100, 150] {
        ensure!(
            locators::verify_same_route(old.raw(), [17; 32], &peers, 0, now, URL, &prior).is_err(),
            "equal bytes still must be current"
        );
    }
    for now in [139, 160] {
        ensure!(
            locators::verify_same_route(new.raw(), [17; 32], &peers, 0, now, URL, &prior).is_err(),
            "new current interval is not weakened for expired prior"
        );
    }
    Ok(())
}
#[test]
fn s54_p11a2_current_requires_signature_scope_owner_peer_interval_and_exact_url_only_route()
-> Result<()> {
    let (devices, peers) = peers()?;
    let old = v2::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    let prior = locators::verify(old.raw(), [17; 32], &peers, 0, 150, Some(URL), None)?;
    for mutation in 0..8 {
        let mut f = fields(&peers, 0);
        f.revision = 3;
        match mutation {
            0 => f.c = [18; 32],
            1 => {
                f.cert = peers[1].cert_ref();
                f.peer_cert = peers[0].cert_ref();
            }
            2 => f.peer_cert = [99; 32],
            3 => f.relay = Some("http://127.0.0.1:40124/".into()),
            4 => f.ips.push("127.0.0.1:12345".parse()?),
            5 => {
                f.not_before = 151;
                f.expires = 251;
            }
            6 => {
                f.not_before = 10;
                f.expires = 150;
            }
            _ => (),
        }
        let signer = if mutation == 1 {
            &devices[1]
        } else {
            &devices[0]
        };
        let signed = v2::ParsedRuntimeLocator2::sign(f, signer)?;
        let mut raw = signed.raw().to_vec();
        if mutation == 7 {
            *raw.last_mut().context("actual signature byte")? ^= 1;
        }
        ensure!(
            locators::verify_same_route(&raw, [17; 32], &peers, 0, 150, URL, &prior).is_err(),
            "newer current authentication/route failure {mutation}"
        );
    }
    ensure!(
        locators::verify_same_route(old.raw(), [17; 32], &peers, 2, 150, URL, &prior).is_err()
            && locators::verify_same_route(&vec![0; 8193], [17; 32], &peers, 0, 150, URL, &prior)
                .is_err(),
        "bounded owner/current bytes"
    );
    Ok(())
}
#[test]
fn s54_p11a2_prior_authentication_prevents_cross_row_or_independent_metadata_substitution()
-> Result<()> {
    let (devices, peers) = peers()?;
    let old = v2::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    let prior = locators::verify(old.raw(), [17; 32], &peers, 0, 150, Some(URL), None)?;
    let mut f = fields(&peers, 0);
    f.revision = 3;
    let current = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?;
    for mutation in 0..8 {
        let mut bad = prior.clone();
        match mutation {
            0 => bad.revision = 1,
            1 => bad.endpoint = [99; 32],
            2 => {
                *bad.raw.last_mut().context("prior signature byte")? ^= 1;
            }
            3 => {
                let mut f = fields(&peers, 0);
                f.c = [18; 32];
                bad.raw = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?
                    .raw()
                    .to_vec();
            }
            4 => {
                let f = fields(&peers, 1);
                bad.endpoint = f.endpoint;
                bad.raw = v2::ParsedRuntimeLocator2::sign(f, &devices[1])?
                    .raw()
                    .to_vec();
            }
            5 => {
                let mut f = fields(&peers, 0);
                f.relay = Some("http://127.0.0.1:40124/".into());
                bad.raw = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?
                    .raw()
                    .to_vec();
            }
            6 => {
                let mut f = fields(&peers, 0);
                f.peer_cert = [99; 32];
                bad.raw = v2::ParsedRuntimeLocator2::sign(f, &devices[0])?
                    .raw()
                    .to_vec();
            }
            _ => bad.raw = vec![0; 8193],
        }
        ensure!(
            locators::verify_same_route(current.raw(), [17; 32], &peers, 0, 150, URL, &bad)
                .is_err(),
            "prior must be actual bounded authenticated original row {mutation}"
        );
    }
    Ok(())
}
