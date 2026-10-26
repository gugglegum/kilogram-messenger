//! Public offline renewal, actual stopped d9b import, IPC3 and current permission.
use super::*;
use kilogram_protocol::ordered_dialog_v1::{self as wire, a, b, u};

pub(super) fn audit(base: &Path, exe: &Path, args: &[String]) -> Result<String> {
    use std::io::Write;
    let start = Instant::now();
    let result = cli(exe, args);
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(base.join("public-commands.jsonl"))?;
    writeln!(
        log,
        "{}",
        serde_json::to_string(
            &json!({"exe":exe,"argv":args,"ok":result.is_ok(),"error":result.as_ref().err().map(ToString::to_string),"elapsed_seconds":start.elapsed().as_secs_f64(),"stdout_blake3":result.as_ref().ok().map(|s|blake3::hash(s.as_bytes()).to_hex().to_string())})
        )?
    )?;
    result
}
fn root_inventory(root: &Path) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "offline Root symlink");
        let name = entry.file_name().to_string_lossy().into_owned();
        if kind.is_dir() {
            out.push((format!("{name}/"), "directory".into()));
            out.extend(
                root_inventory(&entry.path())?
                    .into_iter()
                    .map(|(p, h)| (format!("{name}/{p}"), h)),
            );
        } else {
            ensure!(kind.is_file(), "offline Root artifact shape");
            out.push((
                name,
                blake3::hash(&std::fs::read(entry.path())?)
                    .to_hex()
                    .to_string(),
            ));
        }
    }
    out.sort();
    Ok(out)
}
pub(super) fn snapshot(base: &Path, root: &Path, name: &str) -> Result<PathBuf> {
    let path = base.join(name);
    audit(
        base,
        &frozen(),
        &[
            "account-snapshot".into(),
            "--account-dir".into(),
            text(root),
            "--snapshot-file".into(),
            text(&path),
        ],
    )?;
    Ok(path)
}
fn renewal_args(
    a_node: &Node,
    b_node: &Node,
    base: &Path,
    roots: [&Path; 2],
    authorities: [&Path; 2],
    floors: [&Path; 2],
    role: usize,
) -> Vec<String> {
    let own = if role == 0 { a_node } else { b_node };
    let peer = if role == 0 { b_node } else { a_node };
    vec![
        "--existing-invitation-file".into(),
        text(&base.join("original.invitation")),
        "--expect-context".into(),
        a_node.branch.context.clone(),
        "--expect-account".into(),
        own.branch.account.clone(),
        "--expect-peer-account".into(),
        peer.branch.account.clone(),
        "--authority-a-file".into(),
        text(authorities[0]),
        "--authority-b-file".into(),
        text(authorities[1]),
        "--minimum-authority-a-file".into(),
        text(floors[0]),
        "--minimum-authority-b-file".into(),
        text(floors[1]),
        "--account-dir".into(),
        text(roots[role]),
    ]
}
#[allow(clippy::too_many_arguments)] // Actual public operator inputs, one Root per command.
pub(super) fn issue(
    a_node: &Node,
    b_node: &Node,
    base: &Path,
    roots: [&Path; 2],
    authorities: [&Path; 2],
    floors: [&Path; 2],
    label: &str,
    ttl: u64,
) -> Result<(PathBuf, PathBuf)> {
    let proposal = base.join(format!("{label}.proposal"));
    let invitation = base.join(format!("{label}.invitation"));
    let consent = base.join(format!("{label}.consent"));
    let before = [root_inventory(roots[0])?, root_inventory(roots[1])?];
    let mut args = vec!["ordered".into(), "consent1-renew-propose".into()];
    args.extend(renewal_args(
        a_node,
        b_node,
        base,
        roots,
        authorities,
        floors,
        0,
    ));
    args.extend([
        "--valid-for-seconds".into(),
        ttl.to_string(),
        "--output-file".into(),
        text(&proposal),
    ]);
    audit(base, &current(), &args)?;
    let mut args = vec!["ordered".into(), "consent1-renew-cosign".into()];
    args.extend(renewal_args(
        a_node,
        b_node,
        base,
        roots,
        authorities,
        floors,
        1,
    ));
    args.extend([
        "--proposal-file".into(),
        text(&proposal),
        "--output-invitation-file".into(),
        text(&invitation),
        "--output-consent-file".into(),
        text(&consent),
    ]);
    audit(base, &current(), &args)?;
    ensure!(
        before == [root_inventory(roots[0])?, root_inventory(roots[1])?],
        "offline issuance changed Root files or created a vault"
    );
    if label == "short" {
        let raw = std::fs::read(&proposal)?;
        let value = wire::decode(&raw, 96 * 1024)?;
        for (name, field, replacement) in [
            ("wrong-C", 1, b([0; 32])),
            ("wrong-credential", 2, value.fields(8)?[3].clone()),
            ("wrong-membership", 4, b([])),
            ("bad-signature", 6, b([0; 64])),
        ] {
            let mut f = value.fields(8)?.to_vec();
            f[field] = replacement;
            let bad = base.join(format!("{name}.proposal"));
            std::fs::write(&bad, wire::encode(&a(f), 96 * 1024)?)?;
            let output = base.join(format!("{name}.must-not-exist"));
            let mut negative = vec!["ordered".into(), "consent1-renew-cosign".into()];
            negative.extend(renewal_args(
                a_node,
                b_node,
                base,
                roots,
                authorities,
                floors,
                1,
            ));
            negative.extend([
                "--proposal-file".into(),
                text(&bad),
                "--output-invitation-file".into(),
                text(&output),
                "--output-consent-file".into(),
                text(&base.join(format!("{name}.must-not-exist-consent"))),
            ]);
            ensure!(
                audit(base, &current(), &negative).is_err() && !output.exists(),
                "invalid renewal accepted {name}"
            );
        }
        for name in ["wrong-Root", "expired", "rollback"] {
            let mut negative = vec!["ordered".into(), "consent1-renew-propose".into()];
            negative.extend(renewal_args(
                a_node,
                b_node,
                base,
                roots,
                authorities,
                floors,
                0,
            ));
            let (flag, replacement) = match name {
                "wrong-Root" => ("--account-dir", text(roots[1])),
                "expired" => ("--not-before", "1".into()),
                _ => ("--authority-a-file", text(floors[0])),
            };
            if let Some(i) = negative.iter().position(|v| v == flag) {
                negative[i + 1] = replacement;
            } else {
                negative.extend([flag.into(), replacement]);
            }
            if name == "rollback" {
                let i = negative
                    .iter()
                    .position(|v| v == "--minimum-authority-a-file")
                    .context("minimum")?;
                negative[i + 1] = text(authorities[0]);
            }
            let output = base.join(format!("{name}.must-not-exist"));
            negative.extend([
                "--valid-for-seconds".into(),
                "60".into(),
                "--output-file".into(),
                text(&output),
            ]);
            ensure!(
                audit(base, &current(), &negative).is_err() && !output.exists(),
                "invalid renewal input accepted {name}"
            );
        }
        ensure!(
            before == [root_inventory(roots[0])?, root_inventory(roots[1])?],
            "negative issuance changed Root/vault"
        );
    }
    Ok((invitation, consent))
}
fn frozen_install(
    node: &Node,
    base: &Path,
    authorities: [&Path; 2],
    invitation: &Path,
    peer: &str,
) -> Result<()> {
    for snapshot in authorities {
        audit(
            base,
            &frozen(),
            &[
                "ordered".into(),
                "authority-update".into(),
                "--state-dir".into(),
                text(&node.branch.state),
                "--snapshot-file".into(),
                text(snapshot),
            ],
        )?;
    }
    audit(
        base,
        &frozen(),
        &[
            "ordered".into(),
            "context-import".into(),
            "--state-dir".into(),
            text(&node.branch.state),
            "--invitation-file".into(),
            text(invitation),
            "--expect-account".into(),
            peer.into(),
        ],
    )?;
    Ok(())
}
pub(super) fn import_authority(node: &Node, base: &Path, path: &Path) -> Result<()> {
    audit(
        base,
        &current(),
        &[
            "ordered".into(),
            "authority3-import".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--snapshot-file".into(),
            text(path),
        ],
    )?;
    Ok(())
}
pub(super) fn import_consent(node: &Node, base: &Path, path: &Path, expected: &str) -> Result<()> {
    audit(
        base,
        &current(),
        &[
            "ordered".into(),
            "consent1-import".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--slot".into(),
            node.slot.to_string(),
            "--expected-current".into(),
            expected.into(),
            "--consent-file".into(),
            text(path),
        ],
    )?;
    Ok(())
}
fn ipc_error(node: &Node, value: wire::Value) -> Result<String> {
    use kilogram_runtime_ipc::{RuntimeIpcCommand, RuntimeIpcResponse};
    let response = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(kilogram_runtime_ipc::call(
            &node.ipc,
            RuntimeIpcCommand::OrderedDialog {
                request: wire::encode(&value, 32 * 1024)?,
            },
        ))?;
    match response {
        RuntimeIpcResponse::Error { message } => Ok(message),
        _ => anyhow::bail!("malformed IPC3 unexpectedly accepted"),
    }
}
fn rejection(node: &Node, id: u64, body: &str) -> Result<String> {
    rejected(
        &current(),
        &[
            "ordered".into(),
            "send3".into(),
            "--ipc-file".into(),
            text(&node.ipc),
            "--slot".into(),
            "0".into(),
            "--request-id".into(),
            format!("fa00000000000000{id:016x}"),
            "--text".into(),
            body.into(),
        ],
    )
}

#[test]
fn migration_s5_l1_policy_public_renewal_frozen_import_ipc3_capacity() -> Result<()> {
    let seed =
        PathBuf::from(std::env::var_os("KILOGRAM_S5_FIXTURE_DIR").context("charged origin")?);
    let source: Value = serde_json::from_slice(&std::fs::read(seed.join("result.json"))?)?;
    let c = source["context"].as_str().context("C")?;
    let base = run_dir("l1-policy-")?;
    println!(
        "L1_POLICY root={} origin={}",
        base.display(),
        seed.display()
    );
    let a_side = source["elected_a"]["side"].as_str().context("A")?;
    let b_side = source["elected_b"]["side"].as_str().context("B")?;
    let mut a_node = Node::from_seed(&seed, a_side, &base, c)?;
    let b_node = Node::from_seed(&seed, b_side, &base, c)?;
    let root_a = base.join("offline-root-a");
    let root_b = base.join("offline-root-b");
    copy_tree(&seed.join(format!("{a_side}.account")), &root_a)?;
    copy_tree(&seed.join(format!("{b_side}.account")), &root_b)?;
    std::fs::copy(seed.join("countersigned"), base.join("original.invitation"))?;
    let floor_a = snapshot(&base, &root_a, "floor-a.snapshot")?;
    let auth_b = snapshot(&base, &root_b, "floor-b.snapshot")?;
    audit(
        &base,
        &frozen(),
        &[
            "device-enroll".into(),
            "--profile-mode".into(),
            "ordered-dialog-v1".into(),
            "--account-dir".into(),
            text(&root_a),
            "--state-dir".into(),
            text(&base.join("additional-authorized-device")),
        ],
    )?;
    let auth_a = snapshot(&base, &root_a, "current-a.snapshot")?;
    a_node.migrate()?;
    let clock_a = base.join("a.clock");
    let clock_b = base.join("b.clock");
    std::fs::write(&clock_a, b"0")?;
    std::fs::write(&clock_b, b"0")?;
    let mut ca = a_node.start_controlled("indexed-a", None, Some(&clock_a))?;
    let initial = a_node.status()?;
    let native = initial["contexts"][0]["native_hash"].clone();
    let generation = initial["root_generation"].clone();
    for value in [
        a([u(2), u(1)]),
        a([u(3), u(1), u(99)]),
        a([u(3), u(4), u(0), b([0; 32]), u(1)]),
        a([u(3), u(11), u(0), a([b([0; 32]), b([1; 32])]), b([0])]),
        a([u(3), u(35), u(1), u(1), b([0; 32]), b(b"{\"unknown\":1}")]),
    ] {
        ensure!(
            !ipc_error(&a_node, value)?.is_empty(),
            "closed IPC3 rejection"
        );
    }
    for tag in [7, 8, 9, 10, 32, 33, 34] {
        let error = ipc_error(&a_node, a([u(3), u(tag)]))?;
        ensure!(
            error.contains(if tag >= 32 {
                "MigrationAdminOnly"
            } else {
                "UnsupportedInCut2"
            }),
            "closed cut tag {tag}"
        );
    }
    let after_bad = a_node.status()?;
    ensure!(
        after_bad["root_generation"] == generation
            && after_bad["contexts"][0]["native_hash"] == native
            && after_bad["stats"]["packet_attempts"] == 0,
        "malformed/unsupported IPC mutated state"
    );
    let mut config: Value = serde_json::from_slice(&std::fs::read(&a_node.branch.config)?)?;
    config["logical_quota"][0] =
        json!(config["logical_quota"][0].as_u64().context("quota")? + 4096);
    config["physical_capacity"] =
        json!(config["physical_capacity"].as_u64().context("physical")? + 4096);
    let config_file = base.join("larger-capacity.json");
    std::fs::write(&config_file, serde_json::to_vec_pretty(&config)?)?;
    audit(
        &base,
        &current(),
        &[
            "ordered".into(),
            "storage-increase-capacity-live".into(),
            "--ipc-file".into(),
            text(&a_node.ipc),
            "--config-file".into(),
            text(&config_file),
            "--expected-root-generation".into(),
            generation.to_string(),
            "--expected-config-revision".into(),
            initial["config_revision"].to_string(),
            "--expected-config-hash".into(),
            initial["config_hash"]
                .as_str()
                .context("config hash")?
                .into(),
        ],
    )?;
    let capacity = a_node.status()?;
    ensure!(
        capacity["config_revision"] == 2
            && capacity["contexts"][0]["native_hash"] == native
            && capacity["contexts"][0]["repair_free"] == initial["contexts"][0]["repair_free"],
        "same-owner capacity CAS/reset"
    );
    let (short_inv, short_consent) = issue(
        &a_node,
        &b_node,
        &base,
        [&root_a, &root_b],
        [&auth_a, &auth_b],
        [&floor_a, &auth_b],
        "short",
        60,
    )?;
    import_authority(&a_node, &base, &auth_a)?;
    import_authority(&a_node, &base, &auth_b)?;
    let before = a_node.status()?;
    let expected = before["contexts"][0]["current_consent"]
        .as_str()
        .context("current consent")?;
    import_consent(&a_node, &base, &short_consent, expected)?;
    frozen_install(
        &b_node,
        &base,
        [&auth_a, &auth_b],
        &short_inv,
        &a_node.branch.account,
    )?;
    let issued = a_node.status()?;
    ensure!(
        issued["contexts"][0]["native_hash"] == native && issued["stats"]["packet_attempts"] == 0,
        "offline producer/import used Native"
    );
    let mut cb = b_node.start_controlled("frozen-b-short", None, Some(&clock_b))?;
    b_node.import(&a_node)?;
    a_node.import(&b_node)?;
    a_node.send(21, "short renewed real A")?;
    b_node.send(22, "short renewed real B")?;
    let live = a_node.wait(&mut ca, 6, 6)?;
    b_node.wait(&mut cb, 6, 6)?;
    std::fs::write(&clock_a, b"120")?;
    std::fs::write(&clock_b, b"120")?;
    let denied = rejection(&a_node, 23, "expired must refuse")?;
    ensure!(
        denied.contains("expired") || denied.contains("AuthBlocked"),
        "current expiry refusal {denied}"
    );
    let expired = a_node.status()?;
    ensure!(
        expired["contexts"][0]["native_hash"] == live["contexts"][0]["native_hash"]
            && expired["stats"]["packet_attempts"] == live["stats"]["packet_attempts"],
        "Native before expiry refusal"
    );
    cb.kill_wait()?;
    let (long_inv, long_consent) = issue(
        &a_node,
        &b_node,
        &base,
        [&root_a, &root_b],
        [&auth_a, &auth_b],
        [&auth_a, &auth_b],
        "long",
        3600,
    )?;
    let short_ref = expired["contexts"][0]["current_consent"]
        .as_str()
        .context("short current")?;
    import_consent(&a_node, &base, &long_consent, short_ref)?;
    ensure!(
        import_consent(&a_node, &base, &short_consent, expected).is_err(),
        "stale current-ref CAS unexpectedly accepted"
    );
    frozen_install(
        &b_node,
        &base,
        [&auth_a, &auth_b],
        &long_inv,
        &a_node.branch.account,
    )?;
    cb = b_node.start_controlled("frozen-b-long", None, Some(&clock_b))?;
    b_node.import(&a_node)?;
    a_node.import(&b_node)?;
    a_node.send(24, "long renewed real A")?;
    b_node.send(25, "long renewed real B")?;
    let resumed = a_node.wait(&mut ca, 7, 7)?;
    b_node.wait(&mut cb, 7, 7)?;
    ensure!(
        resumed["contexts"][0]["sid"] == initial["contexts"][0]["sid"]
            && resumed["contexts"][0]["native"]["keys"] == initial["contexts"][0]["native"]["keys"],
        "renewal changed original Session/keys"
    );
    for name in [
        "account_constructors",
        "outbound_constructors",
        "inbound_constructors",
    ] {
        ensure!(resumed["stats"][name] == 0, "renewal constructor {name}");
    }
    cb.kill_wait()?;
    audit(
        &base,
        &frozen(),
        &[
            "device-revoke".into(),
            "--account-dir".into(),
            text(&root_b),
            "--device-id".into(),
            b_node.branch.device.clone(),
            "--revocation-file".into(),
            text(&base.join("revocation")),
        ],
    )?;
    let revoked = snapshot(&base, &root_b, "revoked-b.snapshot")?;
    import_authority(&a_node, &base, &revoked)?;
    ensure!(
        rejection(&a_node, 26, "revoked must refuse").is_ok(),
        "known revocation queue refusal"
    );
    let blocked = a_node.status()?;
    let current = blocked["contexts"][0]["current_consent"]
        .as_str()
        .context("long current")?;
    ensure!(
        import_consent(&a_node, &base, &long_consent, current).is_err(),
        "renewal cleared known revocation"
    );
    ensure!(
        blocked["contexts"][0]["native_hash"] == resumed["contexts"][0]["native_hash"]
            && blocked["stats"]["packet_attempts"] == resumed["stats"]["packet_attempts"],
        "revocation used Native"
    );
    let history = a_node.history(32, None)?;
    ensure!(
        history["items"].as_array().context("history")?.len() == 12,
        "history after revocation"
    );
    ca.kill_wait()?;
    std::fs::write(
        base.join("parent-l1-result.json"),
        serde_json::to_vec_pretty(
            &json!({"initial":initial,"capacity":capacity,"issued_without_native":issued,"expired":expired,"resumed":resumed,"blocked":blocked,"history":history,"actual_public_producer_and_frozen_import":true,"one_root_per_invocation":true,"parent_killed_and_waited":true}),
        )?,
    )?;
    println!(
        "L1_POLICY PASS public_propose_cosign=true frozen_stopped_import=true current_expiry_renewal_revocation=true tag11_cas=true tag35_same_lock=true root={}",
        base.display()
    );
    Ok(())
}
