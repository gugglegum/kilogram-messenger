use super::*;
#[path = "fixture.rs"]
mod fixture;

#[test]
fn cold_b1_small_readers_and_exact_negatives() -> Result<()> {
    // Positive controls precede every negative; only two real Text events.
    let until = Instant::now() + Duration::from_secs(100);
    let mut fixture = fixture::small()?;
    for local in 0..2 {
        let repo = &mut fixture.repositories[local];
        let expected = &fixture.expected[local];
        let before = readers::b0(
            repo,
            &fixture.profiles[local],
            &fixture.configs[local],
            expected,
            &fixture.proofs,
            until,
        )?;
        let first = readers::full(repo, expected, &fixture.proofs, until, None)?;
        let second = readers::full(repo, expected, &fixture.proofs, until, None)?;
        ensure!(
            first.digest == second.digest
                && first.history_digest == second.history_digest
                && first.text_count == 2
                && first.pages == 1,
            "complete logical repeat"
        );
        let after = readers::b0(
            repo,
            &fixture.profiles[local],
            &fixture.configs[local],
            expected,
            &fixture.proofs,
            until,
        )?;
        ensure!(before == after, "B0 full readback");
        let scope = readers::scope(repo, 0, local)?;
        let raw = repo.event_at(0, 0, 1)?.context("positive source")?;
        let parsed = old::Event::parse(&raw, scope.verified.peers()[0].certificate().device_id())?;
        let (auth, _) = readers::credential(repo, parsed.core.author_auth)?;
        let source = readers::verified_source(&scope.verified, &raw, &auth, 0, 1)?;
        let receipt = repo.receipt(0, 0, 1)?.context("positive receipt")?;
        let v = old::decode(&receipt, 1024)?;
        let reference = v.fields(2)?[0].fields(8)?[7].fixed()?;
        let (receiver, _) = readers::credential(repo, reference)?;
        readers::verified_receipt(&scope.verified, &source, &receipt, &receiver)?;
        let mut altered = receipt.clone();
        let end = altered.len() - 1;
        altered[end] ^= 1;
        wire::ParsedReceipt1::parse(&altered)?; // Parsing succeeds; signature must fail.
        ensure!(
            readers::verified_receipt(&scope.verified, &source, &altered, &receiver).is_err(),
            "altered receipt signature refused"
        );
        let mut fields = v.fields(2)?.to_vec();
        let mut body = fields[0].fields(8)?.to_vec();
        body[4] = old::u(2);
        fields[0] = old::a(body);
        let binding = old::encode(&old::a(fields), 1024)?;
        wire::ParsedReceipt1::parse(&binding)?;
        let error = readers::verified_receipt(&scope.verified, &source, &binding, &receiver)
            .err()
            .context("changed receipt binding accepted")?;
        ensure!(
            error.to_string().contains("pre-Q Receipt1 source/receiver"),
            "exact receipt binding premise"
        );
        ensure!(
            readers::verified_receipt(&scope.verified, &source, &receipt, &auth).is_err(),
            "wrong receipt auth refused"
        );
        let account = scope.verified.peers()[0].certificate().account_id();
        readers::authority(&fixture.proofs[0], &fixture.proofs[0], account)?;
        let error = readers::authority(&fixture.proofs[1], &fixture.proofs[0], account)
            .err()
            .context("wrong account accepted")?;
        ensure!(
            error.to_string() == "authority account",
            "exact account premise"
        );
        let mut invalid = fixture.proofs[0].clone();
        let last = invalid.len() - 1;
        invalid[last] ^= 1;
        ensure!(
            readers::authority(&invalid, &fixture.proofs[0], account).is_err(),
            "invalid authority signature"
        );
        let peer = 1 - local;
        let (id, text) = expected.text(peer, 1);
        let event = repo.event_at(0, peer as u8, 1)?.context("peer source")?;
        let event = old::Event::parse(
            &event,
            scope.verified.peers()[peer].certificate().device_id(),
        )?;
        ensure!(
            repo.request(0, id)?.is_none(),
            "positive peer source has no local request"
        );
        readers::local_request(local, peer, None, event.id()?, &text)?;
        let absent = repo.request(0, fixed::future_ids()[local])?;
        readers::absent_request(absent.as_ref())?;
        let present = store::L1Request {
            queue_sequence: Some(1),
            event: None,
            text_hash: [3; 32],
        };
        let error = readers::absent_request(Some(&present))
            .err()
            .context("future request accepted")?;
        ensure!(
            error.to_string() == "future local request present",
            "exact future-request premise"
        );
        println!(
            "cold_b1_small local={local} texts={} pages={} b0={} full={} receipt_signature_refused=true receipt_binding_refused=true wrong_auth_refused=true invalid_authority_refused=true wrong_account_refused=true peer_without_request=true future_request_refused=true",
            first.text_count, first.pages, before, first.digest
        );
    }
    let original = fixture.proofs[0].clone();
    // Genuine Root-signed revision2 floor, revoking an unrelated synthetic ID.
    let root = kilogram_identity::AccountRootState::create(
        Path::new(fixed::WORKSPACE)
            .join(".tmp/C-s53-cold-b1-synthetic")
            .join(format!("floor-{}", std::process::id())),
    )?;
    let old = root.authority_snapshot()?.encode()?;
    root.revoke_device(
        kilogram_identity::DeviceIdentity::from_secret_bytes([193; 32]).device_id(),
    )?;
    let new = root.authority_snapshot()?.encode()?;
    readers::authority(&new, &old, root.account_id())?;
    let error = readers::authority(&old, &new, root.account_id())
        .err()
        .context("stale floor accepted")?;
    ensure!(
        error.to_string() == "authority rollback",
        "exact rollback premise"
    );
    ensure!(
        fixture.proofs[0] == original,
        "original fixture proof unchanged"
    );
    // Database close happens before the actual adopted-or-normal test lease is
    // returned; retaining the value is independently exercised here.
    for repo in fixture.repositories {
        let lease = repo.verification_close_to_held_lease();
        let error = kilogram_state::StateDirectoryLock::acquire(lease.root())
            .err()
            .context("returned lease released early")?;
        let _ = error;
        drop(lease);
    }
    Ok(())
}
