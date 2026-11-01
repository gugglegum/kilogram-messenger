//! Authentic in-memory signed public test credentials/locators; no Native/process fixture.
use super::*;
use kilogram_identity::{
    AccountAuthoritySnapshot, AccountId, DeviceCapability, DeviceCertificate,
    DeviceEncryptionIdentity, DeviceIdentity, EncryptionPublicKey,
};
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
fn peers() -> Result<([DeviceIdentity; 2], [wire::RootVerifiedCredential; 2])> {
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
        credentials.push(wire::RootVerifiedCredential::from_parts(
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
fn fields(peers: &[wire::RootVerifiedCredential; 2], role: usize) -> wire::RuntimeLocatorFields2 {
    wire::RuntimeLocatorFields2 {
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
fn g3_relay_locator_requires_authentic_scope_interval_and_url_only() -> Result<()> {
    let (devices, peers) = peers()?;
    let good = wire::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    for now in [100, 150, 199] {
        relay_locator(good.raw(), [17; 32], &peers, 0, URL, now)?;
    }
    for now in [99, 200] {
        assert!(relay_locator(good.raw(), [17; 32], &peers, 0, URL, now).is_err());
    }
    assert!(relay_locator(good.raw(), [18; 32], &peers, 0, URL, 150).is_err());
    assert!(relay_locator(good.raw(), [17; 32], &peers, 1, URL, 150).is_err());
    for changed in ["cert", "peer", "URL", "IP"] {
        let mut f = fields(&peers, 0);
        match changed {
            "cert" => f.cert = [4; 32],
            "peer" => f.peer_cert = [4; 32],
            "URL" => f.relay = Some("https://example.com/".into()),
            _ => f.ips.push("127.0.0.1:40124".parse()?),
        }
        let wrong = wire::ParsedRuntimeLocator2::sign(f, &devices[0])?;
        assert!(relay_locator(wrong.raw(), [17; 32], &peers, 0, URL, 150).is_err());
    }
    let wrong_signer = wire::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[1])?;
    assert!(relay_locator(wrong_signer.raw(), [17; 32], &peers, 0, URL, 150).is_err());
    let mut forged = good.raw().to_vec();
    *forged.last_mut().context("G3 signed locator bytes")? ^= 1;
    assert!(relay_locator(&forged, [17; 32], &peers, 0, URL, 150).is_err());
    let mut invalid = fields(&peers, 0);
    invalid.expires = invalid.not_before;
    assert!(wire::ParsedRuntimeLocator2::sign(invalid, &devices[0]).is_err());
    let mut invalid = fields(&peers, 0);
    invalid.relay = Some("http://example.com/".into());
    assert!(wire::ParsedRuntimeLocator2::sign(invalid, &devices[0]).is_err());
    Ok(())
}
fn status(c: &str) -> Value {
    json!({"contexts":[{"context":"22".repeat(32),"slot":0},{"context":c,"slot":1,"enabled":true,"active":true,"lease_current":true,"publication_ready":true,"existing_origin_ready":true,"mode":"Active","lease_family":2,"policy_error":null,"handoff_policy_error":null,"publication_error":null,"origin_publication_error":null,"own_locator_revision":2,"peer_locator_revision":2}],"routes":[{"slot":1,"policy":"relay-only","selected_path":"relay"},{"slot":0,"policy":"relay-only","selected_path":null}]})
}
#[test]
fn g3_route_slot_and_publication_join_rejects_ambiguity_and_stale_bytes() -> Result<()> {
    let (devices, peers) = peers()?;
    let own = wire::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?;
    let peer = wire::ParsedRuntimeLocator2::sign(fields(&peers, 1), &devices[1])?;
    let c = state::hex(&[17; 32]);
    let m = oracle::Mapping {
        owner: 1,
        context: c.clone(),
        slot: 1,
        role: 0,
    };
    let full = status(&c);
    selected(&full, &m, &own, &peer)?;
    let mut bad = full.clone();
    bad["routes"][1]["slot"] = json!(1);
    assert!(selected(&bad, &m, &own, &peer).is_err());
    let mut bad = full.clone();
    bad["routes"]
        .as_array_mut()
        .context("G3 test routes")?
        .remove(0);
    assert!(selected(&bad, &m, &own, &peer).is_err());
    for key in ["policy", "selected_path"] {
        let mut bad = full.clone();
        bad["routes"][0][key] = json!("direct");
        assert!(selected(&bad, &m, &own, &peer).is_err());
    }
    for (key, value) in [
        ("publication_ready", json!(false)),
        ("slot", json!(0)),
        ("own_locator_revision", json!(3)),
        ("peer_locator_revision", json!(1)),
    ] {
        let mut bad = full.clone();
        bad["contexts"][1][key] = value;
        assert!(selected(&bad, &m, &own, &peer).is_err());
    }
    revision(own.raw(), own.raw())?;
    let mut changed = fields(&peers, 0);
    changed.expires = 201;
    let changed = wire::ParsedRuntimeLocator2::sign(changed, &devices[0])?;
    scoped(changed.raw(), [17; 32], &peers, 0)?;
    assert!(revision(own.raw(), changed.raw()).is_err());
    let mut stale = fields(&peers, 0);
    stale.revision = 1;
    let stale = wire::ParsedRuntimeLocator2::sign(stale, &devices[0])?;
    assert!(revision(own.raw(), stale.raw()).is_err());
    let mut newer = fields(&peers, 0);
    newer.revision = 3;
    revision(
        own.raw(),
        wire::ParsedRuntimeLocator2::sign(newer, &devices[0])?.raw(),
    )?;
    Ok(())
}
#[test]
fn g3_stopped_locator_join_requires_both_actual_imported_peer_bytes() -> Result<()> {
    let (devices, peers) = peers()?;
    let public = [
        wire::ParsedRuntimeLocator2::sign(fields(&peers, 0), &devices[0])?
            .raw()
            .to_vec(),
        wire::ParsedRuntimeLocator2::sign(fields(&peers, 1), &devices[1])?
            .raw()
            .to_vec(),
    ];
    let c = state::hex(&[17; 32]);
    let mut reports = [Value::Null, Value::Null, Value::Null];
    for (i, owner) in [1, 2].into_iter().enumerate() {
        reports[owner] = json!({"contexts":{(c.clone()):{"routing":Stored{own:public[i].clone(),peer:public[1-i].clone()}}}});
    }
    stored_join(&reports, &c, &public)?;
    for owner in [1, 2] {
        let mut bad = reports.clone();
        bad[owner]["contexts"][&c]["routing"]["peer"] = json!(public[owner - 1]);
        assert!(stored_join(&bad, &c, &public).is_err());
    }
    Ok(())
}
#[test]
fn g3_auto_requires_explicit_relay_and_loopback_ips_without_weakening_relay_only() -> Result<()> {
    let (devices, peers) = peers()?;
    let mut f = fields(&peers, 0);
    f.ips.push("127.0.0.1:40124".parse()?);
    let auto = wire::ParsedRuntimeLocator2::sign(f.clone(), &devices[0])?;
    route_locator(auto.raw(), [17; 32], &peers, 0, URL, 150, Mode::Auto)?;
    assert!(relay_locator(auto.raw(), [17; 32], &peers, 0, URL, 150).is_err());
    f.ips.clear();
    let no_ip = wire::ParsedRuntimeLocator2::sign(f.clone(), &devices[0])?;
    assert!(route_locator(no_ip.raw(), [17; 32], &peers, 0, URL, 150, Mode::Auto).is_err());
    f.ips.push("192.0.2.1:40124".parse()?);
    let foreign = wire::ParsedRuntimeLocator2::sign(f, &devices[0])?;
    assert!(route_locator(foreign.raw(), [17; 32], &peers, 0, URL, 150, Mode::Auto).is_err());
    Ok(())
}
