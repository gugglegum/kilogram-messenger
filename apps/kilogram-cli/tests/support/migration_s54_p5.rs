//! S54 foundation only. No whole-P5 test/activation is defined in block1.
use super::*;
#[path = "migration_s54_oracle.rs"]
mod oracle;

struct OldOrigin {
    pair: Pair,
    contexts: [String; 2],
}
impl OldOrigin {
    fn create(parent: &Path) -> Result<Self> {
        let owned = workspace()
            .canonicalize()?
            .join(".tmp")
            .join("migration-s54-integrated");
        let actual = parent.canonicalize()?;
        ensure!(
            actual.starts_with(&owned) && actual != owned,
            "new owned S54 attempt only"
        );
        let base = parent.join("p5-origin");
        std::fs::create_dir(&base)?;
        let (old, c1) = old_pair(&base)?;
        let c2 = obligations::context_between(&old[0], &old[1], &base, "p5-second")?;
        ensure!(c1 != c2, "two actual independent old contexts");
        for node in &old {
            // The public CLI creates the same original profile, now enabling
            // both real contexts. No queue/send or stored-row constructor here.
            std::fs::remove_file(&node.profile)?;
            cli(
                &frozen(),
                &[
                    "runtime-profile-create".into(),
                    "--profile-mode".into(),
                    "ordered-dialog-v1".into(),
                    "--profile-file".into(),
                    text(&node.profile),
                    "--state-dir".into(),
                    text(&node.state),
                    "--ipc-file".into(),
                    text(&node.ipc),
                    "--locator-directory".into(),
                    text(&node.locators),
                    "--ordered-context".into(),
                    c1.clone(),
                    "--ordered-context".into(),
                    c2.clone(),
                ],
            )?;
        }
        let nodes = [Node::from_old(&old[0], &c1)?, Node::from_old(&old[1], &c1)?];
        let roots = old.map(|n| n.root);
        Ok(Self {
            pair: Pair {
                nodes,
                roots,
                children: [None, None],
                base,
                c: c1.clone(),
                starts: [0, 0],
            },
            contexts: [c1, c2],
        })
    }
    fn start_ready(&mut self) -> Result<()> {
        self.pair.start(0, None)?;
        self.pair.start(1, None)?;
        for context in &self.contexts {
            let views = self.pair.nodes.each_ref().map(|n| n.view(context, 0));
            views[0].import(&views[1])?;
            views[1].import(&views[0])?;
            for view in &views {
                let until = Instant::now() + Duration::from_secs(30);
                loop {
                    s53_deadline::check()?;
                    for child in self.pair.children.iter_mut().flatten() {
                        child.alive()?;
                    }
                    let status = view.status()?;
                    let row = view.context(&status)?;
                    if oracle::old_zero_text_ready(row)? {
                        break;
                    }
                    ensure!(Instant::now() < until, "real zero-Text Ready prerequisite");
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
        Ok(())
    }
    fn migrate_stopped(&mut self, role: usize) -> Result<Value> {
        ensure!(
            role < 2 && self.pair.children[role].is_none(),
            "exclusive in-place migration"
        );
        let before = self.pair.nodes[role].branch.state.clone();
        let ready = self.pair.nodes[role].migrate()?;
        ensure!(
            self.pair.nodes[role].branch.state == before,
            "same two original state directories"
        );
        for context in &self.contexts {
            oracle::ready_slot(&ready, context)?;
        }
        Ok(ready)
    }
}
