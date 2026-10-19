use anyhow::{Result, bail, ensure};
use eframe::egui;

use crate::wizard::RecoveryCommandOutput;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RolloutOperatorStatus {
    pub(crate) current_target: String,
    pub(crate) current_disposition: String,
    pub(crate) current_evidence_bound: bool,
    pub(crate) activation_epoch: u64,
    pub(crate) trusted_store_present: bool,
    pub(crate) trusted_policy_present: bool,
    pub(crate) bundle_id: Option<String>,
    pub(crate) keyring_id: Option<String>,
    pub(crate) signer_key_id: Option<String>,
    pub(crate) keyring_epoch: Option<u64>,
    pub(crate) policy_epoch: Option<u64>,
    pub(crate) phase: Option<String>,
    pub(crate) policy_disposition: Option<String>,
    pub(crate) health_store_present: Option<bool>,
    pub(crate) health_present: Option<bool>,
    pub(crate) zero_tolerance_events: Option<u64>,
    pub(crate) decision: String,
    pub(crate) hold_reason: Option<String>,
    pub(crate) proposed_target: Option<String>,
    pub(crate) proposed_disposition: Option<String>,
}

impl RolloutOperatorStatus {
    pub(crate) fn parse(output: &RecoveryCommandOutput) -> Result<Self> {
        ensure!(
            output.field("process_exit_success") == Some("true")
                && output.status() == "rollout-operator-observed-read-only",
            "rollout operator observer did not complete successfully"
        );
        ensure!(
            parse_bool(output, "rollout_operator_read_only")?,
            "rollout operator output is not read-only"
        );
        ensure!(
            !parse_bool(output, "rollout_operator_writes_state")?,
            "rollout operator observer claimed a state write"
        );
        ensure!(
            required(output, "rollout_operator_device_state")? == "existing",
            "rollout operator observer did not use an existing Device state"
        );

        let trusted_store_present = parse_bool(output, "rollout_operator_trusted_store_present")?;
        let trusted_policy_present = parse_bool(output, "rollout_operator_trusted_policy_present")?;
        ensure!(
            trusted_store_present || !trusted_policy_present,
            "trusted policy cannot exist without a trusted store"
        );
        let decision = required(output, "rollout_operator_decision")?.to_owned();
        ensure!(
            matches!(decision.as_str(), "hold" | "propose-device-activation"),
            "rollout operator returned an unknown decision"
        );
        let hold_reason = optional(output, "rollout_operator_hold_reason");
        let proposed_target = optional(output, "rollout_operator_target");
        let proposed_disposition = optional(output, "rollout_operator_disposition");
        if decision == "hold" {
            ensure!(
                hold_reason.is_some()
                    && proposed_target.is_none()
                    && proposed_disposition.is_none(),
                "rollout hold summary is incomplete or ambiguous"
            );
        } else {
            ensure!(
                hold_reason.is_none()
                    && proposed_target.is_some()
                    && proposed_disposition.is_some(),
                "rollout proposal summary is incomplete or ambiguous"
            );
            ensure!(
                parse_bool(output, "rollout_operator_explicit_device_approval_required")?
                    && !parse_bool(
                        output,
                        "rollout_operator_release_policy_can_sign_activation"
                    )?,
                "rollout proposal bypasses explicit Device approval"
            );
        }

        let status = Self {
            current_target: required(output, "rollout_operator_current_target")?.to_owned(),
            current_disposition: required(output, "rollout_operator_current_disposition")?
                .to_owned(),
            current_evidence_bound: parse_bool(output, "rollout_operator_current_evidence_bound")?,
            activation_epoch: parse_u64(output, "rollout_operator_activation_epoch")?,
            trusted_store_present,
            trusted_policy_present,
            bundle_id: optional_id(output, "rollout_operator_bundle_id")?,
            keyring_id: optional_id(output, "rollout_operator_keyring_id")?,
            signer_key_id: optional_id(output, "rollout_operator_signer_key_id")?,
            keyring_epoch: optional_u64(output, "rollout_operator_keyring_epoch")?,
            policy_epoch: optional_u64(output, "rollout_operator_policy_epoch")?,
            phase: optional(output, "rollout_operator_phase"),
            policy_disposition: optional(output, "rollout_operator_policy_disposition"),
            health_store_present: optional_bool(output, "rollout_operator_health_store_present")?,
            health_present: optional_bool(output, "rollout_operator_health_present")?,
            zero_tolerance_events: optional_u64(output, "rollout_operator_zero_tolerance_events")?,
            decision,
            hold_reason,
            proposed_target,
            proposed_disposition,
        };
        status.validate_shape()?;
        Ok(status)
    }

    fn validate_shape(&self) -> Result<()> {
        let policy_fields = [
            self.bundle_id.is_some(),
            self.keyring_id.is_some(),
            self.signer_key_id.is_some(),
            self.keyring_epoch.is_some(),
            self.policy_epoch.is_some(),
            self.phase.is_some(),
            self.policy_disposition.is_some(),
        ];
        ensure!(
            policy_fields
                .iter()
                .all(|present| *present == self.trusted_policy_present),
            "trusted policy summary is partial"
        );
        if !self.trusted_policy_present {
            ensure!(
                self.health_store_present.is_none()
                    && self.health_present.is_none()
                    && self.zero_tolerance_events.is_none()
                    && self.decision == "hold"
                    && self.hold_reason.as_deref() == Some("missing-trusted-policy"),
                "missing-policy rollout summary is inconsistent"
            );
        } else {
            ensure!(
                self.health_store_present.is_some() && self.health_present.is_some(),
                "trusted policy summary omitted health-store state"
            );
            if self.health_present == Some(false) {
                ensure!(
                    self.zero_tolerance_events.is_none(),
                    "missing health snapshot reported counters"
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RolloutOperatorView {
    pub(crate) status: Option<RolloutOperatorStatus>,
}

impl RolloutOperatorView {
    pub(crate) fn draw(
        &self,
        ui: &mut egui::Ui,
        configured: bool,
        busy: bool,
        compact_id: impl Fn(&str) -> String,
    ) -> bool {
        ui.small(
            "Read-only release trust, signed policy, controller and recovery state. This panel cannot enroll this Device, sign approval, clear containment, or change activation authority.",
        );
        let refresh = ui
            .add_enabled(
                configured && !busy,
                egui::Button::new("Refresh rollout trust state"),
            )
            .clicked();
        if !configured {
            ui.colored_label(
                egui::Color32::from_rgb(246, 195, 93),
                "Rollout observer is not configured; public behavior remains legacy-only.",
            );
            ui.small(
                "Provide all three startup arguments: --rollout-dir, --trusted-policy-dir and --release-trust-root. The trust root is public verification material, not a signing key.",
            );
            return refresh;
        }
        let Some(status) = self.status.as_ref() else {
            ui.small("No rollout snapshot has been loaded. Refresh to inspect the existing stores without creating them.");
            return refresh;
        };
        ui.label(format!(
            "Activation: {} · {} · epoch {} · canonical evidence {}",
            status.current_target,
            status.current_disposition,
            status.activation_epoch,
            if status.current_evidence_bound {
                "bound"
            } else {
                "absent"
            }
        ));
        if !status.trusted_policy_present {
            ui.colored_label(
                egui::Color32::from_rgb(246, 195, 93),
                "No trusted release policy is installed; controller holds fail-closed.",
            );
        } else {
            ui.label(format!(
                "Policy: {} · {} · epoch {} · keyring epoch {}",
                status.phase.as_deref().unwrap_or("unknown"),
                status.policy_disposition.as_deref().unwrap_or("unknown"),
                status.policy_epoch.unwrap_or_default(),
                status.keyring_epoch.unwrap_or_default()
            ));
            if let Some(bundle_id) = status.bundle_id.as_deref() {
                ui.small(format!("Bundle: {}", compact_id(bundle_id)));
            }
            if let Some(keyring_id) = status.keyring_id.as_deref() {
                ui.small(format!("Keyring: {}", compact_id(keyring_id)));
            }
            if let Some(signer_key_id) = status.signer_key_id.as_deref() {
                ui.small(format!("Operational signer: {}", compact_id(signer_key_id)));
            }
            ui.small(format!(
                "Health store={} snapshot={} zero-tolerance events={}",
                status.health_store_present.unwrap_or(false),
                status.health_present.unwrap_or(false),
                status
                    .zero_tolerance_events
                    .map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
            ));
        }
        if status.decision == "hold" {
            ui.colored_label(
                egui::Color32::from_rgb(246, 195, 93),
                format!(
                    "Controller: HOLD ({})",
                    status.hold_reason.as_deref().unwrap_or("unspecified")
                ),
            );
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(92, 201, 137),
                format!(
                    "Controller proposes {} / {}; a separate exact Device-signed CLI ceremony is required.",
                    status.proposed_target.as_deref().unwrap_or("unknown"),
                    status.proposed_disposition.as_deref().unwrap_or("unknown")
                ),
            );
        }
        refresh
    }
}

fn required<'a>(output: &'a RecoveryCommandOutput, name: &str) -> Result<&'a str> {
    output
        .field(name)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("rollout operator output omitted {name}"))
}

fn optional(output: &RecoveryCommandOutput, name: &str) -> Option<String> {
    output
        .field(name)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn parse_bool(output: &RecoveryCommandOutput, name: &str) -> Result<bool> {
    match required(output, name)? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => bail!("rollout operator field {name} is not boolean: {value}"),
    }
}

fn optional_bool(output: &RecoveryCommandOutput, name: &str) -> Result<Option<bool>> {
    output
        .field(name)
        .map(|_| parse_bool(output, name))
        .transpose()
}

fn parse_u64(output: &RecoveryCommandOutput, name: &str) -> Result<u64> {
    required(output, name)?
        .parse()
        .map_err(|error| anyhow::anyhow!("parse rollout operator field {name}: {error}"))
}

fn optional_u64(output: &RecoveryCommandOutput, name: &str) -> Result<Option<u64>> {
    output
        .field(name)
        .map(|_| parse_u64(output, name))
        .transpose()
}

fn optional_id(output: &RecoveryCommandOutput, name: &str) -> Result<Option<String>> {
    let value = optional(output, name);
    if let Some(value) = value.as_deref() {
        ensure!(
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
            "rollout operator field {name} is not a canonical identifier"
        );
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn output(fields: &[(&str, &str)]) -> RecoveryCommandOutput {
        let mut output = RecoveryCommandOutput {
            fields: BTreeMap::new(),
        };
        for (name, value) in fields {
            output
                .fields
                .insert((*name).to_owned(), (*value).to_owned());
        }
        output
    }

    fn base_missing_policy() -> RecoveryCommandOutput {
        output(&[
            ("process_exit_success", "true"),
            ("status", "rollout-operator-observed-read-only"),
            ("rollout_operator_read_only", "true"),
            ("rollout_operator_writes_state", "false"),
            ("rollout_operator_device_state", "existing"),
            ("rollout_operator_current_target", "legacy-only"),
            ("rollout_operator_current_disposition", "normal"),
            ("rollout_operator_current_evidence_bound", "false"),
            ("rollout_operator_activation_epoch", "0"),
            ("rollout_operator_trusted_store_present", "false"),
            ("rollout_operator_trusted_policy_present", "false"),
            ("rollout_operator_decision", "hold"),
            ("rollout_operator_hold_reason", "missing-trusted-policy"),
        ])
    }

    #[test]
    fn missing_policy_status_is_bounded_and_read_only() -> Result<()> {
        let parsed = RolloutOperatorStatus::parse(&base_missing_policy())?;
        assert_eq!(parsed.current_target, "legacy-only");
        assert!(!parsed.trusted_policy_present);
        assert_eq!(
            parsed.hold_reason.as_deref(),
            Some("missing-trusted-policy")
        );
        Ok(())
    }

    #[test]
    fn parser_rejects_write_claim_partial_policy_and_implicit_approval() {
        let mut write = base_missing_policy();
        write.fields.insert(
            "rollout_operator_writes_state".to_owned(),
            "true".to_owned(),
        );
        assert!(RolloutOperatorStatus::parse(&write).is_err());

        let mut partial = base_missing_policy();
        partial.fields.insert(
            "rollout_operator_trusted_policy_present".to_owned(),
            "true".to_owned(),
        );
        assert!(RolloutOperatorStatus::parse(&partial).is_err());

        let mut proposal = base_missing_policy();
        proposal.fields.insert(
            "rollout_operator_decision".to_owned(),
            "propose-device-activation".to_owned(),
        );
        proposal.fields.remove("rollout_operator_hold_reason");
        proposal.fields.insert(
            "rollout_operator_target".to_owned(),
            "advertise-cbor-v2".to_owned(),
        );
        proposal.fields.insert(
            "rollout_operator_disposition".to_owned(),
            "normal".to_owned(),
        );
        proposal.fields.insert(
            "rollout_operator_explicit_device_approval_required".to_owned(),
            "false".to_owned(),
        );
        proposal.fields.insert(
            "rollout_operator_release_policy_can_sign_activation".to_owned(),
            "false".to_owned(),
        );
        assert!(RolloutOperatorStatus::parse(&proposal).is_err());
    }
}
