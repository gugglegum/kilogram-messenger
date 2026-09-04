use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr as _,
};

use anyhow::{Context as _, Result, bail, ensure};
use kilogram_identity::{AccountId, DeviceId};
use serde::Deserialize;

pub(crate) const MAX_WIZARD_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_WIZARD_STDIN_BYTES: usize = 4 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkRequestOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) sas: String,
    pub(crate) expires_at_unix_seconds: u64,
    pub(crate) workspace_dir: PathBuf,
    pub(crate) state_dir: PathBuf,
    pub(crate) request_file: PathBuf,
    pub(crate) prekey_pool_file: PathBuf,
    pub(crate) vault_key_protection: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkInspectOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) sas: String,
    pub(crate) issued_at_unix_seconds: u64,
    pub(crate) expires_at_unix_seconds: u64,
    pub(crate) request_fresh: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkAuthorizeOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) response_file: PathBuf,
    pub(crate) device_list_file: PathBuf,
    pub(crate) response_encrypted_for_device: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceLinkAcceptOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) request_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) workspace_dir: PathBuf,
    pub(crate) state_dir: PathBuf,
    pub(crate) certificate_file: PathBuf,
    pub(crate) device_list_file: PathBuf,
    pub(crate) prekey_pool_file: PathBuf,
    pub(crate) vault_key_protection: String,
    pub(crate) history_recovery: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AccountRecoveryOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) device_count: usize,
    pub(crate) conversation_membership_count: usize,
    pub(crate) package_id: String,
    pub(crate) package_file: PathBuf,
    pub(crate) witness_file: PathBuf,
    pub(crate) account_root_dir: Option<PathBuf>,
    pub(crate) root_key_protection: Option<String>,
    pub(crate) freshness_scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AccountRecoveryStatusOutput {
    pub(crate) status: String,
    pub(crate) checkpoint_state: String,
    pub(crate) account_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) device_count: usize,
    pub(crate) conversation_membership_count: usize,
    pub(crate) current_package_id: String,
    pub(crate) recorded_package_id: Option<String>,
    pub(crate) recorded_authority_revision: Option<u64>,
    pub(crate) account_root_dir: PathBuf,
    pub(crate) lifecycle_scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryQuorumRequestOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) request_id: String,
    pub(crate) package_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) recovery_roster_digest: String,
    pub(crate) roster_count: usize,
    pub(crate) required_approvals: usize,
    pub(crate) issued_at_unix_seconds: u64,
    pub(crate) expires_at_unix_seconds: u64,
    pub(crate) request_file: PathBuf,
    pub(crate) freshness_scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryQuorumListenOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) request_id: String,
    pub(crate) package_id: String,
    pub(crate) approver_device_id: String,
    pub(crate) approval_id: String,
    pub(crate) ticket_file: PathBuf,
    pub(crate) route_policy: String,
    pub(crate) transport_path: String,
    pub(crate) transport_remote_address: String,
    pub(crate) transport_rtt_milliseconds: u64,
    pub(crate) transport_open_paths: usize,
    pub(crate) approval_head_source: String,
    pub(crate) approval_head_committed_before_ticket_publish: bool,
    pub(crate) reused_committed_approval: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryQuorumTransportObservation {
    pub(crate) approver_device_id: String,
    pub(crate) ticket_file: PathBuf,
    pub(crate) approval_file: PathBuf,
    pub(crate) route_policy: String,
    pub(crate) transport_path: String,
    pub(crate) transport_remote_address: String,
    pub(crate) transport_rtt_milliseconds: u64,
    pub(crate) transport_open_paths: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryQuorumCollectOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) request_id: String,
    pub(crate) package_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) recovery_roster_digest: String,
    pub(crate) roster_count: usize,
    pub(crate) observed_approvals: usize,
    pub(crate) required_approvals: usize,
    pub(crate) majority_satisfied: bool,
    pub(crate) freshness_claim: String,
    pub(crate) cross_roster_fork_safety: bool,
    pub(crate) approval_directory: PathBuf,
    pub(crate) transports: Vec<RecoveryQuorumTransportObservation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryQuorumVerifyOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) request_id: String,
    pub(crate) package_id: String,
    pub(crate) authority_revision: u64,
    pub(crate) recovery_roster_digest: String,
    pub(crate) roster_count: usize,
    pub(crate) observed_approvals: usize,
    pub(crate) required_approvals: usize,
    pub(crate) majority_satisfied: bool,
    pub(crate) freshness_claim: String,
    pub(crate) cross_roster_fork_safety: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryPolicyTransitionRequestOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) request_id: String,
    pub(crate) old_epoch: u64,
    pub(crate) new_epoch: u64,
    pub(crate) previous_transition_id: String,
    pub(crate) old_recovery_roster_digest: String,
    pub(crate) new_recovery_roster_digest: String,
    pub(crate) old_roster_count: usize,
    pub(crate) new_roster_count: usize,
    pub(crate) old_required_approvals: usize,
    pub(crate) new_required_approvals: usize,
    pub(crate) expires_at_unix_seconds: u64,
    pub(crate) request_file: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryPolicyTransitionListenOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) request_id: String,
    pub(crate) old_epoch: u64,
    pub(crate) new_epoch: u64,
    pub(crate) approver_device_id: String,
    pub(crate) approval_id: String,
    pub(crate) ticket_file: PathBuf,
    pub(crate) route_policy: String,
    pub(crate) transport_path: String,
    pub(crate) transport_remote_address: String,
    pub(crate) transport_rtt_milliseconds: u64,
    pub(crate) transport_open_paths: usize,
    pub(crate) approval_head_source: String,
    pub(crate) approval_head_committed_before_ticket_publish: bool,
    pub(crate) reused_committed_approval: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryPolicyTransitionTransportObservation {
    pub(crate) approver_device_id: String,
    pub(crate) ticket_file: PathBuf,
    pub(crate) approval_file: PathBuf,
    pub(crate) route_policy: String,
    pub(crate) transport_path: String,
    pub(crate) transport_remote_address: String,
    pub(crate) transport_rtt_milliseconds: u64,
    pub(crate) transport_open_paths: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryPolicyTransitionCollectOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) request_id: String,
    pub(crate) old_epoch: u64,
    pub(crate) new_epoch: u64,
    pub(crate) old_observed_approvals: usize,
    pub(crate) old_required_approvals: usize,
    pub(crate) new_observed_approvals: usize,
    pub(crate) new_required_approvals: usize,
    pub(crate) joint_majority_satisfied: bool,
    pub(crate) cross_roster_fork_safety: bool,
    pub(crate) approval_directory: PathBuf,
    pub(crate) transports: Vec<RecoveryPolicyTransitionTransportObservation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryPolicyTransitionCertificateOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) certificate_id: String,
    pub(crate) old_epoch: u64,
    pub(crate) new_epoch: u64,
    pub(crate) previous_transition_id: String,
    pub(crate) old_recovery_roster_digest: String,
    pub(crate) new_recovery_roster_digest: String,
    pub(crate) old_observed_approvals: usize,
    pub(crate) old_required_approvals: usize,
    pub(crate) new_observed_approvals: usize,
    pub(crate) new_required_approvals: usize,
    pub(crate) joint_majority_satisfied: bool,
    pub(crate) cross_roster_fork_safety: bool,
    pub(crate) certificate_file: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecoveryPolicyInstallOutput {
    pub(crate) status: String,
    pub(crate) account_id: String,
    pub(crate) policy_epoch: u64,
    pub(crate) recovery_roster_digest: String,
    pub(crate) latest_transition_id: String,
    pub(crate) policy_state_source: String,
    pub(crate) installed_certificate_file: PathBuf,
    pub(crate) cross_roster_fork_safety: bool,
}

impl AccountRecoveryOutput {
    pub(crate) fn validate_expected_status(&self, expected: &str) -> Result<()> {
        self.validate()?;
        validate_status(&self.status, expected)
    }
}

pub(crate) trait WizardJsonOutput: Sized + for<'de> Deserialize<'de> {
    fn validate(&self) -> Result<()>;
}

impl WizardJsonOutput for DeviceLinkRequestOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-request-created")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        validate_sas(&self.sas)?;
        validate_workspace_paths(
            &self.workspace_dir,
            [&self.state_dir, &self.request_file, &self.prekey_pool_file],
        )?;
        ensure!(
            self.expires_at_unix_seconds > 0,
            "device-link request expiry is missing"
        );
        ensure!(
            !self.vault_key_protection.is_empty(),
            "device-link vault protection is missing"
        );
        Ok(())
    }
}

impl WizardJsonOutput for DeviceLinkInspectOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-request-verified")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        validate_sas(&self.sas)?;
        ensure!(
            self.issued_at_unix_seconds <= self.expires_at_unix_seconds,
            "device-link request timestamps are invalid"
        );
        Ok(())
    }
}

impl WizardJsonOutput for DeviceLinkAuthorizeOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-authorized")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        ensure!(
            self.authority_revision > 0,
            "device-link authority revision is missing"
        );
        ensure!(
            self.response_encrypted_for_device,
            "device-link response is not recipient encrypted"
        );
        validate_absolute_file_path(&self.response_file, "response_file")?;
        validate_absolute_file_path(&self.device_list_file, "device_list_file")?;
        Ok(())
    }
}

impl WizardJsonOutput for DeviceLinkAcceptOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "device-link-accepted")?;
        validate_identity_fields(&self.account_id, &self.device_id, &self.request_id)?;
        ensure!(
            self.authority_revision > 0,
            "device-link authority revision is missing"
        );
        validate_workspace_paths(
            &self.workspace_dir,
            [
                &self.state_dir,
                &self.certificate_file,
                &self.device_list_file,
                &self.prekey_pool_file,
            ],
        )?;
        ensure!(
            !self.vault_key_protection.is_empty(),
            "device-link vault protection is missing"
        );
        ensure!(
            self.history_recovery == "ready-for-recipient-bound-multi-source-plans",
            "device-link recovery readiness is invalid"
        );
        Ok(())
    }
}

impl WizardJsonOutput for AccountRecoveryOutput {
    fn validate(&self) -> Result<()> {
        ensure!(
            matches!(
                self.status.as_str(),
                "account-root-recovery-exported"
                    | "account-root-recovery-verified"
                    | "account-root-recovery-restored"
            ),
            "wizard helper returned an unknown Account Root recovery status"
        );
        AccountId::from_str(&self.account_id)
            .context("Account Root recovery Account ID is invalid")?;
        ensure!(
            self.authority_revision > 0,
            "Account Root recovery authority revision is missing"
        );
        ensure!(
            self.device_count > 0,
            "Account Root recovery device list is empty"
        );
        validate_hex_id(&self.package_id, "Account Root recovery package ID")?;
        validate_absolute_file_path(&self.package_file, "package_file")?;
        validate_absolute_file_path(&self.witness_file, "witness_file")?;
        ensure!(
            self.package_file != self.witness_file,
            "Account Root recovery package and witness paths are identical"
        );
        ensure!(
            self.freshness_scope == "exact-independent-witness-not-global-monotonic-service",
            "Account Root recovery freshness scope is invalid"
        );
        if self.status == "account-root-recovery-restored" {
            let root = self
                .account_root_dir
                .as_ref()
                .context("restored Account Root path is missing")?;
            ensure!(root.is_absolute(), "restored Account Root path is relative");
            ensure!(
                self.root_key_protection
                    .as_ref()
                    .is_some_and(|value| !value.is_empty()),
                "restored Account Root key protection is missing"
            );
        } else {
            ensure!(
                self.account_root_dir.is_none() && self.root_key_protection.is_none(),
                "non-restore output unexpectedly contains local Root state"
            );
        }
        Ok(())
    }
}

impl WizardJsonOutput for AccountRecoveryStatusOutput {
    fn validate(&self) -> Result<()> {
        ensure!(
            matches!(
                self.status.as_str(),
                "account-root-recovery-current" | "account-root-recovery-update-required"
            ),
            "wizard helper returned an unknown recovery checkpoint status"
        );
        AccountId::from_str(&self.account_id)
            .context("recovery checkpoint Account ID is invalid")?;
        ensure!(
            self.authority_revision > 0,
            "recovery checkpoint authority revision is missing"
        );
        ensure!(
            self.device_count > 0,
            "recovery checkpoint device list is empty"
        );
        validate_hex_id(&self.current_package_id, "current recovery package ID")?;
        ensure!(
            self.account_root_dir.is_absolute(),
            "recovery checkpoint Root path is relative"
        );
        ensure!(
            self.lifecycle_scope == "local-exact-export-receipt-not-global-freshness-proof",
            "recovery checkpoint lifecycle scope is invalid"
        );
        ensure!(
            self.recorded_package_id.is_some() == self.recorded_authority_revision.is_some(),
            "recovery checkpoint recorded fields are incomplete"
        );
        if let Some(package_id) = self.recorded_package_id.as_ref() {
            validate_hex_id(package_id, "recorded recovery package ID")?;
        }
        if let Some(revision) = self.recorded_authority_revision {
            ensure!(
                revision > 0,
                "recorded recovery authority revision is invalid"
            );
        }
        match self.status.as_str() {
            "account-root-recovery-current" => {
                ensure!(
                    self.checkpoint_state == "current"
                        && self.recorded_package_id.as_ref() == Some(&self.current_package_id)
                        && self.recorded_authority_revision == Some(self.authority_revision),
                    "current recovery checkpoint fields disagree"
                );
            }
            "account-root-recovery-update-required" => {
                ensure!(
                    self.checkpoint_state == "update-required",
                    "stale recovery checkpoint state is invalid"
                );
                ensure!(
                    self.recorded_package_id.as_ref() != Some(&self.current_package_id)
                        || self.recorded_authority_revision != Some(self.authority_revision),
                    "stale recovery checkpoint unexpectedly matches current state"
                );
            }
            _ => unreachable!("status was checked above"),
        }
        Ok(())
    }
}

impl WizardJsonOutput for RecoveryQuorumRequestOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "account-recovery-quorum-request-created")?;
        AccountId::from_str(&self.account_id).context("quorum request Account ID is invalid")?;
        validate_hex_id(&self.request_id, "quorum request ID")?;
        validate_hex_id(&self.package_id, "quorum request package ID")?;
        validate_hex_id(&self.recovery_roster_digest, "quorum roster digest")?;
        ensure!(
            self.authority_revision > 0,
            "quorum authority revision is missing"
        );
        ensure!(
            self.roster_count > 0 && self.required_approvals == self.roster_count / 2 + 1,
            "quorum request threshold is invalid"
        );
        ensure!(
            self.issued_at_unix_seconds <= self.expires_at_unix_seconds,
            "quorum request timestamps are invalid"
        );
        validate_absolute_file_path(&self.request_file, "request_file")?;
        ensure!(
            self.freshness_scope == "exact-roster-current-device-observation",
            "quorum request freshness scope is invalid"
        );
        Ok(())
    }
}

impl WizardJsonOutput for RecoveryQuorumListenOutput {
    fn validate(&self) -> Result<()> {
        validate_status(
            &self.status,
            "account-recovery-quorum-approved-over-transport",
        )?;
        validate_quorum_identity_fields(
            &self.account_id,
            &self.request_id,
            &self.package_id,
            &self.approver_device_id,
        )?;
        validate_hex_id(&self.approval_id, "transported approval ID")?;
        validate_absolute_file_path(&self.ticket_file, "ticket_file")?;
        validate_transport_fields(
            &self.route_policy,
            &self.transport_path,
            &self.transport_remote_address,
            self.transport_open_paths,
        )?;
        ensure!(
            self.approval_head_source == "db-primary"
                && self.approval_head_committed_before_ticket_publish,
            "network approval was not committed before ticket publication"
        );
        let _ = self.transport_rtt_milliseconds;
        let _ = self.reused_committed_approval;
        Ok(())
    }
}

impl WizardJsonOutput for RecoveryQuorumCollectOutput {
    fn validate(&self) -> Result<()> {
        ensure!(
            matches!(
                self.status.as_str(),
                "account-recovery-quorum-majority-collected"
                    | "account-recovery-quorum-partial-collected"
            ),
            "unknown quorum collection status"
        );
        validate_quorum_report(
            &self.account_id,
            &self.request_id,
            &self.package_id,
            &self.recovery_roster_digest,
            self.authority_revision,
            self.roster_count,
            self.observed_approvals,
            self.required_approvals,
            self.majority_satisfied,
            &self.freshness_claim,
            self.cross_roster_fork_safety,
        )?;
        ensure!(
            (self.status == "account-recovery-quorum-majority-collected")
                == self.majority_satisfied,
            "quorum collection status disagrees with majority result"
        );
        ensure!(
            self.approval_directory.is_absolute(),
            "approval directory is relative"
        );
        ensure!(
            self.transports.len() == self.observed_approvals,
            "quorum transport count disagrees with approvals"
        );
        let mut devices = BTreeMap::new();
        for transport in &self.transports {
            DeviceId::from_str(&transport.approver_device_id)
                .context("quorum transport Device ID is invalid")?;
            ensure!(
                devices
                    .insert(transport.approver_device_id.clone(), ())
                    .is_none(),
                "duplicate quorum transport device"
            );
            validate_absolute_file_path(&transport.ticket_file, "transport ticket_file")?;
            validate_absolute_file_path(&transport.approval_file, "transport approval_file")?;
            validate_transport_fields(
                &transport.route_policy,
                &transport.transport_path,
                &transport.transport_remote_address,
                transport.transport_open_paths,
            )?;
            let _ = transport.transport_rtt_milliseconds;
        }
        Ok(())
    }
}

impl WizardJsonOutput for RecoveryQuorumVerifyOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "account-recovery-quorum-verified")?;
        validate_quorum_report(
            &self.account_id,
            &self.request_id,
            &self.package_id,
            &self.recovery_roster_digest,
            self.authority_revision,
            self.roster_count,
            self.observed_approvals,
            self.required_approvals,
            self.majority_satisfied,
            &self.freshness_claim,
            self.cross_roster_fork_safety,
        )
    }
}

impl WizardJsonOutput for RecoveryPolicyTransitionRequestOutput {
    fn validate(&self) -> Result<()> {
        validate_status(
            &self.status,
            "account-recovery-policy-transition-request-created",
        )?;
        AccountId::from_str(&self.account_id).context("policy request Account ID is invalid")?;
        validate_hex_id(&self.request_id, "policy request ID")?;
        validate_hex_id(
            &self.previous_transition_id,
            "previous policy transition ID",
        )?;
        validate_hex_id(&self.old_recovery_roster_digest, "old policy roster digest")?;
        validate_hex_id(&self.new_recovery_roster_digest, "new policy roster digest")?;
        ensure!(
            self.new_epoch == self.old_epoch.saturating_add(1),
            "policy request epoch transition is invalid"
        );
        validate_policy_threshold(self.old_roster_count, self.old_required_approvals, "old")?;
        validate_policy_threshold(self.new_roster_count, self.new_required_approvals, "new")?;
        ensure!(
            self.expires_at_unix_seconds > 0,
            "policy request expiry is missing"
        );
        validate_absolute_file_path(&self.request_file, "request_file")
    }
}

impl WizardJsonOutput for RecoveryPolicyTransitionListenOutput {
    fn validate(&self) -> Result<()> {
        validate_status(
            &self.status,
            "account-recovery-policy-transition-approved-over-transport",
        )?;
        AccountId::from_str(&self.account_id).context("policy listener Account ID is invalid")?;
        DeviceId::from_str(&self.approver_device_id)
            .context("policy listener Device ID is invalid")?;
        validate_hex_id(&self.request_id, "policy listener request ID")?;
        validate_hex_id(&self.approval_id, "policy listener approval ID")?;
        ensure!(
            self.new_epoch == self.old_epoch.saturating_add(1),
            "policy listener epoch transition is invalid"
        );
        validate_absolute_file_path(&self.ticket_file, "ticket_file")?;
        validate_transport_fields(
            &self.route_policy,
            &self.transport_path,
            &self.transport_remote_address,
            self.transport_open_paths,
        )?;
        ensure!(
            self.approval_head_source == "db-primary"
                && self.approval_head_committed_before_ticket_publish,
            "policy approval was not committed before ticket publication"
        );
        let _ = self.transport_rtt_milliseconds;
        let _ = self.reused_committed_approval;
        Ok(())
    }
}

impl WizardJsonOutput for RecoveryPolicyTransitionCollectOutput {
    fn validate(&self) -> Result<()> {
        ensure!(
            matches!(
                self.status.as_str(),
                "account-recovery-policy-transition-joint-majority-collected"
                    | "account-recovery-policy-transition-partial-collected"
            ),
            "unknown policy collection status"
        );
        validate_policy_report(
            &self.account_id,
            &self.request_id,
            self.old_epoch,
            self.new_epoch,
            self.old_observed_approvals,
            self.old_required_approvals,
            self.new_observed_approvals,
            self.new_required_approvals,
            self.joint_majority_satisfied,
            self.cross_roster_fork_safety,
        )?;
        ensure!(
            (self.status == "account-recovery-policy-transition-joint-majority-collected")
                == self.joint_majority_satisfied,
            "policy collection status disagrees with threshold result"
        );
        ensure!(
            self.approval_directory.is_absolute(),
            "policy approval directory is relative"
        );
        ensure!(
            !self.transports.is_empty(),
            "policy collection has no transport observations"
        );
        let mut devices = BTreeMap::new();
        for transport in &self.transports {
            DeviceId::from_str(&transport.approver_device_id)
                .context("policy transport Device ID is invalid")?;
            ensure!(
                devices
                    .insert(transport.approver_device_id.clone(), ())
                    .is_none(),
                "duplicate policy transport device"
            );
            validate_absolute_file_path(&transport.ticket_file, "transport ticket_file")?;
            validate_absolute_file_path(&transport.approval_file, "transport approval_file")?;
            validate_transport_fields(
                &transport.route_policy,
                &transport.transport_path,
                &transport.transport_remote_address,
                transport.transport_open_paths,
            )?;
            let _ = transport.transport_rtt_milliseconds;
        }
        Ok(())
    }
}

impl WizardJsonOutput for RecoveryPolicyTransitionCertificateOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "account-recovery-policy-transition-certified")?;
        validate_policy_report(
            &self.account_id,
            &self.certificate_id,
            self.old_epoch,
            self.new_epoch,
            self.old_observed_approvals,
            self.old_required_approvals,
            self.new_observed_approvals,
            self.new_required_approvals,
            self.joint_majority_satisfied,
            self.cross_roster_fork_safety,
        )?;
        validate_hex_id(
            &self.previous_transition_id,
            "previous policy transition ID",
        )?;
        validate_hex_id(&self.old_recovery_roster_digest, "old policy roster digest")?;
        validate_hex_id(&self.new_recovery_roster_digest, "new policy roster digest")?;
        ensure!(
            self.joint_majority_satisfied && self.cross_roster_fork_safety,
            "certified transition lacks joint-majority fork safety"
        );
        validate_absolute_file_path(&self.certificate_file, "certificate_file")
    }
}

impl WizardJsonOutput for RecoveryPolicyInstallOutput {
    fn validate(&self) -> Result<()> {
        validate_status(&self.status, "account-recovery-policy-transition-installed")?;
        AccountId::from_str(&self.account_id).context("installed policy Account ID is invalid")?;
        validate_hex_id(
            &self.recovery_roster_digest,
            "installed policy roster digest",
        )?;
        validate_hex_id(&self.latest_transition_id, "installed policy transition ID")?;
        ensure!(self.policy_epoch > 0, "installed policy epoch is missing");
        ensure!(
            matches!(
                self.policy_state_source.as_str(),
                "db-primary-installed" | "db-primary-idempotent"
            ),
            "installed policy source is invalid"
        );
        ensure!(
            self.cross_roster_fork_safety,
            "installed policy lacks cross-roster fork safety"
        );
        validate_absolute_file_path(
            &self.installed_certificate_file,
            "installed_certificate_file",
        )
    }
}

fn validate_policy_threshold(roster: usize, required: usize, label: &str) -> Result<()> {
    ensure!(
        roster > 0 && required == roster / 2 + 1,
        "{label} policy threshold is invalid"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_policy_report(
    account: &str,
    id: &str,
    old_epoch: u64,
    new_epoch: u64,
    old_observed: usize,
    old_required: usize,
    new_observed: usize,
    new_required: usize,
    joint_majority: bool,
    cross_roster_fork_safety: bool,
) -> Result<()> {
    AccountId::from_str(account).context("policy report Account ID is invalid")?;
    validate_hex_id(id, "policy report ID")?;
    ensure!(
        new_epoch == old_epoch.saturating_add(1),
        "policy report epoch transition is invalid"
    );
    ensure!(
        old_required > 0 && new_required > 0,
        "policy report thresholds are missing"
    );
    ensure!(
        joint_majority == (old_observed >= old_required && new_observed >= new_required),
        "policy report threshold fields disagree"
    );
    ensure!(
        !cross_roster_fork_safety || joint_majority,
        "policy report claims fork safety without joint majority"
    );
    Ok(())
}

fn validate_quorum_identity_fields(
    account: &str,
    request: &str,
    package: &str,
    device: &str,
) -> Result<()> {
    AccountId::from_str(account).context("quorum Account ID is invalid")?;
    DeviceId::from_str(device).context("quorum Device ID is invalid")?;
    validate_hex_id(request, "quorum request ID")?;
    validate_hex_id(package, "quorum package ID")
}

#[allow(clippy::too_many_arguments)]
fn validate_quorum_report(
    account: &str,
    request: &str,
    package: &str,
    roster_digest: &str,
    authority_revision: u64,
    roster_count: usize,
    observed_approvals: usize,
    required_approvals: usize,
    majority_satisfied: bool,
    freshness_claim: &str,
    cross_roster_fork_safety: bool,
) -> Result<()> {
    AccountId::from_str(account).context("quorum report Account ID is invalid")?;
    validate_hex_id(request, "quorum report request ID")?;
    validate_hex_id(package, "quorum report package ID")?;
    validate_hex_id(roster_digest, "quorum report roster digest")?;
    ensure!(
        authority_revision > 0,
        "quorum report authority revision is missing"
    );
    ensure!(
        roster_count > 0
            && required_approvals == roster_count / 2 + 1
            && observed_approvals <= roster_count
            && majority_satisfied == (observed_approvals >= required_approvals),
        "quorum report threshold fields disagree"
    );
    ensure!(
        freshness_claim
            == if majority_satisfied {
                "current-device-majority-observed"
            } else if observed_approvals == 1 {
                "single-current-device-observed"
            } else {
                "artifact-integrity-only"
            },
        "quorum freshness claim disagrees with observed approvals"
    );
    ensure!(
        !cross_roster_fork_safety,
        "M0 quorum must not claim cross-roster fork safety"
    );
    Ok(())
}

fn validate_transport_fields(
    route_policy: &str,
    transport_path: &str,
    remote_address: &str,
    open_paths: usize,
) -> Result<()> {
    ensure!(
        matches!(route_policy, "auto" | "direct-only" | "relay-only"),
        "unknown recovery approval route policy"
    );
    ensure!(
        matches!(transport_path, "direct" | "relay" | "custom"),
        "unknown recovery approval transport path"
    );
    ensure!(
        !(route_policy == "direct-only" && transport_path != "direct")
            && !(route_policy == "relay-only" && transport_path != "relay"),
        "recovery approval transport violated its route policy"
    );
    ensure!(
        !remote_address.is_empty(),
        "transport remote address is empty"
    );
    ensure!(open_paths > 0, "transport has no open path");
    Ok(())
}

fn validate_status(actual: &str, expected: &str) -> Result<()> {
    ensure!(
        actual == expected,
        "wizard helper returned unexpected status"
    );
    Ok(())
}

fn validate_identity_fields(account: &str, device: &str, request: &str) -> Result<()> {
    AccountId::from_str(account).context("wizard Account ID is invalid")?;
    DeviceId::from_str(device).context("wizard Device ID is invalid")?;
    validate_hex_id(request, "request ID")?;
    Ok(())
}

fn validate_sas(sas: &str) -> Result<()> {
    ensure!(
        sas.len() == 12 && sas.bytes().all(|byte| byte.is_ascii_digit()),
        "wizard SAS must contain exactly 12 digits"
    );
    Ok(())
}

fn validate_hex_id(value: &str, label: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "wizard {label} is invalid"
    );
    Ok(())
}

fn validate_workspace_paths<'a>(
    workspace: &Path,
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<()> {
    ensure!(workspace.is_absolute(), "wizard workspace must be absolute");
    for path in paths {
        ensure!(
            path.is_absolute() && path.starts_with(workspace),
            "wizard output path escaped its workspace"
        );
    }
    Ok(())
}

fn validate_absolute_file_path(path: &Path, label: &str) -> Result<()> {
    ensure!(path.is_absolute(), "wizard {label} must be absolute");
    ensure!(
        path.file_name().is_some(),
        "wizard {label} has no file name"
    );
    Ok(())
}

pub(crate) fn run_json<T>(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<T>
where
    T: WizardJsonOutput,
{
    let output = run_process(executable, subcommand, arguments)?;
    ensure_process_success(subcommand, &output)?;
    let value: T = serde_json::from_slice(&output.stdout).context("decode wizard helper JSON")?;
    value.validate()?;
    Ok(value)
}

pub(crate) fn run_json_with_stdin<T>(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
    input: &[u8],
) -> Result<T>
where
    T: WizardJsonOutput,
{
    ensure!(
        input.len() <= MAX_WIZARD_STDIN_BYTES,
        "wizard command input is too large"
    );
    let output = run_process_with_stdin(executable, subcommand, arguments, Some(input))?;
    ensure_process_success(subcommand, &output)?;
    let value: T = serde_json::from_slice(&output.stdout).context("decode wizard helper JSON")?;
    value.validate()?;
    Ok(value)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RecoveryCommandOutput {
    pub(crate) fields: BTreeMap<String, String>,
}

impl RecoveryCommandOutput {
    pub(crate) fn status(&self) -> &str {
        self.fields
            .get("status")
            .map(String::as_str)
            .unwrap_or("unknown")
    }

    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }
}

pub(crate) fn run_recovery_command(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<RecoveryCommandOutput> {
    let output = run_process(executable, subcommand, arguments)?;
    let mut parsed = match parse_key_value_output(&output.stdout) {
        Ok(parsed) => parsed,
        Err(error) if !output.success => {
            return Err(error.context(process_failure_message(subcommand, &output)));
        }
        Err(error) => return Err(error),
    };
    parsed.fields.insert(
        "process_exit_success".to_owned(),
        output.success.to_string(),
    );
    if !output.success {
        parsed.fields.insert(
            "process_error".to_owned(),
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        );
    }
    Ok(parsed)
}

struct ProcessOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: String,
}

fn run_process(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<ProcessOutput> {
    run_process_with_stdin(executable, subcommand, arguments, None)
}

fn run_process_with_stdin(
    executable: &Path,
    subcommand: &str,
    arguments: impl IntoIterator<Item = OsString>,
    input: Option<&[u8]>,
) -> Result<ProcessOutput> {
    let mut command = Command::new(executable);
    command
        .arg(subcommand)
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("start {} {subcommand}", executable.display()))?;
    if let Some(input) = input {
        let mut stdin = child.stdin.take().context("open wizard helper stdin")?;
        stdin
            .write_all(input)
            .context("write wizard helper standard input")?;
        stdin
            .write_all(b"\n")
            .context("finish wizard helper standard input")?;
        stdin
            .flush()
            .context("flush wizard helper standard input")?;
    }
    let output = child
        .wait_with_output()
        .context("wait for wizard helper process")?;
    ensure!(
        output.stdout.len() <= MAX_WIZARD_OUTPUT_BYTES,
        "wizard command output is too large"
    );
    ensure!(
        output.stderr.len() <= MAX_WIZARD_OUTPUT_BYTES,
        "wizard command error output is too large"
    );
    Ok(ProcessOutput {
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
        status: output.status.to_string(),
    })
}

fn ensure_process_success(subcommand: &str, output: &ProcessOutput) -> Result<()> {
    if output.success {
        Ok(())
    } else {
        bail!(process_failure_message(subcommand, output))
    }
}

fn process_failure_message(subcommand: &str, output: &ProcessOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    format!(
        "{subcommand} exited with {}: {}{}{}",
        output.status,
        stderr.trim(),
        if stderr.is_empty() || stdout.is_empty() {
            ""
        } else {
            "; output: "
        },
        stdout.trim()
    )
}

pub(crate) fn parse_key_value_output(bytes: &[u8]) -> Result<RecoveryCommandOutput> {
    ensure!(
        bytes.len() <= MAX_WIZARD_OUTPUT_BYTES,
        "recovery command output is too large"
    );
    let text = std::str::from_utf8(bytes).context("recovery command output is not UTF-8")?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        ensure!(
            line.len() <= 16 * 1024,
            "recovery command output line is too long"
        );
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            fields.insert(key.to_owned(), value.to_owned());
        }
    }
    ensure!(
        fields.contains_key("status"),
        "recovery command omitted terminal status"
    );
    Ok(RecoveryCommandOutput { fields })
}

pub(crate) fn command_arguments(
    pairs: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
) -> Vec<OsString> {
    let mut arguments = Vec::new();
    for (name, value) in pairs {
        arguments.push(name.as_ref().to_os_string());
        arguments.push(value.as_ref().to_os_string());
    }
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_output_keeps_last_structured_value_and_requires_status() -> Result<()> {
        let output = parse_key_value_output(
            b"history_recovery_scheduler_lifecycle=pending\nstatus=scheduled\nstatus=complete\n",
        )?;
        assert_eq!(output.status(), "complete");
        assert_eq!(
            output.field("history_recovery_scheduler_lifecycle"),
            Some("pending")
        );
        assert!(parse_key_value_output(b"not structured\n").is_err());
        Ok(())
    }

    #[test]
    fn device_link_json_rejects_relative_or_unexpected_outputs() {
        let json = br#"{
            "status":"device-link-request-created",
            "account_id":"0101010101010101010101010101010101010101010101010101010101010101",
            "device_id":"0202020202020202020202020202020202020202020202020202020202020202",
            "request_id":"0303030303030303030303030303030303030303030303030303030303030303",
            "sas":"123456789012",
            "expires_at_unix_seconds":1,
            "workspace_dir":"relative",
            "state_dir":"relative/device",
            "request_file":"relative/request",
            "prekey_pool_file":"relative/prekeys",
            "vault_key_protection":"test"
        }"#;
        let output: DeviceLinkRequestOutput = serde_json::from_slice(json)
            .unwrap_or_else(|error| unreachable!("fixture must decode: {error}"));
        assert!(output.validate().is_err());
    }

    #[test]
    fn device_link_json_accepts_bounded_absolute_workspace_and_denies_extensions() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let workspace = temporary.path().join("joining");
        let mut value = serde_json::json!({
            "status": "device-link-request-created",
            "account_id": "0101010101010101010101010101010101010101010101010101010101010101",
            "device_id": "0202020202020202020202020202020202020202020202020202020202020202",
            "request_id": "0303030303030303030303030303030303030303030303030303030303030303",
            "sas": "123456789012",
            "expires_at_unix_seconds": 1,
            "workspace_dir": workspace,
            "state_dir": workspace.join("device"),
            "request_file": workspace.join("device-link").join("request.bin"),
            "prekey_pool_file": workspace.join("public").join("prekeys.bin"),
            "vault_key_protection": "test"
        });
        let output: DeviceLinkRequestOutput = serde_json::from_value(value.clone())?;
        output.validate()?;

        value["unexpected"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<DeviceLinkRequestOutput>(value).is_err());
        Ok(())
    }

    #[test]
    fn account_recovery_json_is_strict_absolute_and_honest_about_freshness() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let package = temporary.path().join("root.karp");
        let witness = temporary.path().join("latest.karw");
        let mut value = serde_json::json!({
            "status": "account-root-recovery-verified",
            "account_id": "0101010101010101010101010101010101010101010101010101010101010101",
            "authority_revision": 7,
            "device_count": 2,
            "conversation_membership_count": 3,
            "package_id": "0202020202020202020202020202020202020202020202020202020202020202",
            "package_file": package,
            "witness_file": witness,
            "account_root_dir": null,
            "root_key_protection": null,
            "freshness_scope": "exact-independent-witness-not-global-monotonic-service"
        });
        let output: AccountRecoveryOutput = serde_json::from_value(value.clone())?;
        output.validate_expected_status("account-root-recovery-verified")?;

        value["freshness_scope"] = serde_json::Value::String("globally-fresh".to_owned());
        let dishonest: AccountRecoveryOutput = serde_json::from_value(value.clone())?;
        assert!(dishonest.validate().is_err());
        value["freshness_scope"] = serde_json::Value::String(
            "exact-independent-witness-not-global-monotonic-service".to_owned(),
        );
        value["unexpected"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<AccountRecoveryOutput>(value).is_err());

        let root = temporary.path().join("account-root");
        let status: AccountRecoveryStatusOutput = serde_json::from_value(serde_json::json!({
            "status": "account-root-recovery-update-required",
            "checkpoint_state": "update-required",
            "account_id": "0101010101010101010101010101010101010101010101010101010101010101",
            "authority_revision": 7,
            "device_count": 2,
            "conversation_membership_count": 3,
            "current_package_id": "0303030303030303030303030303030303030303030303030303030303030303",
            "recorded_package_id": "0202020202020202020202020202020202020202020202020202020202020202",
            "recorded_authority_revision": 7,
            "account_root_dir": root,
            "lifecycle_scope": "local-exact-export-receipt-not-global-freshness-proof"
        }))?;
        status.validate()?;
        Ok(())
    }

    #[test]
    fn recovery_quorum_json_requires_exact_claim_and_route_policy() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut value = serde_json::json!({
            "status": "account-recovery-quorum-majority-collected",
            "account_id": "0101010101010101010101010101010101010101010101010101010101010101",
            "request_id": "0202020202020202020202020202020202020202020202020202020202020202",
            "package_id": "0303030303030303030303030303030303030303030303030303030303030303",
            "authority_revision": 7,
            "recovery_roster_digest": "0404040404040404040404040404040404040404040404040404040404040404",
            "roster_count": 1,
            "observed_approvals": 1,
            "required_approvals": 1,
            "majority_satisfied": true,
            "freshness_claim": "current-device-majority-observed",
            "cross_roster_fork_safety": false,
            "approval_directory": temporary.path().join("approvals"),
            "transports": [{
                "approver_device_id": "0505050505050505050505050505050505050505050505050505050505050505",
                "ticket_file": temporary.path().join("device.kart"),
                "approval_file": temporary.path().join("device.kara"),
                "route_policy": "relay-only",
                "transport_path": "relay",
                "transport_remote_address": "relay:https://example.invalid/",
                "transport_rtt_milliseconds": 42,
                "transport_open_paths": 1
            }]
        });
        let output: RecoveryQuorumCollectOutput = serde_json::from_value(value.clone())?;
        output.validate()?;

        value["cross_roster_fork_safety"] = serde_json::Value::Bool(true);
        let dishonest: RecoveryQuorumCollectOutput = serde_json::from_value(value.clone())?;
        assert!(dishonest.validate().is_err());
        value["cross_roster_fork_safety"] = serde_json::Value::Bool(false);
        value["transports"][0]["transport_path"] = serde_json::Value::String("direct".to_owned());
        let route_violation: RecoveryQuorumCollectOutput = serde_json::from_value(value)?;
        assert!(route_violation.validate().is_err());
        Ok(())
    }

    #[test]
    fn recovery_policy_json_separates_collection_from_certified_fork_safety() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let mut value = serde_json::json!({
            "status": "account-recovery-policy-transition-joint-majority-collected",
            "account_id": "0101010101010101010101010101010101010101010101010101010101010101",
            "request_id": "0202020202020202020202020202020202020202020202020202020202020202",
            "old_epoch": 0,
            "new_epoch": 1,
            "old_observed_approvals": 1,
            "old_required_approvals": 1,
            "new_observed_approvals": 2,
            "new_required_approvals": 2,
            "joint_majority_satisfied": true,
            "cross_roster_fork_safety": false,
            "approval_directory": temporary.path().join("approvals"),
            "transports": [{
                "approver_device_id": "0505050505050505050505050505050505050505050505050505050505050505",
                "ticket_file": temporary.path().join("device.karpticket"),
                "approval_file": temporary.path().join("device.karpa"),
                "route_policy": "relay-only",
                "transport_path": "relay",
                "transport_remote_address": "relay:https://example.invalid/",
                "transport_rtt_milliseconds": 42,
                "transport_open_paths": 1
            }]
        });
        let output: RecoveryPolicyTransitionCollectOutput = serde_json::from_value(value.clone())?;
        output.validate()?;

        value["joint_majority_satisfied"] = serde_json::Value::Bool(false);
        let dishonest: RecoveryPolicyTransitionCollectOutput = serde_json::from_value(value)?;
        assert!(dishonest.validate().is_err());
        Ok(())
    }
}
