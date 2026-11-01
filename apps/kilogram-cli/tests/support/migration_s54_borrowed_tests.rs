//! Pure real argument/binding helpers; no owner, CLI, enrollment or private state.
use super::*;
fn id(n: u8) -> String {
    state::hex(&[n; 32])
}
fn pair() -> Pair {
    let node = |role: usize| {
        let base = PathBuf::from(format!("synthetic-owner-{role}"));
        Node {
            branch: Branch {
                base: base.clone(),
                state: base.join("state"),
                account: id(1 + role as u8),
                device: id(3 + role as u8),
                config: base.join("config"),
                context: id(5),
                claim: id(6 + role as u8),
            },
            profile: base.join("profile"),
            ipc: base.join("ipc"),
            locators: base.join("locators"),
            indexed: true,
            slot: 1 - role as u8,
        }
    };
    Pair {
        nodes: [node(0), node(1)],
        roots: ["root-a".into(), "root-b".into()],
        children: [None, None],
        base: "synthetic-only".into(),
        c: id(5),
        starts: [0; 2],
    }
}
fn files() -> Files {
    Files {
        origin: "origin".into(),
        proposal: "proposal".into(),
        consent: "consent".into(),
        authorities: ["authority-a".into(), "authority-b".into()],
    }
}
fn argument(args: &[String], name: &str) -> Result<String> {
    let at = args
        .iter()
        .position(|a| a == name)
        .context("real command argument")?;
    args.get(at + 1).cloned().context("real command value")
}
#[test]
fn s54_p10a_c1_command_and_upgrade_wrappers_preserve_actual_argument_order() -> Result<()> {
    let p = pair();
    let n = &p.nodes[0];
    let actual = command_arguments(n, "upgrade", &["--request-id".into(), "47".repeat(16)])?;
    ensure!(
        actual
            == vec![
                "ordered".into(),
                "upgrade".into(),
                "--ipc-file".into(),
                text(&n.ipc),
                "--slot".into(),
                "1".into(),
                "--request-id".into(),
                "47".repeat(16)
            ]
            && actual == upgrade_arguments(n, [71; 16])?,
        "same exact old C1 command/upgrade bytes"
    );
    let loc = locator_arguments(&p.nodes[0], &p.nodes[1])?;
    ensure!(
        loc == vec![
            "ordered".into(),
            "locator2-import".into(),
            "--existing".into(),
            "--ipc-file".into(),
            text(&n.ipc),
            "--slot".into(),
            "1".into(),
            "--locator-file".into(),
            text(&p.nodes[1].locators.join(format!("{}.locator2", p.c)))
        ],
        "same exact C1 import argument order"
    );
    Ok(())
}
#[test]
fn s54_p10a_ceremony_uses_original_roots_accounts_and_selected_c2_context() -> Result<()> {
    let p = pair();
    let c2 = id(8);
    let views = p.nodes.each_ref().map(|n| n.view(&c2, 1 - n.slot));
    let context = p.borrowed_for(views, &c2)?;
    for role in 0..2 {
        let args = context.creation_arguments(role, 99, &files())?;
        ensure!(
            argument(&args, "--context")? == c2
                && argument(&args, "--account-dir")? == text(&p.roots[role])
                && argument(&args, "--expect-account")? == p.nodes[role].branch.account
                && argument(&args, "--expect-peer-account")? == p.nodes[1 - role].branch.account,
            "actual C2 command context/root/account binding"
        );
        ensure!(
            argument(&args, "--step")? == if role == 0 { "propose" } else { "cosign" }
                && argument(&args, "--valid-for-seconds")? == "99"
                && args.contains(&"--proposal-file".into()) == (role == 1),
            "original propose/cosign ceremony construction"
        );
    }
    let c1 = p.borrowed_for(p.nodes.each_ref().map(|n| n.view(&p.c, n.slot)), &p.c)?;
    ensure!(
        argument(&c1.creation_arguments(0, 99, &files())?, "--context")? == p.c,
        "old C1 wrapper context unchanged"
    );
    Ok(())
}
#[test]
fn s54_p10a_nondefault_ready_slot_c2_locator_and_upgrade_use_actual_view() -> Result<()> {
    let p = pair();
    let c2 = id(8);
    let a = p.nodes[0].view(&c2, 0);
    let b = p.nodes[1].view(&c2, 1);
    let args = locator_arguments(&a, &b)?;
    ensure!(
        argument(&args, "--slot")? == "0"
            && argument(&args, "--locator-file")?
                == text(&b.locators.join(format!("{c2}.locator2")))
            && argument(&upgrade_arguments(&a, [0xa1; 16])?, "--slot")? == "0",
        "C2 is not assumed to be slot1"
    );
    let mut wrong = b;
    wrong.branch.context = p.c.clone();
    ensure!(
        locator_arguments(&a, &wrong).is_err(),
        "cross-context locator rejects"
    );
    Ok(())
}
#[test]
fn s54_p10a_borrowed_metadata_rejects_swapped_role_context_identity_paths_and_slot() -> Result<()> {
    let p = pair();
    let c2 = id(8);
    for mutation in 0..8 {
        let mut nodes = p.nodes.each_ref().map(|n| n.view(&c2, 1 - n.slot));
        match mutation {
            0 => nodes.swap(0, 1),
            1 => nodes[0].branch.context = p.c.clone(),
            2 => nodes[0].branch.account = id(99),
            3 => nodes[0].branch.device = id(99),
            4 => nodes[0].profile = "foreign".into(),
            5 => nodes[0].branch.state = "foreign".into(),
            6 => nodes[0].slot = 2,
            _ => nodes[0].branch.claim = id(99),
        }
        ensure!(
            p.borrowed_for(nodes, &c2).is_err(),
            "wrong original binding {mutation}"
        );
    }
    ensure!(
        p.borrowed_for(p.nodes.each_ref().map(|n| n.view("bad", 0)), "bad")
            .is_err(),
        "bad canonical context"
    );
    Ok(())
}
