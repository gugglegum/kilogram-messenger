use anyhow::{Context, Result, ensure};
use eframe::egui;

use crate::wizard::RecoveryCommandOutput;

pub(crate) const MAX_ACTIVATION_REASON_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActivationUiAction {
    None,
    Refresh,
    Plan,
    Apply,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MigrationState {
    LegacyOnly,
    AdvertiseCanonicalV2,
    DualReaderPreferLegacy,
    DualReaderPreferCanonical,
    CanonicalRequired,
}

impl MigrationState {
    pub(crate) const ALL: [Self; 5] = [
        Self::LegacyOnly,
        Self::AdvertiseCanonicalV2,
        Self::DualReaderPreferLegacy,
        Self::DualReaderPreferCanonical,
        Self::CanonicalRequired,
    ];

    pub(crate) const fn cli_value(self) -> &'static str {
        match self {
            Self::LegacyOnly => "legacy-only",
            Self::AdvertiseCanonicalV2 => "advertise-cbor-v2",
            Self::DualReaderPreferLegacy => "dual-reader-prefer-legacy",
            Self::DualReaderPreferCanonical => "dual-reader-prefer-canonical",
            Self::CanonicalRequired => "canonical-required",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::LegacyOnly => "Legacy only (public default)",
            Self::AdvertiseCanonicalV2 => "Advertise canonical v2",
            Self::DualReaderPreferLegacy => "Dual reader, prefer legacy",
            Self::DualReaderPreferCanonical => "Dual reader, prefer canonical",
            Self::CanonicalRequired => "Canonical v2 required",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "legacy-only" => Ok(Self::LegacyOnly),
            "advertise-cbor-v2" => Ok(Self::AdvertiseCanonicalV2),
            "dual-reader-prefer-legacy" => Ok(Self::DualReaderPreferLegacy),
            "dual-reader-prefer-canonical" => Ok(Self::DualReaderPreferCanonical),
            "canonical-required" => Ok(Self::CanonicalRequired),
            _ => anyhow::bail!("unknown canonical sync v2 migration state: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Disposition {
    Normal,
    EmergencyContainment,
}

impl Disposition {
    pub(crate) const fn cli_value(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::EmergencyContainment => "emergency-containment",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "normal" => Ok(Self::Normal),
            "emergency-containment" => Ok(Self::EmergencyContainment),
            _ => anyhow::bail!("unknown canonical sync v2 activation disposition: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActivationPermissions {
    pub(crate) legacy_reader: bool,
    pub(crate) publish_capability: bool,
    pub(crate) register_alpn: bool,
    pub(crate) accept_new: bool,
    pub(crate) initiate_new: bool,
    pub(crate) resume_selected: bool,
}

impl ActivationPermissions {
    fn parse(output: &RecoveryCommandOutput) -> Result<Self> {
        Ok(Self {
            legacy_reader: parse_bool(output, "activation_legacy_reader_enabled")?,
            publish_capability: parse_bool(output, "activation_publish_canonical_capability")?,
            register_alpn: parse_bool(output, "activation_register_canonical_alpn")?,
            accept_new: parse_bool(output, "activation_accept_new_canonical_session")?,
            initiate_new: parse_bool(output, "activation_initiate_new_canonical_session")?,
            resume_selected: parse_bool(output, "activation_resume_selected_canonical_session")?,
        })
    }

    const fn expected(migration_state: MigrationState, disposition: Disposition) -> Self {
        if matches!(disposition, Disposition::EmergencyContainment) {
            return Self {
                legacy_reader: !matches!(migration_state, MigrationState::CanonicalRequired),
                publish_capability: false,
                register_alpn: true,
                accept_new: false,
                initiate_new: false,
                resume_selected: true,
            };
        }
        match migration_state {
            MigrationState::LegacyOnly => Self {
                legacy_reader: true,
                publish_capability: false,
                register_alpn: false,
                accept_new: false,
                initiate_new: false,
                resume_selected: false,
            },
            MigrationState::AdvertiseCanonicalV2 => Self {
                legacy_reader: true,
                publish_capability: true,
                register_alpn: true,
                accept_new: false,
                initiate_new: false,
                resume_selected: false,
            },
            MigrationState::DualReaderPreferLegacy => Self {
                legacy_reader: true,
                publish_capability: true,
                register_alpn: true,
                accept_new: true,
                initiate_new: false,
                resume_selected: true,
            },
            MigrationState::DualReaderPreferCanonical => Self {
                legacy_reader: true,
                publish_capability: true,
                register_alpn: true,
                accept_new: true,
                initiate_new: true,
                resume_selected: true,
            },
            MigrationState::CanonicalRequired => Self {
                legacy_reader: false,
                publish_capability: true,
                register_alpn: true,
                accept_new: true,
                initiate_new: true,
                resume_selected: true,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActivationStatus {
    pub(crate) device_id: String,
    pub(crate) policy_source: String,
    pub(crate) migration_epoch: u64,
    pub(crate) record_id: Option<String>,
    pub(crate) previous_record_id: Option<String>,
    pub(crate) migration_state: MigrationState,
    pub(crate) disposition: Disposition,
    pub(crate) actor: Option<String>,
    pub(crate) canonical_evidence_id: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) high_water_verified: bool,
    pub(crate) permissions: ActivationPermissions,
}

impl ActivationStatus {
    pub(crate) fn parse(output: &RecoveryCommandOutput) -> Result<Self> {
        ensure_success(output, "canonical-sync-v2-activation-authority-verified")?;
        let policy_source = required(output, "activation_policy_source")?.to_owned();
        ensure!(
            matches!(
                policy_source.as_str(),
                "implicit-default" | "signed-device-chain"
            ),
            "canonical sync v2 policy source is not recognized"
        );
        let value = Self {
            device_id: parse_hex_id(required(output, "activation_device_id")?, false)?,
            policy_source,
            migration_epoch: required(output, "activation_migration_epoch")?
                .parse()
                .context("parse activation migration epoch")?,
            record_id: parse_optional_id(required(output, "activation_record_id")?)?,
            previous_record_id: parse_optional_id(required(
                output,
                "activation_previous_record_id",
            )?)?,
            migration_state: MigrationState::parse(required(
                output,
                "activation_migration_state",
            )?)?,
            disposition: Disposition::parse(required(output, "activation_disposition")?)?,
            actor: parse_optional_text(required(output, "activation_actor")?),
            canonical_evidence_id: parse_optional_id(required(
                output,
                "activation_canonical_evidence_id",
            )?)?,
            reason: parse_optional_text(required(output, "activation_reason")?),
            high_water_verified: match required(output, "activation_high_water")? {
                "verified" => true,
                "absent" => false,
                value => anyhow::bail!("unknown activation high-water state: {value}"),
            },
            permissions: ActivationPermissions::parse(output)?,
        };
        if value.record_id.is_none() {
            ensure!(
                value.policy_source == "implicit-default"
                    && value.migration_epoch == 0
                    && value.previous_record_id.is_none()
                    && value.migration_state == MigrationState::LegacyOnly
                    && value.disposition == Disposition::Normal
                    && value.actor.is_none()
                    && value.canonical_evidence_id.is_none()
                    && value.reason.is_none()
                    && !value.high_water_verified,
                "implicit canonical sync v2 default is internally inconsistent"
            );
        } else {
            ensure!(
                value.policy_source == "signed-device-chain"
                    && value.migration_epoch > 0
                    && value.actor.is_some()
                    && value.reason.is_some()
                    && value.high_water_verified,
                "signed canonical sync v2 policy is internally inconsistent"
            );
        }
        validate_policy(
            value.migration_state,
            value.disposition,
            value.canonical_evidence_id.as_deref(),
            value.permissions,
        )?;
        if let Some(reason) = value.reason.as_deref() {
            validate_reason(reason)?;
        }
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActivationPlan {
    pub(crate) device_id: String,
    pub(crate) current_record_id: Option<String>,
    pub(crate) record_id: String,
    pub(crate) previous_record_id: Option<String>,
    pub(crate) migration_epoch: u64,
    pub(crate) migration_state: MigrationState,
    pub(crate) disposition: Disposition,
    pub(crate) actor: String,
    pub(crate) canonical_evidence_id: Option<String>,
    pub(crate) reason: String,
    pub(crate) permissions: ActivationPermissions,
}

impl ActivationPlan {
    pub(crate) fn parse(output: &RecoveryCommandOutput) -> Result<Self> {
        ensure_success(output, "canonical-sync-v2-activation-planned")?;
        ensure!(
            required(output, "activation_plan_writes_state")? == "false",
            "canonical sync v2 plan unexpectedly wrote state"
        );
        let actor = required(output, "activation_plan_actor")?.to_owned();
        ensure!(
            actor == "explicit-local",
            "desktop activation plan is not bound to the explicit local actor"
        );
        let plan = Self {
            device_id: parse_hex_id(required(output, "activation_device_id")?, false)?,
            current_record_id: parse_optional_id(required(
                output,
                "activation_current_record_id",
            )?)?,
            record_id: parse_hex_id(required(output, "activation_plan_record_id")?, false)?,
            previous_record_id: parse_optional_id(required(
                output,
                "activation_plan_previous_record_id",
            )?)?,
            migration_epoch: required(output, "activation_plan_migration_epoch")?
                .parse()
                .context("parse planned activation migration epoch")?,
            migration_state: MigrationState::parse(required(
                output,
                "activation_plan_migration_state",
            )?)?,
            disposition: Disposition::parse(required(output, "activation_plan_disposition")?)?,
            actor,
            canonical_evidence_id: parse_optional_id(required(
                output,
                "activation_plan_canonical_evidence_id",
            )?)?,
            reason: required(output, "activation_plan_reason")?.to_owned(),
            permissions: ActivationPermissions::parse(output)?,
        };
        ensure!(
            plan.current_record_id == plan.previous_record_id,
            "planned activation predecessor does not match the current head"
        );
        ensure!(
            plan.migration_epoch > 0,
            "planned activation epoch must be positive"
        );
        validate_policy(
            plan.migration_state,
            plan.disposition,
            plan.canonical_evidence_id.as_deref(),
            plan.permissions,
        )?;
        validate_reason(&plan.reason)?;
        Ok(plan)
    }

    pub(crate) fn confirmation_phrase(&self) -> String {
        format!("APPLY {}", &self.record_id[..12])
    }
}

#[derive(Debug)]
pub(crate) struct ActivationView {
    pub(crate) status: Option<ActivationStatus>,
    pub(crate) plan: Option<ActivationPlan>,
    pub(crate) target_state: MigrationState,
    pub(crate) emergency_containment: bool,
    pub(crate) reason: String,
    pub(crate) confirmation: String,
}

impl Default for ActivationView {
    fn default() -> Self {
        Self {
            status: None,
            plan: None,
            target_state: MigrationState::LegacyOnly,
            emergency_containment: false,
            reason: String::new(),
            confirmation: String::new(),
        }
    }
}

impl ActivationView {
    pub(crate) fn disposition(&self) -> Disposition {
        if self.emergency_containment {
            Disposition::EmergencyContainment
        } else {
            Disposition::Normal
        }
    }

    pub(crate) fn install_status(&mut self, status: ActivationStatus) {
        self.target_state = status.migration_state;
        self.emergency_containment = status.disposition == Disposition::EmergencyContainment;
        self.status = Some(status);
        self.invalidate_plan();
    }

    pub(crate) fn install_plan(&mut self, plan: ActivationPlan) -> Result<()> {
        let status = self
            .status
            .as_ref()
            .context("refresh activation status before installing a plan")?;
        ensure!(
            plan.device_id == status.device_id
                && plan.current_record_id == status.record_id
                && plan.migration_epoch == status.migration_epoch + 1
                && plan.migration_state == self.target_state
                && plan.disposition == self.disposition()
                && plan.canonical_evidence_id == status.canonical_evidence_id
                && plan.reason == self.reason,
            "canonical sync v2 plan does not match the displayed draft and authority head"
        );
        self.plan = Some(plan);
        self.confirmation.clear();
        Ok(())
    }

    pub(crate) fn invalidate_plan(&mut self) {
        self.plan = None;
        self.confirmation.clear();
    }

    pub(crate) fn draft_valid(&self) -> bool {
        self.status.is_some() && validate_reason(&self.reason).is_ok()
    }

    pub(crate) fn apply_ready(&self) -> bool {
        self.plan.as_ref().is_some_and(|plan| {
            plan.migration_state == self.target_state
                && plan.disposition == self.disposition()
                && plan.reason == self.reason
                && self.confirmation == plan.confirmation_phrase()
        })
    }

    pub(crate) fn draw(
        &mut self,
        ui: &mut egui::Ui,
        runtime_offline: bool,
        busy: bool,
    ) -> ActivationUiAction {
        let mut action = ActivationUiAction::None;
        ui.small("Public default remains legacy-only. This advanced control changes only the selected Device's signed local policy and takes effect after a fresh runtime start.");
        if !runtime_offline {
            ui.colored_label(
                egui::Color32::from_rgb(246, 195, 93),
                "Stop and disconnect the runtime before inspecting or changing activation authority.",
            );
        }
        if ui
            .add_enabled(
                runtime_offline && !busy,
                egui::Button::new("Refresh verified policy"),
            )
            .clicked()
        {
            action = ActivationUiAction::Refresh;
        }

        if let Some(status) = &self.status {
            ui.label(format!(
                "Current: {} · {} · epoch {} · {}",
                status.migration_state.cli_value(),
                status.disposition.cli_value(),
                status.migration_epoch,
                status.policy_source
            ));
            ui.small(format!(
                "Record {} · evidence {} · high-water {}",
                compact_optional_id(status.record_id.as_deref()),
                compact_optional_id(status.canonical_evidence_id.as_deref()),
                if status.high_water_verified {
                    "verified"
                } else {
                    "absent"
                }
            ));
            draw_permissions(ui, status.permissions, "Current rights");
        } else {
            ui.small("Policy has not been inspected in this desktop session.");
        }

        ui.separator();
        let mut draft_changed = false;
        let old_state = self.target_state;
        egui::ComboBox::from_label("Exact next migration state")
            .selected_text(self.target_state.label())
            .show_ui(ui, |ui| {
                for state in MigrationState::ALL {
                    ui.selectable_value(&mut self.target_state, state, state.label());
                }
            });
        draft_changed |= old_state != self.target_state;

        let containment_allowed = self.status.as_ref().is_some_and(|status| {
            status.canonical_evidence_id.is_some()
                && !matches!(
                    self.target_state,
                    MigrationState::LegacyOnly | MigrationState::AdvertiseCanonicalV2
                )
        });
        if !containment_allowed && self.emergency_containment {
            self.emergency_containment = false;
            draft_changed = true;
        }
        let containment_response = ui.add_enabled(
            containment_allowed,
            egui::Checkbox::new(
                &mut self.emergency_containment,
                "Emergency containment (no new canonical sessions)",
            ),
        );
        draft_changed |= containment_response.changed();
        let reason_response = ui.add_enabled(
            runtime_offline && !busy,
            egui::TextEdit::singleline(&mut self.reason)
                .hint_text("Reason for this exact signed transition")
                .desired_width(f32::INFINITY),
        );
        draft_changed |= reason_response.changed();
        ui.small(format!(
            "Reason: {}/{} UTF-8 bytes; leading/trailing whitespace and control characters are rejected.",
            self.reason.len(),
            MAX_ACTIVATION_REASON_BYTES
        ));
        if draft_changed {
            self.invalidate_plan();
        }

        if ui
            .add_enabled(
                runtime_offline && !busy && self.draft_valid(),
                egui::Button::new("Plan exact transition (no write)"),
            )
            .clicked()
        {
            action = ActivationUiAction::Plan;
        }

        if let Some(plan) = &self.plan {
            ui.separator();
            ui.colored_label(
                egui::Color32::from_rgb(246, 195, 93),
                format!(
                    "Planned signed successor: epoch {} · {} · {}",
                    plan.migration_epoch,
                    plan.migration_state.cli_value(),
                    plan.disposition.cli_value()
                ),
            );
            ui.monospace(format!("Candidate record ID: {}", plan.record_id));
            draw_permissions(ui, plan.permissions, "Rights after restart");
            let phrase = plan.confirmation_phrase();
            ui.label(format!("Type exactly: {phrase}"));
            ui.add_enabled(
                runtime_offline && !busy,
                egui::TextEdit::singleline(&mut self.confirmation)
                    .hint_text(phrase)
                    .desired_width(240.0),
            );
            if ui
                .add_enabled(
                    runtime_offline && !busy && self.apply_ready(),
                    egui::Button::new("Apply this exact signed successor"),
                )
                .clicked()
            {
                action = ActivationUiAction::Apply;
            }
        }
        action
    }
}

fn draw_permissions(ui: &mut egui::Ui, permissions: ActivationPermissions, label: &str) {
    ui.small(format!(
        "{label}: legacy reader {} · publish {} · register ALPN {} · accept new {} · initiate new {} · resume selected {}",
        on_off(permissions.legacy_reader),
        on_off(permissions.publish_capability),
        on_off(permissions.register_alpn),
        on_off(permissions.accept_new),
        on_off(permissions.initiate_new),
        on_off(permissions.resume_selected)
    ));
}

const fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn compact_optional_id(value: Option<&str>) -> String {
    value.map_or_else(
        || "none".to_owned(),
        |value| format!("{}…{}", &value[..6], &value[value.len() - 6..]),
    )
}

fn required<'a>(output: &'a RecoveryCommandOutput, key: &str) -> Result<&'a str> {
    output
        .field(key)
        .with_context(|| format!("canonical sync v2 helper omitted {key}"))
}

fn ensure_success(output: &RecoveryCommandOutput, expected_status: &str) -> Result<()> {
    ensure!(
        output.field("process_exit_success") == Some("true"),
        "canonical sync v2 helper failed: {}",
        output.field("process_error").unwrap_or("unknown error")
    );
    ensure!(
        output.status() == expected_status,
        "canonical sync v2 helper returned status {}, expected {expected_status}",
        output.status()
    );
    Ok(())
}

fn parse_bool(output: &RecoveryCommandOutput, key: &str) -> Result<bool> {
    match required(output, key)? {
        "true" => Ok(true),
        "false" => Ok(false),
        value => anyhow::bail!("canonical sync v2 helper returned invalid {key}: {value}"),
    }
}

fn parse_optional_text(value: &str) -> Option<String> {
    (value != "none").then(|| value.to_owned())
}

fn parse_optional_id(value: &str) -> Result<Option<String>> {
    if value == "none" {
        Ok(None)
    } else {
        parse_hex_id(value, false).map(Some)
    }
}

fn parse_hex_id(value: &str, allow_uppercase: bool) -> Result<String> {
    ensure!(
        value.len() == 64
            && value.is_ascii()
            && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            && (allow_uppercase || value.bytes().all(|byte| !byte.is_ascii_uppercase())),
        "expected one canonical lowercase 64-hex identifier"
    );
    Ok(value.to_owned())
}

fn validate_reason(reason: &str) -> Result<()> {
    ensure!(
        (1..=MAX_ACTIVATION_REASON_BYTES).contains(&reason.len())
            && reason.trim() == reason
            && !reason.chars().any(char::is_control),
        "activation reason must be 1..={MAX_ACTIVATION_REASON_BYTES} UTF-8 bytes without surrounding whitespace or control characters"
    );
    Ok(())
}

fn validate_policy(
    migration_state: MigrationState,
    disposition: Disposition,
    canonical_evidence_id: Option<&str>,
    permissions: ActivationPermissions,
) -> Result<()> {
    if disposition == Disposition::EmergencyContainment {
        ensure!(
            canonical_evidence_id.is_some()
                && !matches!(
                    migration_state,
                    MigrationState::LegacyOnly | MigrationState::AdvertiseCanonicalV2
                ),
            "emergency containment requires persisted canonical evidence in a session-capable state"
        );
    }
    ensure!(
        permissions == ActivationPermissions::expected(migration_state, disposition),
        "canonical sync v2 helper returned rights inconsistent with the selected policy"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const DEVICE_ID: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const RECORD_ID: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn output(entries: &[(&str, &str)]) -> RecoveryCommandOutput {
        RecoveryCommandOutput {
            fields: entries
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    fn legacy_permission_entries() -> [(&'static str, &'static str); 6] {
        [
            ("activation_legacy_reader_enabled", "true"),
            ("activation_publish_canonical_capability", "false"),
            ("activation_register_canonical_alpn", "false"),
            ("activation_accept_new_canonical_session", "false"),
            ("activation_initiate_new_canonical_session", "false"),
            ("activation_resume_selected_canonical_session", "false"),
        ]
    }

    fn advertise_permission_entries() -> [(&'static str, &'static str); 6] {
        [
            ("activation_legacy_reader_enabled", "true"),
            ("activation_publish_canonical_capability", "true"),
            ("activation_register_canonical_alpn", "true"),
            ("activation_accept_new_canonical_session", "false"),
            ("activation_initiate_new_canonical_session", "false"),
            ("activation_resume_selected_canonical_session", "false"),
        ]
    }

    fn status_output() -> RecoveryCommandOutput {
        let mut entries = vec![
            ("process_exit_success", "true"),
            ("status", "canonical-sync-v2-activation-authority-verified"),
            ("activation_device_id", DEVICE_ID),
            ("activation_policy_source", "implicit-default"),
            ("activation_migration_epoch", "0"),
            ("activation_record_id", "none"),
            ("activation_previous_record_id", "none"),
            ("activation_migration_state", "legacy-only"),
            ("activation_disposition", "normal"),
            ("activation_actor", "none"),
            ("activation_canonical_evidence_id", "none"),
            ("activation_reason", "none"),
            ("activation_high_water", "absent"),
        ];
        entries.extend(legacy_permission_entries());
        output(&entries)
    }

    fn plan_output() -> RecoveryCommandOutput {
        let mut entries = vec![
            ("process_exit_success", "true"),
            ("status", "canonical-sync-v2-activation-planned"),
            ("activation_plan_writes_state", "false"),
            ("activation_device_id", DEVICE_ID),
            ("activation_current_record_id", "none"),
            ("activation_plan_record_id", RECORD_ID),
            ("activation_plan_previous_record_id", "none"),
            ("activation_plan_migration_epoch", "1"),
            ("activation_plan_migration_state", "advertise-cbor-v2"),
            ("activation_plan_disposition", "normal"),
            ("activation_plan_actor", "explicit-local"),
            ("activation_plan_canonical_evidence_id", "none"),
            ("activation_plan_reason", "desktop opt-in"),
        ];
        entries.extend(advertise_permission_entries());
        output(&entries)
    }

    #[test]
    fn exact_plan_confirmation_is_required_and_any_draft_change_invalidates_it() -> Result<()> {
        let status = ActivationStatus::parse(&status_output())?;
        let plan = ActivationPlan::parse(&plan_output())?;
        let mut view = ActivationView::default();
        view.install_status(status);
        view.target_state = MigrationState::AdvertiseCanonicalV2;
        view.reason = "desktop opt-in".to_owned();
        view.install_plan(plan)?;
        assert!(!view.apply_ready());
        view.confirmation = "APPLY 222222222222".to_owned();
        assert!(view.apply_ready());
        view.confirmation = "apply 222222222222".to_owned();
        assert!(!view.apply_ready());
        view.confirmation = "APPLY 222222222222".to_owned();
        view.reason.push('!');
        assert!(!view.apply_ready());
        view.invalidate_plan();
        assert!(view.plan.is_none());
        assert!(view.confirmation.is_empty());
        Ok(())
    }

    #[test]
    fn malformed_or_inconsistent_status_fails_closed() {
        let mut malformed = status_output();
        malformed.fields.insert(
            "activation_policy_source".to_owned(),
            "signed-device-chain".to_owned(),
        );
        assert!(ActivationStatus::parse(&malformed).is_err());

        let mut malformed = status_output();
        malformed.fields.insert(
            "activation_register_canonical_alpn".to_owned(),
            "yes".to_owned(),
        );
        assert!(ActivationStatus::parse(&malformed).is_err());

        let mut inconsistent_rights = status_output();
        inconsistent_rights.fields.insert(
            "activation_publish_canonical_capability".to_owned(),
            "true".to_owned(),
        );
        assert!(ActivationStatus::parse(&inconsistent_rights).is_err());
    }

    #[test]
    fn stale_or_inconsistent_plan_and_unbounded_reason_fail_closed() -> Result<()> {
        let status = ActivationStatus::parse(&status_output())?;
        let mut view = ActivationView::default();
        view.install_status(status);
        view.target_state = MigrationState::AdvertiseCanonicalV2;
        view.reason = "desktop opt-in".to_owned();

        let mut stale = plan_output();
        stale
            .fields
            .insert("activation_plan_migration_epoch".to_owned(), "2".to_owned());
        let stale = ActivationPlan::parse(&stale)?;
        assert!(view.install_plan(stale).is_err());

        let mut inconsistent_rights = plan_output();
        inconsistent_rights.fields.insert(
            "activation_accept_new_canonical_session".to_owned(),
            "true".to_owned(),
        );
        assert!(ActivationPlan::parse(&inconsistent_rights).is_err());

        view.reason = "x".repeat(MAX_ACTIVATION_REASON_BYTES + 1);
        assert!(!view.draft_valid());
        view.reason = "contains\ncontrol".to_owned();
        assert!(!view.draft_valid());
        Ok(())
    }
}
