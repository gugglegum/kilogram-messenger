//! Borrowed original metadata only: no child handles, state opens or owner manager.
use super::*;
use kilogram_cli::ordered_dialog::state;
use kilogram_protocol::ordered_dialog_v2 as v2;

pub(super) fn command_arguments(node: &Node, name: &str, extra: &[String]) -> Result<Vec<String>> {
    ensure!(node.slot < 2, "bounded original physical slot");
    let mut args = vec![
        "ordered".into(),
        name.into(),
        "--ipc-file".into(),
        text(&node.ipc),
        "--slot".into(),
        node.slot.to_string(),
    ];
    args.extend_from_slice(extra);
    Ok(args)
}
pub(super) fn upgrade_arguments(node: &Node, request: [u8; 16]) -> Result<Vec<String>> {
    command_arguments(
        node,
        "upgrade",
        &["--request-id".into(), state::hex(&request)],
    )
}
pub(super) fn locator_arguments(node: &Node, peer: &Node) -> Result<Vec<String>> {
    ensure!(
        node.branch.context == peer.branch.context,
        "same selected context for Locator2 import"
    );
    let mut args = command_arguments(
        node,
        "locator2-import",
        &[
            "--locator-file".into(),
            text(
                &peer
                    .locators
                    .join(format!("{}.locator2", node.branch.context)),
            ),
        ],
    )?;
    args.insert(2, "--existing".into());
    Ok(args)
}
/// Only public metadata views are copied. The original Pair retains all owners.
pub(super) struct ContextCommands {
    nodes: [Node; 2],
    roots: [PathBuf; 2],
    base: PathBuf,
    context: String,
}
impl Pair {
    pub(super) fn borrowed_for(&self, nodes: [Node; 2], context: &str) -> Result<ContextCommands> {
        ensure!(
            state::hex(&state::parse_id(context)?) == context,
            "canonical borrowed context"
        );
        for (role, node) in nodes.iter().enumerate() {
            let original = &self.nodes[role];
            ensure!(
                node.branch.context == context
                    && node.slot < 2
                    && node.indexed == original.indexed
                    && node.profile == original.profile
                    && node.ipc == original.ipc
                    && node.locators == original.locators
                    && node.branch.state == original.branch.state
                    && node.branch.base == original.branch.base
                    && node.branch.account == original.branch.account
                    && node.branch.device == original.branch.device
                    && node.branch.claim == original.branch.claim
                    && node.branch.config == original.branch.config,
                "same original borrowed role/account/device/profile/state binding"
            );
        }
        ensure!(
            nodes[0].branch.account != nodes[1].branch.account
                && nodes[0].branch.device != nodes[1].branch.device,
            "opposite original peers"
        );
        Ok(ContextCommands {
            nodes,
            roots: self.roots.clone(),
            base: self.base.clone(),
            context: context.into(),
        })
    }
}
struct Files {
    origin: PathBuf,
    proposal: PathBuf,
    consent: PathBuf,
    authorities: [PathBuf; 2],
}
impl ContextCommands {
    fn creation_arguments(&self, role: usize, ttl: u64, files: &Files) -> Result<Vec<String>> {
        ensure!(role < 2, "bounded ceremony role");
        let mut args = vec![
            "ordered".into(),
            "consent2-create".into(),
            "--origin".into(),
            "existing".into(),
            "--step".into(),
            if role == 0 { "propose" } else { "cosign" }.into(),
            "--context".into(),
            self.context.clone(),
            "--existing-origin-file".into(),
            text(&files.origin),
            "--account-dir".into(),
            text(&self.roots[role]),
            "--expect-account".into(),
            self.nodes[role].branch.account.clone(),
            "--expect-peer-account".into(),
            self.nodes[1 - role].branch.account.clone(),
            "--authority-a-file".into(),
            text(&files.authorities[0]),
            "--authority-b-file".into(),
            text(&files.authorities[1]),
            "--minimum-authority-a-file".into(),
            text(&files.authorities[0]),
            "--minimum-authority-b-file".into(),
            text(&files.authorities[1]),
            "--valid-for-seconds".into(),
            ttl.to_string(),
            "--output-file".into(),
            text(if role == 0 {
                &files.proposal
            } else {
                &files.consent
            }),
        ];
        if role == 1 {
            args.extend(["--proposal-file".into(), text(&files.proposal)]);
        }
        Ok(args)
    }
    fn command(&self, role: usize, name: &str, extra: &[String]) -> Result<String> {
        cli(
            &current(),
            &command_arguments(
                self.nodes.get(role).context("borrowed original role")?,
                name,
                extra,
            )?,
        )
    }
    pub fn upgrade(
        &self,
        role: usize,
        request: [u8; 16],
        mut check: impl FnMut() -> Result<()>,
    ) -> Result<Value> {
        let args = upgrade_arguments(
            self.nodes.get(role).context("borrowed original role")?,
            request,
        )?;
        check()?;
        let raw = cli(&current(), &args)?;
        check()?;
        Ok(serde_json::from_str(&raw)?)
    }
    pub fn ceremony(
        &self,
        name: &str,
        expected: &str,
        ttl: u64,
        mut check: impl FnMut(Option<kilogram_protocol::ordered_dialog_v1::Id>) -> Result<()>,
    ) -> Result<kilogram_protocol::ordered_dialog_v1::Id> {
        check(None)?;
        let origin = self.base.join(format!("{name}.origin.cbor"));
        cli(
            &current(),
            &[
                "ordered".into(),
                "existing-origin-export".into(),
                "--profile".into(),
                text(&self.nodes[0].profile),
                "--context".into(),
                self.context.clone(),
                "--output-file".into(),
                text(&origin),
            ],
        )?;
        check(None)?;
        let authorities = [
            {
                check(None)?;
                policy::snapshot(&self.base, &self.roots[0], &format!("{name}.a.authority"))?
            },
            {
                check(None)?;
                policy::snapshot(&self.base, &self.roots[1], &format!("{name}.b.authority"))?
            },
        ];
        check(None)?;
        let proposal = self.base.join(format!("{name}.proposal"));
        let consent = self.base.join(format!("{name}.consent"));
        let files = Files {
            origin: origin.clone(),
            proposal,
            consent: consent.clone(),
            authorities,
        };
        for role in 0..2 {
            let args = self.creation_arguments(role, ttl, &files)?;
            check(None)?;
            policy::audit(&self.base, &current(), &args)?;
            check(None)?;
        }
        let scope = v2::VerifiedExistingScope1::verify_export(
            &kilogram_cli::ordered_dialog::admin::read_public(&origin, v2::MAX_EXISTING_ORIGIN)?,
        )?;
        let raw = kilogram_cli::ordered_dialog::admin::read_public(&consent, 4096)?;
        let parsed = v2::ParsedConsent2::parse(&raw)?;
        ensure!(
            scope.c() == state::parse_id(&self.context)?,
            "actual exported ceremony context"
        );
        for (role, node) in self.nodes.iter().enumerate() {
            let peer = scope.peers()[role].certificate();
            ensure!(
                state::hex(peer.account_id().as_bytes()) == node.branch.account
                    && state::hex(peer.device_id().as_bytes()) == node.branch.device,
                "actual public origin original role/account/device"
            );
        }
        scope.verify_consent(&parsed, scope.peers())?;
        let reference = parsed.reference();
        check(Some(reference))?;
        for role in 0..2 {
            check(Some(reference))?;
            self.command(
                role,
                "consent2-import",
                &[
                    "--existing".into(),
                    "--expected-current".into(),
                    expected.into(),
                    "--consent-file".into(),
                    text(&consent),
                ],
            )?;
            check(Some(reference))?;
        }
        Ok(reference)
    }
}
#[cfg(test)]
#[path = "migration_s54_borrowed_tests.rs"]
mod tests;
