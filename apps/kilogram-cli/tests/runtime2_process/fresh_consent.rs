use super::*;

fn args(
    nodes: &[Node; 2],
    role: usize,
    c: &str,
    output: &Path,
    reverse: bool,
) -> Result<Vec<String>> {
    let own = &nodes[role];
    let peer = &nodes[1 - role];
    let mut result = vec![
        "--account-dir".into(),
        text(&own.root),
        "--expect-account".into(),
        own.pins["account"].as_str().context("own account")?.into(),
        "--expect-peer-account".into(),
        peer.pins["account"]
            .as_str()
            .context("peer account")?
            .into(),
        "--expect-device".into(),
        own.pins["device"].as_str().context("own device")?.into(),
        "--expect-peer-device".into(),
        peer.pins["device"].as_str().context("peer device")?.into(),
        "--context".into(),
        c.into(),
        "--output-file".into(),
        text(output),
        "--credentials-file".into(),
    ];
    let order = if reverse { [1, 0] } else { [0, 1] };
    result.extend(order.map(|n| text(&nodes[n].credential)));
    result.push("--minimum-authority-file".into());
    result.extend(order.map(|n| text(&nodes[n].root.with_extension("authority"))));
    Ok(result)
}
fn isolated(nodes: &[Node; 2], role: usize, command_args: Vec<String>) -> Result<String> {
    let peer = &nodes[1 - role].root;
    let hidden = peer.with_extension("unavailable");
    ensure!(!hidden.exists(), "fresh unavailable peer Root destination");
    // The other Root's actual path does not exist while the signing CLI runs.
    std::fs::rename(peer, &hidden)?;
    let result = cli(command_args);
    let restored = std::fs::rename(&hidden, peer);
    restored?;
    result
}
pub(super) fn ceremony(nodes: &[Node; 2], c: &str, output: &Path, reverse: bool) -> Result<()> {
    for node in nodes {
        let path = node.root.with_extension("authority");
        if !path.exists() {
            cli(vec![
                "account-snapshot".into(),
                "--account-dir".into(),
                text(&node.root),
                "--snapshot-file".into(),
                text(&path),
            ])?;
        }
    }
    let proposal = output.with_extension("proposal");
    let mut propose = vec!["ordered".into(), "context2-propose".into()];
    propose.extend(args(nodes, 0, c, &proposal, reverse)?);
    propose.extend([
        "--not-before".into(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_string(),
    ]);
    let blocked = proposal.with_extension("blocked");
    std::fs::write(&blocked, b"existing public output")?;
    let mut conflict = propose.clone();
    let out = conflict
        .iter()
        .position(|s| s == "--output-file")
        .context("output flag")?;
    conflict[out + 1] = text(&blocked);
    let refused = command().args(&conflict).output()?;
    ensure!(
        !refused.status.success() && std::fs::read(&blocked)? == b"existing public output",
        "preexisting output refused without overwrite"
    );
    ensure!(
        AccountRootState::load(&nodes[0].root)?
            .load_root_conversation_membership(kilogram_identity::ConversationScopeId::from_bytes(
                kilogram_cli::ordered_dialog::state::parse_id(c)?
            ))
            .is_err(),
        "output conflict before first proposal created no membership"
    );
    isolated(nodes, 0, propose.clone())?;
    let membership = AccountRootState::load(&nodes[0].root)?
        .load_root_conversation_membership(kilogram_identity::ConversationScopeId::from_bytes(
            kilogram_cli::ordered_dialog::state::parse_id(c)?,
        ))?
        .encode()?;
    let retry = proposal.with_extension("retry");
    let mut again = propose.clone();
    let n = again
        .iter()
        .position(|v| v == "--output-file")
        .context("output argument")?;
    again[n + 1] = text(&retry);
    isolated(nodes, 0, again)?;
    ensure!(
        std::fs::read(&proposal)? == std::fs::read(&retry)?,
        "identical idempotent proposal retry"
    );
    let conflict = command().args(&propose).output()?;
    ensure!(
        !conflict.status.success(),
        "proposal cannot overwrite output"
    );
    ensure!(
        AccountRootState::load(&nodes[0].root)?
            .load_root_conversation_membership(kilogram_identity::ConversationScopeId::from_bytes(
                kilogram_cli::ordered_dialog::state::parse_id(c)?
            ))?
            .encode()?
            == membership,
        "output conflict changes no membership"
    );
    let mut cosign = vec!["ordered".into(), "context2-cosign".into()];
    cosign.extend(args(nodes, 1, c, output, !reverse)?);
    cosign.extend(["--proposal-file".into(), text(&proposal)]);
    isolated(nodes, 1, cosign)?;
    // Incomplete proposals must fail the actual import path, with no install.
    let node = &nodes[0];
    let rejected = command()
        .args(["ordered", "context2-import", "--state-dir"])
        .arg(&node.state)
        .args([
            "--expect-account",
            node.pins["account"].as_str().context("account")?,
            "--expect-claim",
            node.pins["claim"].as_str().context("claim")?,
            "--invitation-file",
        ])
        .arg(&proposal)
        .output()?;
    ensure!(
        !rejected.status.success(),
        "proposal accepted as final artifact"
    );
    println!(
        "CS1_CEREMONY peer_root_unavailable_each_signer=true reversed_public_order={reverse} no_clobber=true retry_identical=true incomplete_import_refused=true"
    );
    Ok(())
}
#[tokio::test]
async fn cs1_independent_fresh_cli_bidirectional_history() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(150), async {
        let mut pair = Pair::prepared_with_ceremony("CS1-", true)?;
        pair.nodes[0].start().await?;
        pair.nodes[1].start().await?;
        pair.import().await?;
        pair.wait_active().await?;
        for slot in 0..=1 {
            for role in 0..2 {
                pair.nodes[role].send(slot, 1).await?;
            }
        }
        for slot in 0..2 {
            pair.wait_counts(slot, 4).await?;
            let (left, _) = positive::exact_history(&pair, 0, slot as u8, [1, 1], None).await?;
            let (right, _) = positive::exact_history(&pair, 1, slot as u8, [1, 1], None).await?;
            ensure!(
                left == right && left.len() == 2,
                "identical bilateral ciphertext event IDs and readable request/text history"
            );
        }
        for node in &mut pair.nodes {
            node.stop(false).await?;
        }
        println!("CS1_FRESH_SMOKE_PASS contexts=2 text=4 import_both=true exact_history=true");
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("CS1 bounded smoke watchdog")?
}
