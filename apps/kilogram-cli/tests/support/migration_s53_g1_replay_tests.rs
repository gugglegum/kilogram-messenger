use super::*;
use kilogram_cli::ordered_dialog::{admin, storage_migration};
use kilogram_identity::AccountRootState;
use kilogram_state::StateDirectoryLock;

fn until() -> Instant {
    Instant::now() + Duration::from_secs(90)
}
fn expected(e: &fixed::Expected) -> fixed::Expected {
    fixed::Expected {
        local: e.local,
        c: e.c,
        accounts: e.accounts,
        devices: e.devices,
        sid: e.sid.clone(),
        next: e.next,
        native_generation: e.native_generation,
        native_hash: e.native_hash.clone(),
    }
}
fn refused<T>(result: Result<T>, premise: &str) -> Result<()> {
    let error = result
        .err()
        .with_context(|| format!("accepted negative: {premise}"))?;
    ensure!(
        error.to_string().contains(premise),
        "wrong refusal premise: {error}, wanted {premise}"
    );
    Ok(())
}
fn read(repo: &mut store::L1Repository, n: usize, cursor: &str) -> Result<Vec<u8>> {
    Ok(repo.history(0, Some(&HistoryCursor::from_base64(cursor)?), n)?)
}
fn capture(repo: &mut store::L1Repository, limit: usize) -> Result<Vec<Vec<u8>>> {
    let mut cursor = None;
    let mut pages = Vec::new();
    loop {
        ensure!(pages.len() < 34, "fixture page cap");
        let raw = repo.history(
            0,
            cursor.as_ref(),
            if pages.is_empty() { limit } else { 32 },
        )?;
        let page = Page::parse(&raw)?;
        let done = page.finished;
        cursor = Some(HistoryCursor::from_base64(&page.cursor)?);
        pages.push(raw);
        if done {
            return Ok(pages);
        }
    }
}
fn logical(pages: &[Vec<u8>]) -> Result<String> {
    let mut map = BTreeMap::new();
    for raw in pages {
        for row in Page::parse(raw)?.items {
            map.insert(row.id.clone(), serde_json::to_value(row)?);
        }
    }
    let mut d = readers::Digest::new(b"kg:s53:cold:history:1\0");
    d.value(&map)?;
    Ok(d.finish())
}
fn original(
    e: &fixed::Expected,
    origin: &[u8],
    proofs: &[Vec<u8>; 2],
    pages: &[Vec<u8>],
    b1: &str,
    logical: &str,
) -> Result<Original> {
    Original::new(
        expected(e),
        origin.to_vec(),
        proofs.clone(),
        pages,
        b1.into(),
        logical.into(),
    )
}

// Existing genuine Pair builder/handshake/data/ACK path; only the number/body of
// tiny fixture messages differs. No process, transport or exhausted fixture.
fn small(next: [u64; 2], escaping: bool) -> Result<fixture::Small> {
    let parent = Path::new(fixed::WORKSPACE).join(".tmp/G1-C1-synthetic");
    std::fs::create_dir_all(&parent)?;
    let mut pair = fixture::Pair::new_in(&parent)?;
    let initial = pair.admit()?;
    pair.activate(&initial)?;
    for (role, end) in next.iter().enumerate() {
        for n in 1..*end {
            let mut id = [0; 16];
            id[..8].copy_from_slice(&(if role == 0 { 2u64 } else { 1 }).to_be_bytes());
            id[8..].copy_from_slice(&n.to_be_bytes());
            let mut text = format!("OD1/{}/{n}/Привет", if role == 0 { "two" } else { "one" });
            if escaping {
                text.push_str(" \"quoted\"\\line\n\t/🙂");
            }
            let (sender, receiver) = if role == 0 {
                (&mut pair.a, &mut pair.b)
            } else {
                (&mut pair.b, &mut pair.a)
            };
            sender.queue(pair.c, id, &text)?;
            let event = sender
                .materialize_one(pair.c, pair.binding)?
                .context("C1 genuine tiny Text")?;
            let frame = sender.deliver_frame(pair.c, &[event], pair.binding, id)?;
            let received = receiver.accept_data_frame(pair.c, &frame)?;
            ensure!(
                received.error.is_none() && received.readable == 1,
                "C1 genuine receive"
            );
            let (ack, ordinals) = receiver
                .ack_intents(pair.c, pair.binding)?
                .context("C1 genuine ACK")?;
            sender.accept_ack_frame(pair.c, &ack)?;
            receiver.mark_ack_intents(pair.c, pair.binding, &ordinals, 2)?;
        }
    }
    let proofs = [
        AccountRootState::load(&pair.accounts[0])?
            .authority_snapshot()?
            .encode()?,
        AccountRootState::load(&pair.accounts[1])?
            .authority_snapshot()?
            .encode()?,
    ];
    let c = pair.c;
    let base = pair._tmp.keep();
    let mut repositories = Vec::new();
    let mut profiles = Vec::new();
    let mut configs = Vec::new();
    let mut expects = Vec::new();
    for (local, engine) in [pair.a, pair.b].into_iter().enumerate() {
        let root = engine.store.root().to_path_buf();
        let credential = admin::credentials(&engine.store)?;
        let account = credential.cert.account_id();
        let device = credential.cert.device_id();
        drop(engine);
        let path = base.join(format!("{local}.profile"));
        let config = db::migration7::MigrationConfig {
            logical_quota: [512 * 1024 * 1024; 2],
            physical_capacity: 2 * 1024 * 1024 * 1024,
            output_profile: path.clone(),
            control_ipc_file: base.join(format!("{local}.ipc")),
            locator_directory: base.join(format!("{local}.locators")),
        };
        storage_migration::upgrade(
            StateDirectoryLock::acquire(&root)?,
            account,
            device,
            &config,
            false,
            None,
        )?;
        let profile = Profile3::load(&path)?;
        let mut repo = profile.open(&path)?;
        let spec = kilogram_cli::ordered_dialog::handshake::Spec::parse(&repo.spec(0)?)?;
        let accounts = spec
            .peers
            .each_ref()
            .map(|p| *p.cert.account_id().as_bytes());
        let mut pins = Vec::new();
        for account in accounts {
            let raw = repo.authority(account)?;
            let parsed = kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&raw)?;
            pins.push(store::L1Authority {
                account,
                revision: parsed.revision(),
                raw,
            });
        }
        repo.initialize(vec![accounts], pins)?;
        for _ in 0..4 {
            if repo.metadata(0)?.request_index_ready {
                break;
            }
            repo.commit(store::L1Operation::PrepareRequests { context: 0 }, None)?;
        }
        ensure!(
            repo.metadata(0)?.request_index_ready,
            "C1 tiny request index"
        );
        repo.commit(store::L1Operation::InitializeHistory { context: 0 }, None)?;
        let scope = readers::scope(&mut repo, 0, local)?;
        let snapshot = repo.snapshot(0)?;
        expects.push(fixed::Expected {
            local,
            c,
            accounts,
            devices: scope
                .verified
                .peers()
                .each_ref()
                .map(|p| *p.certificate().device_id().as_bytes()),
            sid: scope.verified.sid().into(),
            next,
            native_generation: scope.head.native_generation,
            native_hash: state::hex(&old::hash(
                state::NATIVE_TAG,
                &snapshot.native.context("C1 opaque tiny Native")?,
            )),
        });
        profiles.push(std::fs::read(path)?);
        configs.push(serde_json::to_vec(&config)?);
        repositories.push(repo);
    }
    Ok(fixture::Small {
        repositories,
        profiles,
        configs,
        expected: expects,
        proofs,
    })
}

#[test]
fn g1c1_public_bundle_and_pin_negatives() -> Result<()> {
    let original = public_originals()?;
    for (role, o) in original.iter().enumerate() {
        ensure!(
            o.ordered.len() == 528
                && o.pages.len() == 18
                && o.expected.local == role
                && o.b1 == B1[role]
                && o.logical == LOGICAL[role],
            "C1 fixed public original inputs"
        );
    }
    let parent = Path::new(fixed::WORKSPACE).join(".tmp/G1-C1-synthetic");
    std::fs::create_dir_all(&parent)?;
    let tmp = tempfile::Builder::new()
        .prefix("public-pin-")
        .tempdir_in(parent)?;
    let path = tmp.path().join("page.json");
    let raw = original[0].pages[0].encode()?;
    std::fs::write(&path, &raw)?;
    let mut pin = Pin {
        path: String::new(),
        bytes: raw.len(),
        sha256: sha(&raw),
    };
    ensure!(pinned(&path, &pin, PAGE_CAP)? == raw, "genuine public pin");
    pin.sha256 = "0".repeat(64);
    refused(pinned(&path, &pin, PAGE_CAP), "public pin mismatch")?;
    pin.sha256 = sha(&raw);
    pin.bytes += 1;
    refused(pinned(&path, &pin, PAGE_CAP), "public pin mismatch")?;
    Ok(())
}

#[test]
fn g1c1_genuine_b1_seam_multipage_and_negatives() -> Result<()> {
    // 34 texts cross the real32 boundary; B stops at16 to avoid the distinct
    // production B17 body/receipt pin. No511-message fixture is generated.
    let mut f = small([19, 17], false)?;
    for local in 0..2 {
        let repo = &mut f.repositories[local];
        let e = &f.expected[local];
        let deadline = until();
        let scope = readers::scope(repo, 0, local)?;
        // Unmodified Cold-B full recipe independently obtains both anchors from
        // this original L1 fixture, including Some(local request) serialization.
        let baseline = readers::full(repo, e, &f.proofs, deadline, None)?;
        let direct = capture(repo, 32)?;
        let pages = capture(repo, 1)?;
        ensure!(
            direct.len() == 2
                && Page::parse(&direct[0])?.items.len() == 32
                && Page::parse(&direct[1])?.items.len() == 2,
            "C1 actual32-page boundary"
        );
        let mut plan = original(
            e,
            &scope.raw,
            &f.proofs,
            &pages,
            &baseline.digest,
            &baseline.history_digest,
        )?;
        let reconstructed = plan.history_bytes(|n, c| read(repo, n, c), deadline)?;
        ensure!(
            reconstructed == direct,
            "C1 reconstructed equals independent genuine raw pages byte-for-byte"
        );
        let report = plan.replay(repo, deadline)?;
        ensure!(
            report.b1 == baseline.digest
                && report.logical == baseline.history_digest
                && report.sources == 36
                && report.texts == 34
                && report.own_requests == (e.next[local] - 1) as usize
                && report.reconstructed_pages == 2
                && report.frozen_suffix_rows == 33,
            "C1 full independent Cold-B recipe equality"
        );
        for n in 1..e.next[local] {
            let own = repo
                .request(0, e.text(local, n).0)?
                .context("genuine Some(local request)")?;
            ensure!(
                own.queue_sequence.is_none() && own.event.is_some(),
                "C1 imported request identity"
            );
        }
        let mut changed = pages.clone();
        let mut first = Page::parse(&changed[0])?;
        first.items[0].text.push('!');
        changed[0] = first.encode()?;
        refused(
            original(
                e,
                &scope.raw,
                &f.proofs,
                &changed,
                &baseline.digest,
                &baseline.history_digest,
            ),
            "pinned original logical digest",
        )?;
        let mut changed = pages.clone();
        let mut first = Page::parse(&changed[0])?;
        first.frontiers[0] += 1;
        changed[0] = first.encode()?;
        refused(
            original(
                e,
                &scope.raw,
                &f.proofs,
                &changed,
                &baseline.digest,
                &baseline.history_digest,
            ),
            "original page frontier/view",
        )?;
        let mut changed = pages.clone();
        changed.pop();
        refused(
            original(
                e,
                &scope.raw,
                &f.proofs,
                &changed,
                &baseline.digest,
                &baseline.history_digest,
            ),
            "complete original page count/bytes",
        )?;
        let mut changed = pages.clone();
        let mut second = Page::parse(&changed[1])?;
        second.items[0] = Page::parse(&pages[0])?.items[0].clone();
        changed[1] = second.encode()?;
        ensure!(
            original(
                e,
                &scope.raw,
                &f.proofs,
                &changed,
                &baseline.digest,
                &baseline.history_digest
            )
            .is_err(),
            "C1 duplicate/wrong map refused"
        );
        let mut changed = pages.clone();
        let mut first = Page::parse(&changed[0])?;
        let mut bytes = HistoryCursor::from_base64(&first.cursor)?
            .as_bytes()
            .to_vec();
        bytes[274] ^= 1;
        first.cursor = HistoryCursor::from_bytes(&bytes)?.to_base64();
        changed[0] = first.encode()?;
        let bad_mac = original(
            e,
            &scope.raw,
            &f.proofs,
            &changed,
            &baseline.digest,
            &baseline.history_digest,
        )?;
        ensure!(
            bad_mac
                .history_bytes(|n, c| read(repo, n, c), deadline)
                .is_err(),
            "C1 genuine repository rejects wrong cursor MAC"
        );
        refused(
            plan.history_bytes(
                |n, c| {
                    let mut page = Page::parse(&read(repo, n, c)?)?;
                    if n == 31 {
                        page.items.pop();
                    }
                    page.encode()
                },
                deadline,
            ),
            "exact first31-row join",
        )?;
        refused(
            plan.history_bytes(
                |n, c| {
                    let mut page = Page::parse(&read(repo, n, c)?)?;
                    if n == 31 {
                        page.items.swap(0, 1);
                    }
                    page.encode()
                },
                deadline,
            ),
            "exact first31-row join",
        )?;
        plan.b1 = "0".repeat(64);
        refused(
            plan.replay(repo, deadline),
            "immutable Cold-B B1 digest mismatch",
        )?;
        println!(
            "g1c1 local={local} genuine_texts=34 direct_pages=2 reconstructed_bytes_exact=true full_cold_b1_equal=true own_optional_requests={} frozen_suffix=33 negative_pin_row_cursor_frontier_map_digest_truncation=true",
            report.own_requests
        );
    }
    Ok(())
}

#[test]
fn g1c1_genuine_utf8_escaping_and_combined_byte_bound() -> Result<()> {
    let mut f = small([2, 2], true)?;
    for local in 0..2 {
        let repo = &mut f.repositories[local];
        let e = &f.expected[local];
        let scope = readers::scope(repo, 0, local)?;
        let direct = capture(repo, 32)?;
        let pages = capture(repo, 1)?;
        let plan = original(
            e,
            &scope.raw,
            &f.proofs,
            &pages,
            &"0".repeat(64),
            &logical(&direct)?,
        )?;
        // These genuine custom bodies intentionally differ from fixed::text;
        // this test exercises page serialization, not the original body oracle.
        let reconstructed = plan.history_bytes(|n, c| read(repo, n, c), until())?;
        ensure!(
            reconstructed == direct,
            "C1 genuine UTF8/escaping raw bytes"
        );
        let raw = std::str::from_utf8(&direct[0])?;
        ensure!(
            raw.contains("Привет")
                && raw.contains("🙂")
                && raw.contains("\\\"")
                && raw.contains("\\\\")
                && raw.contains("\\n")
                && raw.contains("\\t"),
            "C1 actual escaped fixture bytes"
        );
        let mut first = plan.ordered[0].clone();
        first.text = "x".repeat(40 * 1024);
        let mut tail = Page::parse(&read(repo, 31, &plan.pages[0].cursor)?)?;
        tail.items = vec![tail.items[0].clone(); 31];
        let mut single = tail.clone();
        single.items = vec![first.clone()];
        single.encode()?;
        tail.encode()?;
        refused(joined(&first, &tail), "combined page encoded byte cap")?;
        println!(
            "g1c1 escaping local={local} genuine_utf8_quotes_slash_newline_tab=true exact_raw=true combined_oversize_refused=true"
        );
    }
    Ok(())
}
