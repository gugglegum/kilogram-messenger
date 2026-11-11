use super::*;
use crate::RuntimeIpcServer;
use anyhow::Context;
use kilogram_identity::DeviceIdentity;

#[test]
fn cs32b_profile_schema_and_reply_compatibility() -> Result<()> {
    let sample = serde_json::json!({"version":2,"profile_mode":"ordered-dialog-v2","state_dir":"local","account":"11".repeat(32),"claim":"22".repeat(32),"device":"33".repeat(32),"contexts":["44".repeat(32),"55".repeat(32)],"enabled":[0,1],"ipc_file":"ipc","locator_directory":"locators","route":{"policy":"direct-only","bind":"127.0.0.1:0","relay":null},"logical_quota":[17179869184u64,17179869184u64],"physical_quota":51539607552u64});
    let doc = Profile2Document::decode(&serde_json::to_vec(&sample)?)?;
    ensure!(
        doc.logical_quota == Some([17179869184; 2]),
        "explicit quotas"
    );
    let mut missing = sample.clone();
    missing
        .as_object_mut()
        .context("object")?
        .remove("logical_quota");
    missing
        .as_object_mut()
        .context("object")?
        .remove("physical_quota");
    ensure!(
        Profile2Document::decode(&serde_json::to_vec(&missing)?)?
            .logical_quota
            .is_none(),
        "missing defaults preserved"
    );
    for field in ["logical_quota", "physical_quota"] {
        for bad in [serde_json::Value::Null, serde_json::json!("bad")] {
            let mut v = sample.clone();
            v[field] = bad;
            ensure!(
                Profile2Document::decode(&serde_json::to_vec(&v)?).is_err(),
                "null/type relaxed"
            );
        }
    }
    let text = serde_json::to_string(&sample)?;
    ensure!(
        Profile2Document::decode(text.replacen('{', "{\"version\":2,", 1).as_bytes()).is_err(),
        "duplicate field"
    );
    let mut extra = sample;
    extra["unexpected"] = true.into();
    ensure!(
        Profile2Document::decode(&serde_json::to_vec(&extra)?).is_err(),
        "unknown field"
    );
    for (version, op) in [(1, 1), (2, 5)] {
        let raw = wire::encode(&a([u(version), u(op), a([])]), 100)?;
        ensure!(
            decode_reply(1, RuntimeIpcResponse::OrderedDialog { response: raw }).is_err(),
            "wrong envelope"
        );
    }
    ensure!(
        decode_reply(1, RuntimeIpcResponse::ShutdownAccepted).is_err(),
        "wrong outer kind"
    );
    Ok(())
}
#[tokio::test]
async fn cs32b_same_descriptor_identity_and_replacement() -> Result<()> {
    for same_identity in [false, true] {
        let tmp = tempfile::tempdir()?;
        let first = tmp.path().join("first.json");
        let second = tmp.path().join("second.json");
        let account = AccountId::from_bytes([1; 32]);
        let device = DeviceIdentity::generate()?;
        let other = DeviceIdentity::generate()?;
        let (server, mut work) =
            RuntimeIpcServer::start_ordered2(first.clone(), account, &device).await?;
        let (other_server, mut other_work) = RuntimeIpcServer::start_ordered2(
            second,
            if same_identity {
                account
            } else {
                AccountId::from_bytes([2; 32])
            },
            if same_identity { &device } else { &other },
        )
        .await?;
        let descriptor = RuntimeIpcDescriptor::load(&first)?;
        ensure!(
            super::super::call_with_descriptor(
                &descriptor,
                (account, other.device_id()),
                RuntimeIpcCommand::Shutdown
            )
            .await
            .is_err(),
            "wrong identity accepted"
        );
        ensure!(
            super::super::call_with_descriptor(
                &descriptor,
                (AccountId::from_bytes([9; 32]), device.device_id()),
                RuntimeIpcCommand::Shutdown
            )
            .await
            .is_err(),
            "wrong account accepted"
        );
        ensure!(work.try_recv().is_err(), "wrong identity connected");
        let client = Client2::new(descriptor, account, device.device_id(), [[4; 32], [5; 32]])?;
        std::fs::write(&first, serde_json::to_vec(other_server.descriptor())?)?;
        let response = tokio::spawn(async move {
            let w = tokio::time::timeout(Duration::from_secs(5), work.recv())
                .await?
                .context("request")?;
            let (cmd, tx) = w.into_parts();
            ensure!(matches!(cmd, RuntimeIpcCommand::Shutdown), "command");
            let _ = tx.send(RuntimeIpcResponse::ShutdownAccepted);
            Ok::<_, anyhow::Error>(())
        });
        client.shutdown().await?;
        response.await??;
        ensure!(
            other_work.try_recv().is_err(),
            "replacement received a command"
        );
        server.shutdown().await?;
        ensure!(first.exists(), "foreign replacement deleted");
        other_server.shutdown().await?;
    }
    Ok(())
}

#[test]
fn cs32b_status_shape_and_nullability() -> Result<()> {
    let context = serde_json::json!({"slot":0,"context":"44".repeat(32),"role":0,"phase":0,"active":false,"d":null,"e":null,"sid":null,"q":null,"blocked":null,"policy_error":null,"own_locator_revision":1,"peer_locator_revision":0,"publication_ready":true,"file_exported":true,"live_admitted":false,"lease_expires":null,"native_generation":0,"counts":{},"details":{"current_consent":"66".repeat(32),"native":null,"native_hash":null,"offer":null,"grant":null,"readies":[null,null],"selected_hash":"77".repeat(32),"policy_generation":0,"wall_high_water":0}});
    let mut second = context.clone();
    second["slot"] = 1.into();
    second["context"] = "55".repeat(32).into();
    let status = serde_json::json!({"version":2,"runtime_pid":1,"runtime_instance":"88".repeat(32),"contexts":[context,second],"endpoint":{"id":"99".repeat(32),"ips":["127.0.0.1:1234"],"relays":[]},"route":{"policy":"direct-only","bind":"127.0.0.1:0","relay":null},"root_generation":0,"stats":{},"window":{},"resources":{},"backend":{}});
    Status2::decode(&serde_json::to_vec(&status)?)?;
    for field in ["d", "q", "sid", "lease_expires", "policy_error"] {
        let mut v = status.clone();
        v["contexts"][0]
            .as_object_mut()
            .context("context")?
            .remove(field);
        ensure!(
            Status2::decode(&serde_json::to_vec(&v)?).is_err(),
            "missing required nullable field"
        );
    }
    let mut duplicate = status.clone();
    duplicate["contexts"][1]["slot"] = 0.into();
    ensure!(
        Status2::decode(&serde_json::to_vec(&duplicate)?).is_err(),
        "duplicate slot"
    );
    let mut wrong = status.clone();
    wrong["contexts"][0]["active"] = true.into();
    ensure!(
        Status2::decode(&serde_json::to_vec(&wrong)?).is_err(),
        "phase3 alone/invalid active accepted"
    );
    let mut missing = status;
    missing["contexts"][0] = serde_json::json!({"slot":0,"installed":false,"active":false,"publication_ready":false,"counts":{}});
    Status2::decode(&serde_json::to_vec(&missing)?)?;
    Ok(())
}

#[test]
fn cs41_status_requires_checked_lan_path_for_each_live_lease() -> Result<()> {
    let mut status = serde_json::json!({
        "version":2,"runtime_pid":1,"runtime_instance":"11".repeat(32),
        "contexts":[],"endpoint":{"id":"22".repeat(32),"ips":["192.168.5.1:1234"],"relays":[]},
        "route":{"policy":"direct-only","bind":"192.168.5.1:0","relay":null,"lan_peer_ipv4":"192.168.5.2"},
        "root_generation":0,"stats":{},"window":{},"resources":{},"backend":{}
    });
    for slot in 0..2 {
        status["contexts"].as_array_mut().context("status slots")?.push(serde_json::json!({
            "slot":slot,"context":if slot == 0 {"44".repeat(32)} else {"55".repeat(32)},
            "role":0,"phase":3,"active":true,"d":"66".repeat(32),"e":"77".repeat(32),
            "sid":"fixture","q":"88".repeat(32),"blocked":null,"policy_error":null,
            "own_locator_revision":1,"peer_locator_revision":1,"publication_ready":true,"file_exported":true,
            "live_admitted":true,"lease_expires":123,"selected_lan_peer":"192.168.5.2:2345",
            "native_generation":0,"counts":{},"details":{"current_consent":"99".repeat(32),
            "native":null,"native_hash":null,"offer":null,"grant":null,"readies":[null,null],
            "selected_hash":"77".repeat(32),"policy_generation":0,"wall_high_water":0}
        }));
    }
    let decode = |value: &serde_json::Value| Status2::decode(&serde_json::to_vec(value)?);
    assert!(decode(&status)?.installed(0)?.selected_lan_peer.is_some());
    for invalid in [
        serde_json::json!("192.168.5.3:2345"),
        serde_json::json!("192.168.5.2:0"),
        serde_json::json!("192.168.5.2:3456"),
        serde_json::Value::Null,
    ] {
        let mut bad = status.clone();
        bad["contexts"][0]["selected_lan_peer"] = invalid;
        assert!(decode(&bad).is_err());
    }
    let slot = status["contexts"][0]
        .as_object_mut()
        .context("first slot")?;
    slot.remove("selected_lan_peer");
    assert!(decode(&status).is_err());
    status["contexts"][0]["live_admitted"] = false.into();
    status["contexts"][0]["lease_expires"] = serde_json::Value::Null;
    assert!(decode(&status)?.installed(0)?.selected_lan_peer.is_none());
    status["endpoint"] = serde_json::Value::Null;
    assert!(decode(&status).is_err());
    status["contexts"][1]
        .as_object_mut()
        .context("second slot")?
        .remove("selected_lan_peer");
    status["contexts"][1]["live_admitted"] = false.into();
    status["contexts"][1]["lease_expires"] = serde_json::Value::Null;
    assert!(decode(&status)?.endpoint.is_none());
    Ok(())
}
