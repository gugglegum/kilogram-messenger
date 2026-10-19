use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use eframe::egui;
use kilogram_bootstrap_contract::{DesktopBootstrapOutput, MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES};
use kilogram_identity::{AccountDeviceListSnapshot, AccountId, AccountRecoveryPhrase, DeviceId};
use kilogram_runtime_ipc::{
    MAX_RUNTIME_IPC_CONVERSATION_PAGE_SIZE, MAX_RUNTIME_IPC_PEER_CAPABILITY_PAGE_SIZE,
    RuntimeIpcCanonicalSyncV2RuntimeStatus, RuntimeIpcCommand, RuntimeIpcContactTicketRefresh,
    RuntimeIpcConversationPage, RuntimeIpcConversationSummary, RuntimeIpcDeviceDirectoryStatus,
    RuntimeIpcDeviceDirectoryUpdate, RuntimeIpcEndpointCandidateState, RuntimeIpcHistoryCursor,
    RuntimeIpcHistoryMessage, RuntimeIpcHistoryPage, RuntimeIpcMailboxCapabilityTransition,
    RuntimeIpcMailboxStatus, RuntimeIpcOutboxStatus, RuntimeIpcPeerCapabilityPage,
    RuntimeIpcPeerReenrollmentCommand, RuntimeIpcPeerReenrollmentContactId,
    RuntimeIpcPeerReenrollmentRequest, RuntimeIpcPeerReenrollmentResponse, RuntimeIpcQueueState,
    RuntimeIpcRequestId, RuntimeIpcResponse, RuntimeIpcRoutePolicy,
    RuntimeIpcTicketAutomationStatus, RuntimeIpcTicketPublication, RuntimeLaunchProfile,
    RuntimeLaunchSettings, RuntimeVolunteerStorageSettings,
    SignedRuntimeIpcPeerReenrollmentResponse,
};
#[cfg(test)]
use kilogram_runtime_ipc::{
    RuntimeIpcCanonicalSyncV2Disposition, RuntimeIpcCanonicalSyncV2MigrationState,
    RuntimeIpcCanonicalSyncV2Permissions, RuntimeIpcCanonicalSyncV2PolicySnapshot,
    RuntimeIpcCanonicalSyncV2PolicySource, RuntimeIpcEndpointCandidateStatus,
    RuntimeIpcEndpointTicketRefresh, RuntimeIpcNetworkClass,
    RuntimeIpcTicketAutomationActionStatus,
};
use zeroize::{Zeroize as _, Zeroizing};

mod activation;
mod peer_reenrollment;
mod rollout_operator;
mod wizard;

use activation::{
    ActivationPlan, ActivationStatus, ActivationUiAction, ActivationView, Disposition,
    GuidedRestartState, MigrationState,
};
use peer_reenrollment::{
    PeerReenrollmentRestartState, PeerReenrollmentUiAction, PeerReenrollmentView,
};
use rollout_operator::{RolloutOperatorStatus, RolloutOperatorView};

use wizard::{
    AccountRecoveryOutput, AccountRecoveryStatusOutput, DeviceLinkAcceptOutput,
    DeviceLinkAuthorizeOutput, DeviceLinkInspectOutput, DeviceLinkRequestOutput,
    DeviceRemovalOutput, RecoveryCommandOutput, RecoveryPolicyInstallOutput,
    RecoveryPolicyTransitionCertificateOutput, RecoveryPolicyTransitionCollectOutput,
    RecoveryPolicyTransitionListenOutput, RecoveryPolicyTransitionRequestOutput,
    RecoveryQuorumCollectOutput, RecoveryQuorumListenOutput, RecoveryQuorumRequestOutput,
    RecoveryQuorumVerifyOutput, command_arguments, run_json, run_json_with_stdin,
    run_recovery_command,
};

const CHANGE_WAIT_MILLISECONDS: u32 = 20_000;
const CHANGE_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const MAX_CONVERSATION_BYTES: usize = 4_096;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const HISTORY_PAGE_SIZE: u16 = 50;
const MAX_CONVERSATION_REFRESH_PAGES: usize = 1_024;
const RUNTIME_START_TIMEOUT: Duration = Duration::from_secs(60);
const RUNTIME_START_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const RUNTIME_STOP_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_DESCRIPTOR_PATH_BYTES: usize = 32 * 1024;
const DEFAULT_MAILBOX_VALIDITY_SECONDS: u64 = 7 * 24 * 60 * 60;
const RUNTIME_DEVICE_LIST_DIGEST_DOMAIN: &[u8] = b"kilogram:runtime-device-list-digest:v1\0";
const BYTES_PER_MIB: u64 = 1024 * 1024;

pub fn run() -> eframe::Result {
    let options = DesktopOptions::from_arguments(std::env::args_os().skip(1));
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kilogram")
            .with_inner_size([900.0, 720.0])
            .with_min_inner_size([680.0, 520.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Kilogram",
        native_options,
        Box::new(move |creation_context| Ok(Box::new(KilogramApp::new(creation_context, options)))),
    )
}

#[derive(Clone, Debug)]
struct DesktopOptions {
    descriptor_path: PathBuf,
    runtime_profile_path: PathBuf,
    runtime_executable_path: PathBuf,
    bootstrap_executable_path: PathBuf,
    rollout_directory: Option<PathBuf>,
    trusted_policy_directory: Option<PathBuf>,
    release_trust_root: Option<String>,
    startup_error: Option<String>,
}

impl DesktopOptions {
    fn from_arguments(arguments: impl IntoIterator<Item = OsString>) -> Self {
        let mut descriptor_path = None;
        let mut runtime_profile_path = None;
        let mut runtime_executable_path = None;
        let mut bootstrap_executable_path = None;
        let mut rollout_directory = None;
        let mut trusted_policy_directory = None;
        let mut release_trust_root = None;
        let mut startup_error = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            if argument == "--release-trust-root" {
                if let Some(value) = arguments.next() {
                    release_trust_root = Some(value.to_string_lossy().into_owned());
                } else {
                    startup_error =
                        Some("Desktop argument --release-trust-root requires a value".to_owned());
                    break;
                }
                continue;
            }
            let target = if argument == "--ipc-file" {
                &mut descriptor_path
            } else if argument == "--runtime-profile" {
                &mut runtime_profile_path
            } else if argument == "--runtime-exe" {
                &mut runtime_executable_path
            } else if argument == "--bootstrap-exe" {
                &mut bootstrap_executable_path
            } else if argument == "--rollout-dir" {
                &mut rollout_directory
            } else if argument == "--trusted-policy-dir" {
                &mut trusted_policy_directory
            } else {
                startup_error = Some(format!(
                    "Unknown desktop argument: {}",
                    argument.to_string_lossy()
                ));
                continue;
            };
            if let Some(value) = arguments.next() {
                *target = Some(PathBuf::from(value));
            } else {
                startup_error = Some(format!(
                    "Desktop argument {} requires a path",
                    argument.to_string_lossy()
                ));
                break;
            }
        }

        let rollout_configuration_count = usize::from(rollout_directory.is_some())
            + usize::from(trusted_policy_directory.is_some())
            + usize::from(release_trust_root.is_some());
        if rollout_configuration_count != 0 && rollout_configuration_count != 3 {
            startup_error = Some(
                "Rollout observer requires --rollout-dir, --trusted-policy-dir and --release-trust-root together"
                    .to_owned(),
            );
        }

        let runtime_profile_path =
            runtime_profile_path.unwrap_or_else(|| PathBuf::from("runtime.launch.json"));
        if runtime_profile_path.exists() {
            match RuntimeLaunchProfile::load(&runtime_profile_path) {
                Ok(profile) => {
                    let profile_ipc = profile.settings().ipc_file.clone();
                    if descriptor_path
                        .as_ref()
                        .is_some_and(|explicit| explicit != &profile_ipc)
                    {
                        startup_error =
                            Some("--ipc-file does not match the runtime launch profile".to_owned());
                    }
                    descriptor_path = Some(profile_ipc);
                }
                Err(error) => {
                    startup_error = Some(format!("Load runtime launch profile: {error:#}"));
                }
            }
        }

        Self {
            descriptor_path: descriptor_path.unwrap_or_else(|| PathBuf::from("runtime.ipc.json")),
            runtime_profile_path,
            runtime_executable_path: runtime_executable_path
                .unwrap_or_else(default_runtime_executable),
            bootstrap_executable_path: bootstrap_executable_path
                .unwrap_or_else(default_bootstrap_executable),
            rollout_directory,
            trusted_policy_directory,
            release_trust_root,
            startup_error,
        }
    }
}

#[derive(Clone, Debug)]
struct RolloutOperatorConfig {
    rollout_directory: PathBuf,
    trusted_policy_directory: PathBuf,
    release_trust_root: String,
}

fn default_bootstrap_executable() -> PathBuf {
    let executable_name = if cfg!(windows) {
        "kilogram-bootstrap.exe"
    } else {
        "kilogram-bootstrap"
    };
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(executable_name)))
        .unwrap_or_else(|| PathBuf::from(executable_name))
}

fn default_runtime_executable() -> PathBuf {
    let executable_name = if cfg!(windows) {
        "kilogram-cli.exe"
    } else {
        "kilogram-cli"
    };
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(executable_name)))
        .unwrap_or_else(|| PathBuf::from(executable_name))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

impl ConnectionState {
    fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "Runtime disconnected",
            Self::Connecting => "Connecting to runtime…",
            Self::Connected => "Runtime connected",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Bootstrap,
    AccountRecoveryStatus,
    AccountRecoveryExport,
    AccountRecoveryInspect,
    AccountRecoveryRestore,
    AccountRecoveryQuorumRequest,
    AccountRecoveryQuorumListen,
    AccountRecoveryQuorumCollect,
    AccountRecoveryQuorumVerify,
    RecoveryPolicyRequest,
    RecoveryPolicyListen,
    RecoveryPolicyCollect,
    RecoveryPolicyCertify,
    RecoveryPolicyInstall,
    DeviceLinkRequest,
    DeviceLinkInspect,
    DeviceLinkAuthorize,
    DeviceLinkAccept,
    DeviceRemove,
    ApplyDeviceDirectory,
    PublishTicket,
    RefreshContactTicket,
    ConfigureTicketAutomation,
    TicketAutomationStatus,
    MailboxCreate,
    MailboxRotate,
    MailboxRevoke,
    MailboxStatus,
    RecoveryApprove,
    RecoveryRun,
    RecoveryCancel,
    RecoveryReconcile,
    PublicationChannelRotate,
    PublicationConflictRequest,
    PublicationConflictApply,
    CanonicalActivationStatus,
    CanonicalActivationPlan,
    CanonicalActivationApply,
    RolloutOperatorStatus,
    CanonicalRuntimeStatus,
    PeerCapabilityStatus,
    PeerReenrollmentStatus,
    PeerReenrollmentPlan,
    PeerReenrollmentApprove,
    PeerReenrollmentAccept,
    Connect,
    AddContact,
    Queue,
    Refresh,
    Conversations,
    History,
    HistoryOlder,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeUiAction {
    None,
    Connect,
    Start,
    Stop,
    LoadProfile,
    SaveProfile,
    ApplyDeviceDirectory,
    ReconcileProfile,
    PublishTicket,
    RefreshContactTicket,
    EnableTicketAutomation,
    DisableTicketAutomation,
    RefreshTicketAutomationStatus,
    CreateMailbox,
    RotateMailbox,
    RevokeMailbox,
    RefreshMailboxStatus,
    RefreshCanonicalActivation,
    StopAndInspectCanonicalActivation,
    RetryCanonicalActivationFreshStart,
    PlanCanonicalActivation,
    ApplyCanonicalActivation,
    RefreshRolloutOperatorStatus,
    RefreshCanonicalRuntimeStatus,
    RefreshPeerCapabilityStatus,
    RefreshPeerReenrollmentStatus,
    PlanPeerReenrollment,
    ApprovePeerReenrollment,
    AcceptPeerReenrollment,
    RestartPeerReenrollment,
    RetryPeerReenrollmentFreshVerification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BootstrapUiAction {
    None,
    Create,
    DismissPhrase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeviceLinkUiAction {
    None,
    Request,
    Inspect,
    Authorize,
    Accept,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryPolicyUiAction {
    None,
    Request,
    Listen,
    Collect,
    Certify,
    Install,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccountRecoveryUiAction {
    None,
    Status,
    Export,
    Inspect,
    Restore,
    QuorumRequest,
    QuorumListen,
    QuorumCollect,
    QuorumVerify,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryUiAction {
    None,
    Approve,
    AddExisting,
    RunSelected,
    CancelSelected,
    Reconcile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationConflictUiAction {
    None,
    RotateOwnChannel,
    CreateRequest,
    ApplyResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DropTarget {
    AccountRecoveryPackage,
    AccountRecoveryWitness,
    DeviceLinkRequest,
    DeviceLinkResponse,
    RecoveryLink,
    RecoveryPlan,
}

struct AccountRecoveryView {
    account_root_dir: String,
    checkpoint_status: Option<AccountRecoveryStatusOutput>,
    package_output_file: String,
    witness_output_file: String,
    exported: Option<AccountRecoveryOutput>,
    package_input_file: String,
    witness_input_file: String,
    inspected_package_file: Option<PathBuf>,
    inspected_witness_file: Option<PathBuf>,
    inspected: Option<AccountRecoveryOutput>,
    recovery_phrase: Zeroizing<String>,
    confirm_latest_witness: bool,
    restore_root_dir: String,
    restored: Option<AccountRecoveryOutput>,
    quorum_request_file: String,
    quorum_request: Option<RecoveryQuorumRequestOutput>,
    quorum_state_dir: String,
    quorum_ticket_output_file: String,
    quorum_route_policy: RuntimeIpcRoutePolicy,
    quorum_listener: Option<RecoveryQuorumListenOutput>,
    quorum_ticket_files: String,
    quorum_approval_directory: String,
    quorum_collection: Option<RecoveryQuorumCollectOutput>,
    quorum_approval_files: String,
    quorum_verification: Option<RecoveryQuorumVerifyOutput>,
}

impl Default for AccountRecoveryView {
    fn default() -> Self {
        Self {
            account_root_dir: "kilogram-account/account-root".to_owned(),
            checkpoint_status: None,
            package_output_file: "kilogram-root-authority.karp".to_owned(),
            witness_output_file: "kilogram-root-latest.karw".to_owned(),
            exported: None,
            package_input_file: String::new(),
            witness_input_file: String::new(),
            inspected_package_file: None,
            inspected_witness_file: None,
            inspected: None,
            recovery_phrase: Zeroizing::new(String::new()),
            confirm_latest_witness: false,
            restore_root_dir: "kilogram-account-restored/account-root".to_owned(),
            restored: None,
            quorum_request_file: "kilogram-root-recovery.karq".to_owned(),
            quorum_request: None,
            quorum_state_dir: "kilogram-account/device".to_owned(),
            quorum_ticket_output_file: "kilogram-recovery-approval.kart".to_owned(),
            quorum_route_policy: RuntimeIpcRoutePolicy::Auto,
            quorum_listener: None,
            quorum_ticket_files: String::new(),
            quorum_approval_directory: "kilogram-recovery-approvals".to_owned(),
            quorum_collection: None,
            quorum_approval_files: String::new(),
            quorum_verification: None,
        }
    }
}

#[derive(Debug)]
struct DeviceLinkView {
    joining_workspace: String,
    account_id: String,
    request: Option<DeviceLinkRequestOutput>,
    owner_request_file: String,
    inspected_request_file: Option<PathBuf>,
    inspected: Option<DeviceLinkInspectOutput>,
    account_root_dir: String,
    confirmed_sas: String,
    response_output_file: String,
    device_list_output_file: String,
    authorization: Option<DeviceLinkAuthorizeOutput>,
    response_input_file: String,
    accepted: Option<DeviceLinkAcceptOutput>,
    removal: DeviceRemovalView,
    policy: RecoveryPolicyView,
}

#[derive(Debug)]
struct DeviceRemovalView {
    account_root_dir: String,
    device_id: String,
    confirmed_device_id: String,
    before_package_file: String,
    before_witness_file: String,
    after_package_file: String,
    after_witness_file: String,
    device_list_file: String,
    revocation_file: String,
    result: Option<DeviceRemovalOutput>,
}

impl Default for DeviceRemovalView {
    fn default() -> Self {
        Self {
            account_root_dir: "kilogram-account/account-root".to_owned(),
            device_id: String::new(),
            confirmed_device_id: String::new(),
            before_package_file: "kilogram-root-before-removal.karp".to_owned(),
            before_witness_file: "kilogram-root-before-removal.karw".to_owned(),
            after_package_file: "kilogram-root-after-removal.karp".to_owned(),
            after_witness_file: "kilogram-root-after-removal.karw".to_owned(),
            device_list_file: "kilogram-active-devices-after-removal.snapshot".to_owned(),
            revocation_file: "kilogram-removed-device.revocation".to_owned(),
            result: None,
        }
    }
}

#[derive(Debug)]
struct RecoveryPolicyView {
    old_package_file: String,
    old_witness_file: String,
    new_package_file: String,
    new_witness_file: String,
    old_epoch: String,
    previous_transition_id: String,
    request_file: String,
    request: Option<RecoveryPolicyTransitionRequestOutput>,
    approver_state_dir: String,
    ticket_output_file: String,
    route_policy: RuntimeIpcRoutePolicy,
    listener: Option<RecoveryPolicyTransitionListenOutput>,
    ticket_files: String,
    approval_directory: String,
    collection: Option<RecoveryPolicyTransitionCollectOutput>,
    approval_files: String,
    certificate_file: String,
    certificate: Option<RecoveryPolicyTransitionCertificateOutput>,
    install_state_dir: String,
    installation: Option<RecoveryPolicyInstallOutput>,
}

impl Default for RecoveryPolicyView {
    fn default() -> Self {
        Self {
            old_package_file: "kilogram-root-before-change.karp".to_owned(),
            old_witness_file: "kilogram-root-before-change.karw".to_owned(),
            new_package_file: "kilogram-root-after-change.karp".to_owned(),
            new_witness_file: "kilogram-root-after-change.karw".to_owned(),
            old_epoch: "0".to_owned(),
            previous_transition_id: "0".repeat(64),
            request_file: "kilogram-recovery-policy-transition.karpt".to_owned(),
            request: None,
            approver_state_dir: "kilogram-account/device".to_owned(),
            ticket_output_file: "kilogram-recovery-policy-approval.karpticket".to_owned(),
            route_policy: RuntimeIpcRoutePolicy::Auto,
            listener: None,
            ticket_files: String::new(),
            approval_directory: "kilogram-recovery-policy-approvals".to_owned(),
            collection: None,
            approval_files: String::new(),
            certificate_file: "kilogram-recovery-policy-transition.karpc".to_owned(),
            certificate: None,
            install_state_dir: "kilogram-account/device".to_owned(),
            installation: None,
        }
    }
}

impl Default for DeviceLinkView {
    fn default() -> Self {
        Self {
            joining_workspace: "kilogram-linked-device".to_owned(),
            account_id: String::new(),
            request: None,
            owner_request_file: String::new(),
            inspected_request_file: None,
            inspected: None,
            account_root_dir: "kilogram-account/account-root".to_owned(),
            confirmed_sas: String::new(),
            response_output_file: "device-link-response.bin".to_owned(),
            device_list_output_file: "account-device-list.snapshot".to_owned(),
            authorization: None,
            response_input_file: String::new(),
            accepted: None,
            removal: DeviceRemovalView::default(),
            policy: RecoveryPolicyView::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecoveryPlanView {
    path: PathBuf,
    status: String,
    lifecycle: String,
    attempts: String,
    complete: String,
}

impl RecoveryPlanView {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            status: "not run".to_owned(),
            lifecycle: "unknown".to_owned(),
            attempts: "0".to_owned(),
            complete: "unknown".to_owned(),
        }
    }

    fn update(&mut self, output: &RecoveryCommandOutput) {
        self.status = output.status().to_owned();
        self.lifecycle = output
            .field("history_recovery_scheduler_lifecycle")
            .unwrap_or("unknown")
            .to_owned();
        self.attempts = output
            .field("history_recovery_scheduler_total_attempts")
            .unwrap_or("0")
            .to_owned();
        self.complete = output
            .field("history_recovery_complete")
            .unwrap_or("unknown")
            .to_owned();
    }
}

#[derive(Debug)]
struct RecoveryView {
    state_dir: String,
    conversation: String,
    link_file: String,
    confirmed_sas: String,
    plan_output_file: String,
    allow_ethernet: bool,
    allow_wifi: bool,
    allow_mobile: bool,
    allow_unknown_network: bool,
    require_external_power: bool,
    existing_plan_file: String,
    plans: Vec<RecoveryPlanView>,
    selected: Option<usize>,
    confirm_cancel: bool,
    reconciliation: Option<RecoveryCommandOutput>,
}

impl Default for RecoveryView {
    fn default() -> Self {
        Self {
            state_dir: String::new(),
            conversation: String::new(),
            link_file: String::new(),
            confirmed_sas: String::new(),
            plan_output_file: "history-recovery-plan.bin".to_owned(),
            allow_ethernet: true,
            allow_wifi: true,
            allow_mobile: false,
            allow_unknown_network: false,
            require_external_power: false,
            existing_plan_file: String::new(),
            plans: Vec::new(),
            selected: None,
            confirm_cancel: false,
            reconciliation: None,
        }
    }
}

#[derive(Debug)]
struct PublicationConflictRecoveryView {
    channel_id: String,
    replacement_ticket_file: String,
    request_output_file: String,
    response_input_file: String,
    rotation_result: Option<RecoveryCommandOutput>,
    request_result: Option<RecoveryCommandOutput>,
    apply_result: Option<RecoveryCommandOutput>,
}

impl Default for PublicationConflictRecoveryView {
    fn default() -> Self {
        Self {
            channel_id: String::new(),
            replacement_ticket_file: String::new(),
            request_output_file: "publication-conflict-resolution.pcrq".to_owned(),
            response_input_file: String::new(),
            rotation_result: None,
            request_result: None,
            apply_result: None,
        }
    }
}

#[derive(Clone, Debug)]
struct RuntimeProfileDraft {
    state_dir: String,
    allowed_requester_account_id: String,
    device_list_file: String,
    peer_prekey_pool_files: String,
    ticket_file: String,
    ipc_file: String,
    route_policy: RuntimeIpcRoutePolicy,
    relay_url: String,
    relay_wait_seconds: String,
    poll_milliseconds: String,
    retry_base_seconds: String,
    retry_max_seconds: String,
    auto_sync_seconds: String,
    volunteer_storage_enabled: bool,
    volunteer_storage_data_dir: String,
    volunteer_storage_mib: String,
    volunteer_ethernet_transfer_mib: String,
    volunteer_wifi_transfer_mib: String,
    volunteer_mobile_transfer_mib: String,
    volunteer_unknown_transfer_mib: String,
}

impl Default for RuntimeProfileDraft {
    fn default() -> Self {
        Self {
            state_dir: "state".to_owned(),
            allowed_requester_account_id: String::new(),
            device_list_file: "account-device-list.bin".to_owned(),
            peer_prekey_pool_files: String::new(),
            ticket_file: "runtime.ticket".to_owned(),
            ipc_file: "runtime.ipc.json".to_owned(),
            route_policy: RuntimeIpcRoutePolicy::Auto,
            relay_url: String::new(),
            relay_wait_seconds: "15".to_owned(),
            poll_milliseconds: "250".to_owned(),
            retry_base_seconds: "1".to_owned(),
            retry_max_seconds: "60".to_owned(),
            auto_sync_seconds: "30".to_owned(),
            volunteer_storage_enabled: true,
            volunteer_storage_data_dir: String::new(),
            volunteer_storage_mib: "200".to_owned(),
            volunteer_ethernet_transfer_mib: "500".to_owned(),
            volunteer_wifi_transfer_mib: "500".to_owned(),
            volunteer_mobile_transfer_mib: "0".to_owned(),
            volunteer_unknown_transfer_mib: "0".to_owned(),
        }
    }
}

impl RuntimeProfileDraft {
    fn from_profile(profile: &RuntimeLaunchProfile) -> Self {
        let settings = profile.settings();
        let mut draft = Self {
            state_dir: settings.state_dir.display().to_string(),
            allowed_requester_account_id: settings.allowed_requester_account_id.to_string(),
            device_list_file: settings.device_list_file.display().to_string(),
            peer_prekey_pool_files: settings
                .peer_prekey_pool_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            ticket_file: settings
                .ticket_file
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            ipc_file: settings.ipc_file.display().to_string(),
            route_policy: settings.route_policy,
            relay_url: settings.relay_url.clone().unwrap_or_default(),
            relay_wait_seconds: settings.relay_wait_seconds.to_string(),
            poll_milliseconds: settings.poll_milliseconds.to_string(),
            retry_base_seconds: settings.retry_base_seconds.to_string(),
            retry_max_seconds: settings.retry_max_seconds.to_string(),
            auto_sync_seconds: settings.auto_sync_seconds.to_string(),
            ..Self::default()
        };
        if let Some(volunteer) = &settings.volunteer_storage {
            draft.volunteer_storage_enabled = true;
            draft.volunteer_storage_data_dir = volunteer.data_dir.display().to_string();
            draft.volunteer_storage_mib = bytes_to_mib(volunteer.max_total_storage_bytes);
            draft.volunteer_ethernet_transfer_mib =
                bytes_to_mib(volunteer.ethernet_transfer_bytes_per_30_days);
            draft.volunteer_wifi_transfer_mib =
                bytes_to_mib(volunteer.wifi_transfer_bytes_per_30_days);
            draft.volunteer_mobile_transfer_mib =
                bytes_to_mib(volunteer.mobile_transfer_bytes_per_30_days);
            draft.volunteer_unknown_transfer_mib =
                bytes_to_mib(volunteer.unknown_network_transfer_bytes_per_30_days);
        } else {
            draft.volunteer_storage_enabled = false;
        }
        draft
    }

    fn build(&self) -> Result<RuntimeLaunchProfile> {
        let state_dir = canonical_input_path(&self.state_dir, "State directory", false)?;
        let device_list_file = canonical_input_path(&self.device_list_file, "Device list", true)?;
        let peer_prekey_pool_files = self
            .peer_prekey_pool_files
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| canonical_input_path(value, "Peer prekey pool", true))
            .collect::<Result<Vec<_>>>()?;
        let ticket_file = optional_output_path(&self.ticket_file, "Runtime ticket", &state_dir)?;
        let ipc_file = absolute_output_path(&self.ipc_file, "IPC descriptor", &state_dir)?;
        let volunteer_storage = if self.volunteer_storage_enabled {
            let volunteer_data_dir = if self.volunteer_storage_data_dir.trim().is_empty() {
                default_volunteer_storage_path(&state_dir)?
            } else {
                absolute_output_path(
                    &self.volunteer_storage_data_dir,
                    "Volunteer storage directory",
                    &state_dir,
                )?
            };
            Some(RuntimeVolunteerStorageSettings::with_limits(
                volunteer_data_dir,
                0,
                parse_profile_mib(&self.volunteer_storage_mib, "Volunteer storage")?,
                parse_profile_mib(
                    &self.volunteer_ethernet_transfer_mib,
                    "Volunteer Ethernet transfer",
                )?,
                parse_profile_mib(
                    &self.volunteer_wifi_transfer_mib,
                    "Volunteer Wi-Fi transfer",
                )?,
                parse_profile_mib(
                    &self.volunteer_mobile_transfer_mib,
                    "Volunteer mobile transfer",
                )?,
                parse_profile_mib(
                    &self.volunteer_unknown_transfer_mib,
                    "Volunteer unknown-network transfer",
                )?,
            ))
        } else {
            None
        };
        RuntimeLaunchProfile::new(RuntimeLaunchSettings {
            state_dir,
            allowed_requester_account_id: AccountId::from_str(
                self.allowed_requester_account_id.trim(),
            )
            .context("Allowed requester Account ID is invalid")?,
            device_list_file,
            peer_prekey_pool_files,
            ticket_file,
            relay_wait_seconds: parse_profile_number(
                &self.relay_wait_seconds,
                "Relay wait seconds",
            )?,
            route_policy: self.route_policy,
            relay_url: (!self.relay_url.trim().is_empty())
                .then(|| self.relay_url.trim().to_owned()),
            poll_milliseconds: parse_profile_number(&self.poll_milliseconds, "Poll milliseconds")?,
            retry_base_seconds: parse_profile_number(
                &self.retry_base_seconds,
                "Retry base seconds",
            )?,
            retry_max_seconds: parse_profile_number(&self.retry_max_seconds, "Retry max seconds")?,
            auto_sync_seconds: parse_profile_number(&self.auto_sync_seconds, "Auto sync seconds")?,
            ipc_file,
            volunteer_storage,
        })
    }
}

#[derive(Debug, Default)]
struct ContactDraft {
    conversation: String,
    peer_account_id: String,
    descriptor_file: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedContactDraft {
    conversation: String,
    peer_account_id: AccountId,
    descriptor_file: PathBuf,
}

impl ContactDraft {
    fn validate(&self) -> Result<ValidatedContactDraft> {
        let conversation = self.conversation.trim();
        ensure!(!conversation.is_empty(), "Conversation is required");
        ensure!(
            conversation.len() <= MAX_CONVERSATION_BYTES,
            "Conversation must not exceed {MAX_CONVERSATION_BYTES} bytes"
        );
        let peer_text = self.peer_account_id.trim();
        ensure!(!peer_text.is_empty(), "Peer Account ID is required");
        let peer_account_id =
            AccountId::from_str(peer_text).context("Peer Account ID is invalid")?;
        let descriptor_text = self.descriptor_file.trim();
        ensure!(
            !descriptor_text.is_empty(),
            "Peer descriptor path is required"
        );
        ensure!(
            descriptor_text.len() <= MAX_DESCRIPTOR_PATH_BYTES,
            "Peer descriptor path is too long"
        );
        Ok(ValidatedContactDraft {
            conversation: conversation.to_owned(),
            peer_account_id,
            descriptor_file: PathBuf::from(descriptor_text),
        })
    }
}

#[derive(Debug)]
struct QueueDraft {
    conversation: String,
    peer_account_id: String,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedQueueDraft {
    conversation: String,
    peer_account_id: AccountId,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueueAttempt {
    request_id: RuntimeIpcRequestId,
    draft: ValidatedQueueDraft,
}

impl QueueDraft {
    fn validate(&self) -> Result<ValidatedQueueDraft> {
        let conversation = self.conversation.trim();
        ensure!(!conversation.is_empty(), "Conversation is required");
        ensure!(
            conversation.len() <= MAX_CONVERSATION_BYTES,
            "Conversation must not exceed {MAX_CONVERSATION_BYTES} bytes"
        );
        let peer_text = self.peer_account_id.trim();
        ensure!(!peer_text.is_empty(), "Peer Account ID is required");
        let peer_account_id =
            AccountId::from_str(peer_text).context("Peer Account ID is invalid")?;
        ensure!(!self.message.trim().is_empty(), "Message is required");
        ensure!(
            self.message.len() <= MAX_MESSAGE_BYTES,
            "Message must not exceed {MAX_MESSAGE_BYTES} bytes"
        );
        Ok(ValidatedQueueDraft {
            conversation: conversation.to_owned(),
            peer_account_id,
            message: self.message.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutboxItemView {
    queue_id: String,
    peer_account_id: String,
    conversation_id: String,
    state: &'static str,
    acknowledgement_event_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutboxView {
    contact_count: usize,
    queue_count: usize,
    pending_count: usize,
    materialized_count: usize,
    delivered_count: usize,
    mailbox_pending_count: usize,
    mailbox_stored_count: usize,
    mailbox_expired_count: usize,
    mailbox_failed_count: usize,
    retry_state_count: usize,
    items: Vec<OutboxItemView>,
}

impl From<RuntimeIpcOutboxStatus> for OutboxView {
    fn from(status: RuntimeIpcOutboxStatus) -> Self {
        Self {
            contact_count: status.contact_count,
            queue_count: status.queue_count,
            pending_count: status.pending_count,
            materialized_count: status.materialized_count,
            delivered_count: status.delivered_count,
            mailbox_pending_count: status.mailbox_pending_count,
            mailbox_stored_count: status.mailbox_stored_count,
            mailbox_expired_count: status.mailbox_expired_count,
            mailbox_failed_count: status.mailbox_failed_count,
            retry_state_count: status.retry_state_count,
            items: status
                .items
                .into_iter()
                .map(|item| OutboxItemView {
                    queue_id: item.queue_id,
                    peer_account_id: item.peer_account_id.to_string(),
                    conversation_id: item.conversation_id.to_string(),
                    state: match item.state {
                        RuntimeIpcQueueState::Queued => "queued",
                        RuntimeIpcQueueState::Materialized => "materialized",
                        RuntimeIpcQueueState::Delivered => "delivered",
                        RuntimeIpcQueueState::MailboxPending => "mailbox-pending",
                        RuntimeIpcQueueState::MailboxStored => "mailbox-stored",
                        RuntimeIpcQueueState::MailboxExpired => "mailbox-expired",
                        RuntimeIpcQueueState::MailboxFailed => "mailbox-failed",
                    },
                    acknowledgement_event_id: item
                        .acknowledgement_event_id
                        .map(|event_id| event_id.to_string()),
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
struct ViewModel {
    descriptor_path: String,
    connection: ConnectionState,
    pending: Option<Operation>,
    account_id: Option<String>,
    device_id: Option<String>,
    conversation: String,
    peer_account_id: String,
    conversations: Vec<RuntimeIpcConversationSummary>,
    selected_contact_id: Option<String>,
    history: Vec<RuntimeIpcHistoryMessage>,
    history_total: u32,
    history_next_cursor: Option<RuntimeIpcHistoryCursor>,
    contact_draft: ContactDraft,
    show_contact_form: bool,
    message: String,
    queue_attempt: Option<QueueAttempt>,
    outbox: Option<OutboxView>,
    device_directory_update: Option<RuntimeIpcDeviceDirectoryUpdate>,
    device_directory_status: Option<RuntimeIpcDeviceDirectoryStatus>,
    ticket_publication_service_url: String,
    ticket_publication: Option<RuntimeIpcTicketPublication>,
    contact_ticket_refresh: Option<RuntimeIpcContactTicketRefresh>,
    ticket_automation_allow_ethernet: bool,
    ticket_automation_allow_wifi: bool,
    ticket_automation_allow_mobile: bool,
    ticket_automation_allow_unknown_network: bool,
    ticket_automation_statuses: Vec<RuntimeIpcTicketAutomationStatus>,
    mailbox_peer_device_id: String,
    mailbox_valid_for_seconds: String,
    mailbox_status: Option<RuntimeIpcMailboxStatus>,
    mailbox_transition: Option<RuntimeIpcMailboxCapabilityTransition>,
    canonical_runtime_status: Option<RuntimeIpcCanonicalSyncV2RuntimeStatus>,
    peer_capability_status: Option<RuntimeIpcPeerCapabilityPage>,
    notice: Option<String>,
    error: Option<String>,
}

impl ViewModel {
    fn new(descriptor_path: PathBuf) -> Self {
        Self {
            descriptor_path: descriptor_path.display().to_string(),
            connection: ConnectionState::Disconnected,
            pending: None,
            account_id: None,
            device_id: None,
            conversation: String::new(),
            peer_account_id: String::new(),
            conversations: Vec::new(),
            selected_contact_id: None,
            history: Vec::new(),
            history_total: 0,
            history_next_cursor: None,
            contact_draft: ContactDraft::default(),
            show_contact_form: false,
            message: String::new(),
            queue_attempt: None,
            outbox: None,
            device_directory_update: None,
            device_directory_status: None,
            ticket_publication_service_url: String::new(),
            ticket_publication: None,
            contact_ticket_refresh: None,
            ticket_automation_allow_ethernet: true,
            ticket_automation_allow_wifi: true,
            ticket_automation_allow_mobile: false,
            ticket_automation_allow_unknown_network: false,
            ticket_automation_statuses: Vec::new(),
            mailbox_peer_device_id: String::new(),
            mailbox_valid_for_seconds: DEFAULT_MAILBOX_VALIDITY_SECONDS.to_string(),
            mailbox_status: None,
            mailbox_transition: None,
            canonical_runtime_status: None,
            peer_capability_status: None,
            notice: None,
            error: None,
        }
    }

    fn descriptor(&self) -> Result<PathBuf> {
        let value = self.descriptor_path.trim();
        ensure!(!value.is_empty(), "Runtime descriptor path is required");
        Ok(PathBuf::from(value))
    }

    fn queue_draft(&self) -> QueueDraft {
        QueueDraft {
            conversation: self.conversation.clone(),
            peer_account_id: self.peer_account_id.clone(),
            message: self.message.clone(),
        }
    }

    fn select_conversation(&mut self, contact_id: &str) -> bool {
        let Some(summary) = self
            .conversations
            .iter()
            .find(|summary| summary.contact_id == contact_id)
        else {
            return false;
        };
        let changed = self.selected_contact_id.as_deref() != Some(contact_id);
        let mailbox_peer_device_id = summary
            .endpoint_candidates
            .iter()
            .find(|candidate| {
                candidate.primary && candidate.state == RuntimeIpcEndpointCandidateState::Usable
            })
            .or_else(|| {
                summary
                    .endpoint_candidates
                    .iter()
                    .find(|candidate| candidate.state == RuntimeIpcEndpointCandidateState::Usable)
            })
            .map_or(summary.peer_device_id, |candidate| candidate.peer_device_id)
            .to_string();
        self.selected_contact_id = Some(contact_id.to_owned());
        self.conversation.clone_from(&summary.conversation_label);
        self.peer_account_id = summary.peer_account_id.to_string();
        if changed {
            self.mailbox_peer_device_id = mailbox_peer_device_id;
            self.mailbox_transition = None;
            self.history.clear();
            self.history_total = 0;
            self.history_next_cursor = None;
            self.queue_attempt = None;
        }
        changed
    }

    fn apply_conversations(&mut self, conversations: Vec<RuntimeIpcConversationSummary>) {
        let selected = self.selected_contact_id.clone();
        self.conversations = conversations;
        let contact_id = selected
            .filter(|selected| {
                self.conversations
                    .iter()
                    .any(|summary| summary.contact_id == *selected)
            })
            .or_else(|| {
                self.conversations
                    .first()
                    .map(|summary| summary.contact_id.clone())
            });
        if let Some(contact_id) = contact_id {
            self.select_conversation(&contact_id);
        } else {
            self.selected_contact_id = None;
            self.conversation.clear();
            self.peer_account_id.clear();
            self.mailbox_peer_device_id.clear();
            self.mailbox_transition = None;
            self.history.clear();
            self.history_total = 0;
            self.history_next_cursor = None;
        }
    }

    fn prepare_queue_attempt(&mut self) -> Result<QueueAttempt> {
        let draft = self.queue_draft().validate()?;
        if let Some(attempt) = self
            .queue_attempt
            .as_ref()
            .filter(|attempt| attempt.draft == draft)
        {
            return Ok(attempt.clone());
        }
        let attempt = QueueAttempt {
            request_id: RuntimeIpcRequestId::generate()?,
            draft,
        };
        self.queue_attempt = Some(attempt.clone());
        Ok(attempt)
    }

    fn begin(&mut self, operation: Operation) {
        self.pending = Some(operation);
        self.error = None;
        if operation == Operation::Connect {
            self.connection = ConnectionState::Connecting;
            self.notice = None;
            self.device_directory_status = None;
            self.mailbox_status = None;
            self.mailbox_transition = None;
            self.canonical_runtime_status = None;
            self.peer_capability_status = None;
        }
    }

    fn fail(&mut self, operation: Operation, message: String) {
        self.pending = None;
        if operation == Operation::Connect {
            self.connection = ConnectionState::Disconnected;
        }
        if operation == Operation::Queue {
            self.notice = self.queue_attempt.as_ref().map(|attempt| {
                format!(
                    "Safe retry will reuse request {}",
                    compact_id(&attempt.request_id.to_string())
                )
            });
        }
        self.error = Some(message);
    }

    fn apply(&mut self, response: WorkerResponse) {
        self.pending = None;
        match response.result {
            Ok(WorkerSuccess::Connected {
                account_id,
                device_id,
                directory_status,
                canonical_runtime_status,
            }) => {
                self.connection = ConnectionState::Connected;
                self.account_id = Some(account_id);
                self.device_id = Some(device_id);
                self.device_directory_status = Some(directory_status);
                self.canonical_runtime_status = Some(*canonical_runtime_status);
                self.notice = Some("Authenticated runtime connection established".to_owned());
                self.error = None;
            }
            Ok(WorkerSuccess::Queued {
                queue_id,
                contact_id,
                inserted,
            }) => {
                self.message.clear();
                self.queue_attempt = None;
                let action = if inserted { "Queued" } else { "Already queued" };
                self.notice = Some(format!(
                    "{action}: {} · contact {}",
                    compact_id(&queue_id),
                    compact_id(&contact_id)
                ));
                self.error = None;
            }
            Ok(WorkerSuccess::ContactAdded {
                contact_id,
                peer_account_id,
                peer_device_id,
                endpoint_candidate_count,
                endpoint_candidate_added,
                inserted,
            }) => {
                self.contact_draft = ContactDraft::default();
                self.show_contact_form = false;
                let action = if endpoint_candidate_added && endpoint_candidate_count > 1 {
                    "Endpoint candidate added"
                } else if inserted {
                    "Contact added"
                } else {
                    "Contact already present"
                };
                self.notice = Some(format!(
                    "{action}: {} · peer {} / device {} · {} endpoint(s)",
                    compact_id(&contact_id),
                    compact_id(&peer_account_id),
                    compact_id(&peer_device_id),
                    endpoint_candidate_count
                ));
                self.error = None;
            }
            Ok(WorkerSuccess::Refreshed(status)) => {
                self.connection = ConnectionState::Connected;
                self.outbox = Some(status.into());
                self.error = None;
            }
            Ok(WorkerSuccess::DeviceDirectoryApplied(update)) => {
                self.connection = ConnectionState::Connected;
                self.notice = Some(format!(
                    "Runtime directory applied at authority revision {}. {} ratchet session record(s) retired; old history copies remain readable.",
                    update.authority_revision, update.ratchet_session_records_retired
                ));
                self.device_directory_status = Some(update.directory_status.clone());
                self.device_directory_update = Some(update);
                self.error = None;
            }
            Ok(WorkerSuccess::TicketPublished(publication)) => {
                self.connection = ConnectionState::Connected;
                self.notice = Some(format!(
                    "Current ticket published for {} device(s), generation {}.",
                    publication.recipient_device_count, publication.publication_generation
                ));
                self.ticket_publication = Some(publication);
                self.error = None;
            }
            Ok(WorkerSuccess::ContactTicketRefreshed(refresh)) => {
                self.connection = ConnectionState::Connected;
                self.notice = Some(if refresh.complete {
                    format!(
                        "All {}/{} endpoint ticket(s) refreshed and installed atomically.",
                        refresh.refreshed_endpoint_candidate_count,
                        refresh.endpoint_candidate_count
                    )
                } else {
                    format!(
                        "Partial endpoint refresh: {}/{} usable. Stale candidates still require retry.",
                        refresh.refreshed_endpoint_candidate_count,
                        refresh.endpoint_candidate_count
                    )
                });
                self.contact_ticket_refresh = Some(refresh);
                self.error = None;
            }
            Ok(WorkerSuccess::TicketAutomationConfigured(status)) => {
                self.connection = ConnectionState::Connected;
                let enabled = status.enabled;
                if let Some(existing) = self
                    .ticket_automation_statuses
                    .iter_mut()
                    .find(|existing| existing.contact_id == status.contact_id)
                {
                    *existing = status;
                } else {
                    self.ticket_automation_statuses.push(status);
                }
                self.notice = Some(format!(
                    "Automatic ticket exchange {} for the selected contact.",
                    if enabled { "enabled" } else { "disabled" }
                ));
                self.error = None;
            }
            Ok(WorkerSuccess::TicketAutomationStatus(statuses)) => {
                self.connection = ConnectionState::Connected;
                self.ticket_automation_statuses = statuses;
                self.notice = Some("Automatic ticket-exchange status refreshed".to_owned());
                self.error = None;
            }
            Ok(WorkerSuccess::MailboxChanged(transition)) => {
                self.connection = ConnectionState::Connected;
                self.notice = Some(format!(
                    "Mailbox capability {} at generation {} for device {} ({})",
                    transition.action,
                    transition.generation,
                    compact_id(&transition.peer_device_id.to_string()),
                    transition.delivery_state
                ));
                self.mailbox_transition = Some(transition);
                self.mailbox_status = None;
                self.error = None;
            }
            Ok(WorkerSuccess::MailboxStatus(status)) => {
                self.connection = ConnectionState::Connected;
                self.mailbox_status = Some(status);
                self.error = None;
            }
            Ok(WorkerSuccess::CanonicalRuntimeStatus(status)) => {
                self.connection = ConnectionState::Connected;
                self.canonical_runtime_status = Some(*status);
                self.error = None;
            }
            Ok(WorkerSuccess::PeerCapabilityStatus(status)) => {
                self.connection = ConnectionState::Connected;
                self.peer_capability_status = Some(*status);
                self.error = None;
            }
            Ok(WorkerSuccess::Conversations(conversations)) => {
                self.connection = ConnectionState::Connected;
                self.apply_conversations(conversations);
                self.error = None;
            }
            Ok(WorkerSuccess::History { page, older }) => {
                self.connection = ConnectionState::Connected;
                if older {
                    let mut messages = page.messages;
                    messages.append(&mut self.history);
                    self.history = messages;
                } else {
                    self.history = page.messages;
                }
                self.history_total = page.total_messages;
                self.history_next_cursor = page.next_cursor;
                self.error = None;
            }
            Ok(WorkerSuccess::Shutdown) => {
                self.connection = ConnectionState::Disconnected;
                self.account_id = None;
                self.device_id = None;
                self.mailbox_status = None;
                self.mailbox_transition = None;
                self.canonical_runtime_status = None;
                self.peer_capability_status = None;
                self.notice = Some("Runtime stopped cleanly".to_owned());
                self.error = None;
            }
            Ok(
                WorkerSuccess::Bootstrapped(_)
                | WorkerSuccess::AccountRecoveryStatus(_)
                | WorkerSuccess::AccountRecoveryExported(_)
                | WorkerSuccess::AccountRecoveryInspected { .. }
                | WorkerSuccess::AccountRecoveryRestored(_)
                | WorkerSuccess::AccountRecoveryQuorumRequested(_)
                | WorkerSuccess::AccountRecoveryQuorumListened(_)
                | WorkerSuccess::AccountRecoveryQuorumCollected(_)
                | WorkerSuccess::AccountRecoveryQuorumVerified(_)
                | WorkerSuccess::RecoveryPolicyRequested(_)
                | WorkerSuccess::RecoveryPolicyListened(_)
                | WorkerSuccess::RecoveryPolicyCollected(_)
                | WorkerSuccess::RecoveryPolicyCertified(_)
                | WorkerSuccess::RecoveryPolicyInstalled(_)
                | WorkerSuccess::DeviceLinkRequested(_)
                | WorkerSuccess::DeviceLinkInspected { .. }
                | WorkerSuccess::DeviceLinkAuthorized(_)
                | WorkerSuccess::DeviceLinkAccepted(_)
                | WorkerSuccess::DeviceRemoved(_)
                | WorkerSuccess::RecoveryApproved { .. }
                | WorkerSuccess::RecoveryRan { .. }
                | WorkerSuccess::RecoveryCancelled { .. }
                | WorkerSuccess::RecoveryReconciled(_)
                | WorkerSuccess::PublicationChannelRotated(_)
                | WorkerSuccess::PublicationConflictRequested(_)
                | WorkerSuccess::PublicationConflictApplied(_)
                | WorkerSuccess::CanonicalActivationStatus(_)
                | WorkerSuccess::CanonicalActivationPlanned(_)
                | WorkerSuccess::CanonicalActivationApplied { .. }
                | WorkerSuccess::RolloutOperatorStatus(_)
                | WorkerSuccess::PeerReenrollmentStatus { .. },
            ) => {
                self.error = Some("Wizard result reached the runtime view model".to_owned());
            }
            Err(message) => self.fail(response.operation, message),
        }
    }
}

#[derive(Debug)]
enum WorkerRequest {
    Bootstrap {
        executable: PathBuf,
        workspace: PathBuf,
    },
    AccountRecoveryStatus {
        executable: PathBuf,
        account_root_dir: PathBuf,
    },
    AccountRecoveryExport {
        executable: PathBuf,
        account_root_dir: PathBuf,
        package_file: PathBuf,
        witness_file: PathBuf,
    },
    AccountRecoveryInspect {
        executable: PathBuf,
        package_file: PathBuf,
        witness_file: PathBuf,
    },
    AccountRecoveryRestore {
        executable: PathBuf,
        account_root_dir: PathBuf,
        package_file: PathBuf,
        witness_file: PathBuf,
        expected_package_id: String,
        expected_authority_revision: u64,
        recovery_phrase: AccountRecoveryPhrase,
    },
    AccountRecoveryQuorumRequest {
        executable: PathBuf,
        package_file: PathBuf,
        witness_file: PathBuf,
        request_file: PathBuf,
    },
    AccountRecoveryQuorumListen {
        executable: PathBuf,
        state_dir: PathBuf,
        request_file: PathBuf,
        ticket_file: PathBuf,
        route_policy: RuntimeIpcRoutePolicy,
    },
    AccountRecoveryQuorumCollect {
        executable: PathBuf,
        request_file: PathBuf,
        ticket_files: Vec<PathBuf>,
        approval_directory: PathBuf,
    },
    AccountRecoveryQuorumVerify {
        executable: PathBuf,
        request_file: PathBuf,
        approval_files: Vec<PathBuf>,
    },
    RecoveryPolicyRequest {
        executable: PathBuf,
        old_package_file: PathBuf,
        old_witness_file: PathBuf,
        new_package_file: PathBuf,
        new_witness_file: PathBuf,
        old_epoch: u64,
        previous_transition_id: String,
        request_file: PathBuf,
    },
    RecoveryPolicyListen {
        executable: PathBuf,
        state_dir: PathBuf,
        request_file: PathBuf,
        ticket_file: PathBuf,
        route_policy: RuntimeIpcRoutePolicy,
    },
    RecoveryPolicyCollect {
        executable: PathBuf,
        request_file: PathBuf,
        ticket_files: Vec<PathBuf>,
        approval_directory: PathBuf,
    },
    RecoveryPolicyCertify {
        executable: PathBuf,
        request_file: PathBuf,
        approval_files: Vec<PathBuf>,
        certificate_file: PathBuf,
    },
    RecoveryPolicyInstall {
        executable: PathBuf,
        state_dir: PathBuf,
        certificate_file: PathBuf,
    },
    DeviceLinkRequest {
        executable: PathBuf,
        workspace: PathBuf,
        account_id: AccountId,
    },
    DeviceLinkInspect {
        executable: PathBuf,
        request_file: PathBuf,
    },
    DeviceLinkAuthorize {
        executable: PathBuf,
        account_root_dir: PathBuf,
        request_file: PathBuf,
        confirmed_sas: String,
        response_file: PathBuf,
        device_list_file: PathBuf,
    },
    DeviceLinkAccept {
        executable: PathBuf,
        workspace: PathBuf,
        response_file: PathBuf,
    },
    DeviceRemove {
        executable: PathBuf,
        account_root_dir: PathBuf,
        device_id: kilogram_identity::DeviceId,
        before_package_file: PathBuf,
        before_witness_file: PathBuf,
        after_package_file: PathBuf,
        after_witness_file: PathBuf,
        device_list_file: PathBuf,
        revocation_file: PathBuf,
    },
    RecoveryApprove {
        executable: PathBuf,
        state_dir: PathBuf,
        link_file: PathBuf,
        conversation: String,
        confirmed_sas: String,
        plan_file: PathBuf,
        deny_ethernet: bool,
        deny_wifi: bool,
        allow_mobile: bool,
        allow_unknown_network: bool,
        require_external_power: bool,
    },
    RecoveryRun {
        executable: PathBuf,
        state_dir: PathBuf,
        plan_file: PathBuf,
        conversation: String,
    },
    RecoveryCancel {
        executable: PathBuf,
        state_dir: PathBuf,
        plan_file: PathBuf,
        conversation: String,
    },
    RecoveryReconcile {
        executable: PathBuf,
        state_dir: PathBuf,
        conversation: String,
    },
    PublicationChannelRotate {
        executable: PathBuf,
        ipc_file: PathBuf,
        peer_account_id: AccountId,
    },
    PublicationConflictRequest {
        executable: PathBuf,
        ipc_file: PathBuf,
        channel_id: String,
        replacement_ticket_file: PathBuf,
        request_file: PathBuf,
    },
    PublicationConflictApply {
        executable: PathBuf,
        ipc_file: PathBuf,
        response_file: PathBuf,
    },
    CanonicalActivationStatus {
        executable: PathBuf,
        state_dir: PathBuf,
    },
    CanonicalActivationPlan {
        executable: PathBuf,
        state_dir: PathBuf,
        migration_state: MigrationState,
        disposition: Disposition,
        reason: String,
        canonical_evidence_id: Option<String>,
    },
    CanonicalActivationApply {
        executable: PathBuf,
        state_dir: PathBuf,
        migration_state: MigrationState,
        disposition: Disposition,
        reason: String,
        canonical_evidence_id: Option<String>,
        expected_record_id: String,
    },
    RolloutOperatorStatus {
        executable: PathBuf,
        state_dir: PathBuf,
        rollout_directory: PathBuf,
        trusted_policy_directory: PathBuf,
        release_trust_root: String,
    },
    Connect {
        descriptor: PathBuf,
    },
    CanonicalRuntimeStatus {
        descriptor: PathBuf,
    },
    PeerCapabilityStatus {
        descriptor: PathBuf,
    },
    PeerReenrollmentStatus {
        descriptor: PathBuf,
        contact_id: RuntimeIpcPeerReenrollmentContactId,
    },
    PeerReenrollmentPlan {
        descriptor: PathBuf,
        command: RuntimeIpcPeerReenrollmentCommand,
    },
    PeerReenrollmentApprove {
        descriptor: PathBuf,
        command: RuntimeIpcPeerReenrollmentCommand,
    },
    PeerReenrollmentAccept {
        descriptor: PathBuf,
        command: RuntimeIpcPeerReenrollmentCommand,
    },
    AddContact {
        descriptor: PathBuf,
        draft: ValidatedContactDraft,
    },
    Queue {
        descriptor: PathBuf,
        request_id: RuntimeIpcRequestId,
        draft: ValidatedQueueDraft,
    },
    Refresh {
        descriptor: PathBuf,
    },
    ApplyDeviceDirectory {
        descriptor: PathBuf,
        device_list_file: PathBuf,
    },
    PublishTicket {
        descriptor: PathBuf,
        conversation: String,
        peer_account_id: AccountId,
        service_base_url: String,
    },
    RefreshContactTicket {
        descriptor: PathBuf,
        conversation: String,
        peer_account_id: AccountId,
        service_base_url: String,
    },
    ConfigureTicketAutomation {
        descriptor: PathBuf,
        conversation: String,
        peer_account_id: AccountId,
        enabled: bool,
        service_base_url: String,
        allow_ethernet: bool,
        allow_wifi: bool,
        allow_mobile: bool,
        allow_unknown_network: bool,
    },
    TicketAutomationStatus {
        descriptor: PathBuf,
    },
    MailboxCreate {
        descriptor: PathBuf,
        conversation: String,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
        valid_for_seconds: u64,
    },
    MailboxRotate {
        descriptor: PathBuf,
        conversation: String,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
        valid_for_seconds: u64,
    },
    MailboxRevoke {
        descriptor: PathBuf,
        conversation: String,
        peer_account_id: AccountId,
        peer_device_id: DeviceId,
    },
    MailboxStatus {
        descriptor: PathBuf,
    },
    Conversations {
        descriptor: PathBuf,
    },
    History {
        descriptor: PathBuf,
        conversation: String,
        cursor: Option<RuntimeIpcHistoryCursor>,
        older: bool,
    },
    Shutdown {
        descriptor: PathBuf,
    },
}

impl WorkerRequest {
    fn operation(&self) -> Operation {
        match self {
            Self::Bootstrap { .. } => Operation::Bootstrap,
            Self::AccountRecoveryStatus { .. } => Operation::AccountRecoveryStatus,
            Self::AccountRecoveryExport { .. } => Operation::AccountRecoveryExport,
            Self::AccountRecoveryInspect { .. } => Operation::AccountRecoveryInspect,
            Self::AccountRecoveryRestore { .. } => Operation::AccountRecoveryRestore,
            Self::AccountRecoveryQuorumRequest { .. } => Operation::AccountRecoveryQuorumRequest,
            Self::AccountRecoveryQuorumListen { .. } => Operation::AccountRecoveryQuorumListen,
            Self::AccountRecoveryQuorumCollect { .. } => Operation::AccountRecoveryQuorumCollect,
            Self::AccountRecoveryQuorumVerify { .. } => Operation::AccountRecoveryQuorumVerify,
            Self::RecoveryPolicyRequest { .. } => Operation::RecoveryPolicyRequest,
            Self::RecoveryPolicyListen { .. } => Operation::RecoveryPolicyListen,
            Self::RecoveryPolicyCollect { .. } => Operation::RecoveryPolicyCollect,
            Self::RecoveryPolicyCertify { .. } => Operation::RecoveryPolicyCertify,
            Self::RecoveryPolicyInstall { .. } => Operation::RecoveryPolicyInstall,
            Self::DeviceLinkRequest { .. } => Operation::DeviceLinkRequest,
            Self::DeviceLinkInspect { .. } => Operation::DeviceLinkInspect,
            Self::DeviceLinkAuthorize { .. } => Operation::DeviceLinkAuthorize,
            Self::DeviceLinkAccept { .. } => Operation::DeviceLinkAccept,
            Self::DeviceRemove { .. } => Operation::DeviceRemove,
            Self::RecoveryApprove { .. } => Operation::RecoveryApprove,
            Self::RecoveryRun { .. } => Operation::RecoveryRun,
            Self::RecoveryCancel { .. } => Operation::RecoveryCancel,
            Self::RecoveryReconcile { .. } => Operation::RecoveryReconcile,
            Self::PublicationChannelRotate { .. } => Operation::PublicationChannelRotate,
            Self::PublicationConflictRequest { .. } => Operation::PublicationConflictRequest,
            Self::PublicationConflictApply { .. } => Operation::PublicationConflictApply,
            Self::CanonicalActivationStatus { .. } => Operation::CanonicalActivationStatus,
            Self::CanonicalActivationPlan { .. } => Operation::CanonicalActivationPlan,
            Self::CanonicalActivationApply { .. } => Operation::CanonicalActivationApply,
            Self::RolloutOperatorStatus { .. } => Operation::RolloutOperatorStatus,
            Self::Connect { .. } => Operation::Connect,
            Self::CanonicalRuntimeStatus { .. } => Operation::CanonicalRuntimeStatus,
            Self::PeerCapabilityStatus { .. } => Operation::PeerCapabilityStatus,
            Self::PeerReenrollmentStatus { .. } => Operation::PeerReenrollmentStatus,
            Self::PeerReenrollmentPlan { .. } => Operation::PeerReenrollmentPlan,
            Self::PeerReenrollmentApprove { .. } => Operation::PeerReenrollmentApprove,
            Self::PeerReenrollmentAccept { .. } => Operation::PeerReenrollmentAccept,
            Self::AddContact { .. } => Operation::AddContact,
            Self::Queue { .. } => Operation::Queue,
            Self::Refresh { .. } => Operation::Refresh,
            Self::ApplyDeviceDirectory { .. } => Operation::ApplyDeviceDirectory,
            Self::PublishTicket { .. } => Operation::PublishTicket,
            Self::RefreshContactTicket { .. } => Operation::RefreshContactTicket,
            Self::ConfigureTicketAutomation { .. } => Operation::ConfigureTicketAutomation,
            Self::TicketAutomationStatus { .. } => Operation::TicketAutomationStatus,
            Self::MailboxCreate { .. } => Operation::MailboxCreate,
            Self::MailboxRotate { .. } => Operation::MailboxRotate,
            Self::MailboxRevoke { .. } => Operation::MailboxRevoke,
            Self::MailboxStatus { .. } => Operation::MailboxStatus,
            Self::Conversations { .. } => Operation::Conversations,
            Self::History { older: false, .. } => Operation::History,
            Self::History { older: true, .. } => Operation::HistoryOlder,
            Self::Shutdown { .. } => Operation::Shutdown,
        }
    }
}

#[derive(Debug)]
enum WorkerSuccess {
    Bootstrapped(Box<DesktopBootstrapOutput>),
    AccountRecoveryStatus(Box<AccountRecoveryStatusOutput>),
    AccountRecoveryExported(Box<AccountRecoveryOutput>),
    AccountRecoveryInspected {
        package_file: PathBuf,
        witness_file: PathBuf,
        output: Box<AccountRecoveryOutput>,
    },
    AccountRecoveryRestored(Box<AccountRecoveryOutput>),
    AccountRecoveryQuorumRequested(Box<RecoveryQuorumRequestOutput>),
    AccountRecoveryQuorumListened(Box<RecoveryQuorumListenOutput>),
    AccountRecoveryQuorumCollected(Box<RecoveryQuorumCollectOutput>),
    AccountRecoveryQuorumVerified(Box<RecoveryQuorumVerifyOutput>),
    RecoveryPolicyRequested(Box<RecoveryPolicyTransitionRequestOutput>),
    RecoveryPolicyListened(Box<RecoveryPolicyTransitionListenOutput>),
    RecoveryPolicyCollected(Box<RecoveryPolicyTransitionCollectOutput>),
    RecoveryPolicyCertified(Box<RecoveryPolicyTransitionCertificateOutput>),
    RecoveryPolicyInstalled(Box<RecoveryPolicyInstallOutput>),
    DeviceLinkRequested(Box<DeviceLinkRequestOutput>),
    DeviceLinkInspected {
        request_file: PathBuf,
        output: Box<DeviceLinkInspectOutput>,
    },
    DeviceLinkAuthorized(Box<DeviceLinkAuthorizeOutput>),
    DeviceLinkAccepted(Box<DeviceLinkAcceptOutput>),
    DeviceRemoved(Box<DeviceRemovalOutput>),
    RecoveryApproved {
        plan_file: PathBuf,
        output: RecoveryCommandOutput,
    },
    RecoveryRan {
        plan_file: PathBuf,
        output: RecoveryCommandOutput,
    },
    RecoveryCancelled {
        plan_file: PathBuf,
        output: RecoveryCommandOutput,
    },
    RecoveryReconciled(RecoveryCommandOutput),
    PublicationChannelRotated(RecoveryCommandOutput),
    PublicationConflictRequested(RecoveryCommandOutput),
    PublicationConflictApplied(RecoveryCommandOutput),
    CanonicalActivationStatus(ActivationStatus),
    CanonicalActivationPlanned(ActivationPlan),
    CanonicalActivationApplied {
        applied_record_id: String,
        status: ActivationStatus,
    },
    RolloutOperatorStatus(RolloutOperatorStatus),
    Connected {
        account_id: String,
        device_id: String,
        directory_status: RuntimeIpcDeviceDirectoryStatus,
        canonical_runtime_status: Box<RuntimeIpcCanonicalSyncV2RuntimeStatus>,
    },
    CanonicalRuntimeStatus(Box<RuntimeIpcCanonicalSyncV2RuntimeStatus>),
    PeerCapabilityStatus(Box<RuntimeIpcPeerCapabilityPage>),
    PeerReenrollmentStatus {
        descriptor_binding: [u8; 32],
        response: Box<RuntimeIpcPeerReenrollmentResponse>,
    },
    Queued {
        queue_id: String,
        contact_id: String,
        inserted: bool,
    },
    ContactAdded {
        contact_id: String,
        peer_account_id: String,
        peer_device_id: String,
        endpoint_candidate_count: u8,
        endpoint_candidate_added: bool,
        inserted: bool,
    },
    Refreshed(RuntimeIpcOutboxStatus),
    DeviceDirectoryApplied(RuntimeIpcDeviceDirectoryUpdate),
    TicketPublished(RuntimeIpcTicketPublication),
    ContactTicketRefreshed(RuntimeIpcContactTicketRefresh),
    TicketAutomationConfigured(RuntimeIpcTicketAutomationStatus),
    TicketAutomationStatus(Vec<RuntimeIpcTicketAutomationStatus>),
    MailboxChanged(RuntimeIpcMailboxCapabilityTransition),
    MailboxStatus(RuntimeIpcMailboxStatus),
    Conversations(Vec<RuntimeIpcConversationSummary>),
    History {
        page: RuntimeIpcHistoryPage,
        older: bool,
    },
    Shutdown,
}

#[derive(Debug)]
struct WorkerResponse {
    operation: Operation,
    result: Result<WorkerSuccess, String>,
}

struct RuntimeWorker {
    requests: Sender<WorkerRequest>,
    responses: Receiver<WorkerResponse>,
}

enum ChangeWatcherCommand {
    Subscribe(PathBuf),
    Stop,
}

enum ChangeWatcherEvent {
    Changed,
    Unavailable(String),
}

struct RuntimeChangeWatcher {
    commands: Sender<ChangeWatcherCommand>,
    events: Receiver<ChangeWatcherEvent>,
}

impl RuntimeChangeWatcher {
    fn spawn() -> Result<Self> {
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        thread::Builder::new()
            .name("kilogram-runtime-changes".to_owned())
            .spawn(move || change_watcher_main(command_receiver, event_sender))
            .context("start runtime change watcher")?;
        Ok(Self {
            commands: command_sender,
            events: event_receiver,
        })
    }

    fn subscribe(&self, descriptor: PathBuf) -> Result<()> {
        self.commands
            .send(ChangeWatcherCommand::Subscribe(descriptor))
            .context("runtime change watcher stopped")
    }

    fn stop(&self) {
        let _ = self.commands.send(ChangeWatcherCommand::Stop);
    }

    fn try_receive(&self) -> Option<ChangeWatcherEvent> {
        self.events.try_recv().ok()
    }
}

fn change_watcher_main(
    commands: Receiver<ChangeWatcherCommand>,
    events: Sender<ChangeWatcherEvent>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = events.send(ChangeWatcherEvent::Unavailable(format!(
                "Create change-watcher runtime: {error}"
            )));
            return;
        }
    };
    while let Ok(command) = commands.recv() {
        let ChangeWatcherCommand::Subscribe(mut descriptor) = command else {
            continue;
        };
        let mut revision = 0_u64;
        let mut failure_reported = false;
        loop {
            let result = runtime.block_on(kilogram_runtime_ipc::call(
                &descriptor,
                RuntimeIpcCommand::WaitForChange {
                    after_revision: revision,
                    timeout_milliseconds: CHANGE_WAIT_MILLISECONDS,
                },
            ));

            let mut restart = false;
            let mut stop = false;
            while let Ok(command) = commands.try_recv() {
                match command {
                    ChangeWatcherCommand::Subscribe(next) => {
                        descriptor = next;
                        revision = 0;
                        failure_reported = false;
                        restart = true;
                    }
                    ChangeWatcherCommand::Stop => {
                        stop = true;
                        break;
                    }
                }
            }
            if stop {
                break;
            }
            if restart {
                continue;
            }

            match result {
                Ok(RuntimeIpcResponse::ChangeState {
                    revision: next,
                    changed,
                }) => {
                    revision = next;
                    failure_reported = false;
                    if changed && events.send(ChangeWatcherEvent::Changed).is_err() {
                        return;
                    }
                }
                Ok(RuntimeIpcResponse::Error { message }) => {
                    if !failure_reported {
                        if events
                            .send(ChangeWatcherEvent::Unavailable(format!(
                                "Runtime change subscription rejected: {message}"
                            )))
                            .is_err()
                        {
                            return;
                        }
                        failure_reported = true;
                    }
                    thread::sleep(CHANGE_RETRY_INTERVAL);
                }
                Ok(_) => {
                    if !failure_reported {
                        let _ = events.send(ChangeWatcherEvent::Unavailable(
                            "Runtime returned an unexpected change response".to_owned(),
                        ));
                        failure_reported = true;
                    }
                    thread::sleep(CHANGE_RETRY_INTERVAL);
                }
                Err(error) => {
                    if !failure_reported {
                        if events
                            .send(ChangeWatcherEvent::Unavailable(format!(
                                "Runtime change subscription unavailable: {error:#}"
                            )))
                            .is_err()
                        {
                            return;
                        }
                        failure_reported = true;
                    }
                    thread::sleep(CHANGE_RETRY_INTERVAL);
                }
            }
        }
    }
}

impl RuntimeWorker {
    fn spawn() -> Result<Self> {
        let (request_sender, request_receiver) = mpsc::channel();
        let (response_sender, response_receiver) = mpsc::channel();
        thread::Builder::new()
            .name("kilogram-runtime-ipc".to_owned())
            .spawn(move || worker_main(request_receiver, response_sender))
            .context("start runtime IPC worker")?;
        Ok(Self {
            requests: request_sender,
            responses: response_receiver,
        })
    }

    fn send(&self, request: WorkerRequest) -> Result<()> {
        self.requests
            .send(request)
            .context("runtime IPC worker stopped")
    }

    fn try_receive(&self) -> Option<WorkerResponse> {
        self.responses.try_recv().ok()
    }
}

fn worker_main(requests: Receiver<WorkerRequest>, responses: Sender<WorkerResponse>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = responses.send(WorkerResponse {
                operation: Operation::Connect,
                result: Err(format!("Create IPC runtime: {error}")),
            });
            return;
        }
    };
    while let Ok(request) = requests.recv() {
        let operation = request.operation();
        let result = runtime
            .block_on(execute_request(request))
            .map_err(|error| format!("{error:#}"));
        if responses
            .send(WorkerResponse { operation, result })
            .is_err()
        {
            return;
        }
    }
}

async fn execute_peer_reenrollment_request(
    descriptor: PathBuf,
    peer_command: RuntimeIpcPeerReenrollmentCommand,
    mutation: bool,
) -> Result<WorkerSuccess> {
    let runtime_descriptor = kilogram_runtime_ipc::RuntimeIpcDescriptor::load(&descriptor)?;
    let descriptor_binding = *blake3::hash(
        &serde_json::to_vec(&runtime_descriptor)
            .context("encode verified runtime descriptor for desktop state binding")?,
    )
    .as_bytes();
    let request = RuntimeIpcPeerReenrollmentRequest::new(
        &runtime_descriptor,
        RuntimeIpcRequestId::generate()?,
        peer_command,
    )?;
    let command = if mutation {
        RuntimeIpcCommand::PeerReenrollmentMutation {
            request: request.encode()?,
        }
    } else {
        RuntimeIpcCommand::PeerReenrollmentRead {
            request: request.encode()?,
        }
    };
    let response = kilogram_runtime_ipc::call(&descriptor, command).await?;
    let signed_response = match (mutation, response) {
        (false, RuntimeIpcResponse::PeerReenrollmentRead { signed_response })
        | (true, RuntimeIpcResponse::PeerReenrollmentMutation { signed_response }) => {
            signed_response
        }
        (_, RuntimeIpcResponse::Error { message }) => {
            bail!("Runtime rejected peer re-enrollment request: {message}")
        }
        _ => bail!("Runtime returned an unexpected peer re-enrollment response"),
    };
    let signed = SignedRuntimeIpcPeerReenrollmentResponse::decode_and_verify(
        &signed_response,
        &request,
        &runtime_descriptor,
    )?;
    Ok(WorkerSuccess::PeerReenrollmentStatus {
        descriptor_binding,
        response: Box::new(signed.response().clone()),
    })
}

async fn execute_request(request: WorkerRequest) -> Result<WorkerSuccess> {
    match request {
        WorkerRequest::Bootstrap {
            executable,
            workspace,
        } => run_bootstrap_process(&executable, &workspace)
            .map(Box::new)
            .map(WorkerSuccess::Bootstrapped),
        WorkerRequest::AccountRecoveryStatus {
            executable,
            account_root_dir,
        } => {
            let output: AccountRecoveryStatusOutput = run_json(
                &executable,
                "account-recovery-status",
                command_arguments([("--account-root-dir", account_root_dir.as_os_str())]),
            )?;
            ensure!(
                output.account_root_dir == account_root_dir,
                "Account Root recovery helper reported a different Root path"
            );
            Ok(WorkerSuccess::AccountRecoveryStatus(Box::new(output)))
        }
        WorkerRequest::AccountRecoveryExport {
            executable,
            account_root_dir,
            package_file,
            witness_file,
        } => {
            let output: AccountRecoveryOutput = run_json(
                &executable,
                "account-recovery-export",
                command_arguments([
                    ("--account-root-dir", account_root_dir.as_os_str()),
                    ("--package-file", package_file.as_os_str()),
                    ("--witness-file", witness_file.as_os_str()),
                ]),
            )?;
            output.validate_expected_status("account-root-recovery-exported")?;
            ensure!(
                output.package_file == package_file && output.witness_file == witness_file,
                "Account Root recovery helper returned different export paths"
            );
            Ok(WorkerSuccess::AccountRecoveryExported(Box::new(output)))
        }
        WorkerRequest::AccountRecoveryInspect {
            executable,
            package_file,
            witness_file,
        } => {
            let output: AccountRecoveryOutput = run_json(
                &executable,
                "account-recovery-inspect",
                command_arguments([
                    ("--package-file", package_file.as_os_str()),
                    ("--witness-file", witness_file.as_os_str()),
                ]),
            )?;
            output.validate_expected_status("account-root-recovery-verified")?;
            ensure!(
                output.package_file == package_file && output.witness_file == witness_file,
                "Account Root recovery helper inspected different artifacts"
            );
            Ok(WorkerSuccess::AccountRecoveryInspected {
                package_file,
                witness_file,
                output: Box::new(output),
            })
        }
        WorkerRequest::AccountRecoveryRestore {
            executable,
            account_root_dir,
            package_file,
            witness_file,
            expected_package_id,
            expected_authority_revision,
            recovery_phrase,
        } => {
            let mut arguments = command_arguments([
                ("--account-root-dir", account_root_dir.as_os_str()),
                ("--package-file", package_file.as_os_str()),
                ("--witness-file", witness_file.as_os_str()),
            ]);
            let expected_authority_revision = expected_authority_revision.to_string();
            arguments.extend(command_arguments([
                (
                    "--expected-package-id",
                    std::ffi::OsStr::new(&expected_package_id),
                ),
                (
                    "--expected-authority-revision",
                    std::ffi::OsStr::new(&expected_authority_revision),
                ),
            ]));
            arguments.push("--recovery-phrase-stdin".into());
            let output: AccountRecoveryOutput = run_json_with_stdin(
                &executable,
                "account-recovery-restore",
                arguments,
                recovery_phrase.expose_secret().as_bytes(),
            )?;
            output.validate_expected_status("account-root-recovery-restored")?;
            ensure!(
                output.package_file == package_file
                    && output.witness_file == witness_file
                    && output.account_root_dir.as_ref() == Some(&account_root_dir),
                "Account Root recovery helper restored different paths"
            );
            Ok(WorkerSuccess::AccountRecoveryRestored(Box::new(output)))
        }
        WorkerRequest::AccountRecoveryQuorumRequest {
            executable,
            package_file,
            witness_file,
            request_file,
        } => {
            let output: RecoveryQuorumRequestOutput = run_json(
                &executable,
                "account-recovery-quorum-request",
                command_arguments([
                    ("--package-file", package_file.as_os_str()),
                    ("--witness-file", witness_file.as_os_str()),
                    ("--request-file", request_file.as_os_str()),
                ]),
            )?;
            ensure!(
                output.request_file == request_file,
                "quorum helper returned a different request path"
            );
            Ok(WorkerSuccess::AccountRecoveryQuorumRequested(Box::new(
                output,
            )))
        }
        WorkerRequest::AccountRecoveryQuorumListen {
            executable,
            state_dir,
            request_file,
            ticket_file,
            route_policy,
        } => {
            let route_policy = route_policy_argument(route_policy);
            let output: RecoveryQuorumListenOutput = run_json(
                &executable,
                "account-recovery-quorum-listen",
                command_arguments([
                    ("--state-dir", state_dir.as_os_str()),
                    ("--request-file", request_file.as_os_str()),
                    ("--ticket-file", ticket_file.as_os_str()),
                    ("--route-policy", std::ffi::OsStr::new(route_policy)),
                ]),
            )?;
            ensure!(
                output.ticket_file == ticket_file,
                "quorum listener returned a different ticket path"
            );
            Ok(WorkerSuccess::AccountRecoveryQuorumListened(Box::new(
                output,
            )))
        }
        WorkerRequest::AccountRecoveryQuorumCollect {
            executable,
            request_file,
            ticket_files,
            approval_directory,
        } => {
            let mut arguments = command_arguments([
                ("--request-file", request_file.as_os_str()),
                ("--approval-dir", approval_directory.as_os_str()),
            ]);
            for ticket_file in &ticket_files {
                arguments.push("--ticket-file".into());
                arguments.push(ticket_file.as_os_str().into());
            }
            let output: RecoveryQuorumCollectOutput =
                run_json(&executable, "account-recovery-quorum-collect", arguments)?;
            ensure!(
                output.approval_directory == approval_directory,
                "quorum collector returned a different approval directory"
            );
            Ok(WorkerSuccess::AccountRecoveryQuorumCollected(Box::new(
                output,
            )))
        }
        WorkerRequest::AccountRecoveryQuorumVerify {
            executable,
            request_file,
            approval_files,
        } => {
            let mut arguments = command_arguments([("--request-file", request_file.as_os_str())]);
            for approval_file in &approval_files {
                arguments.push("--approval-file".into());
                arguments.push(approval_file.as_os_str().into());
            }
            let output: RecoveryQuorumVerifyOutput =
                run_json(&executable, "account-recovery-quorum-verify", arguments)?;
            Ok(WorkerSuccess::AccountRecoveryQuorumVerified(Box::new(
                output,
            )))
        }
        WorkerRequest::RecoveryPolicyRequest {
            executable,
            old_package_file,
            old_witness_file,
            new_package_file,
            new_witness_file,
            old_epoch,
            previous_transition_id,
            request_file,
        } => {
            let old_epoch = old_epoch.to_string();
            let output: RecoveryPolicyTransitionRequestOutput = run_json(
                &executable,
                "account-recovery-policy-transition-request",
                command_arguments([
                    ("--old-package-file", old_package_file.as_os_str()),
                    ("--old-witness-file", old_witness_file.as_os_str()),
                    ("--new-package-file", new_package_file.as_os_str()),
                    ("--new-witness-file", new_witness_file.as_os_str()),
                    ("--old-epoch", std::ffi::OsStr::new(&old_epoch)),
                    (
                        "--previous-transition-id",
                        std::ffi::OsStr::new(&previous_transition_id),
                    ),
                    ("--request-file", request_file.as_os_str()),
                ]),
            )?;
            ensure!(
                output.request_file == request_file,
                "policy helper returned a different request path"
            );
            Ok(WorkerSuccess::RecoveryPolicyRequested(Box::new(output)))
        }
        WorkerRequest::RecoveryPolicyListen {
            executable,
            state_dir,
            request_file,
            ticket_file,
            route_policy,
        } => {
            let route_policy = route_policy_argument(route_policy);
            let output: RecoveryPolicyTransitionListenOutput = run_json(
                &executable,
                "account-recovery-policy-transition-listen",
                command_arguments([
                    ("--state-dir", state_dir.as_os_str()),
                    ("--request-file", request_file.as_os_str()),
                    ("--ticket-file", ticket_file.as_os_str()),
                    ("--route-policy", std::ffi::OsStr::new(route_policy)),
                ]),
            )?;
            ensure!(
                output.ticket_file == ticket_file,
                "policy listener returned a different ticket path"
            );
            Ok(WorkerSuccess::RecoveryPolicyListened(Box::new(output)))
        }
        WorkerRequest::RecoveryPolicyCollect {
            executable,
            request_file,
            ticket_files,
            approval_directory,
        } => {
            let mut arguments = command_arguments([
                ("--request-file", request_file.as_os_str()),
                ("--approval-dir", approval_directory.as_os_str()),
            ]);
            for ticket_file in &ticket_files {
                arguments.push("--ticket-file".into());
                arguments.push(ticket_file.as_os_str().into());
            }
            let output: RecoveryPolicyTransitionCollectOutput = run_json(
                &executable,
                "account-recovery-policy-transition-collect",
                arguments,
            )?;
            ensure!(
                output.approval_directory == approval_directory,
                "policy collector returned a different approval directory"
            );
            Ok(WorkerSuccess::RecoveryPolicyCollected(Box::new(output)))
        }
        WorkerRequest::RecoveryPolicyCertify {
            executable,
            request_file,
            approval_files,
            certificate_file,
        } => {
            let mut arguments = command_arguments([
                ("--request-file", request_file.as_os_str()),
                ("--certificate-file", certificate_file.as_os_str()),
            ]);
            for approval_file in &approval_files {
                arguments.push("--approval-file".into());
                arguments.push(approval_file.as_os_str().into());
            }
            let output: RecoveryPolicyTransitionCertificateOutput = run_json(
                &executable,
                "account-recovery-policy-transition-certify",
                arguments,
            )?;
            ensure!(
                output.certificate_file == certificate_file,
                "policy helper returned a different certificate path"
            );
            Ok(WorkerSuccess::RecoveryPolicyCertified(Box::new(output)))
        }
        WorkerRequest::RecoveryPolicyInstall {
            executable,
            state_dir,
            certificate_file,
        } => {
            let output: RecoveryPolicyInstallOutput = run_json(
                &executable,
                "account-recovery-policy-transition-install",
                command_arguments([
                    ("--state-dir", state_dir.as_os_str()),
                    ("--certificate-file", certificate_file.as_os_str()),
                ]),
            )?;
            Ok(WorkerSuccess::RecoveryPolicyInstalled(Box::new(output)))
        }
        WorkerRequest::DeviceLinkRequest {
            executable,
            workspace,
            account_id,
        } => {
            let account_text = account_id.to_string();
            let output: DeviceLinkRequestOutput = run_json(
                &executable,
                "device-link-request",
                command_arguments([
                    ("--workspace-dir", workspace.as_os_str()),
                    ("--account-id", std::ffi::OsStr::new(&account_text)),
                ]),
            )?;
            ensure!(
                output.workspace_dir == workspace && output.account_id == account_text,
                "device-link helper returned a different request target"
            );
            Ok(WorkerSuccess::DeviceLinkRequested(Box::new(output)))
        }
        WorkerRequest::DeviceLinkInspect {
            executable,
            request_file,
        } => {
            let output = run_json(
                &executable,
                "device-link-inspect",
                command_arguments([("--request-file", request_file.as_os_str())]),
            )?;
            Ok(WorkerSuccess::DeviceLinkInspected {
                request_file,
                output: Box::new(output),
            })
        }
        WorkerRequest::DeviceLinkAuthorize {
            executable,
            account_root_dir,
            request_file,
            confirmed_sas,
            response_file,
            device_list_file,
        } => {
            let output: DeviceLinkAuthorizeOutput = run_json(
                &executable,
                "device-link-authorize",
                command_arguments([
                    ("--account-root-dir", account_root_dir.as_os_str()),
                    ("--request-file", request_file.as_os_str()),
                    ("--confirm-sas", std::ffi::OsStr::new(&confirmed_sas)),
                    ("--response-file", response_file.as_os_str()),
                    ("--device-list-file", device_list_file.as_os_str()),
                ]),
            )?;
            ensure!(
                output.response_file == response_file
                    && output.device_list_file == device_list_file,
                "device-link helper returned different authorization output paths"
            );
            Ok(WorkerSuccess::DeviceLinkAuthorized(Box::new(output)))
        }
        WorkerRequest::DeviceLinkAccept {
            executable,
            workspace,
            response_file,
        } => {
            let output: DeviceLinkAcceptOutput = run_json(
                &executable,
                "device-link-accept",
                command_arguments([
                    ("--workspace-dir", workspace.as_os_str()),
                    ("--response-file", response_file.as_os_str()),
                ]),
            )?;
            ensure!(
                output.workspace_dir == workspace,
                "device-link helper returned a different accepted workspace"
            );
            Ok(WorkerSuccess::DeviceLinkAccepted(Box::new(output)))
        }
        WorkerRequest::DeviceRemove {
            executable,
            account_root_dir,
            device_id,
            before_package_file,
            before_witness_file,
            after_package_file,
            after_witness_file,
            device_list_file,
            revocation_file,
        } => {
            let device_id = device_id.to_string();
            let output: DeviceRemovalOutput = run_json(
                &executable,
                "device-remove",
                command_arguments([
                    ("--account-root-dir", account_root_dir.as_os_str()),
                    ("--device-id", std::ffi::OsStr::new(&device_id)),
                    ("--before-package-file", before_package_file.as_os_str()),
                    ("--before-witness-file", before_witness_file.as_os_str()),
                    ("--after-package-file", after_package_file.as_os_str()),
                    ("--after-witness-file", after_witness_file.as_os_str()),
                    ("--device-list-file", device_list_file.as_os_str()),
                    ("--revocation-file", revocation_file.as_os_str()),
                ]),
            )?;
            ensure!(
                output.removed_device_id == device_id
                    && output.before_package_file == before_package_file
                    && output.before_witness_file == before_witness_file
                    && output.after_package_file == after_package_file
                    && output.after_witness_file == after_witness_file
                    && output.device_list_file == device_list_file
                    && output.revocation_file == revocation_file,
                "device-removal helper returned different identity or artifact paths"
            );
            Ok(WorkerSuccess::DeviceRemoved(Box::new(output)))
        }
        WorkerRequest::RecoveryApprove {
            executable,
            state_dir,
            link_file,
            conversation,
            confirmed_sas,
            plan_file,
            deny_ethernet,
            deny_wifi,
            allow_mobile,
            allow_unknown_network,
            require_external_power,
        } => {
            let mut arguments = command_arguments([
                ("--state-dir", state_dir.as_os_str()),
                ("--link-file", link_file.as_os_str()),
                ("--conversation", std::ffi::OsStr::new(&conversation)),
                ("--confirm-sas", std::ffi::OsStr::new(&confirmed_sas)),
                ("--plan-file", plan_file.as_os_str()),
            ]);
            for (enabled, name) in [
                (deny_ethernet, "--deny-ethernet"),
                (deny_wifi, "--deny-wifi"),
                (allow_mobile, "--allow-mobile"),
                (allow_unknown_network, "--allow-unknown-network"),
                (require_external_power, "--require-external-power"),
            ] {
                if enabled {
                    arguments.push(name.into());
                }
            }
            run_recovery_command(&executable, "history-recovery-plan-approve", arguments)
                .map(|output| WorkerSuccess::RecoveryApproved { plan_file, output })
        }
        WorkerRequest::RecoveryRun {
            executable,
            state_dir,
            plan_file,
            conversation,
        } => run_recovery_command(
            &executable,
            "history-recovery-plan-run",
            command_arguments([
                ("--state-dir", state_dir.as_os_str()),
                ("--plan-file", plan_file.as_os_str()),
                ("--conversation", std::ffi::OsStr::new(&conversation)),
                ("--max-attempts", std::ffi::OsStr::new("1")),
                ("--discovery-wait-seconds", std::ffi::OsStr::new("5")),
            ]),
        )
        .map(|output| WorkerSuccess::RecoveryRan { plan_file, output }),
        WorkerRequest::RecoveryCancel {
            executable,
            state_dir,
            plan_file,
            conversation,
        } => run_recovery_command(
            &executable,
            "history-recovery-plan-cancel",
            command_arguments([
                ("--state-dir", state_dir.as_os_str()),
                ("--plan-file", plan_file.as_os_str()),
                ("--conversation", std::ffi::OsStr::new(&conversation)),
            ]),
        )
        .map(|output| WorkerSuccess::RecoveryCancelled { plan_file, output }),
        WorkerRequest::RecoveryReconcile {
            executable,
            state_dir,
            conversation,
        } => run_recovery_command(
            &executable,
            "history-rewrap-reconcile",
            command_arguments([
                ("--state-dir", state_dir.as_os_str()),
                ("--conversation", std::ffi::OsStr::new(&conversation)),
            ]),
        )
        .map(WorkerSuccess::RecoveryReconciled),
        WorkerRequest::PublicationChannelRotate {
            executable,
            ipc_file,
            peer_account_id,
        } => {
            let peer_account_id = peer_account_id.to_string();
            let output = run_recovery_command(
                &executable,
                "runtime-ipc-rotate-publication-channel",
                command_arguments([
                    ("--ipc-file", ipc_file.as_os_str()),
                    ("--peer-account", std::ffi::OsStr::new(&peer_account_id)),
                ]),
            )?;
            ensure!(
                output.field("process_exit_success") == Some("true")
                    && output.status() == "runtime-publication-channel-rotated"
                    && output.field("runtime_restart_required") == Some("false"),
                "Live publication-channel rotation did not complete successfully"
            );
            Ok(WorkerSuccess::PublicationChannelRotated(output))
        }
        WorkerRequest::PublicationConflictRequest {
            executable,
            ipc_file,
            channel_id,
            replacement_ticket_file,
            request_file,
        } => {
            let output = run_recovery_command(
                &executable,
                "runtime-ipc-publication-conflict-create-request",
                command_arguments([
                    ("--ipc-file", ipc_file.as_os_str()),
                    ("--channel", std::ffi::OsStr::new(&channel_id)),
                    (
                        "--replacement-ticket-file",
                        replacement_ticket_file.as_os_str(),
                    ),
                    ("--output-file", request_file.as_os_str()),
                ]),
            )?;
            ensure!(
                output.field("process_exit_success") == Some("true")
                    && output.status() == "publication-conflict-resolution-requested"
                    && output.field("runtime_restart_required") == Some("false")
                    && output
                        .field("request_artifact_digest")
                        .is_some_and(|value| {
                            value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                        })
                    && output
                        .field("confirmation_code")
                        .is_some_and(valid_publication_conflict_confirmation_code),
                "Conflict-resolution request helper did not complete successfully"
            );
            Ok(WorkerSuccess::PublicationConflictRequested(output))
        }
        WorkerRequest::PublicationConflictApply {
            executable,
            ipc_file,
            response_file,
        } => {
            let output = run_recovery_command(
                &executable,
                "runtime-ipc-publication-conflict-apply-response",
                command_arguments([
                    ("--ipc-file", ipc_file.as_os_str()),
                    ("--response-file", response_file.as_os_str()),
                ]),
            )?;
            ensure!(
                output.field("process_exit_success") == Some("true")
                    && output.status() == "publication-conflict-resolved"
                    && output.field("runtime_restart_required") == Some("false"),
                "Conflict-resolution apply helper did not complete successfully"
            );
            Ok(WorkerSuccess::PublicationConflictApplied(output))
        }
        WorkerRequest::CanonicalActivationStatus {
            executable,
            state_dir,
        } => {
            let output = run_recovery_command(
                &executable,
                "canonical-sync-policy-status",
                command_arguments([("--state-dir", state_dir.as_os_str())]),
            )?;
            Ok(WorkerSuccess::CanonicalActivationStatus(
                ActivationStatus::parse(&output)?,
            ))
        }
        WorkerRequest::CanonicalActivationPlan {
            executable,
            state_dir,
            migration_state,
            disposition,
            reason,
            canonical_evidence_id,
        } => {
            let mut arguments = command_arguments([
                ("--state-dir", state_dir.as_os_str()),
                (
                    "--migration-state",
                    std::ffi::OsStr::new(migration_state.cli_value()),
                ),
                (
                    "--disposition",
                    std::ffi::OsStr::new(disposition.cli_value()),
                ),
                ("--actor", std::ffi::OsStr::new("explicit-local")),
                ("--reason", std::ffi::OsStr::new(&reason)),
            ]);
            if let Some(evidence_id) = canonical_evidence_id {
                arguments.extend(command_arguments([(
                    "--canonical-evidence-id",
                    std::ffi::OsStr::new(&evidence_id),
                )]));
            }
            let output =
                run_recovery_command(&executable, "canonical-sync-policy-plan", arguments)?;
            Ok(WorkerSuccess::CanonicalActivationPlanned(
                ActivationPlan::parse(&output)?,
            ))
        }
        WorkerRequest::CanonicalActivationApply {
            executable,
            state_dir,
            migration_state,
            disposition,
            reason,
            canonical_evidence_id,
            expected_record_id,
        } => {
            let mut arguments = command_arguments([
                ("--state-dir", state_dir.as_os_str()),
                (
                    "--migration-state",
                    std::ffi::OsStr::new(migration_state.cli_value()),
                ),
                (
                    "--disposition",
                    std::ffi::OsStr::new(disposition.cli_value()),
                ),
                ("--actor", std::ffi::OsStr::new("explicit-local")),
                ("--reason", std::ffi::OsStr::new(&reason)),
                (
                    "--expected-record-id",
                    std::ffi::OsStr::new(&expected_record_id),
                ),
            ]);
            if let Some(evidence_id) = canonical_evidence_id {
                arguments.extend(command_arguments([(
                    "--canonical-evidence-id",
                    std::ffi::OsStr::new(&evidence_id),
                )]));
            }
            let applied =
                run_recovery_command(&executable, "canonical-sync-policy-apply", arguments)?;
            ensure!(
                applied.field("process_exit_success") == Some("true")
                    && matches!(
                        applied.status(),
                        "canonical-sync-v2-activation-applied"
                            | "canonical-sync-v2-activation-repaired"
                    ),
                "canonical sync v2 activation apply failed: {}",
                applied
                    .field("process_error")
                    .unwrap_or("unexpected status")
            );
            let applied_record_id = applied
                .field("activation_plan_record_id")
                .or_else(|| applied.field("activation_record_id"))
                .context("activation apply omitted the applied record ID")?
                .to_owned();
            ensure!(
                applied_record_id == expected_record_id,
                "activation apply returned a different record ID"
            );
            let status_output = run_recovery_command(
                &executable,
                "canonical-sync-policy-status",
                command_arguments([("--state-dir", state_dir.as_os_str())]),
            )?;
            let status = ActivationStatus::parse(&status_output)?;
            ensure!(
                status.record_id.as_deref() == Some(applied_record_id.as_str()),
                "verified activation head differs from the applied successor"
            );
            Ok(WorkerSuccess::CanonicalActivationApplied {
                applied_record_id,
                status,
            })
        }
        WorkerRequest::RolloutOperatorStatus {
            executable,
            state_dir,
            rollout_directory,
            trusted_policy_directory,
            release_trust_root,
        } => {
            let output = run_recovery_command(
                &executable,
                "rollout-operator-status",
                command_arguments([
                    ("--state-dir", state_dir.as_os_str()),
                    ("--rollout-dir", rollout_directory.as_os_str()),
                    ("--trusted-policy-dir", trusted_policy_directory.as_os_str()),
                    (
                        "--release-trust-root",
                        std::ffi::OsStr::new(&release_trust_root),
                    ),
                ]),
            )?;
            Ok(WorkerSuccess::RolloutOperatorStatus(
                RolloutOperatorStatus::parse(&output)?,
            ))
        }
        WorkerRequest::Connect { descriptor } => {
            match kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::Ping).await? {
                RuntimeIpcResponse::Pong {
                    account_id,
                    device_id,
                } => {
                    let directory_status = match kilogram_runtime_ipc::call(
                        &descriptor,
                        RuntimeIpcCommand::OwnDeviceDirectoryStatus,
                    )
                    .await?
                    {
                        RuntimeIpcResponse::OwnDeviceDirectoryStatus(status) => status,
                        RuntimeIpcResponse::Error { message } => {
                            bail!("Runtime rejected device-directory status: {message}")
                        }
                        _ => bail!("Runtime returned an unexpected device-directory status"),
                    };
                    ensure!(
                        directory_status.account_id == account_id
                            && directory_status.local_device_id == device_id,
                        "Runtime device-directory status identity differs from ping"
                    );
                    let canonical_runtime_status = match kilogram_runtime_ipc::call(
                        &descriptor,
                        RuntimeIpcCommand::CanonicalSyncV2RuntimeStatus,
                    )
                    .await?
                    {
                        RuntimeIpcResponse::CanonicalSyncV2RuntimeStatus(status) => status,
                        RuntimeIpcResponse::Error { message } => {
                            bail!("Runtime rejected canonical sync status: {message}")
                        }
                        _ => bail!("Runtime returned an unexpected canonical sync status"),
                    };
                    Ok(WorkerSuccess::Connected {
                        account_id: account_id.to_string(),
                        device_id: device_id.to_string(),
                        directory_status,
                        canonical_runtime_status,
                    })
                }
                RuntimeIpcResponse::Error { message } => bail!("Runtime rejected ping: {message}"),
                _ => bail!("Runtime returned an unexpected ping response"),
            }
        }
        WorkerRequest::CanonicalRuntimeStatus { descriptor } => {
            match kilogram_runtime_ipc::call(
                &descriptor,
                RuntimeIpcCommand::CanonicalSyncV2RuntimeStatus,
            )
            .await?
            {
                RuntimeIpcResponse::CanonicalSyncV2RuntimeStatus(status) => {
                    Ok(WorkerSuccess::CanonicalRuntimeStatus(status))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected canonical sync status: {message}")
                }
                _ => bail!("Runtime returned an unexpected canonical sync status"),
            }
        }
        WorkerRequest::PeerCapabilityStatus { descriptor } => {
            let command = RuntimeIpcCommand::PeerCapabilityQuarantinePage {
                cursor: None,
                limit: MAX_RUNTIME_IPC_PEER_CAPABILITY_PAGE_SIZE,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::PeerCapabilityQuarantinePage(status) => {
                    status.validate_for_request(None, MAX_RUNTIME_IPC_PEER_CAPABILITY_PAGE_SIZE)?;
                    Ok(WorkerSuccess::PeerCapabilityStatus(status))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected peer capability status: {message}")
                }
                _ => bail!("Runtime returned an unexpected peer capability status"),
            }
        }
        WorkerRequest::PeerReenrollmentStatus {
            descriptor,
            contact_id,
        } => {
            execute_peer_reenrollment_request(
                descriptor,
                RuntimeIpcPeerReenrollmentCommand::Status { contact_id },
                false,
            )
            .await
        }
        WorkerRequest::PeerReenrollmentPlan {
            descriptor,
            command,
        } => {
            ensure!(
                matches!(command, RuntimeIpcPeerReenrollmentCommand::Plan { .. }),
                "desktop peer re-enrollment plan worker received another command"
            );
            execute_peer_reenrollment_request(descriptor, command, true).await
        }
        WorkerRequest::PeerReenrollmentApprove {
            descriptor,
            command,
        } => {
            ensure!(
                matches!(command, RuntimeIpcPeerReenrollmentCommand::Approve { .. }),
                "desktop peer re-enrollment approve worker received another command"
            );
            execute_peer_reenrollment_request(descriptor, command, true).await
        }
        WorkerRequest::PeerReenrollmentAccept {
            descriptor,
            command,
        } => {
            ensure!(
                matches!(command, RuntimeIpcPeerReenrollmentCommand::Accept { .. }),
                "desktop peer re-enrollment accept worker received another command"
            );
            execute_peer_reenrollment_request(descriptor, command, true).await
        }
        WorkerRequest::AddContact { descriptor, draft } => {
            let command = RuntimeIpcCommand::AddContact {
                conversation: draft.conversation,
                expected_peer_account_id: draft.peer_account_id,
                descriptor_file: draft.descriptor_file,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::ContactAdded {
                    contact_id,
                    peer_account_id,
                    peer_device_id,
                    endpoint_candidate_count,
                    endpoint_candidate_added,
                    inserted,
                } => Ok(WorkerSuccess::ContactAdded {
                    contact_id,
                    peer_account_id: peer_account_id.to_string(),
                    peer_device_id: peer_device_id.to_string(),
                    endpoint_candidate_count,
                    endpoint_candidate_added,
                    inserted,
                }),
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected the contact: {message}")
                }
                _ => bail!("Runtime returned an unexpected add-contact response"),
            }
        }
        WorkerRequest::Queue {
            descriptor,
            request_id,
            draft,
        } => {
            let command = RuntimeIpcCommand::QueueMessage {
                request_id,
                conversation: draft.conversation,
                peer_account_id: draft.peer_account_id,
                message: draft.message,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::MessageQueued {
                    queue_id,
                    contact_id,
                    inserted,
                } => Ok(WorkerSuccess::Queued {
                    queue_id,
                    contact_id,
                    inserted,
                }),
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected the message: {message}")
                }
                _ => bail!("Runtime returned an unexpected queue response"),
            }
        }
        WorkerRequest::Refresh { descriptor } => {
            match kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::OutboxStatus).await? {
                RuntimeIpcResponse::OutboxStatus(status) => Ok(WorkerSuccess::Refreshed(status)),
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected outbox status: {message}")
                }
                _ => bail!("Runtime returned an unexpected outbox response"),
            }
        }
        WorkerRequest::ApplyDeviceDirectory {
            descriptor,
            device_list_file,
        } => {
            let command = RuntimeIpcCommand::ApplyOwnDeviceDirectory { device_list_file };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::OwnDeviceDirectoryApplied(update) => {
                    Ok(WorkerSuccess::DeviceDirectoryApplied(*update))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected the device directory: {message}")
                }
                _ => bail!("Runtime returned an unexpected device-directory response"),
            }
        }
        WorkerRequest::PublishTicket {
            descriptor,
            conversation,
            peer_account_id,
            service_base_url,
        } => {
            let command = RuntimeIpcCommand::PublishOwnTicket {
                conversation,
                peer_account_id,
                service_base_url,
                ttl_seconds: 15 * 60,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::OwnTicketPublished(publication) => {
                    Ok(WorkerSuccess::TicketPublished(*publication))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected ticket publication: {message}")
                }
                _ => bail!("Runtime returned an unexpected ticket-publication response"),
            }
        }
        WorkerRequest::RefreshContactTicket {
            descriptor,
            conversation,
            peer_account_id,
            service_base_url,
        } => {
            let command = RuntimeIpcCommand::RefreshContactTicket {
                conversation,
                peer_account_id,
                service_base_url,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::ContactTicketRefreshed(refresh) => {
                    Ok(WorkerSuccess::ContactTicketRefreshed(*refresh))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected contact-ticket refresh: {message}")
                }
                _ => bail!("Runtime returned an unexpected contact-ticket response"),
            }
        }
        WorkerRequest::ConfigureTicketAutomation {
            descriptor,
            conversation,
            peer_account_id,
            enabled,
            service_base_url,
            allow_ethernet,
            allow_wifi,
            allow_mobile,
            allow_unknown_network,
        } => {
            let command = RuntimeIpcCommand::ConfigureTicketAutomation {
                conversation,
                peer_account_id,
                enabled,
                service_base_url,
                ttl_seconds: 15 * 60,
                refresh_before_seconds: 5 * 60,
                retry_base_seconds: 5,
                retry_max_seconds: 5 * 60,
                allow_ethernet,
                allow_wifi,
                allow_mobile,
                allow_unknown_network,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::TicketAutomationConfigured(status) => {
                    Ok(WorkerSuccess::TicketAutomationConfigured(*status))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected ticket automation: {message}")
                }
                _ => bail!("Runtime returned an unexpected ticket-automation response"),
            }
        }
        WorkerRequest::TicketAutomationStatus { descriptor } => {
            match kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::TicketAutomationStatus)
                .await?
            {
                RuntimeIpcResponse::TicketAutomationStatus(statuses) => {
                    Ok(WorkerSuccess::TicketAutomationStatus(statuses))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected ticket-automation status: {message}")
                }
                _ => bail!("Runtime returned an unexpected ticket-automation status response"),
            }
        }
        WorkerRequest::MailboxCreate {
            descriptor,
            conversation,
            peer_account_id,
            peer_device_id,
            valid_for_seconds,
        } => {
            let command = RuntimeIpcCommand::CreateExactMailboxCapability {
                conversation,
                peer_account_id,
                peer_device_id,
                valid_for_seconds,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::MailboxCapabilityChanged(transition) => {
                    Ok(WorkerSuccess::MailboxChanged(*transition))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected mailbox activation: {message}")
                }
                _ => bail!("Runtime returned an unexpected mailbox-activation response"),
            }
        }
        WorkerRequest::MailboxRotate {
            descriptor,
            conversation,
            peer_account_id,
            peer_device_id,
            valid_for_seconds,
        } => {
            let command = RuntimeIpcCommand::RotateExactMailboxCapability {
                conversation,
                peer_account_id,
                peer_device_id,
                valid_for_seconds,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::MailboxCapabilityChanged(transition) => {
                    Ok(WorkerSuccess::MailboxChanged(*transition))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected mailbox rotation: {message}")
                }
                _ => bail!("Runtime returned an unexpected mailbox-rotation response"),
            }
        }
        WorkerRequest::MailboxRevoke {
            descriptor,
            conversation,
            peer_account_id,
            peer_device_id,
        } => {
            let command = RuntimeIpcCommand::RevokeMailboxCapability {
                conversation,
                peer_account_id,
                peer_device_id,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::MailboxCapabilityChanged(transition) => {
                    Ok(WorkerSuccess::MailboxChanged(*transition))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected mailbox revocation: {message}")
                }
                _ => bail!("Runtime returned an unexpected mailbox-revocation response"),
            }
        }
        WorkerRequest::MailboxStatus { descriptor } => {
            match kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::MailboxStatus).await? {
                RuntimeIpcResponse::MailboxStatus(status) => {
                    Ok(WorkerSuccess::MailboxStatus(status))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected mailbox status: {message}")
                }
                _ => bail!("Runtime returned an unexpected mailbox-status response"),
            }
        }
        WorkerRequest::Conversations { descriptor } => {
            let mut cursor = None;
            let mut conversations = Vec::new();
            for _ in 0..MAX_CONVERSATION_REFRESH_PAGES {
                let request_cursor = cursor.clone();
                let command = RuntimeIpcCommand::ConversationListPage {
                    cursor: request_cursor.clone(),
                    limit: MAX_RUNTIME_IPC_CONVERSATION_PAGE_SIZE,
                };
                match kilogram_runtime_ipc::call(&descriptor, command).await? {
                    RuntimeIpcResponse::ConversationListPage(page) => {
                        page.validate_for_request(
                            request_cursor.as_ref(),
                            MAX_RUNTIME_IPC_CONVERSATION_PAGE_SIZE,
                        )?;
                        let RuntimeIpcConversationPage {
                            conversations: page_conversations,
                            next_cursor,
                        } = page;
                        conversations.extend(page_conversations);
                        let Some(next_cursor) = next_cursor else {
                            return Ok(WorkerSuccess::Conversations(conversations));
                        };
                        cursor = Some(next_cursor);
                    }
                    RuntimeIpcResponse::Error { message } => {
                        bail!("Runtime rejected conversation list: {message}")
                    }
                    _ => bail!("Runtime returned an unexpected conversation-list response"),
                }
            }
            bail!(
                "Runtime conversation list exceeded the desktop safety limit of {} pages",
                MAX_CONVERSATION_REFRESH_PAGES
            )
        }
        WorkerRequest::History {
            descriptor,
            conversation,
            cursor,
            older,
        } => {
            let command = RuntimeIpcCommand::HistoryPage {
                conversation,
                cursor,
                limit: HISTORY_PAGE_SIZE,
            };
            match kilogram_runtime_ipc::call(&descriptor, command).await? {
                RuntimeIpcResponse::HistoryPage(page) => Ok(WorkerSuccess::History { page, older }),
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected history page: {message}")
                }
                _ => bail!("Runtime returned an unexpected history response"),
            }
        }
        WorkerRequest::Shutdown { descriptor } => {
            match kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::Shutdown).await? {
                RuntimeIpcResponse::ShutdownAccepted => Ok(WorkerSuccess::Shutdown),
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected shutdown: {message}")
                }
                _ => bail!("Runtime returned an unexpected shutdown response"),
            }
        }
    }
}

struct BootstrapView {
    recovery_phrase: Zeroizing<String>,
    account_id: String,
    device_id: String,
    account_root_dir: String,
    prekey_pool_file: String,
    root_key_protection: String,
    vault_key_protection: String,
    phrase_saved: bool,
}

struct KilogramApp {
    model: ViewModel,
    worker: Option<RuntimeWorker>,
    change_watcher: Option<RuntimeChangeWatcher>,
    pending_change_refresh: bool,
    runtime_profile_path: String,
    runtime_profile_draft: RuntimeProfileDraft,
    show_profile_editor: bool,
    runtime_executable_path: String,
    bootstrap_executable_path: String,
    bootstrap_workspace_path: String,
    bootstrap_view: Option<BootstrapView>,
    account_recovery: AccountRecoveryView,
    device_link: DeviceLinkView,
    recovery: RecoveryView,
    publication_conflict_recovery: PublicationConflictRecoveryView,
    canonical_activation: ActivationView,
    peer_reenrollment: PeerReenrollmentView,
    rollout_operator_config: Option<RolloutOperatorConfig>,
    rollout_operator: RolloutOperatorView,
    drop_target: Option<DropTarget>,
    runtime_process: Option<Child>,
    guided_stop_deadline: Option<Instant>,
    peer_reenrollment_stop_deadline: Option<Instant>,
    runtime_start_deadline: Option<Instant>,
    runtime_next_connect_attempt: Instant,
    runtime_start_error: Option<String>,
}

impl KilogramApp {
    fn new(creation_context: &eframe::CreationContext<'_>, options: DesktopOptions) -> Self {
        creation_context.egui_ctx.set_visuals(egui::Visuals::dark());
        creation_context.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(14.0, 7.0);
            style.interaction.selectable_labels = true;
        });
        let mut model = ViewModel::new(options.descriptor_path);
        model.error = options.startup_error;
        let worker = match RuntimeWorker::spawn() {
            Ok(worker) => Some(worker),
            Err(error) => {
                model.error = Some(format!("Start IPC worker: {error:#}"));
                None
            }
        };
        let change_watcher = match RuntimeChangeWatcher::spawn() {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                model.error = Some(format!("Start runtime change watcher: {error:#}"));
                None
            }
        };
        let runtime_profile_exists = options.runtime_profile_path.exists();
        let runtime_profile_draft = if runtime_profile_exists {
            RuntimeLaunchProfile::load(&options.runtime_profile_path)
                .map(|profile| RuntimeProfileDraft::from_profile(&profile))
                .unwrap_or_default()
        } else {
            RuntimeProfileDraft::default()
        };
        let rollout_operator_config = match (
            options.rollout_directory.as_ref(),
            options.trusted_policy_directory.as_ref(),
            options.release_trust_root.as_ref(),
        ) {
            (Some(rollout_directory), Some(trusted_policy_directory), Some(release_trust_root)) => {
                Some(RolloutOperatorConfig {
                    rollout_directory: rollout_directory.clone(),
                    trusted_policy_directory: trusted_policy_directory.clone(),
                    release_trust_root: release_trust_root.clone(),
                })
            }
            _ => None,
        };
        Self {
            model,
            worker,
            change_watcher,
            pending_change_refresh: false,
            runtime_profile_path: options.runtime_profile_path.display().to_string(),
            runtime_profile_draft,
            show_profile_editor: !runtime_profile_exists,
            runtime_executable_path: options.runtime_executable_path.display().to_string(),
            bootstrap_executable_path: options.bootstrap_executable_path.display().to_string(),
            bootstrap_workspace_path: PathBuf::from("kilogram-account").display().to_string(),
            bootstrap_view: None,
            account_recovery: AccountRecoveryView::default(),
            device_link: DeviceLinkView::default(),
            recovery: RecoveryView::default(),
            publication_conflict_recovery: PublicationConflictRecoveryView::default(),
            canonical_activation: ActivationView::default(),
            peer_reenrollment: PeerReenrollmentView::default(),
            rollout_operator_config,
            rollout_operator: RolloutOperatorView::default(),
            drop_target: None,
            runtime_process: None,
            guided_stop_deadline: None,
            peer_reenrollment_stop_deadline: None,
            runtime_start_deadline: None,
            runtime_next_connect_attempt: Instant::now(),
            runtime_start_error: None,
        }
    }

    fn receive_worker_responses(&mut self) {
        while let Some(response) = self.worker.as_ref().and_then(RuntimeWorker::try_receive) {
            if response.operation == Operation::Bootstrap {
                self.apply_bootstrap_response(response.result);
                continue;
            }
            if matches!(
                response.operation,
                Operation::AccountRecoveryStatus
                    | Operation::AccountRecoveryExport
                    | Operation::AccountRecoveryInspect
                    | Operation::AccountRecoveryRestore
                    | Operation::AccountRecoveryQuorumRequest
                    | Operation::AccountRecoveryQuorumListen
                    | Operation::AccountRecoveryQuorumCollect
                    | Operation::AccountRecoveryQuorumVerify
                    | Operation::RecoveryPolicyRequest
                    | Operation::RecoveryPolicyListen
                    | Operation::RecoveryPolicyCollect
                    | Operation::RecoveryPolicyCertify
                    | Operation::RecoveryPolicyInstall
                    | Operation::DeviceLinkRequest
                    | Operation::DeviceLinkInspect
                    | Operation::DeviceLinkAuthorize
                    | Operation::DeviceLinkAccept
                    | Operation::DeviceRemove
                    | Operation::RecoveryApprove
                    | Operation::RecoveryRun
                    | Operation::RecoveryCancel
                    | Operation::RecoveryReconcile
                    | Operation::PublicationChannelRotate
                    | Operation::PublicationConflictRequest
                    | Operation::PublicationConflictApply
            ) {
                self.apply_wizard_response(response);
                continue;
            }
            if matches!(
                response.operation,
                Operation::CanonicalActivationStatus
                    | Operation::CanonicalActivationPlan
                    | Operation::CanonicalActivationApply
            ) {
                self.apply_canonical_activation_response(response);
                continue;
            }
            if response.operation == Operation::RolloutOperatorStatus {
                self.apply_rollout_operator_response(response);
                continue;
            }
            if matches!(
                response.operation,
                Operation::PeerReenrollmentStatus
                    | Operation::PeerReenrollmentPlan
                    | Operation::PeerReenrollmentApprove
                    | Operation::PeerReenrollmentAccept
            ) {
                self.apply_peer_reenrollment_response(response);
                continue;
            }
            if response.operation == Operation::Shutdown
                && self.model.pending != Some(Operation::Shutdown)
            {
                // The desktop may observe the owned child exit before the
                // asynchronous shutdown acknowledgement reaches this queue.
                // Actual process exit is stronger evidence; never let the
                // stale acknowledgement clear a newer offline inspection.
                continue;
            }
            if response.operation == Operation::Connect
                && self.runtime_process.is_none()
                && matches!(
                    self.canonical_activation.guided_restart,
                    GuidedRestartState::Failed {
                        expectation: Some(_),
                        ..
                    }
                )
            {
                // A response that raced with fresh-process exit/termination
                // cannot complete the retained exact verification. Only the
                // explicit retry may consume the expectation.
                self.model.pending = None;
                continue;
            }
            if response.operation == Operation::Connect
                && response.result.is_err()
                && self.runtime_start_deadline.is_some()
            {
                self.runtime_start_error = response.result.err();
                self.model.pending = None;
                self.model.connection = ConnectionState::Connecting;
                self.runtime_next_connect_attempt = Instant::now() + RUNTIME_START_RETRY_INTERVAL;
                continue;
            }
            let guided_restart_verified = if response.operation == Operation::Connect
                && self.canonical_activation.awaiting_fresh_runtime()
            {
                match &response.result {
                    Ok(WorkerSuccess::Connected {
                        device_id,
                        canonical_runtime_status,
                        ..
                    }) => match self
                        .canonical_activation
                        .verify_fresh_runtime(device_id, canonical_runtime_status)
                    {
                        Ok(()) => true,
                        Err(error) => {
                            self.abort_guided_fresh_runtime(format!(
                                "Post-restart activation verification failed: {error:#}"
                            ));
                            continue;
                        }
                    },
                    Ok(_) => {
                        self.abort_guided_fresh_runtime(
                            "Post-restart activation verification received an unexpected worker result"
                                .to_owned(),
                        );
                        continue;
                    }
                    Err(_) => false,
                }
            } else {
                false
            };
            if response.operation == Operation::Shutdown
                && matches!(
                    self.canonical_activation.guided_restart,
                    GuidedRestartState::StopRequested
                )
                && let Err(message) = &response.result
            {
                self.canonical_activation.fail_guided(message.clone());
                self.guided_stop_deadline = None;
            }
            if response.operation == Operation::Shutdown
                && matches!(
                    self.peer_reenrollment.restart,
                    PeerReenrollmentRestartState::StopRequested(_)
                )
                && let Err(message) = &response.result
            {
                self.peer_reenrollment.fail_restart(message.clone());
                self.peer_reenrollment_stop_deadline = None;
            }
            let connected = matches!(response.result, Ok(WorkerSuccess::Connected { .. }));
            let shutdown = matches!(response.result, Ok(WorkerSuccess::Shutdown));
            let conversations = matches!(response.result, Ok(WorkerSuccess::Conversations(_)));
            let initial_history = matches!(
                response.result,
                Ok(WorkerSuccess::History { older: false, .. })
            );
            let refreshed = matches!(response.result, Ok(WorkerSuccess::Refreshed(_)));
            let mailbox_changed = matches!(response.result, Ok(WorkerSuccess::MailboxChanged(_)));
            let mailbox_status = matches!(response.result, Ok(WorkerSuccess::MailboxStatus(_)));
            let canonical_runtime_status = matches!(
                response.result,
                Ok(WorkerSuccess::CanonicalRuntimeStatus(_))
            );
            self.model.apply(response);
            let peer_verification_contact = if connected {
                self.peer_reenrollment.contact_for_fresh_verification()
            } else {
                None
            };
            if connected {
                self.runtime_start_deadline = None;
                self.runtime_start_error = None;
                self.start_change_subscription();
                if guided_restart_verified {
                    self.model.notice = Some(
                        "Guided activation completed: the fresh authenticated runtime loaded the exact applied signed policy."
                            .to_owned(),
                    );
                }
            }
            if shutdown {
                self.stop_change_subscription();
            }
            if peer_verification_contact.is_some() {
                self.start_peer_reenrollment_status();
                if self.model.pending != Some(Operation::PeerReenrollmentStatus) {
                    let detail = self.model.error.clone().unwrap_or_else(|| {
                        "Failed to submit exact peer re-enrollment verification".to_owned()
                    });
                    self.abort_peer_reenrollment_fresh_runtime(detail);
                }
            } else if connected {
                self.start_conversations();
            } else if conversations && self.model.selected_contact_id.is_some() {
                self.start_history(false);
            } else if conversations || initial_history {
                self.start_refresh();
            } else if refreshed || mailbox_changed {
                self.start_mailbox_status();
            } else if mailbox_status {
                self.start_canonical_runtime_status();
            } else if canonical_runtime_status {
                self.start_peer_capability_status();
            }
        }
    }

    fn apply_canonical_activation_response(&mut self, response: WorkerResponse) {
        self.model.pending = None;
        let success = match response.result {
            Ok(success) => success,
            Err(message) => {
                if matches!(
                    self.canonical_activation.guided_restart,
                    GuidedRestartState::InspectingOffline | GuidedRestartState::Applying
                ) {
                    self.canonical_activation.fail_guided(message.clone());
                }
                self.model.error = Some(message);
                return;
            }
        };
        let mut start_fresh_runtime = false;
        let result: Result<()> = match success {
            WorkerSuccess::CanonicalActivationStatus(status) => {
                self.canonical_activation.install_status(status);
                self.canonical_activation.mark_offline_status_ready();
                self.model.notice = Some(
                    "Canonical sync v2 activation authority verified from the stopped runtime state."
                        .to_owned(),
                );
                Ok(())
            }
            WorkerSuccess::CanonicalActivationPlanned(plan) => {
                let record_id = plan.record_id.clone();
                self.canonical_activation.install_plan(plan).map(|()| {
                    self.model.notice = Some(format!(
                        "Exact activation successor {} planned without writing state.",
                        compact_id(&record_id)
                    ));
                })
            }
            WorkerSuccess::CanonicalActivationApplied {
                applied_record_id,
                status,
            } => self
                .canonical_activation
                .expect_fresh_runtime(&applied_record_id, &status)
                .map(|()| {
                    self.canonical_activation.install_status(status);
                    self.model.notice = Some(format!(
                        "Signed activation successor {} applied. Starting a fresh runtime for exact verification.",
                        compact_id(&applied_record_id)
                    ));
                    start_fresh_runtime = true;
                }),
            _ => Err(anyhow::anyhow!(
                "non-activation worker result reached activation ceremony"
            )),
        };
        match result {
            Ok(()) => {
                self.model.error = None;
                if start_fresh_runtime {
                    self.start_runtime();
                }
            }
            Err(error) => {
                let detail = format!("Activation ceremony: {error:#}");
                if matches!(
                    self.canonical_activation.guided_restart,
                    GuidedRestartState::InspectingOffline
                        | GuidedRestartState::Applying
                        | GuidedRestartState::StartingFresh(_)
                ) {
                    self.canonical_activation.fail_guided(detail.clone());
                }
                self.model.error = Some(detail);
            }
        }
    }

    fn apply_rollout_operator_response(&mut self, response: WorkerResponse) {
        self.model.pending = None;
        match response.result {
            Ok(WorkerSuccess::RolloutOperatorStatus(status)) => {
                self.rollout_operator.status = Some(status);
                self.model.notice = Some(
                    "Release trust and rollout controller state inspected without writing state."
                        .to_owned(),
                );
                self.model.error = None;
            }
            Ok(_) => {
                self.model.error =
                    Some("Unexpected result reached the rollout observer".to_owned());
            }
            Err(message) => self.model.error = Some(message),
        }
    }

    fn apply_peer_reenrollment_response(&mut self, response: WorkerResponse) {
        self.model.pending = None;
        let operation = response.operation;
        let verifying_fresh_runtime = matches!(
            self.peer_reenrollment.restart,
            PeerReenrollmentRestartState::Verifying(_)
        );
        let result: Result<()> = (|| {
            let WorkerSuccess::PeerReenrollmentStatus {
                descriptor_binding,
                response,
            } = response.result.map_err(anyhow::Error::msg)?
            else {
                bail!("Unexpected result reached the peer re-enrollment ceremony");
            };
            self.peer_reenrollment
                .install_response(descriptor_binding, *response)?;
            self.model.connection = ConnectionState::Connected;
            if matches!(
                self.peer_reenrollment.restart,
                PeerReenrollmentRestartState::Verifying(_)
            ) {
                self.peer_reenrollment.verify_fresh_response()?;
                self.model.notice = Some(
                    "Peer re-enrollment completed: the fresh authenticated runtime consumed the exact durable transition."
                        .to_owned(),
                );
            } else {
                self.model.notice = Some(
                    match operation {
                        Operation::PeerReenrollmentStatus => {
                            "Signed peer re-enrollment status refreshed"
                        }
                        Operation::PeerReenrollmentPlan => {
                            "Exact peer re-enrollment plan prepared; explicit approval is still required"
                        }
                        Operation::PeerReenrollmentApprove => {
                            "Exact peer re-enrollment plan approved durably; acceptance remains separate"
                        }
                        Operation::PeerReenrollmentAccept => {
                            "Peer re-enrollment acceptance committed; a fresh runtime is required"
                        }
                        _ => unreachable!("filtered peer re-enrollment operation"),
                    }
                    .to_owned(),
                );
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.model.error = None,
            Err(error) => {
                let detail = format!("Peer re-enrollment ceremony: {error:#}");
                if verifying_fresh_runtime {
                    self.abort_peer_reenrollment_fresh_runtime(detail);
                    return;
                } else if self.peer_reenrollment.awaiting_fresh_runtime() {
                    self.peer_reenrollment.fail_restart(detail.clone());
                } else {
                    self.peer_reenrollment.invalidate_runtime_context();
                }
                self.model.error = Some(detail);
            }
        }
    }

    fn apply_wizard_response(&mut self, response: WorkerResponse) {
        self.model.pending = None;
        let success = match response.result {
            Ok(success) => success,
            Err(message) => {
                self.model.error = Some(message);
                return;
            }
        };
        match success {
            WorkerSuccess::AccountRecoveryStatus(output) => {
                let current = output.checkpoint_state == "current";
                self.account_recovery.checkpoint_status = Some(*output);
                self.model.notice = Some(if current {
                    "The current Account Root state has an exact successful recovery export."
                        .to_owned()
                } else {
                    "Recovery export update required: Root authority or membership state differs from the last successful export."
                        .to_owned()
                });
            }
            WorkerSuccess::AccountRecoveryExported(output) => {
                self.account_recovery.package_input_file =
                    output.package_file.display().to_string();
                self.account_recovery.witness_input_file =
                    output.witness_file.display().to_string();
                self.account_recovery.exported = Some(*output);
                self.account_recovery.checkpoint_status = None;
                self.account_recovery.inspected = None;
                self.account_recovery.inspected_package_file = None;
                self.account_recovery.inspected_witness_file = None;
                self.account_recovery.confirm_latest_witness = false;
                self.account_recovery.restored = None;
                self.account_recovery.quorum_request = None;
                self.account_recovery.quorum_listener = None;
                self.account_recovery.quorum_collection = None;
                self.account_recovery.quorum_verification = None;
                self.model.notice = Some(
                    "Root authority package exported. Store the latest witness independently, then inspect the exact pair before recovery."
                        .to_owned(),
                );
            }
            WorkerSuccess::AccountRecoveryInspected {
                package_file,
                witness_file,
                output,
            } => {
                self.account_recovery.inspected_package_file = Some(package_file);
                self.account_recovery.inspected_witness_file = Some(witness_file);
                self.account_recovery.inspected = Some(*output);
                self.account_recovery.confirm_latest_witness = false;
                self.account_recovery.restored = None;
                self.account_recovery.quorum_request = None;
                self.account_recovery.quorum_listener = None;
                self.account_recovery.quorum_collection = None;
                self.account_recovery.quorum_verification = None;
                self.model.notice = Some(
                    "Package signatures and exact witness binding verified. Confirm independently that this witness is the newest before entering the phrase."
                        .to_owned(),
                );
            }
            WorkerSuccess::AccountRecoveryRestored(output) => {
                let account_id = output.account_id.clone();
                let account_root_dir = output
                    .account_root_dir
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
                self.account_recovery.recovery_phrase.zeroize();
                self.account_recovery.confirm_latest_witness = false;
                self.account_recovery.account_root_dir = account_root_dir.clone();
                self.account_recovery.checkpoint_status = None;
                self.device_link.account_id = account_id;
                self.device_link.account_root_dir = account_root_dir.clone();
                self.device_link.removal.account_root_dir = account_root_dir;
                self.account_recovery.restored = Some(*output);
                self.model.notice = Some(
                    "Account Root restored. Use the device-link ceremony below to enroll a new device, then recover message history separately."
                        .to_owned(),
                );
            }
            WorkerSuccess::AccountRecoveryQuorumRequested(output) => {
                self.account_recovery.quorum_request_file =
                    output.request_file.display().to_string();
                self.account_recovery.quorum_listener = None;
                self.account_recovery.quorum_collection = None;
                self.account_recovery.quorum_verification = None;
                self.account_recovery.quorum_request = Some(*output);
                self.model.notice = Some(
                    "Fresh exact-roster recovery request created. Obtain one signed ticket from each available current device, then collect their approvals before expiry."
                        .to_owned(),
                );
            }
            WorkerSuccess::AccountRecoveryQuorumListened(output) => {
                self.model.notice = Some(format!(
                    "Approval delivered over {} and was committed before ticket publication.",
                    output.transport_path
                ));
                self.account_recovery.quorum_listener = Some(*output);
            }
            WorkerSuccess::AccountRecoveryQuorumCollected(output) => {
                self.account_recovery.quorum_approval_files = output
                    .transports
                    .iter()
                    .map(|transport| transport.approval_file.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.model.notice = Some(format!(
                    "Recovery freshness claim: {} ({}/{} approvals).",
                    output.freshness_claim, output.observed_approvals, output.required_approvals
                ));
                self.account_recovery.quorum_verification = None;
                self.account_recovery.quorum_collection = Some(*output);
            }
            WorkerSuccess::AccountRecoveryQuorumVerified(output) => {
                self.model.notice = Some(format!(
                    "Recovery freshness claim: {} ({}/{} approvals).",
                    output.freshness_claim, output.observed_approvals, output.required_approvals
                ));
                self.account_recovery.quorum_verification = Some(*output);
            }
            WorkerSuccess::RecoveryPolicyRequested(output) => {
                self.device_link.policy.request_file = output.request_file.display().to_string();
                self.device_link.policy.listener = None;
                self.device_link.policy.collection = None;
                self.device_link.policy.certificate = None;
                self.device_link.policy.installation = None;
                self.model.notice = Some(format!(
                    "Recovery-policy transition {}→{} created; Root roster mutation is already complete, but policy activation and history recovery are still separate.",
                    output.old_epoch, output.new_epoch
                ));
                self.device_link.policy.request = Some(*output);
            }
            WorkerSuccess::RecoveryPolicyListened(output) => {
                self.model.notice = Some(format!(
                    "Policy approval delivered over {} after its DB-primary anti-equivocation head was committed.",
                    output.transport_path
                ));
                self.device_link.policy.listener = Some(*output);
            }
            WorkerSuccess::RecoveryPolicyCollected(output) => {
                self.device_link.policy.approval_files = output
                    .transports
                    .iter()
                    .map(|transport| transport.approval_file.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.model.notice = Some(format!(
                    "Policy approvals: old {}/{}, new {}/{}. Activation is {}.",
                    output.old_observed_approvals,
                    output.old_required_approvals,
                    output.new_observed_approvals,
                    output.new_required_approvals,
                    if output.joint_majority_satisfied {
                        "ready for certification"
                    } else {
                        "still incomplete"
                    }
                ));
                self.device_link.policy.certificate = None;
                self.device_link.policy.installation = None;
                self.device_link.policy.collection = Some(*output);
            }
            WorkerSuccess::RecoveryPolicyCertified(output) => {
                self.device_link.policy.certificate_file =
                    output.certificate_file.display().to_string();
                self.model.notice = Some(
                    "Joint old/new-majority policy certificate created. Activation remains incomplete on each device until that device installs it."
                        .to_owned(),
                );
                self.device_link.policy.installation = None;
                self.device_link.policy.certificate = Some(*output);
            }
            WorkerSuccess::RecoveryPolicyInstalled(output) => {
                self.model.notice = Some(format!(
                    "Recovery policy epoch {} activated on this device. Message-history recovery has not been implied or started.",
                    output.policy_epoch
                ));
                self.device_link.policy.installation = Some(*output);
            }
            WorkerSuccess::DeviceLinkRequested(output) => {
                self.device_link.joining_workspace = output.workspace_dir.display().to_string();
                self.device_link.owner_request_file = output.request_file.display().to_string();
                self.device_link.request = Some(*output);
                self.device_link.accepted = None;
                self.model.notice = Some(
                    "Device-link request created. Compare its 12-digit SAS through an independent channel."
                        .to_owned(),
                );
            }
            WorkerSuccess::DeviceLinkInspected {
                request_file,
                output,
            } => {
                self.device_link.confirmed_sas.clear();
                self.device_link.inspected_request_file = Some(request_file);
                self.device_link.inspected = Some(*output);
                self.model.notice = Some(
                    "Signed request inspected. Confirm the large SAS with the new-device owner before authorizing."
                        .to_owned(),
                );
            }
            WorkerSuccess::DeviceLinkAuthorized(output) => {
                self.account_recovery.account_root_dir = self.device_link.account_root_dir.clone();
                self.device_link.removal.account_root_dir =
                    self.device_link.account_root_dir.clone();
                self.account_recovery.checkpoint_status = None;
                self.account_recovery.exported = None;
                self.account_recovery.inspected = None;
                self.account_recovery.confirm_latest_witness = false;
                self.device_link.response_input_file = output.response_file.display().to_string();
                self.device_link.authorization = Some(*output);
                self.model.notice = Some(
                    "Exact device enrolled at the Root layer. Return the encrypted response, export the new Root checkpoint, and complete recovery-policy activation separately."
                        .to_owned(),
                );
            }
            WorkerSuccess::DeviceLinkAccepted(output) => {
                let workspace = output.workspace_dir.clone();
                self.canonical_activation = ActivationView::default();
                self.peer_reenrollment.invalidate_runtime_context();
                self.runtime_profile_path =
                    workspace.join("runtime.launch.json").display().to_string();
                self.model.descriptor_path =
                    workspace.join("runtime.ipc.json").display().to_string();
                self.runtime_profile_draft.state_dir = output.state_dir.display().to_string();
                self.runtime_profile_draft
                    .allowed_requester_account_id
                    .clear();
                self.runtime_profile_draft.device_list_file =
                    output.device_list_file.display().to_string();
                self.runtime_profile_draft.ticket_file = workspace
                    .join("public")
                    .join("runtime.ticket")
                    .display()
                    .to_string();
                self.runtime_profile_draft.ipc_file =
                    workspace.join("runtime.ipc.json").display().to_string();
                self.runtime_profile_draft.volunteer_storage_data_dir =
                    workspace.join("volunteer-storage").display().to_string();
                self.runtime_profile_draft.peer_prekey_pool_files.clear();
                self.show_profile_editor = true;
                self.recovery.state_dir = output.state_dir.display().to_string();
                self.device_link.accepted = Some(*output);
                self.device_link.policy.install_state_dir = self.recovery.state_dir.clone();
                self.model.notice = Some(
                    "Device link accepted at the Root layer. Runtime setup, recovery-policy activation, and message-history recovery are three separate remaining states."
                        .to_owned(),
                );
            }
            WorkerSuccess::DeviceRemoved(output) => {
                self.model.device_directory_update = None;
                if let Some(installed) = self.device_link.policy.installation.as_ref() {
                    self.device_link.policy.old_epoch = installed.policy_epoch.to_string();
                    self.device_link.policy.previous_transition_id =
                        installed.latest_transition_id.clone();
                }
                self.device_link.policy.old_package_file =
                    output.before_package_file.display().to_string();
                self.device_link.policy.old_witness_file =
                    output.before_witness_file.display().to_string();
                self.device_link.policy.new_package_file =
                    output.after_package_file.display().to_string();
                self.device_link.policy.new_witness_file =
                    output.after_witness_file.display().to_string();
                self.runtime_profile_draft.device_list_file =
                    output.device_list_file.display().to_string();
                self.device_link.account_root_dir =
                    self.device_link.removal.account_root_dir.clone();
                self.account_recovery.account_root_dir =
                    self.device_link.removal.account_root_dir.clone();
                self.account_recovery.checkpoint_status = None;
                self.account_recovery.package_input_file =
                    output.after_package_file.display().to_string();
                self.account_recovery.witness_input_file =
                    output.after_witness_file.display().to_string();
                self.device_link.policy.request = None;
                self.device_link.policy.listener = None;
                self.device_link.policy.collection = None;
                self.device_link.policy.certificate = None;
                self.device_link.policy.installation = None;
                self.device_link.removal.confirmed_device_id.clear();
                self.model.notice = Some(format!(
                    "Device {} removed. Exact policy inputs are filled in; runtime directory refresh and ratchet-session retirement remain explicit pending steps.",
                    compact_id(&output.removed_device_id)
                ));
                self.device_link.removal.result = Some(*output);
            }
            WorkerSuccess::RecoveryApproved { plan_file, output } => {
                let index = self.upsert_recovery_plan(plan_file);
                self.recovery.plans[index].update(&output);
                self.recovery.selected = Some(index);
                self.model.notice = Some(
                    "Recipient-signed recovery plan approved. The source must publish its matching recovery link while a bounded attempt runs."
                        .to_owned(),
                );
            }
            WorkerSuccess::RecoveryRan { plan_file, output } => {
                let index = self.upsert_recovery_plan(plan_file);
                self.recovery.plans[index].update(&output);
                self.recovery.selected = Some(index);
                self.model.notice = Some(format!(
                    "Recovery attempt finished with status {}",
                    output.status()
                ));
            }
            WorkerSuccess::RecoveryCancelled { plan_file, output } => {
                let index = self.upsert_recovery_plan(plan_file);
                self.recovery.plans[index].update(&output);
                self.recovery.selected = Some(index);
                self.recovery.confirm_cancel = false;
                self.model.notice = Some("Recovery plan cancellation signed and stored".to_owned());
            }
            WorkerSuccess::RecoveryReconciled(output) => {
                let agreement = output
                    .field("source_claim_agreement")
                    .unwrap_or("incomplete");
                self.model.notice = Some(format!(
                    "Recovery sources reconciled: {agreement}; global completeness remains unproven"
                ));
                self.recovery.reconciliation = Some(output);
            }
            WorkerSuccess::PublicationChannelRotated(output) => {
                self.publication_conflict_recovery.rotation_result = Some(output);
                self.model.notice = Some(
                    "Publication channel rotated by the live runtime. No restart is required; transfer or publish the refreshed ticket to the peer."
                        .to_owned(),
                );
            }
            WorkerSuccess::PublicationConflictRequested(output) => {
                self.publication_conflict_recovery.apply_result = None;
                self.publication_conflict_recovery.request_result = Some(output);
                self.model.notice = Some(
                    "Offline Root request created. Copy only the .pcrq artifact to the isolated Root signer."
                        .to_owned(),
                );
            }
            WorkerSuccess::PublicationConflictApplied(output) => {
                self.publication_conflict_recovery.apply_result = Some(output);
                self.model.notice = Some(
                    "Root-authorized replacement applied live. No restart is required; exact-current sibling devices can receive the same resolution automatically."
                        .to_owned(),
                );
            }
            _ => {
                self.model.error = Some(format!(
                    "Operation {:?} returned an unexpected wizard result",
                    response.operation
                ));
                return;
            }
        }
        self.model.error = None;
    }

    fn upsert_recovery_plan(&mut self, path: PathBuf) -> usize {
        if let Some(index) = self
            .recovery
            .plans
            .iter()
            .position(|plan| plan.path == path)
        {
            index
        } else {
            self.recovery.plans.push(RecoveryPlanView::new(path));
            self.recovery.plans.len() - 1
        }
    }

    fn apply_bootstrap_response(&mut self, result: Result<WorkerSuccess, String>) {
        self.model.pending = None;
        let mut output = match result {
            Ok(WorkerSuccess::Bootstrapped(output)) => *output,
            Ok(_) => {
                self.model.error =
                    Some("Bootstrap helper returned an unexpected result".to_owned());
                return;
            }
            Err(message) => {
                self.model.error = Some(message);
                return;
            }
        };
        let workspace = output.workspace_dir().clone();
        let runtime_profile = workspace.join("runtime.launch.json");
        let runtime_ticket = workspace.join("public").join("runtime.ticket");
        let runtime_ipc = workspace.join("runtime.ipc.json");
        self.runtime_profile_path = runtime_profile.display().to_string();
        self.model.descriptor_path = runtime_ipc.display().to_string();
        self.peer_reenrollment.invalidate_runtime_context();
        self.runtime_profile_draft.state_dir = output.state_dir().display().to_string();
        self.runtime_profile_draft
            .allowed_requester_account_id
            .clear();
        self.runtime_profile_draft.device_list_file =
            output.device_list_file().display().to_string();
        self.runtime_profile_draft.peer_prekey_pool_files.clear();
        self.runtime_profile_draft.ticket_file = runtime_ticket.display().to_string();
        self.runtime_profile_draft.ipc_file = runtime_ipc.display().to_string();
        self.runtime_profile_draft.volunteer_storage_data_dir =
            workspace.join("volunteer-storage").display().to_string();
        self.show_profile_editor = true;
        self.bootstrap_workspace_path = workspace.display().to_string();
        self.account_recovery.account_root_dir = output.account_root_dir().display().to_string();
        self.device_link.removal.account_root_dir = output.account_root_dir().display().to_string();
        self.account_recovery.checkpoint_status = None;
        self.account_recovery.exported = None;
        self.account_recovery.inspected = None;
        self.bootstrap_view = Some(BootstrapView {
            recovery_phrase: Zeroizing::new(output.take_recovery_phrase()),
            account_id: output.account_id().to_string(),
            device_id: output.device_id().to_string(),
            account_root_dir: output.account_root_dir().display().to_string(),
            prekey_pool_file: output.prekey_pool_file().display().to_string(),
            root_key_protection: output.root_key_protection().to_owned(),
            vault_key_protection: output.vault_key_protection().to_owned(),
            phrase_saved: false,
        });
        self.model.notice = Some(
            "Account and first device created. Save the recovery phrase before continuing."
                .to_owned(),
        );
        self.model.error = None;
    }

    fn start_change_subscription(&mut self) {
        let Some(watcher) = self.change_watcher.as_ref() else {
            return;
        };
        match self.model.descriptor() {
            Ok(descriptor) => {
                if let Err(error) = watcher.subscribe(descriptor) {
                    self.model.error = Some(format!("Subscribe to runtime changes: {error:#}"));
                }
            }
            Err(error) => self.model.error = Some(format!("{error:#}")),
        }
    }

    fn stop_change_subscription(&mut self) {
        if let Some(watcher) = self.change_watcher.as_ref() {
            watcher.stop();
        }
        self.pending_change_refresh = false;
    }

    fn receive_change_events(&mut self) {
        while let Some(event) = self
            .change_watcher
            .as_ref()
            .and_then(RuntimeChangeWatcher::try_receive)
        {
            match event {
                ChangeWatcherEvent::Changed => self.pending_change_refresh = true,
                ChangeWatcherEvent::Unavailable(message) => {
                    if self.model.connection == ConnectionState::Connected {
                        self.model.error = Some(message);
                    }
                }
            }
        }
        if self.pending_change_refresh
            && self.model.connection == ConnectionState::Connected
            && self.model.pending.is_none()
        {
            self.pending_change_refresh = false;
            self.start_conversations();
        }
    }

    fn submit(&mut self, operation: Operation, request: WorkerRequest) {
        let Some(worker) = self.worker.as_ref() else {
            self.model
                .fail(operation, "Runtime IPC worker is unavailable".to_owned());
            return;
        };
        match worker.send(request) {
            Ok(()) => self.model.begin(operation),
            Err(error) => self.model.fail(operation, format!("{error:#}")),
        }
    }

    fn start_connect(&mut self) {
        if self.model.connection == ConnectionState::Disconnected
            && !self.peer_reenrollment.awaiting_fresh_runtime()
        {
            self.peer_reenrollment.invalidate_runtime_context();
        }
        match self.model.descriptor() {
            Ok(descriptor) => {
                self.submit(Operation::Connect, WorkerRequest::Connect { descriptor })
            }
            Err(error) => self.model.fail(Operation::Connect, format!("{error:#}")),
        }
    }

    fn start_bootstrap(&mut self) {
        if self.runtime_process.is_some() || self.model.connection != ConnectionState::Disconnected
        {
            self.model.error = Some("Stop or disconnect the runtime before bootstrap".to_owned());
            return;
        }
        let result: Result<WorkerRequest> = (|| {
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let workspace =
                absolute_new_directory_path(&self.bootstrap_workspace_path, "Account workspace")?;
            Ok(WorkerRequest::Bootstrap {
                executable,
                workspace,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::Bootstrap, request),
            Err(error) => self.model.fail(Operation::Bootstrap, format!("{error:#}")),
        }
    }

    fn require_offline_wizard(&self) -> Result<()> {
        ensure!(
            self.runtime_process.is_none()
                && self.model.connection == ConnectionState::Disconnected,
            "Stop or disconnect the runtime before changing enrolled-device or recovery state"
        );
        ensure!(
            self.model.pending.is_none(),
            "Another operation is still running"
        );
        Ok(())
    }

    fn start_account_recovery_status(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let account_root_dir = canonical_input_path(
                &self.account_recovery.account_root_dir,
                "Account Root directory",
                false,
            )?;
            Ok(WorkerRequest::AccountRecoveryStatus {
                executable,
                account_root_dir,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::AccountRecoveryStatus, request),
            Err(error) => self
                .model
                .fail(Operation::AccountRecoveryStatus, format!("{error:#}")),
        }
    }

    fn start_account_recovery_export(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let account_root_dir = canonical_input_path(
                &self.account_recovery.account_root_dir,
                "Account Root directory",
                false,
            )?;
            let package_file = absolute_output_path(
                &self.account_recovery.package_output_file,
                "Account Root recovery package",
                &account_root_dir,
            )?;
            let witness_file = absolute_output_path(
                &self.account_recovery.witness_output_file,
                "Account Root recovery witness",
                &account_root_dir,
            )?;
            ensure!(
                package_file != witness_file,
                "Recovery package and witness paths must differ"
            );
            ensure!(
                !package_file.exists() && !witness_file.exists(),
                "Recovery export outputs must be new files"
            );
            Ok(WorkerRequest::AccountRecoveryExport {
                executable,
                account_root_dir,
                package_file,
                witness_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::AccountRecoveryExport, request),
            Err(error) => self
                .model
                .fail(Operation::AccountRecoveryExport, format!("{error:#}")),
        }
    }

    fn start_account_recovery_inspect(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let package_file = canonical_nonsymlink_input_file(
                &self.account_recovery.package_input_file,
                "Account Root recovery package",
            )?;
            let witness_file = canonical_nonsymlink_input_file(
                &self.account_recovery.witness_input_file,
                "Account Root recovery witness",
            )?;
            ensure!(
                package_file != witness_file,
                "Recovery package and witness paths must differ"
            );
            Ok(WorkerRequest::AccountRecoveryInspect {
                executable,
                package_file,
                witness_file,
            })
        })();
        match result {
            Ok(request) => {
                self.account_recovery.recovery_phrase.zeroize();
                self.account_recovery.inspected = None;
                self.account_recovery.inspected_package_file = None;
                self.account_recovery.inspected_witness_file = None;
                self.account_recovery.confirm_latest_witness = false;
                self.submit(Operation::AccountRecoveryInspect, request);
            }
            Err(error) => self
                .model
                .fail(Operation::AccountRecoveryInspect, format!("{error:#}")),
        }
    }

    fn start_account_recovery_restore(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let inspected = self
                .account_recovery
                .inspected
                .as_ref()
                .context("Inspect the exact recovery package and witness before restore")?;
            let quorum_majority = self
                .account_recovery
                .quorum_collection
                .as_ref()
                .is_some_and(|report| {
                    report.majority_satisfied && report.package_id == inspected.package_id
                })
                || self
                    .account_recovery
                    .quorum_verification
                    .as_ref()
                    .is_some_and(|report| {
                        report.majority_satisfied && report.package_id == inspected.package_id
                    });
            ensure!(
                quorum_majority || self.account_recovery.confirm_latest_witness,
                "Collect and verify a current-device majority, or explicitly confirm the reduced-assurance offline fallback"
            );
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let package_file = canonical_nonsymlink_input_file(
                &self.account_recovery.package_input_file,
                "Account Root recovery package",
            )?;
            let witness_file = canonical_nonsymlink_input_file(
                &self.account_recovery.witness_input_file,
                "Account Root recovery witness",
            )?;
            ensure!(
                self.account_recovery.inspected_package_file.as_ref() == Some(&package_file)
                    && self.account_recovery.inspected_witness_file.as_ref() == Some(&witness_file),
                "Recovery artifacts changed after inspection; inspect them again"
            );
            let recovery_phrase =
                AccountRecoveryPhrase::parse(self.account_recovery.recovery_phrase.trim())
                    .context("Recovery phrase is invalid")?;
            ensure!(
                recovery_phrase.account_id()?.to_string() == inspected.account_id,
                "Recovery phrase belongs to a different Account ID"
            );
            let account_root_dir = absolute_new_directory_path(
                &self.account_recovery.restore_root_dir,
                "Restored Account Root directory",
            )?;
            Ok(WorkerRequest::AccountRecoveryRestore {
                executable,
                account_root_dir,
                package_file,
                witness_file,
                expected_package_id: inspected.package_id.clone(),
                expected_authority_revision: inspected.authority_revision,
                recovery_phrase,
            })
        })();
        match result {
            Ok(request) => {
                self.account_recovery.recovery_phrase.zeroize();
                self.submit(Operation::AccountRecoveryRestore, request);
            }
            Err(error) => self
                .model
                .fail(Operation::AccountRecoveryRestore, format!("{error:#}")),
        }
    }

    fn start_account_recovery_quorum_request(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let package_file = canonical_nonsymlink_input_file(
                &self.account_recovery.package_input_file,
                "Account Root recovery package",
            )?;
            let witness_file = canonical_nonsymlink_input_file(
                &self.account_recovery.witness_input_file,
                "Account Root recovery witness",
            )?;
            ensure!(
                self.account_recovery.inspected_package_file.as_ref() == Some(&package_file)
                    && self.account_recovery.inspected_witness_file.as_ref() == Some(&witness_file),
                "Inspect the exact package and witness before creating a quorum request"
            );
            let request_file = absolute_artifact_output_path(
                &self.account_recovery.quorum_request_file,
                "Recovery quorum request",
            )?;
            ensure!(
                !request_file.exists(),
                "Quorum request output must be a new file"
            );
            Ok(WorkerRequest::AccountRecoveryQuorumRequest {
                executable,
                package_file,
                witness_file,
                request_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::AccountRecoveryQuorumRequest, request),
            Err(error) => self.model.fail(
                Operation::AccountRecoveryQuorumRequest,
                format!("{error:#}"),
            ),
        }
    }

    fn start_account_recovery_quorum_listen(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let state_dir = canonical_input_path(
                &self.account_recovery.quorum_state_dir,
                "Approving device state",
                false,
            )?;
            let request_file = canonical_nonsymlink_input_file(
                &self.account_recovery.quorum_request_file,
                "Recovery quorum request",
            )?;
            let ticket_file = absolute_output_path(
                &self.account_recovery.quorum_ticket_output_file,
                "Recovery approval ticket",
                &state_dir,
            )?;
            ensure!(
                !ticket_file.exists(),
                "Approval ticket output must be a new file"
            );
            Ok(WorkerRequest::AccountRecoveryQuorumListen {
                executable,
                state_dir,
                request_file,
                ticket_file,
                route_policy: self.account_recovery.quorum_route_policy,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::AccountRecoveryQuorumListen, request),
            Err(error) => self
                .model
                .fail(Operation::AccountRecoveryQuorumListen, format!("{error:#}")),
        }
    }

    fn start_account_recovery_quorum_collect(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let request_file = canonical_nonsymlink_input_file(
                &self.account_recovery.quorum_request_file,
                "Recovery quorum request",
            )?;
            let ticket_files = canonical_artifact_lines(
                &self.account_recovery.quorum_ticket_files,
                "Recovery approval ticket",
            )?;
            let approval_directory = absolute_directory_target(
                &self.account_recovery.quorum_approval_directory,
                "Recovery approval directory",
            )?;
            Ok(WorkerRequest::AccountRecoveryQuorumCollect {
                executable,
                request_file,
                ticket_files,
                approval_directory,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::AccountRecoveryQuorumCollect, request),
            Err(error) => self.model.fail(
                Operation::AccountRecoveryQuorumCollect,
                format!("{error:#}"),
            ),
        }
    }

    fn start_account_recovery_quorum_verify(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let request_file = canonical_nonsymlink_input_file(
                &self.account_recovery.quorum_request_file,
                "Recovery quorum request",
            )?;
            let approval_files = canonical_artifact_lines(
                &self.account_recovery.quorum_approval_files,
                "Recovery device approval",
            )?;
            Ok(WorkerRequest::AccountRecoveryQuorumVerify {
                executable,
                request_file,
                approval_files,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::AccountRecoveryQuorumVerify, request),
            Err(error) => self
                .model
                .fail(Operation::AccountRecoveryQuorumVerify, format!("{error:#}")),
        }
    }

    fn start_recovery_policy_request(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let old_package_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.old_package_file,
                "Old recovery package",
            )?;
            let old_witness_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.old_witness_file,
                "Old recovery witness",
            )?;
            let new_package_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.new_package_file,
                "New recovery package",
            )?;
            let new_witness_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.new_witness_file,
                "New recovery witness",
            )?;
            let old_epoch = self
                .device_link
                .policy
                .old_epoch
                .trim()
                .parse::<u64>()
                .context("Old recovery-policy epoch is invalid")?;
            let previous_transition_id = self
                .device_link
                .policy
                .previous_transition_id
                .trim()
                .to_ascii_lowercase();
            ensure!(
                previous_transition_id.len() == 64
                    && previous_transition_id
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit()),
                "Previous transition ID must contain 64 hexadecimal characters"
            );
            let request_file = absolute_artifact_output_path(
                &self.device_link.policy.request_file,
                "Recovery-policy transition request",
            )?;
            ensure!(
                !request_file.exists(),
                "Policy transition request output must be a new file"
            );
            Ok(WorkerRequest::RecoveryPolicyRequest {
                executable,
                old_package_file,
                old_witness_file,
                new_package_file,
                new_witness_file,
                old_epoch,
                previous_transition_id,
                request_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryPolicyRequest, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryPolicyRequest, format!("{error:#}")),
        }
    }

    fn start_recovery_policy_listen(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let state_dir = canonical_input_path(
                &self.device_link.policy.approver_state_dir,
                "Policy approving device state",
                false,
            )?;
            let request_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.request_file,
                "Recovery-policy transition request",
            )?;
            let ticket_file = absolute_output_path(
                &self.device_link.policy.ticket_output_file,
                "Recovery-policy approval ticket",
                &state_dir,
            )?;
            ensure!(
                !ticket_file.exists(),
                "Policy approval ticket output must be a new file"
            );
            Ok(WorkerRequest::RecoveryPolicyListen {
                executable,
                state_dir,
                request_file,
                ticket_file,
                route_policy: self.device_link.policy.route_policy,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryPolicyListen, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryPolicyListen, format!("{error:#}")),
        }
    }

    fn start_recovery_policy_collect(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let request_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.request_file,
                "Recovery-policy transition request",
            )?;
            let ticket_files = canonical_artifact_lines(
                &self.device_link.policy.ticket_files,
                "Recovery-policy approval ticket",
            )?;
            let approval_directory = absolute_directory_target(
                &self.device_link.policy.approval_directory,
                "Recovery-policy approval directory",
            )?;
            Ok(WorkerRequest::RecoveryPolicyCollect {
                executable,
                request_file,
                ticket_files,
                approval_directory,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryPolicyCollect, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryPolicyCollect, format!("{error:#}")),
        }
    }

    fn start_recovery_policy_certify(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let request_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.request_file,
                "Recovery-policy transition request",
            )?;
            let approval_files = canonical_artifact_lines(
                &self.device_link.policy.approval_files,
                "Recovery-policy transition approval",
            )?;
            let certificate_file = absolute_artifact_output_path(
                &self.device_link.policy.certificate_file,
                "Recovery-policy transition certificate",
            )?;
            ensure!(
                !certificate_file.exists(),
                "Policy certificate output must be a new file"
            );
            Ok(WorkerRequest::RecoveryPolicyCertify {
                executable,
                request_file,
                approval_files,
                certificate_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryPolicyCertify, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryPolicyCertify, format!("{error:#}")),
        }
    }

    fn start_recovery_policy_install(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let state_dir = canonical_input_path(
                &self.device_link.policy.install_state_dir,
                "Policy install device state",
                false,
            )?;
            let certificate_file = canonical_nonsymlink_input_file(
                &self.device_link.policy.certificate_file,
                "Recovery-policy transition certificate",
            )?;
            Ok(WorkerRequest::RecoveryPolicyInstall {
                executable,
                state_dir,
                certificate_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryPolicyInstall, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryPolicyInstall, format!("{error:#}")),
        }
    }

    fn start_device_link_request(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let workspace = absolute_new_directory_path(
                &self.device_link.joining_workspace,
                "Joining-device workspace",
            )?;
            let account_id = AccountId::from_str(self.device_link.account_id.trim())
                .context("Existing Account ID is invalid")?;
            Ok(WorkerRequest::DeviceLinkRequest {
                executable,
                workspace,
                account_id,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::DeviceLinkRequest, request),
            Err(error) => self
                .model
                .fail(Operation::DeviceLinkRequest, format!("{error:#}")),
        }
    }

    fn start_device_link_inspect(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let request_file = canonical_input_path(
                &self.device_link.owner_request_file,
                "Device-link request",
                true,
            )?;
            Ok(WorkerRequest::DeviceLinkInspect {
                executable,
                request_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::DeviceLinkInspect, request),
            Err(error) => self
                .model
                .fail(Operation::DeviceLinkInspect, format!("{error:#}")),
        }
    }

    fn start_device_link_authorize(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let inspected = self
                .device_link
                .inspected
                .as_ref()
                .context("Inspect the request before authorizing it")?;
            ensure!(inspected.request_fresh, "The inspected request has expired");
            ensure!(
                self.device_link.confirmed_sas.trim() == inspected.sas,
                "Typed SAS does not match the inspected signed request"
            );
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let account_root_dir = canonical_input_path(
                &self.device_link.account_root_dir,
                "Account Root directory",
                false,
            )?;
            let request_file = canonical_input_path(
                &self.device_link.owner_request_file,
                "Device-link request",
                true,
            )?;
            ensure!(
                self.device_link.inspected_request_file.as_ref() == Some(&request_file),
                "The request path changed after inspection; inspect it again"
            );
            let response_file = absolute_output_path(
                &self.device_link.response_output_file,
                "Device-link response",
                &account_root_dir,
            )?;
            let device_list_file = absolute_output_path(
                &self.device_link.device_list_output_file,
                "Published device list",
                &account_root_dir,
            )?;
            Ok(WorkerRequest::DeviceLinkAuthorize {
                executable,
                account_root_dir,
                request_file,
                confirmed_sas: self.device_link.confirmed_sas.trim().to_owned(),
                response_file,
                device_list_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::DeviceLinkAuthorize, request),
            Err(error) => self
                .model
                .fail(Operation::DeviceLinkAuthorize, format!("{error:#}")),
        }
    }

    fn start_device_link_accept(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let workspace = canonical_input_path(
                &self.device_link.joining_workspace,
                "Joining-device workspace",
                false,
            )?;
            let response_file = canonical_input_path(
                &self.device_link.response_input_file,
                "Device-link response",
                true,
            )?;
            Ok(WorkerRequest::DeviceLinkAccept {
                executable,
                workspace,
                response_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::DeviceLinkAccept, request),
            Err(error) => self
                .model
                .fail(Operation::DeviceLinkAccept, format!("{error:#}")),
        }
    }

    fn start_device_remove(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            self.require_offline_wizard()?;
            let executable = canonical_input_path(
                &self.bootstrap_executable_path,
                "Bootstrap executable",
                true,
            )?;
            let device_id =
                kilogram_identity::DeviceId::from_str(self.device_link.removal.device_id.trim())
                    .context("Device ID to remove is invalid")?;
            ensure!(
                self.device_link.removal.confirmed_device_id.trim() == device_id.to_string(),
                "Type the full Device ID again to confirm permanent removal"
            );
            let account_root_dir = canonical_input_path(
                &self.device_link.removal.account_root_dir,
                "Account Root directory",
                false,
            )?;
            let before_package_file = canonical_input_path(
                &self.device_link.removal.before_package_file,
                "Before-removal recovery package",
                true,
            )?;
            let before_witness_file = canonical_input_path(
                &self.device_link.removal.before_witness_file,
                "Before-removal recovery witness",
                true,
            )?;
            let after_package_file = absolute_output_path(
                &self.device_link.removal.after_package_file,
                "After-removal recovery package",
                &account_root_dir,
            )?;
            let after_witness_file = absolute_output_path(
                &self.device_link.removal.after_witness_file,
                "After-removal recovery witness",
                &account_root_dir,
            )?;
            let device_list_file = absolute_output_path(
                &self.device_link.removal.device_list_file,
                "Refreshed active-device list",
                &account_root_dir,
            )?;
            let revocation_file = absolute_output_path(
                &self.device_link.removal.revocation_file,
                "Device revocation",
                &account_root_dir,
            )?;
            Ok(WorkerRequest::DeviceRemove {
                executable,
                account_root_dir,
                device_id,
                before_package_file,
                before_witness_file,
                after_package_file,
                after_witness_file,
                device_list_file,
                revocation_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::DeviceRemove, request),
            Err(error) => self
                .model
                .fail(Operation::DeviceRemove, format!("{error:#}")),
        }
    }

    fn validate_recovery_context(&self) -> Result<(PathBuf, PathBuf, String)> {
        self.require_offline_wizard()?;
        let executable = canonical_input_path(
            &self.runtime_executable_path,
            "Kilogram CLI executable",
            true,
        )?;
        let state_dir = canonical_input_path(&self.recovery.state_dir, "Recovery state", false)?;
        let conversation = self.recovery.conversation.trim();
        ensure!(
            !conversation.is_empty(),
            "Recovery conversation is required"
        );
        ensure!(
            conversation.len() <= MAX_CONVERSATION_BYTES,
            "Recovery conversation is too long"
        );
        Ok((executable, state_dir, conversation.to_owned()))
    }

    fn start_recovery_approve(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            let (executable, state_dir, conversation) = self.validate_recovery_context()?;
            let link_file = canonical_input_path(&self.recovery.link_file, "Recovery link", true)?;
            let confirmed_sas = self.recovery.confirmed_sas.trim();
            ensure!(
                confirmed_sas.len() == 12
                    && confirmed_sas.bytes().all(|byte| byte.is_ascii_digit()),
                "Recovery SAS must contain exactly 12 digits"
            );
            let plan_file =
                absolute_output_path(&self.recovery.plan_output_file, "Recovery plan", &state_dir)?;
            Ok(WorkerRequest::RecoveryApprove {
                executable,
                state_dir,
                link_file,
                conversation,
                confirmed_sas: confirmed_sas.to_owned(),
                plan_file,
                deny_ethernet: !self.recovery.allow_ethernet,
                deny_wifi: !self.recovery.allow_wifi,
                allow_mobile: self.recovery.allow_mobile,
                allow_unknown_network: self.recovery.allow_unknown_network,
                require_external_power: self.recovery.require_external_power,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryApprove, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryApprove, format!("{error:#}")),
        }
    }

    fn add_existing_recovery_plan(&mut self) {
        let result = canonical_input_path(
            &self.recovery.existing_plan_file,
            "Existing recovery plan",
            true,
        );
        match result {
            Ok(path) => {
                let index = self.upsert_recovery_plan(path);
                self.recovery.selected = Some(index);
                self.model.notice = Some("Existing recovery plan added to this view".to_owned());
                self.model.error = None;
            }
            Err(error) => self.model.error = Some(format!("{error:#}")),
        }
    }

    fn start_recovery_run(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            let (executable, state_dir, conversation) = self.validate_recovery_context()?;
            let index = self.recovery.selected.context("Select a recovery plan")?;
            let plan_file = self
                .recovery
                .plans
                .get(index)
                .context("Selected recovery plan no longer exists")?
                .path
                .clone();
            ensure!(plan_file.is_file(), "Selected recovery plan is not a file");
            Ok(WorkerRequest::RecoveryRun {
                executable,
                state_dir,
                plan_file,
                conversation,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryRun, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryRun, format!("{error:#}")),
        }
    }

    fn start_recovery_cancel(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.recovery.confirm_cancel,
                "Explicitly confirm irreversible plan cancellation"
            );
            let (executable, state_dir, conversation) = self.validate_recovery_context()?;
            let index = self.recovery.selected.context("Select a recovery plan")?;
            let plan_file = self
                .recovery
                .plans
                .get(index)
                .context("Selected recovery plan no longer exists")?
                .path
                .clone();
            ensure!(plan_file.is_file(), "Selected recovery plan is not a file");
            Ok(WorkerRequest::RecoveryCancel {
                executable,
                state_dir,
                plan_file,
                conversation,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RecoveryCancel, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryCancel, format!("{error:#}")),
        }
    }

    fn start_recovery_reconcile(&mut self) {
        let result =
            self.validate_recovery_context()
                .map(
                    |(executable, state_dir, conversation)| WorkerRequest::RecoveryReconcile {
                        executable,
                        state_dir,
                        conversation,
                    },
                );
        match result {
            Ok(request) => self.submit(Operation::RecoveryReconcile, request),
            Err(error) => self
                .model
                .fail(Operation::RecoveryReconcile, format!("{error:#}")),
        }
    }

    fn start_publication_channel_rotation(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            ensure!(
                self.model.selected_contact_id.is_some(),
                "Select an enrolled contact first"
            );
            let executable = canonical_input_path(
                &self.runtime_executable_path,
                "Kilogram CLI executable",
                true,
            )?;
            let peer_account_id = AccountId::from_str(self.model.peer_account_id.trim())
                .context("Selected peer Account ID is invalid")?;
            Ok(WorkerRequest::PublicationChannelRotate {
                executable,
                ipc_file: self.model.descriptor()?,
                peer_account_id,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::PublicationChannelRotate, request),
            Err(error) => self
                .model
                .fail(Operation::PublicationChannelRotate, format!("{error:#}")),
        }
    }

    fn start_publication_conflict_request(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            let executable = canonical_input_path(
                &self.runtime_executable_path,
                "Kilogram CLI executable",
                true,
            )?;
            let state_dir = canonical_input_path(
                &self.runtime_profile_draft.state_dir,
                "Runtime state",
                false,
            )?;
            let channel_id = self
                .publication_conflict_recovery
                .channel_id
                .trim()
                .to_owned();
            ensure!(
                channel_id.len() == 64 && channel_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "Select a quarantined 64-hex publication channel"
            );
            let replacement_ticket_file = canonical_input_path(
                &self.publication_conflict_recovery.replacement_ticket_file,
                "Rotated peer ticket",
                true,
            )?;
            let request_file = absolute_output_path(
                &self.publication_conflict_recovery.request_output_file,
                "Conflict-resolution request",
                &state_dir,
            )?;
            Ok(WorkerRequest::PublicationConflictRequest {
                executable,
                ipc_file: self.model.descriptor()?,
                channel_id,
                replacement_ticket_file,
                request_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::PublicationConflictRequest, request),
            Err(error) => self
                .model
                .fail(Operation::PublicationConflictRequest, format!("{error:#}")),
        }
    }

    fn start_publication_conflict_apply(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            let executable = canonical_input_path(
                &self.runtime_executable_path,
                "Kilogram CLI executable",
                true,
            )?;
            let response_file = canonical_input_path(
                &self.publication_conflict_recovery.response_input_file,
                "Offline Root response",
                true,
            )?;
            Ok(WorkerRequest::PublicationConflictApply {
                executable,
                ipc_file: self.model.descriptor()?,
                response_file,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::PublicationConflictApply, request),
            Err(error) => self
                .model
                .fail(Operation::PublicationConflictApply, format!("{error:#}")),
        }
    }

    fn load_runtime_profile(&mut self) {
        let result: Result<RuntimeLaunchProfile> = (|| {
            let path = required_path(&self.runtime_profile_path, "Runtime profile")?;
            RuntimeLaunchProfile::load(&path)
        })();
        match result {
            Ok(profile) => {
                self.model.descriptor_path = profile.settings().ipc_file.display().to_string();
                self.runtime_profile_draft = RuntimeProfileDraft::from_profile(&profile);
                self.canonical_activation = ActivationView::default();
                self.peer_reenrollment.invalidate_runtime_context();
                self.model.notice = Some("Runtime launch profile loaded".to_owned());
                self.model.error = None;
            }
            Err(error) => self.model.error = Some(format!("Load runtime profile: {error:#}")),
        }
    }

    fn save_runtime_profile(&mut self) {
        if self.runtime_process.is_some()
            || self.model.connection != ConnectionState::Disconnected
            || self.model.pending.is_some()
        {
            self.model.error =
                Some("Stop or disconnect the runtime before editing its profile".to_owned());
            return;
        }
        let result: Result<RuntimeLaunchProfile> = (|| {
            let path = required_path(&self.runtime_profile_path, "Runtime profile")?;
            let profile = self.runtime_profile_draft.build()?;
            profile.write_replace(&path)?;
            Ok(profile)
        })();
        match result {
            Ok(profile) => {
                self.model.descriptor_path = profile.settings().ipc_file.display().to_string();
                self.runtime_profile_draft = RuntimeProfileDraft::from_profile(&profile);
                self.canonical_activation = ActivationView::default();
                self.peer_reenrollment.invalidate_runtime_context();
                self.model.notice = Some("Runtime launch profile saved atomically".to_owned());
                self.model.error = None;
            }
            Err(error) => self.model.error = Some(format!("Save runtime profile: {error:#}")),
        }
    }

    fn reconcile_runtime_profile(&mut self) {
        let result: Result<RuntimeLaunchProfile> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            let status = self
                .model
                .device_directory_status
                .as_ref()
                .context("Runtime device-directory status is unavailable")?;
            let profile_path = required_path(&self.runtime_profile_path, "Runtime profile")?;
            let descriptor_path = self.model.descriptor()?;
            reconcile_runtime_launch_profile(&profile_path, &descriptor_path, status)
        })();
        match result {
            Ok(profile) => {
                self.model.descriptor_path = profile.settings().ipc_file.display().to_string();
                self.runtime_profile_draft = RuntimeProfileDraft::from_profile(&profile);
                if let Some(status) = self.model.device_directory_status.as_mut() {
                    status.launch_device_list_file = status.applied_device_list_file.clone();
                    status.profile_convergence_status =
                        "current-after-verified-profile-write".to_owned();
                }
                if let Some(update) = self.model.device_directory_update.as_mut() {
                    update.launch_profile_update_required = false;
                    update.directory_status.launch_device_list_file =
                        update.directory_status.applied_device_list_file.clone();
                    update.directory_status.profile_convergence_status =
                        "current-after-verified-profile-write".to_owned();
                }
                self.model.notice = Some(
                    "Launch profile reconciled to the authenticated runtime roster".to_owned(),
                );
                self.model.error = None;
            }
            Err(error) => {
                self.model.error = Some(format!("Reconcile runtime profile: {error:#}"));
            }
        }
    }

    fn start_runtime(&mut self) {
        if self.runtime_process.is_some() {
            self.model.error = Some("A desktop-owned runtime is already active".to_owned());
            return;
        }
        let result: Result<Child> = (|| {
            let profile_path = required_path(&self.runtime_profile_path, "Runtime profile")?;
            let executable_path =
                required_path(&self.runtime_executable_path, "Runtime executable")?;
            let profile = RuntimeLaunchProfile::load(&profile_path)?;
            self.model.descriptor_path = profile.settings().ipc_file.display().to_string();
            let child = spawn_runtime_process(&executable_path, &profile_path)?;
            Ok(child)
        })();
        match result {
            Ok(child) => {
                if !self.canonical_activation.awaiting_fresh_runtime() {
                    self.canonical_activation.guided_restart = GuidedRestartState::Idle;
                }
                self.canonical_activation.status = None;
                self.canonical_activation.invalidate_plan();
                if !self.peer_reenrollment.awaiting_fresh_runtime() {
                    self.peer_reenrollment.invalidate_runtime_context();
                }
                self.runtime_process = Some(child);
                self.runtime_start_deadline = Some(Instant::now() + RUNTIME_START_TIMEOUT);
                self.runtime_next_connect_attempt = Instant::now();
                self.runtime_start_error = None;
                self.model.connection = ConnectionState::Connecting;
                self.model.notice = Some("Runtime process started; waiting for IPC".to_owned());
                self.model.error = None;
            }
            Err(error) => {
                let detail = format!("Start fresh runtime: {error:#}");
                if self.canonical_activation.awaiting_fresh_runtime() {
                    self.canonical_activation.fail_guided(detail.clone());
                }
                if self.peer_reenrollment.awaiting_fresh_runtime() {
                    self.peer_reenrollment.fail_restart(detail.clone());
                }
                self.model.fail(Operation::Connect, detail);
            }
        }
    }

    fn start_add_contact(&mut self) {
        let result = self.model.descriptor().and_then(|descriptor| {
            let draft = self.model.contact_draft.validate()?;
            Ok(WorkerRequest::AddContact { descriptor, draft })
        });
        match result {
            Ok(request) => self.submit(Operation::AddContact, request),
            Err(error) => self.model.fail(Operation::AddContact, format!("{error:#}")),
        }
    }

    fn start_shutdown(&mut self) {
        match self.model.descriptor() {
            Ok(descriptor) => {
                self.submit(Operation::Shutdown, WorkerRequest::Shutdown { descriptor })
            }
            Err(error) => self.model.fail(Operation::Shutdown, format!("{error:#}")),
        }
    }

    fn start_guided_canonical_activation(&mut self) {
        let result: Result<()> = (|| {
            ensure!(
                self.runtime_process.is_some()
                    && self.model.connection == ConnectionState::Connected,
                "The guided stop requires a connected desktop-owned runtime"
            );
            ensure!(
                self.model.pending.is_none(),
                "Wait for the current desktop operation to finish"
            );
            ensure!(
                !self.peer_reenrollment.restart_in_progress()
                    && self.peer_reenrollment_stop_deadline.is_none(),
                "Finish the peer re-enrollment restart before canonical activation"
            );
            self.canonical_activation.begin_guided_stop()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.start_shutdown();
                if self.model.pending != Some(Operation::Shutdown) {
                    let detail = self
                        .model
                        .error
                        .clone()
                        .unwrap_or_else(|| "Failed to request a clean runtime stop".to_owned());
                    self.canonical_activation.fail_guided(detail);
                    self.guided_stop_deadline = None;
                } else {
                    self.guided_stop_deadline = Some(Instant::now() + RUNTIME_STOP_TIMEOUT);
                }
            }
            Err(error) => {
                self.model.error = Some(format!("Guided activation: {error:#}"));
            }
        }
    }

    fn retry_guided_canonical_activation_fresh_start(&mut self) {
        let result: Result<()> = (|| {
            ensure!(
                self.runtime_process.is_none()
                    && self.model.connection == ConnectionState::Disconnected
                    && self.model.pending.is_none(),
                "Stop and disconnect the runtime before retrying exact verification"
            );
            self.canonical_activation.retry_fresh_runtime()?;
            Ok(())
        })();
        match result {
            Ok(()) => self.start_runtime(),
            Err(error) => {
                self.model.error = Some(format!("Guided activation retry: {error:#}"));
            }
        }
    }

    fn advance_guided_canonical_activation(&mut self) {
        if !self.canonical_activation.should_inspect_stopped_runtime()
            || self.runtime_process.is_some()
            || self.model.connection != ConnectionState::Disconnected
            || self.model.pending.is_some()
        {
            return;
        }
        self.canonical_activation.mark_offline_inspection_started();
        self.start_canonical_activation_status();
        if self.model.pending != Some(Operation::CanonicalActivationStatus) {
            let detail = self
                .model
                .error
                .clone()
                .unwrap_or_else(|| "Failed to inspect stopped activation authority".to_owned());
            self.canonical_activation.fail_guided(detail);
        }
    }

    fn canonical_activation_offline_context(&self) -> Result<(PathBuf, PathBuf)> {
        ensure!(
            self.runtime_process.is_none()
                && self.model.connection == ConnectionState::Disconnected,
            "Stop and disconnect the runtime before using activation authority"
        );
        let executable = canonical_input_path(
            &self.runtime_executable_path,
            "Kilogram CLI executable",
            true,
        )?;
        let state_dir = canonical_input_path(
            &self.runtime_profile_draft.state_dir,
            "Runtime state directory",
            false,
        )?;
        Ok((executable, state_dir))
    }

    fn start_canonical_activation_status(&mut self) {
        let result = self
            .canonical_activation_offline_context()
            .map(
                |(executable, state_dir)| WorkerRequest::CanonicalActivationStatus {
                    executable,
                    state_dir,
                },
            );
        match result {
            Ok(request) => self.submit(Operation::CanonicalActivationStatus, request),
            Err(error) => self
                .model
                .fail(Operation::CanonicalActivationStatus, format!("{error:#}")),
        }
    }

    fn start_rollout_operator_status(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            let config = self
                .rollout_operator_config
                .as_ref()
                .context("Configure the rollout observer at desktop startup")?;
            let executable = canonical_input_path(
                &self.runtime_executable_path,
                "Kilogram CLI executable",
                true,
            )?;
            let state_dir = canonical_input_path(
                &self.runtime_profile_draft.state_dir,
                "Runtime state directory",
                false,
            )?;
            Ok(WorkerRequest::RolloutOperatorStatus {
                executable,
                state_dir,
                rollout_directory: config.rollout_directory.clone(),
                trusted_policy_directory: config.trusted_policy_directory.clone(),
                release_trust_root: config.release_trust_root.clone(),
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RolloutOperatorStatus, request),
            Err(error) => self
                .model
                .fail(Operation::RolloutOperatorStatus, format!("{error:#}")),
        }
    }

    fn start_canonical_runtime_status(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            Ok(WorkerRequest::CanonicalRuntimeStatus {
                descriptor: self.model.descriptor()?,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::CanonicalRuntimeStatus, request),
            Err(error) => self
                .model
                .fail(Operation::CanonicalRuntimeStatus, format!("{error:#}")),
        }
    }

    fn start_peer_capability_status(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            Ok(WorkerRequest::PeerCapabilityStatus {
                descriptor: self.model.descriptor()?,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::PeerCapabilityStatus, request),
            Err(error) => self
                .model
                .fail(Operation::PeerCapabilityStatus, format!("{error:#}")),
        }
    }

    fn start_peer_reenrollment_status(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            let contact_id = self.peer_reenrollment.parsed_contact_id()?;
            Ok(WorkerRequest::PeerReenrollmentStatus {
                descriptor: self.model.descriptor()?,
                contact_id,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::PeerReenrollmentStatus, request),
            Err(error) => self
                .model
                .fail(Operation::PeerReenrollmentStatus, format!("{error:#}")),
        }
    }

    fn start_peer_reenrollment_mutation(&mut self, operation: Operation) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            let descriptor = self.model.descriptor()?;
            match operation {
                Operation::PeerReenrollmentPlan => Ok(WorkerRequest::PeerReenrollmentPlan {
                    descriptor,
                    command: self.peer_reenrollment.plan_command()?,
                }),
                Operation::PeerReenrollmentApprove => Ok(WorkerRequest::PeerReenrollmentApprove {
                    descriptor,
                    command: self.peer_reenrollment.approve_command()?,
                }),
                Operation::PeerReenrollmentAccept => Ok(WorkerRequest::PeerReenrollmentAccept {
                    descriptor,
                    command: self.peer_reenrollment.accept_command()?,
                }),
                _ => bail!("Unsupported peer re-enrollment mutation operation"),
            }
        })();
        match result {
            Ok(request) => self.submit(operation, request),
            Err(error) => self.model.fail(operation, format!("{error:#}")),
        }
    }

    fn start_guided_peer_reenrollment_restart(&mut self) {
        let result: Result<()> = (|| {
            ensure!(
                self.runtime_process.is_some()
                    && self.model.connection == ConnectionState::Connected,
                "Guided peer re-enrollment restart requires a connected desktop-owned runtime"
            );
            ensure!(
                self.model.pending.is_none(),
                "Wait for the current desktop operation to finish"
            );
            ensure!(
                !self.canonical_activation.awaiting_fresh_runtime()
                    && self.guided_stop_deadline.is_none(),
                "Finish the canonical activation restart before peer re-enrollment"
            );
            self.peer_reenrollment.begin_guided_restart()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.start_shutdown();
                if self.model.pending != Some(Operation::Shutdown) {
                    let detail = self
                        .model
                        .error
                        .clone()
                        .unwrap_or_else(|| "Failed to request a clean runtime stop".to_owned());
                    self.peer_reenrollment.fail_restart(detail);
                } else {
                    self.peer_reenrollment_stop_deadline =
                        Some(Instant::now() + RUNTIME_STOP_TIMEOUT);
                }
            }
            Err(error) => {
                self.model.error = Some(format!("Guided peer re-enrollment restart: {error:#}"));
            }
        }
    }

    fn retry_peer_reenrollment_fresh_verification(&mut self) {
        let result: Result<()> = (|| {
            ensure!(
                self.runtime_process.is_none()
                    && self.model.connection == ConnectionState::Disconnected
                    && self.model.pending.is_none(),
                "Stop and disconnect the runtime before retrying exact verification"
            );
            self.peer_reenrollment.retry_fresh_verification()?;
            Ok(())
        })();
        match result {
            Ok(()) => self.start_runtime(),
            Err(error) => {
                self.model.error = Some(format!(
                    "Guided peer re-enrollment verification retry: {error:#}"
                ));
            }
        }
    }

    fn start_canonical_activation_plan(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.canonical_activation.draft_valid(),
                "Refresh policy and enter a valid bounded reason before planning"
            );
            let (executable, state_dir) = self.canonical_activation_offline_context()?;
            let status = self
                .canonical_activation
                .status
                .as_ref()
                .context("Refresh the verified activation policy first")?;
            Ok(WorkerRequest::CanonicalActivationPlan {
                executable,
                state_dir,
                migration_state: self.canonical_activation.target_state,
                disposition: self.canonical_activation.disposition(),
                reason: self.canonical_activation.reason.clone(),
                canonical_evidence_id: status.canonical_evidence_id.clone(),
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::CanonicalActivationPlan, request),
            Err(error) => self
                .model
                .fail(Operation::CanonicalActivationPlan, format!("{error:#}")),
        }
    }

    fn start_canonical_activation_apply(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.canonical_activation.apply_ready(),
                "The exact plan confirmation is missing or the draft changed"
            );
            let (executable, state_dir) = self.canonical_activation_offline_context()?;
            let plan = self
                .canonical_activation
                .plan
                .as_ref()
                .context("Create an exact activation plan first")?;
            let status = self
                .canonical_activation
                .status
                .as_ref()
                .context("Refresh the verified activation policy first")?;
            ensure!(
                plan.current_record_id == status.record_id
                    && plan.device_id == status.device_id
                    && plan.canonical_evidence_id == status.canonical_evidence_id,
                "Displayed activation authority changed since planning"
            );
            Ok(WorkerRequest::CanonicalActivationApply {
                executable,
                state_dir,
                migration_state: plan.migration_state,
                disposition: plan.disposition,
                reason: plan.reason.clone(),
                canonical_evidence_id: plan.canonical_evidence_id.clone(),
                expected_record_id: plan.record_id.clone(),
            })
        })();
        match result {
            Ok(request) => {
                self.canonical_activation.begin_guided_apply();
                self.submit(Operation::CanonicalActivationApply, request);
                if self.model.pending != Some(Operation::CanonicalActivationApply) {
                    let detail = self.model.error.clone().unwrap_or_else(|| {
                        "Failed to submit the exact activation successor".to_owned()
                    });
                    self.canonical_activation.fail_guided(detail);
                }
            }
            Err(error) => self
                .model
                .fail(Operation::CanonicalActivationApply, format!("{error:#}")),
        }
    }

    fn start_queue(&mut self) {
        let result = self.model.descriptor().and_then(|descriptor| {
            let attempt = self.model.prepare_queue_attempt()?;
            Ok(WorkerRequest::Queue {
                descriptor,
                request_id: attempt.request_id,
                draft: attempt.draft,
            })
        });
        match result {
            Ok(request) => self.submit(Operation::Queue, request),
            Err(error) => self.model.fail(Operation::Queue, format!("{error:#}")),
        }
    }

    fn start_refresh(&mut self) {
        if self.model.pending.is_some() {
            return;
        }
        match self.model.descriptor() {
            Ok(descriptor) => {
                self.submit(Operation::Refresh, WorkerRequest::Refresh { descriptor });
            }
            Err(error) => self.model.fail(Operation::Refresh, format!("{error:#}")),
        }
    }

    fn start_apply_device_directory(&mut self) {
        let result = self.model.descriptor().and_then(|descriptor| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            let device_list_file = canonical_nonsymlink_input_file(
                &self.runtime_profile_draft.device_list_file,
                "Refreshed device list",
            )?;
            Ok(WorkerRequest::ApplyDeviceDirectory {
                descriptor,
                device_list_file,
            })
        });
        match result {
            Ok(request) => self.submit(Operation::ApplyDeviceDirectory, request),
            Err(error) => self
                .model
                .fail(Operation::ApplyDeviceDirectory, format!("{error:#}")),
        }
    }

    fn start_publish_ticket(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            ensure!(
                self.model.selected_contact_id.is_some(),
                "Select an enrolled contact first"
            );
            let descriptor = self.model.descriptor()?;
            let conversation = self.model.conversation.trim();
            ensure!(!conversation.is_empty(), "Conversation label is required");
            let peer_account_id = AccountId::from_str(self.model.peer_account_id.trim())
                .context("Peer Account ID is invalid")?;
            let service_base_url = self.model.ticket_publication_service_url.trim();
            ensure!(
                !service_base_url.is_empty(),
                "Ticket publication service URL is required"
            );
            Ok(WorkerRequest::PublishTicket {
                descriptor,
                conversation: conversation.to_owned(),
                peer_account_id,
                service_base_url: service_base_url.to_owned(),
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::PublishTicket, request),
            Err(error) => self
                .model
                .fail(Operation::PublishTicket, format!("{error:#}")),
        }
    }

    fn start_refresh_contact_ticket(&mut self) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            ensure!(
                self.model.selected_contact_id.is_some(),
                "Select an enrolled contact first"
            );
            let descriptor = self.model.descriptor()?;
            let conversation = self.model.conversation.trim();
            ensure!(!conversation.is_empty(), "Conversation label is required");
            let peer_account_id = AccountId::from_str(self.model.peer_account_id.trim())
                .context("Peer Account ID is invalid")?;
            let service_base_url = self.model.ticket_publication_service_url.trim();
            ensure!(
                !service_base_url.is_empty(),
                "Ticket publication service URL is required"
            );
            Ok(WorkerRequest::RefreshContactTicket {
                descriptor,
                conversation: conversation.to_owned(),
                peer_account_id,
                service_base_url: service_base_url.to_owned(),
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::RefreshContactTicket, request),
            Err(error) => self
                .model
                .fail(Operation::RefreshContactTicket, format!("{error:#}")),
        }
    }

    fn start_configure_ticket_automation(&mut self, enabled: bool) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            ensure!(
                self.model.selected_contact_id.is_some(),
                "Select an enrolled contact first"
            );
            let descriptor = self.model.descriptor()?;
            let conversation = self.model.conversation.trim();
            ensure!(!conversation.is_empty(), "Conversation label is required");
            let peer_account_id = AccountId::from_str(self.model.peer_account_id.trim())
                .context("Peer Account ID is invalid")?;
            let service_base_url = self.model.ticket_publication_service_url.trim();
            ensure!(
                !service_base_url.is_empty(),
                "Ticket publication service URL is required"
            );
            Ok(WorkerRequest::ConfigureTicketAutomation {
                descriptor,
                conversation: conversation.to_owned(),
                peer_account_id,
                enabled,
                service_base_url: service_base_url.to_owned(),
                allow_ethernet: self.model.ticket_automation_allow_ethernet,
                allow_wifi: self.model.ticket_automation_allow_wifi,
                allow_mobile: self.model.ticket_automation_allow_mobile,
                allow_unknown_network: self.model.ticket_automation_allow_unknown_network,
            })
        })();
        match result {
            Ok(request) => self.submit(Operation::ConfigureTicketAutomation, request),
            Err(error) => self
                .model
                .fail(Operation::ConfigureTicketAutomation, format!("{error:#}")),
        }
    }

    fn start_ticket_automation_status(&mut self) {
        let result = self.model.descriptor().and_then(|descriptor| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            Ok(WorkerRequest::TicketAutomationStatus { descriptor })
        });
        match result {
            Ok(request) => self.submit(Operation::TicketAutomationStatus, request),
            Err(error) => self
                .model
                .fail(Operation::TicketAutomationStatus, format!("{error:#}")),
        }
    }

    fn start_mailbox_change(&mut self, operation: Operation) {
        let result: Result<WorkerRequest> = (|| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            ensure!(
                self.model.selected_contact_id.is_some(),
                "Select an enrolled contact first"
            );
            let descriptor = self.model.descriptor()?;
            let conversation = self.model.conversation.trim();
            ensure!(!conversation.is_empty(), "Conversation label is required");
            let peer_account_id = AccountId::from_str(self.model.peer_account_id.trim())
                .context("Peer Account ID is invalid")?;
            let peer_device_id = DeviceId::from_str(self.model.mailbox_peer_device_id.trim())
                .context("Peer Device ID is invalid")?;
            if operation == Operation::MailboxRevoke {
                return Ok(WorkerRequest::MailboxRevoke {
                    descriptor,
                    conversation: conversation.to_owned(),
                    peer_account_id,
                    peer_device_id,
                });
            }
            let valid_for_seconds = self
                .model
                .mailbox_valid_for_seconds
                .trim()
                .parse::<u64>()
                .context("Mailbox validity seconds must be an integer")?;
            ensure!(valid_for_seconds > 0, "Mailbox validity must be positive");
            let fields = (
                descriptor,
                conversation.to_owned(),
                peer_account_id,
                peer_device_id,
                valid_for_seconds,
            );
            match operation {
                Operation::MailboxCreate => Ok(WorkerRequest::MailboxCreate {
                    descriptor: fields.0,
                    conversation: fields.1,
                    peer_account_id: fields.2,
                    peer_device_id: fields.3,
                    valid_for_seconds: fields.4,
                }),
                Operation::MailboxRotate => Ok(WorkerRequest::MailboxRotate {
                    descriptor: fields.0,
                    conversation: fields.1,
                    peer_account_id: fields.2,
                    peer_device_id: fields.3,
                    valid_for_seconds: fields.4,
                }),
                _ => bail!("Unsupported mailbox lifecycle operation"),
            }
        })();
        match result {
            Ok(request) => self.submit(operation, request),
            Err(error) => self.model.fail(operation, format!("{error:#}")),
        }
    }

    fn start_mailbox_status(&mut self) {
        if self.model.pending.is_some() {
            return;
        }
        let result = self.model.descriptor().and_then(|descriptor| {
            ensure!(
                self.model.connection == ConnectionState::Connected,
                "Connect to the running runtime first"
            );
            Ok(WorkerRequest::MailboxStatus { descriptor })
        });
        match result {
            Ok(request) => self.submit(Operation::MailboxStatus, request),
            Err(error) => self
                .model
                .fail(Operation::MailboxStatus, format!("{error:#}")),
        }
    }

    fn start_conversations(&mut self) {
        if self.model.pending.is_some() {
            return;
        }
        match self.model.descriptor() {
            Ok(descriptor) => {
                self.submit(
                    Operation::Conversations,
                    WorkerRequest::Conversations { descriptor },
                );
            }
            Err(error) => self
                .model
                .fail(Operation::Conversations, format!("{error:#}")),
        }
    }

    fn start_history(&mut self, older: bool) {
        if self.model.pending.is_some() {
            return;
        }
        let Some(conversation) =
            (!self.model.conversation.is_empty()).then(|| self.model.conversation.clone())
        else {
            return;
        };
        let cursor = if older {
            self.model.history_next_cursor
        } else {
            None
        };
        if older && cursor.is_none() {
            return;
        }
        match self.model.descriptor() {
            Ok(descriptor) => self.submit(
                if older {
                    Operation::HistoryOlder
                } else {
                    Operation::History
                },
                WorkerRequest::History {
                    descriptor,
                    conversation,
                    cursor,
                    older,
                },
            ),
            Err(error) => self.model.fail(
                if older {
                    Operation::HistoryOlder
                } else {
                    Operation::History
                },
                format!("{error:#}"),
            ),
        }
    }

    fn observe_runtime_process(&mut self) {
        let Some(child) = self.runtime_process.as_mut() else {
            return;
        };
        let outcome = child.try_wait();
        match outcome {
            Ok(Some(status)) => {
                self.runtime_process = None;
                self.guided_stop_deadline = None;
                self.peer_reenrollment_stop_deadline = None;
                let was_starting = self.runtime_start_deadline.take().is_some();
                let guided_starting = self.canonical_activation.awaiting_fresh_runtime();
                let peer_guided_starting = self.peer_reenrollment.awaiting_fresh_runtime();
                self.canonical_activation
                    .mark_runtime_exited_for_guided_stop();
                let start_peer_fresh_runtime =
                    self.peer_reenrollment.mark_runtime_exited_for_restart();
                self.model.pending = None;
                self.model.connection = ConnectionState::Disconnected;
                self.model.account_id = None;
                self.model.device_id = None;
                self.model.mailbox_status = None;
                self.model.mailbox_transition = None;
                self.model.canonical_runtime_status = None;
                self.model.peer_capability_status = None;
                self.stop_change_subscription();
                if was_starting {
                    let detail = self
                        .runtime_start_error
                        .take()
                        .unwrap_or_else(|| format!("runtime exited with {status}"));
                    let detail = format!("Runtime failed to start: {detail}");
                    if guided_starting {
                        self.canonical_activation.fail_guided(detail.clone());
                    }
                    if peer_guided_starting {
                        self.peer_reenrollment.fail_restart(detail.clone());
                    }
                    self.model.error = Some(detail);
                } else if start_peer_fresh_runtime {
                    self.model.notice = Some(
                        "Old runtime exited cleanly; starting a fresh runtime to consume the exact peer re-enrollment transition."
                            .to_owned(),
                    );
                    self.start_runtime();
                } else if self.canonical_activation.should_inspect_stopped_runtime() {
                    self.model.notice = Some(
                        "Runtime process exited; verifying the stopped activation authority."
                            .to_owned(),
                    );
                } else {
                    if !self.peer_reenrollment.can_retry_fresh_verification() {
                        self.peer_reenrollment.invalidate_runtime_context();
                    }
                    self.model.notice = Some(format!("Runtime process exited: {status}"));
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.model.error = Some(format!("Observe runtime process: {error}"));
            }
        }
    }

    fn maybe_connect_started_runtime(&mut self) {
        let Some(deadline) = self.runtime_start_deadline else {
            return;
        };
        if Instant::now() >= deadline {
            if let Some(child) = self.runtime_process.as_mut() {
                let _ = child.kill();
            }
            self.runtime_start_deadline = None;
            let detail = self
                .runtime_start_error
                .take()
                .unwrap_or_else(|| "Runtime startup timed out".to_owned());
            if self.canonical_activation.awaiting_fresh_runtime() {
                self.canonical_activation.fail_guided(detail.clone());
            }
            if self.peer_reenrollment.awaiting_fresh_runtime() {
                self.peer_reenrollment.fail_restart(detail.clone());
            }
            self.model.fail(Operation::Connect, detail);
            return;
        }
        if self.model.pending.is_none() && Instant::now() >= self.runtime_next_connect_attempt {
            self.start_connect();
        }
    }

    fn enforce_guided_stop_deadline(&mut self) {
        let Some(deadline) = self.guided_stop_deadline else {
            return;
        };
        if Instant::now() < deadline {
            return;
        }
        self.guided_stop_deadline = None;
        if let Some(child) = self.runtime_process.as_mut() {
            let _ = child.kill();
        }
        let detail = format!(
            "Old runtime did not exit within {:.1}s; the desktop terminated its child and stopped before offline activation",
            RUNTIME_STOP_TIMEOUT.as_secs_f32()
        );
        self.canonical_activation.fail_guided(detail.clone());
        self.model.pending = None;
        self.model.connection = ConnectionState::Disconnected;
        self.stop_change_subscription();
        self.model.error = Some(detail);
    }

    fn enforce_peer_reenrollment_stop_deadline(&mut self) {
        let Some(deadline) = self.peer_reenrollment_stop_deadline else {
            return;
        };
        if Instant::now() < deadline {
            return;
        }
        self.peer_reenrollment_stop_deadline = None;
        if let Some(child) = self.runtime_process.as_mut() {
            let _ = child.kill();
        }
        let detail = format!(
            "Old runtime did not exit within {:.1}s; the desktop terminated its child and stopped before peer re-enrollment verification",
            RUNTIME_STOP_TIMEOUT.as_secs_f32()
        );
        self.peer_reenrollment.fail_restart(detail.clone());
        self.model.pending = None;
        self.model.connection = ConnectionState::Disconnected;
        self.stop_change_subscription();
        self.model.error = Some(detail);
    }

    fn abort_guided_fresh_runtime(&mut self, detail: String) {
        self.canonical_activation.fail_guided(detail.clone());
        self.runtime_start_deadline = None;
        self.runtime_start_error = None;
        if let Some(mut child) = self.runtime_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.model.pending = None;
        self.model.connection = ConnectionState::Disconnected;
        self.model.account_id = None;
        self.model.device_id = None;
        self.model.mailbox_status = None;
        self.model.mailbox_transition = None;
        self.model.canonical_runtime_status = None;
        self.model.peer_capability_status = None;
        self.stop_change_subscription();
        self.model.notice = None;
        self.model.error = Some(detail);
    }

    fn abort_peer_reenrollment_fresh_runtime(&mut self, detail: String) {
        self.peer_reenrollment.fail_restart(detail.clone());
        self.runtime_start_deadline = None;
        self.runtime_start_error = None;
        if let Some(mut child) = self.runtime_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.model.pending = None;
        self.model.connection = ConnectionState::Disconnected;
        self.model.account_id = None;
        self.model.device_id = None;
        self.model.mailbox_status = None;
        self.model.mailbox_transition = None;
        self.model.canonical_runtime_status = None;
        self.model.peer_capability_status = None;
        self.stop_change_subscription();
        self.model.notice = None;
        self.model.error = Some(detail);
    }

    fn accept_dropped_descriptor(&mut self, context: &egui::Context) {
        let dropped_path = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .find_map(|file| file.path.clone())
        });
        if let Some(path) = dropped_path {
            if let Some(target) = self.drop_target.take() {
                let value = path.display().to_string();
                match target {
                    DropTarget::AccountRecoveryPackage => {
                        self.account_recovery.package_input_file = value;
                        self.account_recovery.inspected = None;
                        self.account_recovery.inspected_package_file = None;
                        self.account_recovery.inspected_witness_file = None;
                        self.account_recovery.confirm_latest_witness = false;
                        self.account_recovery.recovery_phrase.zeroize();
                        self.account_recovery.quorum_request = None;
                        self.account_recovery.quorum_collection = None;
                        self.account_recovery.quorum_verification = None;
                        self.model.notice =
                            Some("Account Root recovery package path updated".to_owned());
                    }
                    DropTarget::AccountRecoveryWitness => {
                        self.account_recovery.witness_input_file = value;
                        self.account_recovery.inspected = None;
                        self.account_recovery.inspected_package_file = None;
                        self.account_recovery.inspected_witness_file = None;
                        self.account_recovery.confirm_latest_witness = false;
                        self.account_recovery.recovery_phrase.zeroize();
                        self.account_recovery.quorum_request = None;
                        self.account_recovery.quorum_collection = None;
                        self.account_recovery.quorum_verification = None;
                        self.model.notice =
                            Some("Account Root recovery witness path updated".to_owned());
                    }
                    DropTarget::DeviceLinkRequest => {
                        self.device_link.owner_request_file = value;
                        self.device_link.inspected_request_file = None;
                        self.device_link.inspected = None;
                        self.device_link.confirmed_sas.clear();
                        self.model.notice = Some("Device-link request path updated".to_owned());
                    }
                    DropTarget::DeviceLinkResponse => {
                        self.device_link.response_input_file = value;
                        self.model.notice = Some("Device-link response path updated".to_owned());
                    }
                    DropTarget::RecoveryLink => {
                        self.recovery.link_file = value;
                        self.model.notice = Some("Recovery-link path updated".to_owned());
                    }
                    DropTarget::RecoveryPlan => {
                        self.recovery.existing_plan_file = value;
                        self.model.notice = Some("Recovery-plan path updated".to_owned());
                    }
                }
            } else if self.model.show_contact_form {
                self.model.contact_draft.descriptor_file = path.display().to_string();
                self.model.notice = Some("Peer descriptor path updated".to_owned());
            } else if self.runtime_process.is_none() {
                self.model.descriptor_path = path.display().to_string();
                self.model.connection = ConnectionState::Disconnected;
                self.model.mailbox_status = None;
                self.model.mailbox_transition = None;
                self.peer_reenrollment.invalidate_runtime_context();
                self.model.notice = Some("Runtime descriptor path updated".to_owned());
            }
        }
    }

    fn draw_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Kilogram").size(28.0).strong());
            ui.label(egui::RichText::new("M0.9.57").color(egui::Color32::from_rgb(88, 166, 255)));
        });
        ui.label("Desktop client · authenticated local runtime IPC");
    }

    fn draw_bootstrap(&mut self, ui: &mut egui::Ui) -> BootstrapUiAction {
        let mut action = BootstrapUiAction::None;
        egui::CollapsingHeader::new("First run · create account")
            .default_open(self.bootstrap_view.is_some())
            .show(ui, |ui| {
                if let Some(view) = self.bootstrap_view.as_mut() {
                    ui.colored_label(
                        egui::Color32::from_rgb(246, 195, 93),
                        "Write down these 24 words now. They are shown once and are not stored in the receipt or launch profile.",
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(view.recovery_phrase.as_str())
                            .monospace()
                            .size(15.0),
                    );
                    ui.add_space(4.0);
                    ui.label(format!("Account: {}", view.account_id));
                    ui.label(format!("First device: {}", view.device_id));
                    ui.label(format!("Account Root: {}", view.account_root_dir));
                    ui.label(format!("Public prekey pool: {}", view.prekey_pool_file));
                    ui.small(format!(
                        "Local protection: root={} · device vault={}",
                        view.root_key_protection, view.vault_key_protection
                    ));
                    ui.small(
                        "The phrase encodes the Account Root key. Export the Root authority package and independently retain its latest witness below; encrypted message history still requires a backup or another enrolled device.",
                    );
                    ui.checkbox(&mut view.phrase_saved, "I saved the recovery phrase offline");
                    if ui
                        .add_enabled(
                            view.phrase_saved,
                            egui::Button::new("Hide recovery phrase permanently"),
                        )
                        .clicked()
                    {
                        action = BootstrapUiAction::DismissPhrase;
                    }
                } else {
                    ui.label("Create a new Account Root and its first enrolled device in one atomic workspace.");
                    ui.horizontal(|ui| {
                        ui.label("Bootstrap executable");
                        ui.add_enabled(
                            self.model.pending.is_none(),
                            egui::TextEdit::singleline(&mut self.bootstrap_executable_path)
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("New account workspace");
                        ui.add_enabled(
                            self.model.pending.is_none(),
                            egui::TextEdit::singleline(&mut self.bootstrap_workspace_path)
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.small("The destination must not exist. No seed or private key is passed on a command line.");
                    if ui
                        .add_enabled(
                            self.model.pending.is_none()
                                && self.runtime_process.is_none()
                                && self.model.connection == ConnectionState::Disconnected,
                            egui::Button::new("Create new account"),
                        )
                        .clicked()
                    {
                        action = BootstrapUiAction::Create;
                    }
                }
            });
        action
    }

    fn draw_account_recovery(&mut self, ui: &mut egui::Ui) -> AccountRecoveryUiAction {
        let mut action = AccountRecoveryUiAction::None;
        let idle = self.model.pending.is_none()
            && self.runtime_process.is_none()
            && self.model.connection == ConnectionState::Disconnected;
        egui::CollapsingHeader::new("Account Root backup and recovery")
            .default_open(self.account_recovery.inspected.is_some())
            .show(ui, |ui| {
                ui.small("Root recovery restores authority, not a messaging device or its history. Keep the 24-word phrase, package, and latest witness as separate recovery inputs.");
                ui.separator();
                ui.label("Current Root: export a fresh authority checkpoint");
                let root_changed = ui
                    .horizontal(|ui| {
                        ui.label("Account Root directory");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.account_recovery.account_root_dir,
                            )
                            .desired_width(f32::INFINITY),
                        )
                        .changed()
                    })
                    .inner;
                if root_changed {
                    self.account_recovery.checkpoint_status = None;
                    self.account_recovery.exported = None;
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Check recovery export status"))
                    .clicked()
                {
                    action = AccountRecoveryUiAction::Status;
                }
                if let Some(checkpoint) = self.account_recovery.checkpoint_status.as_ref() {
                    let current = checkpoint.checkpoint_state == "current";
                    ui.colored_label(
                        if current {
                            egui::Color32::from_rgb(92, 201, 137)
                        } else {
                            egui::Color32::from_rgb(239, 112, 112)
                        },
                        if current {
                            "Recovery export is exact for the current Root state."
                        } else {
                            "Recovery export update required."
                        },
                    );
                    ui.label(format!(
                        "Authority revision {} · {} devices · {} memberships",
                        checkpoint.authority_revision,
                        checkpoint.device_count,
                        checkpoint.conversation_membership_count
                    ));
                    ui.monospace(format!(
                        "Current package ID: {}",
                        checkpoint.current_package_id
                    ));
                    if let Some(recorded) = checkpoint.recorded_package_id.as_ref() {
                        ui.monospace(format!("Last exported package ID: {recorded}"));
                    }
                    if let Some(revision) = checkpoint.recorded_authority_revision {
                        ui.label(format!("Last exported authority revision: {revision}"));
                    }
                    ui.small("This is a local exact-export receipt, not proof that independently stored artifacts are globally newest.");
                }
                ui.horizontal(|ui| {
                    ui.label("New package file");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.account_recovery.package_output_file,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("New latest-witness file");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.account_recovery.witness_output_file,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                ui.small("Both outputs are no-clobber and must be outside the Root. Copy the witness to storage independent from the package and replace that independent copy after every authority or membership change.");
                if ui
                    .add_enabled(idle, egui::Button::new("Export fresh package + witness"))
                    .clicked()
                {
                    action = AccountRecoveryUiAction::Export;
                }
                if let Some(exported) = self.account_recovery.exported.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Exported revision {} · {} devices · {} memberships",
                            exported.authority_revision,
                            exported.device_count,
                            exported.conversation_membership_count
                        ),
                    );
                    ui.monospace(format!("Package ID: {}", exported.package_id));
                }

                ui.separator();
                ui.label("Offline check: authenticate the exact package/witness pair");
                let package_changed = ui
                    .horizontal(|ui| {
                        ui.label("Package file");
                        let changed = ui
                            .add_enabled(
                                idle,
                                egui::TextEdit::singleline(
                                    &mut self.account_recovery.package_input_file,
                                )
                                .desired_width(f32::INFINITY),
                            )
                            .changed();
                        if ui
                            .add_enabled(idle, egui::Button::new("Drop next file"))
                            .clicked()
                        {
                            self.drop_target = Some(DropTarget::AccountRecoveryPackage);
                        }
                        changed
                    })
                    .inner;
                let witness_changed = ui
                    .horizontal(|ui| {
                        ui.label("Latest witness file");
                        let changed = ui
                            .add_enabled(
                                idle,
                                egui::TextEdit::singleline(
                                    &mut self.account_recovery.witness_input_file,
                                )
                                .desired_width(f32::INFINITY),
                            )
                            .changed();
                        if ui
                            .add_enabled(idle, egui::Button::new("Drop next file"))
                            .clicked()
                        {
                            self.drop_target = Some(DropTarget::AccountRecoveryWitness);
                        }
                        changed
                    })
                    .inner;
                if package_changed || witness_changed {
                    self.account_recovery.inspected = None;
                    self.account_recovery.inspected_package_file = None;
                    self.account_recovery.inspected_witness_file = None;
                    self.account_recovery.confirm_latest_witness = false;
                    self.account_recovery.recovery_phrase.zeroize();
                    self.account_recovery.restored = None;
                    self.account_recovery.quorum_request = None;
                    self.account_recovery.quorum_collection = None;
                    self.account_recovery.quorum_verification = None;
                }
                if matches!(
                    self.drop_target,
                    Some(
                        DropTarget::AccountRecoveryPackage
                            | DropTarget::AccountRecoveryWitness
                    )
                ) {
                    ui.small("Drop the selected recovery artifact anywhere in this window.");
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Inspect signatures and binding"))
                    .clicked()
                {
                    action = AccountRecoveryUiAction::Inspect;
                }

                ui.separator();
                ui.label("Live current-device quorum");
                ui.small("The request contains the exact package and witness. Each current device commits one approval locally, then exposes it through a signed one-shot Iroh ticket over LAN or relay.");
                ui.horizontal(|ui| {
                    ui.label("Quorum request (.karq)");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.account_recovery.quorum_request_file,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                if ui
                    .add_enabled(
                        idle && self.account_recovery.inspected.is_some(),
                        egui::Button::new("1 · Create fresh quorum request"),
                    )
                    .clicked()
                {
                    action = AccountRecoveryUiAction::QuorumRequest;
                }
                if let Some(request) = self.account_recovery.quorum_request.as_ref() {
                    ui.monospace(format!("Request: {}", request.request_id));
                    ui.label(format!(
                        "Need {} of {} current devices · expires at Unix {}",
                        request.required_approvals,
                        request.roster_count,
                        request.expires_at_unix_seconds
                    ));
                }

                ui.group(|ui| {
                    ui.label("On each current device: approve and listen once");
                    ui.horizontal(|ui| {
                        ui.label("Device state directory");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.account_recovery.quorum_state_dir,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("New signed ticket (.kart)");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.account_recovery.quorum_ticket_output_file,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Route");
                        ui.add_enabled_ui(idle, |ui| {
                            ui.selectable_value(
                                &mut self.account_recovery.quorum_route_policy,
                                RuntimeIpcRoutePolicy::Auto,
                                "Auto",
                            );
                            ui.selectable_value(
                                &mut self.account_recovery.quorum_route_policy,
                                RuntimeIpcRoutePolicy::DirectOnly,
                                "Direct only",
                            );
                            ui.selectable_value(
                                &mut self.account_recovery.quorum_route_policy,
                                RuntimeIpcRoutePolicy::RelayOnly,
                                "Relay only",
                            );
                        });
                    });
                    if ui
                        .add_enabled(idle, egui::Button::new("2A · Approve and wait for collector"))
                        .clicked()
                    {
                        action = AccountRecoveryUiAction::QuorumListen;
                    }
                    if self.model.pending == Some(Operation::AccountRecoveryQuorumListen) {
                        ui.colored_label(
                            egui::Color32::from_rgb(246, 195, 93),
                            "The helper is validating local state. The .kart file appears only after the approval head commits; send that ticket to the recovering device and keep this one-shot listener running until collection or request expiry.",
                        );
                    }
                    if let Some(listener) = self.account_recovery.quorum_listener.as_ref() {
                        ui.colored_label(
                            egui::Color32::from_rgb(92, 201, 137),
                            format!(
                                "Delivered by {} · device {} · {} ms",
                                listener.transport_path,
                                compact_id(&listener.approver_device_id),
                                listener.transport_rtt_milliseconds
                            ),
                        );
                    }
                });

                ui.group(|ui| {
                    ui.label("On the recovering device: collect distinct approvals");
                    ui.label("Signed ticket files, one path per line");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::multiline(
                            &mut self.account_recovery.quorum_ticket_files,
                        )
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Approval directory");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.account_recovery.quorum_approval_directory,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    if ui
                        .add_enabled(idle, egui::Button::new("2B · Collect listed tickets"))
                        .clicked()
                    {
                        action = AccountRecoveryUiAction::QuorumCollect;
                    }
                    ui.label("Existing .kara files, one path per line");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::multiline(
                            &mut self.account_recovery.quorum_approval_files,
                        )
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                    );
                    if ui
                        .add_enabled(idle, egui::Button::new("Verify saved approvals"))
                        .clicked()
                    {
                        action = AccountRecoveryUiAction::QuorumVerify;
                    }
                });

                let claim = self
                    .account_recovery
                    .quorum_verification
                    .as_ref()
                    .map(|report| {
                        (
                            report.freshness_claim.as_str(),
                            report.observed_approvals,
                            report.required_approvals,
                            report.majority_satisfied,
                            report.package_id.as_str(),
                        )
                    })
                    .or_else(|| {
                        self.account_recovery.quorum_collection.as_ref().map(|report| {
                            (
                                report.freshness_claim.as_str(),
                                report.observed_approvals,
                                report.required_approvals,
                                report.majority_satisfied,
                                report.package_id.as_str(),
                            )
                        })
                    });
                if let Some((claim, observed, required, majority, package_id)) = claim {
                    ui.colored_label(
                        if majority {
                            egui::Color32::from_rgb(92, 201, 137)
                        } else {
                            egui::Color32::from_rgb(246, 195, 93)
                        },
                        format!("Freshness: {claim} · {observed}/{required} approvals"),
                    );
                    ui.monospace(format!("Approved package: {package_id}"));
                    ui.small("Cross-roster fork safety: false. Roster epochs and joint old/new-majority transitions are not implemented yet.");
                }

                if let Some(inspected) = self.account_recovery.inspected.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        "Package and witness signatures match exactly.",
                    );
                    ui.monospace(format!("Account: {}", inspected.account_id));
                    ui.monospace(format!("Package ID: {}", inspected.package_id));
                    ui.label(format!(
                        "Authority revision {} · {} devices · {} memberships",
                        inspected.authority_revision,
                        inspected.device_count,
                        inspected.conversation_membership_count
                    ));
                    ui.colored_label(
                        egui::Color32::from_rgb(246, 195, 93),
                        "Cryptographic validity does not prove global freshness. A matching old package and old witness can still be rolled back together.",
                    );
                    let quorum_majority = claim.is_some_and(
                        |(_, _, _, majority, package_id)| {
                            majority && package_id == inspected.package_id
                        },
                    );
                    if quorum_majority {
                        ui.colored_label(
                            egui::Color32::from_rgb(92, 201, 137),
                            "Restore gate: current-device majority observed for this exact package.",
                        );
                    } else {
                        ui.checkbox(
                            &mut self.account_recovery.confirm_latest_witness,
                            "Proceed with reduced-assurance offline fallback: I independently verified this as my newest known witness",
                        );
                    }
                    ui.horizontal(|ui| {
                        ui.label("24-word recovery phrase");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut *self.account_recovery.recovery_phrase,
                            )
                            .password(true)
                            .desired_width(f32::INFINITY),
                        );
                        if ui
                            .add_enabled(
                                idle && !self.account_recovery.recovery_phrase.is_empty(),
                                egui::Button::new("Clear"),
                            )
                            .clicked()
                        {
                            self.account_recovery.recovery_phrase.zeroize();
                        }
                    });
                    ui.small(format!(
                        "Words entered: {}. The phrase is sent only through helper stdin, never as a command-line argument.",
                        self.account_recovery
                            .recovery_phrase
                            .split_whitespace()
                            .count()
                    ));
                    ui.horizontal(|ui| {
                        ui.label("New restored Root directory");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.account_recovery.restore_root_dir,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    if ui
                        .add_enabled(
                            idle
                                && (quorum_majority
                                    || self.account_recovery.confirm_latest_witness)
                                && self
                                    .account_recovery
                                    .recovery_phrase
                                    .split_whitespace()
                                    .count()
                                    == 24,
                            egui::Button::new("Restore Account Root into new directory"),
                        )
                        .clicked()
                    {
                        action = AccountRecoveryUiAction::Restore;
                    }
                }
                if let Some(restored) = self.account_recovery.restored.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Root restored at revision {} with {}",
                            restored.authority_revision,
                            restored.root_key_protection.as_deref().unwrap_or("unknown provider")
                        ),
                    );
                    if let Some(path) = restored.account_root_dir.as_ref() {
                        ui.monospace(format!("Root: {}", path.display()));
                    }
                    ui.small("Next: create a device-link request for this Account ID, authorize it with the restored Root, then run multi-source message-history recovery.");
                }
            });
        action
    }

    fn draw_device_link(&mut self, ui: &mut egui::Ui) -> DeviceLinkUiAction {
        let mut action = DeviceLinkUiAction::None;
        let idle = self.model.pending.is_none()
            && self.runtime_process.is_none()
            && self.model.connection == ConnectionState::Disconnected;
        egui::CollapsingHeader::new("Existing account · link another device")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("New device: create a short-lived request");
                ui.horizontal(|ui| {
                    ui.label("Existing Account ID");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.account_id)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("New device workspace");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.joining_workspace)
                            .desired_width(f32::INFINITY),
                    );
                });
                if ui
                    .add_enabled(idle, egui::Button::new("1 · Create request"))
                    .clicked()
                {
                    action = DeviceLinkUiAction::Request;
                }
                if let Some(request) = self.device_link.request.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(246, 195, 93),
                        "Compare this code through an independent channel:",
                    );
                    ui.label(egui::RichText::new(&request.sas).monospace().size(28.0).strong());
                    ui.monospace(format!("Request: {}", request.request_file.display()));
                    ui.small(format!(
                        "Device {} · expires at Unix {} · vault {}",
                        compact_id(&request.device_id),
                        request.expires_at_unix_seconds,
                        request.vault_key_protection
                    ));
                }

                ui.separator();
                ui.label("Existing device owner: inspect, compare, then authorize");
                ui.horizontal(|ui| {
                    ui.label("Request file");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.owner_request_file)
                            .desired_width(f32::INFINITY),
                    );
                    if ui.add_enabled(idle, egui::Button::new("Drop next file")).clicked() {
                        self.drop_target = Some(DropTarget::DeviceLinkRequest);
                    }
                });
                if self.drop_target == Some(DropTarget::DeviceLinkRequest) {
                    ui.small("Drop the signed device-link request anywhere in this window.");
                }
                if ui
                    .add_enabled(idle, egui::Button::new("2 · Inspect signed request"))
                    .clicked()
                {
                    action = DeviceLinkUiAction::Inspect;
                }
                if let Some(inspected) = self.device_link.inspected.as_ref() {
                    let color = if inspected.request_fresh {
                        egui::Color32::from_rgb(92, 201, 137)
                    } else {
                        egui::Color32::from_rgb(239, 112, 112)
                    };
                    ui.colored_label(
                        color,
                        if inspected.request_fresh {
                            "Signature valid and request is fresh"
                        } else {
                            "Request signature decoded, but the request is expired or not yet valid"
                        },
                    );
                    ui.label(egui::RichText::new(&inspected.sas).monospace().size(28.0).strong());
                    ui.small(format!(
                        "Account {} · device {} · issued {} · expires {}",
                        compact_id(&inspected.account_id),
                        compact_id(&inspected.device_id),
                        inspected.issued_at_unix_seconds,
                        inspected.expires_at_unix_seconds
                    ));
                    ui.horizontal(|ui| {
                        ui.label("Account Root directory");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(&mut self.device_link.account_root_dir)
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.label("Type the independently confirmed 12-digit SAS");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.confirmed_sas)
                            .desired_width(220.0),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Encrypted response output");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.device_link.response_output_file,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Updated public device list");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.device_link.device_list_output_file,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    let sas_matches = self.device_link.confirmed_sas.trim() == inspected.sas;
                    if ui
                        .add_enabled(
                            idle && inspected.request_fresh && sas_matches,
                            egui::Button::new("3 · Authorize exact device"),
                        )
                        .clicked()
                    {
                        action = DeviceLinkUiAction::Authorize;
                    }
                    if !self.device_link.confirmed_sas.is_empty() && !sas_matches {
                        ui.colored_label(
                            egui::Color32::from_rgb(239, 112, 112),
                            "Typed SAS does not match the signed request.",
                        );
                    }
                }
                if let Some(authorization) = self.device_link.authorization.as_ref() {
                    ui.small(format!(
                        "Authorized device {} at authority revision {}. Response is encrypted for that device: {}",
                        compact_id(&authorization.device_id),
                        authorization.authority_revision,
                        authorization.response_encrypted_for_device
                    ));
                    ui.monospace(format!("Response: {}", authorization.response_file.display()));
                    ui.monospace(format!(
                        "Device list: {}",
                        authorization.device_list_file.display()
                    ));
                }

                ui.separator();
                ui.label("New device: accept the returned encrypted response");
                ui.horizontal(|ui| {
                    ui.label("Response file");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.response_input_file)
                            .desired_width(f32::INFINITY),
                    );
                    if ui.add_enabled(idle, egui::Button::new("Drop next file")).clicked() {
                        self.drop_target = Some(DropTarget::DeviceLinkResponse);
                    }
                });
                if self.drop_target == Some(DropTarget::DeviceLinkResponse) {
                    ui.small("Drop the encrypted response anywhere in this window.");
                }
                if ui
                    .add_enabled(idle, egui::Button::new("4 · Accept enrollment"))
                    .clicked()
                {
                    action = DeviceLinkUiAction::Accept;
                }
                if let Some(accepted) = self.device_link.accepted.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Device {} linked at authority revision {}",
                            compact_id(&accepted.device_id),
                            accepted.authority_revision
                        ),
                    );
                    ui.small(format!(
                        "Certificate: {} · prekey pool: {}",
                        accepted.certificate_file.display(),
                        accepted.prekey_pool_file.display()
                    ));
                    ui.small("History is not inside the response. Use one or more recipient-bound recovery plans below.");
                }
                ui.small("The GUI passes only public paths, IDs and typed SAS values. Root and device private keys remain inside their protected stores.");
            });
        action
    }

    fn draw_device_removal(&mut self, ui: &mut egui::Ui) -> DeviceLinkUiAction {
        let mut action = DeviceLinkUiAction::None;
        let idle = self.model.pending.is_none()
            && self.runtime_process.is_none()
            && self.model.connection == ConnectionState::Disconnected;
        egui::CollapsingHeader::new("Remove a device · permanent Root revocation")
            .default_open(self.device_link.removal.result.is_some())
            .show(ui, |ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(239, 112, 112),
                    "Removal is permanent. Stop the runtime and verify the full Device ID through a trusted channel.",
                );
                ui.small("First export the current Root package and independent witness. They become the exact old-roster input; this operation publishes the exact new-roster pair.");
                for (label, value) in [
                    ("Account Root directory", &mut self.device_link.removal.account_root_dir),
                    ("Device ID to remove", &mut self.device_link.removal.device_id),
                    ("Before package (.karp)", &mut self.device_link.removal.before_package_file),
                    ("Before witness (.karw)", &mut self.device_link.removal.before_witness_file),
                    ("After package (.karp)", &mut self.device_link.removal.after_package_file),
                    ("After witness (.karw)", &mut self.device_link.removal.after_witness_file),
                    ("New public device list", &mut self.device_link.removal.device_list_file),
                    ("Public revocation", &mut self.device_link.removal.revocation_file),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(value).desired_width(f32::INFINITY),
                        );
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Type the full Device ID again");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.device_link.removal.confirmed_device_id,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                let confirmed = !self.device_link.removal.device_id.trim().is_empty()
                    && self.device_link.removal.device_id.trim()
                        == self.device_link.removal.confirmed_device_id.trim();
                if ui
                    .add_enabled(
                        idle && confirmed,
                        egui::Button::new("Permanently remove exact device"),
                    )
                    .clicked()
                {
                    action = DeviceLinkUiAction::Remove;
                }
                if !self.device_link.removal.confirmed_device_id.is_empty() && !confirmed {
                    ui.colored_label(
                        egui::Color32::from_rgb(239, 112, 112),
                        "Confirmation does not match the full Device ID.",
                    );
                }
                if let Some(result) = self.device_link.removal.result.as_ref() {
                    let applied = self
                        .model
                        .device_directory_update
                        .as_ref()
                        .filter(|update| {
                            update.authority_revision == result.after_authority_revision
                                && update
                                    .removed_device_ids
                                    .iter()
                                    .any(|device_id| {
                                        device_id.to_string() == result.removed_device_id
                                    })
                        });
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Removal: {} · device {} · authority {}→{}",
                            result.removal_status,
                            compact_id(&result.removed_device_id),
                            result.before_authority_revision,
                            result.after_authority_revision
                        ),
                    );
                    ui.label(format!(
                        "Recovery-policy activation: {}",
                        result.policy_activation_status
                    ));
                    ui.label(format!(
                        "Runtime peer-directory refresh: {}",
                        if applied.is_some() {
                            "applied"
                        } else {
                            &result.runtime_peer_directory_status
                        }
                    ));
                    ui.label(format!(
                        "Ratchet/session retirement: {}",
                        if applied.is_some() {
                            "complete"
                        } else {
                            &result.ratchet_session_retirement_status
                        }
                    ));
                    ui.label(format!(
                        "History availability: {}",
                        result.history_availability_status
                    ));
                    ui.small(format!(
                        "Active devices {}→{}. Existing copies of old history are not erased by cryptographic revocation.",
                        result.before_device_count, result.after_device_count
                    ));
                    ui.monospace(format!("New device list: {}", result.device_list_file.display()));
                    ui.monospace(format!("Revocation: {}", result.revocation_file.display()));
                }
            });
        action
    }

    fn draw_recovery_policy(&mut self, ui: &mut egui::Ui) -> RecoveryPolicyUiAction {
        let mut action = RecoveryPolicyUiAction::None;
        let idle = self.model.pending.is_none()
            && self.runtime_process.is_none()
            && self.model.connection == ConnectionState::Disconnected;
        egui::CollapsingHeader::new("Device roster change · activate recovery policy")
            .default_open(self.device_link.policy.request.is_some())
            .show(ui, |ui| {
                let root_state = if self.device_link.authorization.is_some()
                    || self.device_link.accepted.is_some()
                    || self.device_link.removal.result.is_some()
                {
                    "Root operation: device roster changed"
                } else {
                    "Root operation: not observed in this desktop session"
                };
                let policy_state = if let Some(installed) =
                    self.device_link.policy.installation.as_ref()
                {
                    format!(
                        "Recovery-policy activation: installed at epoch {}",
                        installed.policy_epoch
                    )
                } else if self.device_link.policy.certificate.is_some() {
                    "Recovery-policy activation: certified, not installed on this device"
                        .to_owned()
                } else if let Some(collected) = self.device_link.policy.collection.as_ref() {
                    if collected.joint_majority_satisfied {
                        "Recovery-policy activation: joint majority collected, not certified"
                            .to_owned()
                    } else {
                        "Recovery-policy activation: approvals incomplete".to_owned()
                    }
                } else if self.device_link.policy.request.is_some() {
                    "Recovery-policy activation: request created, approvals incomplete".to_owned()
                } else {
                    "Recovery-policy activation: not started".to_owned()
                };
                ui.label(root_state);
                ui.label(policy_state);
                ui.label("Message-history recovery: separate; use the recovery panel below");
                ui.small("A successful device-link or revocation changes Root authority immediately. It does not by itself activate the new recovery voter set or copy message history.");

                ui.separator();
                ui.label("1 · Bind the exact Root checkpoints before and after the roster change");
                for (label, value) in [
                    (
                        "Old package (.karp)",
                        &mut self.device_link.policy.old_package_file,
                    ),
                    (
                        "Old witness (.karw)",
                        &mut self.device_link.policy.old_witness_file,
                    ),
                    (
                        "New package (.karp)",
                        &mut self.device_link.policy.new_package_file,
                    ),
                    (
                        "New witness (.karw)",
                        &mut self.device_link.policy.new_witness_file,
                    ),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(value).desired_width(f32::INFINITY),
                        );
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Old policy epoch");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.policy.old_epoch)
                            .desired_width(80.0),
                    );
                    ui.label("Previous transition ID");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.device_link.policy.previous_transition_id,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("New transition request (.karpt)");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.policy.request_file)
                            .desired_width(f32::INFINITY),
                    );
                });
                if ui
                    .add_enabled(idle, egui::Button::new("Create transition request"))
                    .clicked()
                {
                    action = RecoveryPolicyUiAction::Request;
                }
                if let Some(request) = self.device_link.policy.request.as_ref() {
                    ui.label(format!(
                        "Epoch {}→{} · old threshold {}/{} · new threshold {}/{}",
                        request.old_epoch,
                        request.new_epoch,
                        request.old_required_approvals,
                        request.old_roster_count,
                        request.new_required_approvals,
                        request.new_roster_count
                    ));
                    ui.monospace(format!("Request: {}", request.request_id));
                }

                ui.separator();
                ui.group(|ui| {
                    ui.label("2A · On each old/new voter: approve and listen once");
                    ui.horizontal(|ui| {
                        ui.label("Device state directory");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.device_link.policy.approver_state_dir,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("New ticket (.karpticket)");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.device_link.policy.ticket_output_file,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Route");
                        ui.add_enabled_ui(idle, |ui| {
                            ui.selectable_value(
                                &mut self.device_link.policy.route_policy,
                                RuntimeIpcRoutePolicy::Auto,
                                "Auto",
                            );
                            ui.selectable_value(
                                &mut self.device_link.policy.route_policy,
                                RuntimeIpcRoutePolicy::DirectOnly,
                                "Direct only",
                            );
                            ui.selectable_value(
                                &mut self.device_link.policy.route_policy,
                                RuntimeIpcRoutePolicy::RelayOnly,
                                "Relay only",
                            );
                        });
                    });
                    if ui
                        .add_enabled(idle, egui::Button::new("Approve and wait for collector"))
                        .clicked()
                    {
                        action = RecoveryPolicyUiAction::Listen;
                    }
                    if self.model.pending == Some(Operation::RecoveryPolicyListen) {
                        ui.colored_label(
                            egui::Color32::from_rgb(246, 195, 93),
                            "The ticket appears only after the DB-primary anti-equivocation head commits. Share it and keep this one-shot listener open.",
                        );
                    }
                    if let Some(listener) = self.device_link.policy.listener.as_ref() {
                        ui.colored_label(
                            egui::Color32::from_rgb(92, 201, 137),
                            format!(
                                "Delivered by {} · device {} · {} ms",
                                listener.transport_path,
                                compact_id(&listener.approver_device_id),
                                listener.transport_rtt_milliseconds
                            ),
                        );
                    }
                });

                ui.group(|ui| {
                    ui.label("2B · Collector: fetch distinct signed approvals");
                    ui.label("Ticket files, one path per line");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::multiline(&mut self.device_link.policy.ticket_files)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Approval directory");
                        ui.add_enabled(
                            idle,
                            egui::TextEdit::singleline(
                                &mut self.device_link.policy.approval_directory,
                            )
                            .desired_width(f32::INFINITY),
                        );
                    });
                    if ui
                        .add_enabled(idle, egui::Button::new("Collect listed tickets"))
                        .clicked()
                    {
                        action = RecoveryPolicyUiAction::Collect;
                    }
                    if let Some(collection) = self.device_link.policy.collection.as_ref() {
                        ui.colored_label(
                            if collection.joint_majority_satisfied {
                                egui::Color32::from_rgb(92, 201, 137)
                            } else {
                                egui::Color32::from_rgb(246, 195, 93)
                            },
                            format!(
                                "Old {}/{} · new {}/{} · joint majority: {}",
                                collection.old_observed_approvals,
                                collection.old_required_approvals,
                                collection.new_observed_approvals,
                                collection.new_required_approvals,
                                collection.joint_majority_satisfied
                            ),
                        );
                    }
                });

                ui.separator();
                ui.label("3 · Certify once, then install on every active new-roster device");
                ui.label("Approval files, one path per line");
                ui.add_enabled(
                    idle,
                    egui::TextEdit::multiline(&mut self.device_link.policy.approval_files)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.label("New certificate (.karpc)");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.device_link.policy.certificate_file)
                            .desired_width(f32::INFINITY),
                    );
                });
                if ui
                    .add_enabled(
                        idle && !self.device_link.policy.approval_files.trim().is_empty(),
                        egui::Button::new("Certify joint majority"),
                    )
                    .clicked()
                {
                    action = RecoveryPolicyUiAction::Certify;
                }
                if let Some(certificate) = self.device_link.policy.certificate.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Certified epoch {} · cross-roster fork safety: {}",
                            certificate.new_epoch, certificate.cross_roster_fork_safety
                        ),
                    );
                }
                ui.horizontal(|ui| {
                    ui.label("Install into device state");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.device_link.policy.install_state_dir,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                if ui
                    .add_enabled(
                        idle && !self.device_link.policy.certificate_file.trim().is_empty(),
                        egui::Button::new("Install certified policy on this device"),
                    )
                    .clicked()
                {
                    action = RecoveryPolicyUiAction::Install;
                }
                if let Some(installed) = self.device_link.policy.installation.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Installed epoch {} · source {}",
                            installed.policy_epoch, installed.policy_state_source
                        ),
                    );
                }
            });
        action
    }

    fn draw_recovery(&mut self, ui: &mut egui::Ui) -> RecoveryUiAction {
        let mut action = RecoveryUiAction::None;
        let idle = self.model.pending.is_none()
            && self.runtime_process.is_none()
            && self.model.connection == ConnectionState::Disconnected;
        egui::CollapsingHeader::new("History recovery · multiple source devices")
            .default_open(false)
            .show(ui, |ui| {
                ui.small("Each plan is signed by this recipient and bound to one exact source, range, policy and expiry. Run only while the matching source publishes its recovery link.");
                ui.horizontal(|ui| {
                    ui.label("Recipient state directory");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.recovery.state_dir)
                            .desired_width(f32::INFINITY),
                    );
                    if ui.add_enabled(idle, egui::Button::new("Use profile")).clicked() {
                        self.recovery.state_dir = self.runtime_profile_draft.state_dir.clone();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Conversation label");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.recovery.conversation)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.separator();
                ui.label("Approve another source plan");
                ui.horizontal(|ui| {
                    ui.label("Signed recovery link file");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.recovery.link_file)
                            .desired_width(f32::INFINITY),
                    );
                    if ui.add_enabled(idle, egui::Button::new("Drop next file")).clicked() {
                        self.drop_target = Some(DropTarget::RecoveryLink);
                    }
                });
                if self.drop_target == Some(DropTarget::RecoveryLink) {
                    ui.small("Drop the signed recovery-link text file anywhere in this window.");
                }
                ui.horizontal(|ui| {
                    ui.label("Confirmed 12-digit SAS");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.recovery.confirmed_sas)
                            .desired_width(220.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("New signed plan file");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.recovery.plan_output_file)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.add_enabled_ui(idle, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut self.recovery.allow_ethernet, "Ethernet");
                        ui.checkbox(&mut self.recovery.allow_wifi, "Wi-Fi");
                        ui.checkbox(&mut self.recovery.allow_mobile, "Mobile/metered");
                        ui.checkbox(
                            &mut self.recovery.allow_unknown_network,
                            "Unknown network",
                        );
                        ui.checkbox(
                            &mut self.recovery.require_external_power,
                            "External power required",
                        );
                    });
                });
                if ui
                    .add_enabled(idle, egui::Button::new("Approve and add plan"))
                    .clicked()
                {
                    action = RecoveryUiAction::Approve;
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Existing plan file");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(&mut self.recovery.existing_plan_file)
                            .desired_width(f32::INFINITY),
                    );
                    if ui.add_enabled(idle, egui::Button::new("Drop next file")).clicked() {
                        self.drop_target = Some(DropTarget::RecoveryPlan);
                    }
                    if ui.add_enabled(idle, egui::Button::new("Add")).clicked() {
                        action = RecoveryUiAction::AddExisting;
                    }
                });
                if self.drop_target == Some(DropTarget::RecoveryPlan) {
                    ui.small("Drop an existing recipient-signed plan anywhere in this window, then click Add.");
                }
                if self.recovery.plans.is_empty() {
                    ui.small("No recovery plans added yet.");
                } else {
                    egui::Grid::new("recovery-plan-list")
                        .striped(true)
                        .num_columns(5)
                        .show(ui, |ui| {
                            ui.strong("Plan");
                            ui.strong("Status");
                            ui.strong("Lifecycle");
                            ui.strong("Attempts");
                            ui.strong("Complete");
                            ui.end_row();
                            for (index, plan) in self.recovery.plans.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        self.recovery.selected == Some(index),
                                        plan.path
                                            .file_name()
                                            .map(|name| name.to_string_lossy())
                                            .unwrap_or_else(|| plan.path.display().to_string().into()),
                                    )
                                    .clicked()
                                {
                                    self.recovery.selected = Some(index);
                                    self.recovery.confirm_cancel = false;
                                }
                                ui.label(&plan.status);
                                ui.label(&plan.lifecycle);
                                ui.label(&plan.attempts);
                                ui.label(&plan.complete);
                                ui.end_row();
                            }
                        });
                }
                if ui
                    .add_enabled(
                        idle && self.recovery.selected.is_some(),
                        egui::Button::new("Run one bounded attempt for selected plan"),
                    )
                    .clicked()
                {
                    action = RecoveryUiAction::RunSelected;
                }
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        idle && self.recovery.selected.is_some(),
                        egui::Checkbox::new(
                            &mut self.recovery.confirm_cancel,
                            "I understand cancellation permanently revokes this plan's local retry consent",
                        ),
                    );
                    if ui
                        .add_enabled(
                            idle
                                && self.recovery.selected.is_some()
                                && self.recovery.confirm_cancel,
                            egui::Button::new("Cancel selected plan"),
                        )
                        .clicked()
                    {
                        action = RecoveryUiAction::CancelSelected;
                    }
                });

                ui.separator();
                if ui
                    .add_enabled(idle, egui::Button::new("Reconcile all received source claims"))
                    .clicked()
                {
                    action = RecoveryUiAction::Reconcile;
                }
                if let Some(reconciliation) = self.recovery.reconciliation.as_ref() {
                    let agreement = reconciliation
                        .field("source_claim_agreement")
                        .unwrap_or("incomplete");
                    let color = match agreement {
                        "agreed" => egui::Color32::from_rgb(92, 201, 137),
                        "divergent" => egui::Color32::from_rgb(239, 112, 112),
                        _ => egui::Color32::from_rgb(246, 195, 93),
                    };
                    ui.colored_label(color, format!("Source claims: {agreement}"));
                    ui.horizontal_wrapped(|ui| {
                        for (label, key) in [
                            ("sources", "source_device_count"),
                            ("complete sources", "complete_source_count"),
                            ("covered events", "covered_event_count"),
                            ("equivocations", "source_equivocation_count"),
                        ] {
                            ui.label(format!(
                                "{label}: {}",
                                reconciliation.field(key).unwrap_or("0")
                            ));
                        }
                    });
                    ui.small(format!(
                        "Global completeness proven: {} (this remains false without an external completeness witness)",
                        reconciliation
                            .field("global_completeness_proven")
                            .unwrap_or("false")
                    ));
                }
            });
        action
    }

    fn draw_publication_conflict_recovery(
        &mut self,
        ui: &mut egui::Ui,
    ) -> PublicationConflictUiAction {
        let mut action = PublicationConflictUiAction::None;
        let conflicts = self
            .model
            .conversations
            .iter()
            .filter(|summary| {
                self.model.selected_contact_id.as_deref() == Some(summary.contact_id.as_str())
            })
            .flat_map(|summary| summary.endpoint_candidates.iter())
            .filter_map(|candidate| {
                Some((
                    candidate.publication_channel_id.clone()?,
                    candidate.peer_device_id.to_string(),
                    candidate.publication_conflict_proof_id.clone()?,
                ))
            })
            .collect::<Vec<_>>();
        let selected_peer_account = self
            .model
            .conversations
            .iter()
            .find(|summary| {
                self.model.selected_contact_id.as_deref() == Some(summary.contact_id.as_str())
            })
            .map(|summary| summary.peer_account_id.to_string());
        let idle = self.model.pending.is_none();
        let connected_idle = idle && self.model.connection == ConnectionState::Connected;
        egui::CollapsingHeader::new("Security incident · publication conflict")
            .default_open(!conflicts.is_empty())
            .show(ui, |ui| {
                ui.small("One live incident workflow: the publishing peer rotates its channel, the affected side creates a public request, the isolated Root signs it, and the running client applies the response. The desktop never opens or receives the Account Root secret.");
                if self.publication_conflict_recovery.apply_result.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        "Phase: resolved locally · sibling propagation pending · restart not required",
                    );
                } else if self.publication_conflict_recovery.request_result.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(246, 195, 93),
                        "Phase: offline Root authorization pending · runtime remains online",
                    );
                } else if !conflicts.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(239, 112, 112),
                        "Phase: quarantined · obtain the peer's rotated ticket",
                    );
                }
                if self.model.connection != ConnectionState::Connected {
                    ui.colored_label(
                        egui::Color32::from_rgb(246, 195, 93),
                        "Connect to the authenticated local runtime to perform incident actions.",
                    );
                }

                ui.separator();
                ui.strong("0. Peer-side action: rotate my channel for this contact");
                ui.small("Use this only when the peer reports a conflicting publication from this Device. The runtime atomically republishes its ticket and stays online.");
                if ui
                    .add_enabled(
                        connected_idle && selected_peer_account.is_some(),
                        egui::Button::new("Rotate my publication channel live"),
                    )
                    .clicked()
                {
                    action = PublicationConflictUiAction::RotateOwnChannel;
                }
                if let Some(output) = self.publication_conflict_recovery.rotation_result.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "My channel rotated to epoch {} · restart required: {}",
                            output.field("publication_channel_epoch").unwrap_or("unknown"),
                            output.field("runtime_restart_required").unwrap_or("unknown")
                        ),
                    );
                    ui.small(format!(
                        "Refreshed ticket: {} · next: {}",
                        output.field("ticket_file").unwrap_or("not configured"),
                        output.field("incident_next_action").unwrap_or("transfer fresh ticket to peer")
                    ));
                }
                if conflicts.is_empty() {
                    ui.small("The selected chat has no unresolved publication-channel conflict.");
                } else {
                    for (channel, device, proof) in &conflicts {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!(
                                "Peer device {} · channel {} · proof {}",
                                compact_id(device),
                                compact_id(channel),
                                compact_id(proof)
                            ));
                            if ui
                                .add_enabled(idle, egui::Button::new("Select conflict"))
                                .clicked()
                            {
                                self.publication_conflict_recovery
                                    .channel_id
                                    .clone_from(channel);
                                self.publication_conflict_recovery.request_result = None;
                                self.publication_conflict_recovery.apply_result = None;
                            }
                        });
                    }
                }

                ui.separator();
                ui.strong("1. Create a request for the offline Root signer");
                ui.horizontal(|ui| {
                    ui.label("Quarantined channel");
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(
                            &mut self.publication_conflict_recovery.channel_id,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Peer's rotated ticket");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self
                                .publication_conflict_recovery
                                .replacement_ticket_file,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Request output (.pcrq)");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.publication_conflict_recovery.request_output_file,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                if ui
                    .add_enabled(
                        connected_idle
                            && !self
                                .publication_conflict_recovery
                                .channel_id
                                .is_empty()
                            && !self
                                .publication_conflict_recovery
                                .replacement_ticket_file
                                .trim()
                                .is_empty(),
                        egui::Button::new("Create offline Root request"),
                    )
                    .clicked()
                {
                    action = PublicationConflictUiAction::CreateRequest;
                }
                if let Some(output) = self.publication_conflict_recovery.request_result.as_ref() {
                    let request_file = output.field("request_file").unwrap_or("request.pcrq");
                    let confirmation_code = output
                        .field("confirmation_code")
                        .unwrap_or("missing-confirmation-code");
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Request {} created at {}",
                            compact_id(
                                output
                                    .field("publication_conflict_resolution_request_id")
                                    .unwrap_or("unknown")
                            ),
                            output.field("request_file").unwrap_or("unknown")
                        ),
                    );
                    ui.colored_label(
                        egui::Color32::from_rgb(246, 195, 93),
                        format!("Independent confirmation code: {confirmation_code}"),
                    );
                    ui.small(format!(
                        "Request digest: {}",
                        output.field("request_artifact_digest").unwrap_or("unknown")
                    ));
                    ui.small("Keep the full .pcrq on removable media. Its QR is a compact exact-match claim, not the request itself. With the minimal offline package, run:");
                    ui.label(
                        egui::RichText::new(format!(
                            "kilogram-offline inspect-request --request-file \"{request_file}\" --qr-output-file publication-conflict-request.png"
                        ))
                        .monospace(),
                    );
                    ui.small("Compare the KPC1 code through an independent channel. Then, on the isolated Root host, inspect the artifact (and optionally its QR) before authorizing:");
                    ui.label(
                        egui::RichText::new(format!(
                            "kilogram-offline authorize --account-dir <offline-root> --request-file \"{request_file}\" --confirm-code \"{confirmation_code}\" --verification-qr-file publication-conflict-request.png --output-file publication-conflict-resolution.pcrp"
                        ))
                        .monospace(),
                    );
                }

                ui.separator();
                ui.strong("2. Apply the self-contained Root response");
                ui.horizontal(|ui| {
                    ui.label("Root response (.pcrp)");
                    ui.add_enabled(
                        idle,
                        egui::TextEdit::singleline(
                            &mut self.publication_conflict_recovery.response_input_file,
                        )
                        .desired_width(f32::INFINITY),
                    );
                });
                if !self
                    .publication_conflict_recovery
                    .response_input_file
                    .trim()
                    .is_empty()
                {
                    ui.small("Optional public verification before import:");
                    ui.label(
                        egui::RichText::new(format!(
                            "kilogram-offline inspect-response --response-file \"{}\" --qr-output-file publication-conflict-response.png",
                            self.publication_conflict_recovery.response_input_file.trim()
                        ))
                        .monospace(),
                    );
                }
                if ui
                    .add_enabled(
                        connected_idle
                            && !self
                                .publication_conflict_recovery
                                .response_input_file
                                .trim()
                                .is_empty(),
                        egui::Button::new("Verify and apply Root response"),
                    )
                    .clicked()
                {
                    action = PublicationConflictUiAction::ApplyResponse;
                }
                if let Some(output) = self.publication_conflict_recovery.apply_result.as_ref() {
                    ui.colored_label(
                        egui::Color32::from_rgb(92, 201, 137),
                        format!(
                            "Resolved: {} → {}",
                            compact_id(
                                output
                                    .field("old_publication_channel_id")
                                    .unwrap_or("unknown")
                            ),
                            compact_id(
                                output
                                    .field("new_publication_channel_id")
                                    .unwrap_or("unknown")
                            )
                        ),
                    );
                    ui.small("No restart is required. The retained proof and Root resolution will be propagated to exact-current sibling devices through their encrypted announcement exchange.");
                }
            });
        action
    }

    fn draw_runtime(&mut self, ui: &mut egui::Ui) -> RuntimeUiAction {
        let mut action = RuntimeUiAction::None;
        let color = match self.model.connection {
            ConnectionState::Disconnected => egui::Color32::from_rgb(239, 112, 112),
            ConnectionState::Connecting => egui::Color32::from_rgb(246, 195, 93),
            ConnectionState::Connected => egui::Color32::from_rgb(92, 201, 137),
        };
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.colored_label(color, "●");
                ui.strong(self.model.connection.label());
            });
            ui.horizontal(|ui| {
                ui.label("IPC descriptor");
                if ui.add_enabled(
                    self.model.pending.is_none() && self.runtime_process.is_none(),
                    egui::TextEdit::singleline(&mut self.model.descriptor_path)
                        .desired_width(f32::INFINITY),
                ).changed() {
                    self.peer_reenrollment.invalidate_runtime_context();
                }
            });
            ui.collapsing("Runtime launch settings", |ui| {
                let editable = self.model.pending.is_none()
                    && self.runtime_process.is_none()
                    && self.model.connection == ConnectionState::Disconnected;
                ui.horizontal(|ui| {
                    ui.label("Launch profile");
                    ui.add_enabled(
                        self.model.pending.is_none() && self.runtime_process.is_none(),
                        egui::TextEdit::singleline(&mut self.runtime_profile_path)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Runtime executable");
                    ui.add_enabled(
                        self.model.pending.is_none() && self.runtime_process.is_none(),
                        egui::TextEdit::singleline(&mut self.runtime_executable_path)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(editable, egui::Button::new("Load profile"))
                        .clicked()
                    {
                        action = RuntimeUiAction::LoadProfile;
                    }
                    if ui
                        .add_enabled(editable, egui::Button::new("Save profile"))
                        .clicked()
                    {
                        action = RuntimeUiAction::SaveProfile;
                    }
                    if ui.button("Edit settings").clicked() {
                        self.show_profile_editor = !self.show_profile_editor;
                    }
                });
                if self.show_profile_editor {
                    ui.separator();
                    ui.label("Enrolled device state directory");
                    ui.add_enabled(
                        editable,
                        egui::TextEdit::singleline(&mut self.runtime_profile_draft.state_dir)
                            .hint_text("Enrolled device state directory"),
                    );
                    ui.label("Allowed requester Account ID");
                    ui.add_enabled(
                        editable,
                        egui::TextEdit::singleline(
                            &mut self.runtime_profile_draft.allowed_requester_account_id,
                        )
                        .hint_text("Allowed requester Account ID"),
                    );
                    ui.label("Signed device-list file");
                    ui.add_enabled(
                        editable,
                        egui::TextEdit::singleline(
                            &mut self.runtime_profile_draft.device_list_file,
                        )
                        .hint_text("Signed device-list file"),
                    );
                    ui.label("Peer prekey-pool files (one path per line)");
                    ui.add_enabled(
                        editable,
                        egui::TextEdit::multiline(
                            &mut self.runtime_profile_draft.peer_prekey_pool_files,
                        )
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                    );
                    ui.label("Published runtime ticket (optional)");
                    ui.add_enabled(
                        editable,
                        egui::TextEdit::singleline(&mut self.runtime_profile_draft.ticket_file)
                            .hint_text("Published runtime ticket (optional)"),
                    );
                    ui.label("Local IPC descriptor");
                    ui.add_enabled(
                        editable,
                        egui::TextEdit::singleline(&mut self.runtime_profile_draft.ipc_file)
                            .hint_text("Local IPC descriptor"),
                    );
                    ui.add_enabled_ui(editable, |ui| {
                        egui::ComboBox::from_label("Route policy")
                            .selected_text(self.runtime_profile_draft.route_policy.as_str())
                            .show_ui(ui, |ui| {
                                for policy in [
                                    RuntimeIpcRoutePolicy::Auto,
                                    RuntimeIpcRoutePolicy::DirectOnly,
                                    RuntimeIpcRoutePolicy::RelayOnly,
                                ] {
                                    ui.selectable_value(
                                        &mut self.runtime_profile_draft.route_policy,
                                        policy,
                                        policy.as_str(),
                                    );
                                }
                            });
                        ui.label("Custom relay URL (optional)");
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut self.runtime_profile_draft.relay_url,
                            )
                            .hint_text("Custom relay URL (optional)"),
                        );
                    });
                    egui::Grid::new("runtime-profile-numbers")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for (label, value) in [
                                (
                                    "Relay wait seconds",
                                    &mut self.runtime_profile_draft.relay_wait_seconds,
                                ),
                                (
                                    "Runtime poll ms",
                                    &mut self.runtime_profile_draft.poll_milliseconds,
                                ),
                                (
                                    "Retry base seconds",
                                    &mut self.runtime_profile_draft.retry_base_seconds,
                                ),
                                (
                                    "Retry max seconds",
                                    &mut self.runtime_profile_draft.retry_max_seconds,
                                ),
                                (
                                    "Auto sync seconds",
                                    &mut self.runtime_profile_draft.auto_sync_seconds,
                                ),
                            ] {
                                ui.label(label);
                                ui.add_enabled(
                                    editable,
                                    egui::TextEdit::singleline(value).desired_width(100.0),
                                );
                                ui.end_row();
                            }
                        });
                    ui.separator();
                    ui.add_enabled(
                        editable,
                        egui::Checkbox::new(
                            &mut self.runtime_profile_draft.volunteer_storage_enabled,
                            "Help store encrypted offline messages for other users",
                        ),
                    );
                    ui.small(
                        "Enabled by default. Kilogram cannot read the ciphertext. Mobile and unknown networks are disabled by default.",
                    );
                    let volunteer_editable =
                        editable && self.runtime_profile_draft.volunteer_storage_enabled;
                    ui.label("Volunteer ciphertext directory");
                    ui.add_enabled(
                        volunteer_editable,
                        egui::TextEdit::singleline(
                            &mut self.runtime_profile_draft.volunteer_storage_data_dir,
                        )
                        .hint_text("Default: beside the protected state directory"),
                    );
                    egui::Grid::new("runtime-profile-volunteer-storage")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for (label, value) in [
                                (
                                    "Storage limit MiB",
                                    &mut self.runtime_profile_draft.volunteer_storage_mib,
                                ),
                                (
                                    "Ethernet / 30 days MiB",
                                    &mut self
                                        .runtime_profile_draft
                                        .volunteer_ethernet_transfer_mib,
                                ),
                                (
                                    "Wi-Fi / 30 days MiB",
                                    &mut self.runtime_profile_draft.volunteer_wifi_transfer_mib,
                                ),
                                (
                                    "Mobile / 30 days MiB",
                                    &mut self.runtime_profile_draft.volunteer_mobile_transfer_mib,
                                ),
                                (
                                    "Unknown network / 30 days MiB",
                                    &mut self.runtime_profile_draft.volunteer_unknown_transfer_mib,
                                ),
                            ] {
                                ui.label(label);
                                ui.add_enabled(
                                    volunteer_editable,
                                    egui::TextEdit::singleline(value).desired_width(100.0),
                                );
                                ui.end_row();
                            }
                        });
                }
                ui.small("This profile contains paths and public runtime settings, never device or vault secrets.");
                ui.small("First-device bootstrap fills these local paths. Peer Account ID and peer prekey pools are added during contact setup.");
            });
            ui.small("Pass --ipc-file/--runtime-profile or drop a descriptor onto this window.");
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.model.pending.is_none(),
                    egui::Button::new(if self.model.connection == ConnectionState::Connected {
                        "Reconnect"
                    } else {
                        "Connect"
                    }),
                )
                .clicked()
            {
                action = RuntimeUiAction::Connect;
            }
            if ui
                .add_enabled(
                    self.model.pending.is_none()
                        && self.runtime_process.is_none()
                        && self.model.connection == ConnectionState::Disconnected,
                    egui::Button::new("Start runtime"),
                )
                .clicked()
            {
                action = RuntimeUiAction::Start;
            }
            if ui
                .add_enabled(
                    self.model.pending.is_none()
                        && self.model.connection == ConnectionState::Connected,
                    egui::Button::new("Stop runtime"),
                )
                .clicked()
            {
                action = RuntimeUiAction::Stop;
            }
            if ui
                .add_enabled(
                    self.model.pending.is_none()
                        && self.model.connection == ConnectionState::Connected
                        && !self
                            .runtime_profile_draft
                            .device_list_file
                            .trim()
                            .is_empty(),
                    egui::Button::new("Apply refreshed device list"),
                )
                .clicked()
            {
                action = RuntimeUiAction::ApplyDeviceDirectory;
            }
            let convergence_required = self
                .model
                .device_directory_status
                .as_ref()
                .is_some_and(|status| status.profile_convergence_status == "convergence-required");
            if ui
                .add_enabled(
                    self.model.pending.is_none()
                        && self.model.connection == ConnectionState::Connected
                        && convergence_required,
                    egui::Button::new("Reconcile launch profile"),
                )
                .clicked()
            {
                action = RuntimeUiAction::ReconcileProfile;
            }
        });
        if let Some(status) = self.model.device_directory_status.as_ref() {
            let converged = status.profile_convergence_status != "convergence-required";
            ui.colored_label(
                if converged {
                    egui::Color32::from_rgb(92, 201, 137)
                } else {
                    egui::Color32::from_rgb(246, 195, 93)
                },
                format!(
                    "Authenticated roster: revision {} · {} device(s) · profile {}",
                    status.authority_revision,
                    status.active_device_count,
                    status.profile_convergence_status
                ),
            );
            ui.small(format!(
                "Restart source: {} · receipt generation {}",
                status.restart_recovery_status,
                status
                    .receipt_generation
                    .map(|generation| generation.to_string())
                    .unwrap_or_else(|| "none (launch profile)".to_owned())
            ));
            if !converged {
                ui.small(format!(
                    "Runtime will keep using {} after a crash; the bounded action only updates the stale device-list path in the selected profile.",
                    status.applied_device_list_file.display()
                ));
            }
        }
        if let Some(update) = self.model.device_directory_update.as_ref() {
            ui.colored_label(
                egui::Color32::from_rgb(92, 201, 137),
                format!(
                    "Runtime roster active: revision {} · {} active device(s)",
                    update.authority_revision, update.active_device_count
                ),
            );
            ui.small(format!(
                "Retired: {} session record(s), {} prekey observation(s). Local pending queues: {} sealed, {} already materialized.",
                update.ratchet_session_records_retired,
                update.prekey_observations_retired,
                update.pending_unmaterialized_messages,
                update.pending_materialized_messages
            ));
            if update.launch_profile_update_required {
                ui.colored_label(
                    egui::Color32::from_rgb(246, 195, 93),
                    "The launch profile still points at the previous roster; use the bounded reconcile action above.",
                );
            }
            ui.small("The refreshed ticket excludes removed devices from future peer fanout. Previously signed recipient slots cannot be rewritten, and history already copied to a removed device cannot be erased.");
        }
        let activation_runtime_offline = self.runtime_process.is_none()
            && self.model.connection == ConnectionState::Disconnected;
        let activation_desktop_owned_runtime_connected =
            self.runtime_process.is_some() && self.model.connection == ConnectionState::Connected;
        let activation_busy = self.model.pending.is_some();
        ui.collapsing("Canonical sync v2 activation (advanced)", |ui| {
            ui.strong("Running runtime (read only)");
            if ui
                .add_enabled(
                    !activation_busy && self.model.connection == ConnectionState::Connected,
                    egui::Button::new("Refresh loaded policy and sync paths"),
                )
                .clicked()
            {
                action = RuntimeUiAction::RefreshCanonicalRuntimeStatus;
            }
            if let Some(status) = self.model.canonical_runtime_status.as_ref() {
                let loaded = &status.loaded;
                let configured = &status.configured;
                ui.colored_label(
                    if status.restart_required {
                        egui::Color32::from_rgb(246, 195, 93)
                    } else {
                        egui::Color32::from_rgb(92, 201, 137)
                    },
                    if status.restart_required {
                        "Configured activation differs from this process; restart required"
                    } else {
                        "Configured activation exactly matches the policy loaded at process start"
                    },
                );
                ui.small(format!(
                    "Loaded: {} · {} · epoch {} · {} · head {}",
                    loaded.source.as_str(),
                    loaded.migration_state.as_str(),
                    loaded.migration_epoch,
                    loaded.disposition.as_str(),
                    loaded
                        .record_id
                        .as_deref()
                        .map(compact_id)
                        .unwrap_or_else(|| "implicit".to_owned())
                ));
                ui.small(format!(
                    "Configured: {} · {} · epoch {} · {} · head {}",
                    configured.source.as_str(),
                    configured.migration_state.as_str(),
                    configured.migration_epoch,
                    configured.disposition.as_str(),
                    configured
                        .record_id
                        .as_deref()
                        .map(compact_id)
                        .unwrap_or_else(|| "implicit".to_owned())
                ));
                ui.small(format!(
                    "Loaded rights: legacy={} publish={} register={} accept={} initiate={} resume={}",
                    loaded.permissions.legacy_reader_enabled,
                    loaded.permissions.publish_canonical_capability,
                    loaded.permissions.register_canonical_alpn,
                    loaded.permissions.accept_new_canonical_session,
                    loaded.permissions.initiate_new_canonical_session,
                    loaded.permissions.resume_selected_canonical_session
                ));
                ui.separator();
                ui.strong(format!(
                    "Observed sync paths: {}/{} retained in memory",
                    status.observations.len(),
                    status.observation_capacity
                ));
                if status.observations.is_empty() {
                    ui.small("No legacy or canonical sync session has been observed by this runtime process yet.");
                } else {
                    for observation in status.observations.iter().rev().take(8) {
                        ui.small(format!(
                            "#{} {} · {} · {} · peer {}{}{}",
                            observation.sequence,
                            observation.direction.as_str(),
                            observation.protocol.as_str(),
                            observation.outcome.as_str(),
                            compact_id(&observation.peer_endpoint_id),
                            observation
                                .conversation_id
                                .map(|value| format!(" · conversation {}", compact_id(&value.to_string())))
                                .unwrap_or_default(),
                            observation
                                .selection_id
                                .as_deref()
                                .map(|value| format!(" · selection {}", compact_id(value)))
                                .unwrap_or_default()
                        ));
                    }
                }
            } else {
                ui.small("Connect to a v33 runtime to inspect its loaded policy and observed sync path. This view cannot change activation authority.");
            }
            ui.separator();
            ui.strong("Peer capability safety (read only)");
            if ui
                .add_enabled(
                    !activation_busy && self.model.connection == ConnectionState::Connected,
                    egui::Button::new("Refresh quarantine status"),
                )
                .clicked()
            {
                action = RuntimeUiAction::RefreshPeerCapabilityStatus;
            }
            if let Some(status) = self.model.peer_capability_status.as_ref() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Durable quarantine: {} contact(s)",
                        status.durable_quarantined_contact_count
                    ));
                    ui.label(format!(
                        "Scheduler suppressed: {} contact(s) / {} queue item(s)",
                        status.scheduler_suppressed_contact_count,
                        status.scheduler_suppressed_queue_count
                    ));
                    ui.label(format!(
                        "Normal network retry: {} contact(s) / {} queue item(s)",
                        status.network_retry_contact_count, status.network_retry_queue_count
                    ));
                });
                ui.small(if status.capability_store_initialized {
                    "The durable capability store existed and was opened read-only."
                } else {
                    "No durable capability store exists; this observer did not create one."
                });
                if status.items.is_empty() {
                    ui.small("No quarantined, scheduler-suppressed, or retrying contacts are visible.");
                } else {
                    for item in &status.items {
                        let reasons = if item.durable_reasons.is_empty() {
                            "none".to_owned()
                        } else {
                            item.durable_reasons
                                .iter()
                                .map(|reason| reason.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        ui.small(format!(
                            "Contact {} · durable candidates {} ({}) · scheduler sync {} · scheduler queues {} · network retries {} · pending {}",
                            compact_id(&item.contact_id),
                            item.durable_quarantined_candidate_count,
                            reasons,
                            item.scheduler_sync_suppressed,
                            item.scheduler_queue_suppressed_count,
                            item.network_retry_queue_count,
                            item.pending_queue_count
                        ));
                    }
                    if status.next_cursor.is_some() {
                        ui.small(format!(
                            "Showing the first {} of {} relevant contacts; the IPC contract supports bounded continuation pages.",
                            status.items.len(), status.total_relevant_contact_count
                        ));
                    }
                }
                ui.small("This panel grants no clear, retry, downgrade, or recovery authority.");
            } else {
                ui.small("Refresh to distinguish durable quarantine, this-process scheduler suppression, and ordinary network retry.");
            }
            ui.separator();
            match self.peer_reenrollment.draw(
                ui,
                self.model.connection == ConnectionState::Connected,
                self.runtime_process.is_some(),
                activation_busy,
                compact_id,
            ) {
                PeerReenrollmentUiAction::None => {}
                PeerReenrollmentUiAction::Refresh => {
                    action = RuntimeUiAction::RefreshPeerReenrollmentStatus;
                }
                PeerReenrollmentUiAction::Plan => {
                    action = RuntimeUiAction::PlanPeerReenrollment;
                }
                PeerReenrollmentUiAction::Approve => {
                    action = RuntimeUiAction::ApprovePeerReenrollment;
                }
                PeerReenrollmentUiAction::Accept => {
                    action = RuntimeUiAction::AcceptPeerReenrollment;
                }
                PeerReenrollmentUiAction::GuidedRestart => {
                    action = RuntimeUiAction::RestartPeerReenrollment;
                }
                PeerReenrollmentUiAction::RetryFreshVerification => {
                    action = RuntimeUiAction::RetryPeerReenrollmentFreshVerification;
                }
            }
            ui.separator();
            ui.strong("Stopped runtime authority ceremony");
            match self
                .canonical_activation
                .draw(
                    ui,
                    activation_runtime_offline,
                    activation_desktop_owned_runtime_connected,
                    activation_busy,
                )
            {
                ActivationUiAction::None => {}
                ActivationUiAction::StopAndInspect => {
                    action = RuntimeUiAction::StopAndInspectCanonicalActivation;
                }
                ActivationUiAction::RetryFreshStart => {
                    action = RuntimeUiAction::RetryCanonicalActivationFreshStart;
                }
                ActivationUiAction::Refresh => {
                    action = RuntimeUiAction::RefreshCanonicalActivation;
                }
                ActivationUiAction::Plan => {
                    action = RuntimeUiAction::PlanCanonicalActivation;
                }
                ActivationUiAction::Apply => {
                    action = RuntimeUiAction::ApplyCanonicalActivation;
                }
            }
            ui.separator();
            ui.strong("Signed release rollout observer");
            if self.rollout_operator.draw(
                ui,
                self.rollout_operator_config.is_some(),
                activation_busy,
                compact_id,
            ) {
                action = RuntimeUiAction::RefreshRolloutOperatorStatus;
            }
        });
        ui.collapsing("Wide-area ticket exchange", |ui| {
            ui.small("For an already enrolled contact, publish/fetch replaces manual synchronized ticket files. The store receives an opaque HPKE envelope under a pseudonymous directional channel; request timing and size remain visible.");
            ui.horizontal(|ui| {
                ui.label("Publication service");
                ui.add_enabled(
                    self.model.pending.is_none(),
                    egui::TextEdit::singleline(
                        &mut self.model.ticket_publication_service_url,
                    )
                    .hint_text("https://ticket-store.example")
                    .desired_width(f32::INFINITY),
                );
            });
            let exchange_enabled = self.model.pending.is_none()
                && self.model.connection == ConnectionState::Connected
                && self.model.selected_contact_id.is_some()
                && !self.model.ticket_publication_service_url.trim().is_empty();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(exchange_enabled, egui::Button::new("Publish my ticket"))
                    .clicked()
                {
                    action = RuntimeUiAction::PublishTicket;
                }
                if ui
                    .add_enabled(exchange_enabled, egui::Button::new("Refresh peer ticket"))
                    .clicked()
                {
                    action = RuntimeUiAction::RefreshContactTicket;
                }
            });
            ui.separator();
            ui.strong("Automatic exchange (opt-in)");
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled(
                    self.model.pending.is_none(),
                    egui::Checkbox::new(
                        &mut self.model.ticket_automation_allow_ethernet,
                        "Ethernet",
                    ),
                );
                ui.add_enabled(
                    self.model.pending.is_none(),
                    egui::Checkbox::new(&mut self.model.ticket_automation_allow_wifi, "Wi-Fi"),
                );
                ui.add_enabled(
                    self.model.pending.is_none(),
                    egui::Checkbox::new(
                        &mut self.model.ticket_automation_allow_mobile,
                        "Mobile",
                    ),
                );
                ui.add_enabled(
                    self.model.pending.is_none(),
                    egui::Checkbox::new(
                        &mut self.model.ticket_automation_allow_unknown_network,
                        "Unknown network",
                    ),
                );
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(exchange_enabled, egui::Button::new("Enable automatic exchange"))
                    .clicked()
                {
                    action = RuntimeUiAction::EnableTicketAutomation;
                }
                if ui
                    .add_enabled(exchange_enabled, egui::Button::new("Disable"))
                    .clicked()
                {
                    action = RuntimeUiAction::DisableTicketAutomation;
                }
                if ui
                    .add_enabled(
                        self.model.pending.is_none()
                            && self.model.connection == ConnectionState::Connected,
                        egui::Button::new("Refresh automation status"),
                    )
                    .clicked()
                {
                    action = RuntimeUiAction::RefreshTicketAutomationStatus;
                }
            });
            ui.small("Runs only while this runtime process is open. It does not register Task Scheduler, install a background service, or relay other users' traffic. Ethernet and Wi-Fi are allowed by default; mobile and unknown networks require explicit opt-in.");
            let selected_status = self
                .model
                .ticket_automation_statuses
                .iter()
                .find(|status| {
                    status.conversation == self.model.conversation
                        && status.peer_account_id.to_string() == self.model.peer_account_id
                });
            if let Some(status) = selected_status {
                let color = if status.enabled && status.network_allowed {
                    egui::Color32::from_rgb(92, 201, 137)
                } else {
                    egui::Color32::from_rgb(246, 195, 93)
                };
                ui.colored_label(
                    color,
                    format!(
                        "Automation {} · current network {} ({}) · policy generation {}",
                        if status.enabled { "enabled" } else { "disabled" },
                        status.current_network.as_str(),
                        if status.network_allowed { "allowed" } else { "blocked" },
                        status.policy_generation
                    ),
                );
                ui.small(format!(
                    "Publish: {} / {} · next {:?} · failures {}",
                    status.publish.state,
                    status.publish.last_result,
                    status.publish.next_attempt_unix_seconds,
                    status.publish.consecutive_failures
                ));
                ui.small(format!(
                    "Refresh: {} / {} · next {:?} · failures {}",
                    status.refresh.state,
                    status.refresh.last_result,
                    status.refresh.next_attempt_unix_seconds,
                    status.refresh.consecutive_failures
                ));
            }
            ui.small("This channel cannot establish first-contact trust: exchange one verified initial contact ticket out of band. Freshness is then protected by signed expiry and this device's persistent monotonic high-water mark.");
            if let Some(publication) = self.model.ticket_publication.as_ref() {
                ui.colored_label(
                    egui::Color32::from_rgb(92, 201, 137),
                    format!(
                        "Published generation {} for {} active peer device(s) · expires at Unix {}",
                        publication.publication_generation,
                        publication.recipient_device_count,
                        publication.expires_at_unix_seconds
                    ),
                );
                ui.small(format!(
                    "Channel {} · encrypted record {} bytes · {}",
                    compact_id(&publication.channel_id),
                    publication.encrypted_record_bytes,
                    publication.lookup_privacy_status
                ));
            }
            if let Some(refresh) = self.model.contact_ticket_refresh.as_ref() {
                ui.colored_label(
                    if refresh.complete {
                        egui::Color32::from_rgb(92, 201, 137)
                    } else {
                        egui::Color32::from_rgb(236, 184, 75)
                    },
                    format!(
                        "Endpoint refresh: {}/{} usable · complete {}",
                        refresh.refreshed_endpoint_candidate_count,
                        refresh.endpoint_candidate_count,
                        refresh.complete
                    ),
                );
                for result in &refresh.results {
                    ui.small(format!(
                        "{} device {} · {} · generation {} · {}",
                        if result.primary { "Primary" } else { "Alternate" },
                        compact_id(&result.peer_device_id.to_string()),
                        result.state.as_str(),
                        result
                            .publication_generation
                            .map_or_else(|| "none".to_owned(), |value| value.to_string()),
                        result.detail
                    ));
                }
            }
        });
        let mailbox_candidates = self
            .model
            .selected_contact_id
            .as_deref()
            .and_then(|contact_id| {
                self.model
                    .conversations
                    .iter()
                    .find(|summary| summary.contact_id == contact_id)
            })
            .map(|summary| {
                summary
                    .endpoint_candidates
                    .iter()
                    .map(|candidate| {
                        (
                            candidate.peer_device_id.to_string(),
                            candidate.primary,
                            candidate.state,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ui.collapsing("Blind mailbox fallback", |ui| {
            ui.small("Direct or relay delivery remains preferred. A blind mailbox stores only padded ciphertext for bounded offline delivery; its operator can still observe timing and volume.");
            if mailbox_candidates.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Exact peer Device ID");
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(&mut self.model.mailbox_peer_device_id)
                            .hint_text("Select an enrolled contact"),
                    );
                });
            } else {
                egui::ComboBox::from_label("Exact peer device")
                    .selected_text(compact_id(&self.model.mailbox_peer_device_id))
                    .show_ui(ui, |ui| {
                        for (device_id, primary, state) in &mailbox_candidates {
                            ui.add_enabled_ui(
                                *state == RuntimeIpcEndpointCandidateState::Usable,
                                |ui| {
                                    ui.selectable_value(
                                        &mut self.model.mailbox_peer_device_id,
                                        device_id.clone(),
                                        format!(
                                            "{} · {}{}",
                                            compact_id(device_id),
                                            state.as_str(),
                                            if *primary { " · primary" } else { "" }
                                        ),
                                    );
                                },
                            );
                        }
                    });
            }
            ui.label(
                "Storage: exact signed volunteer replica set (no central HTTPS mailbox)",
            );
            ui.horizontal(|ui| {
                ui.label("Validity (seconds)");
                ui.add_enabled(
                    self.model.pending.is_none(),
                    egui::TextEdit::singleline(&mut self.model.mailbox_valid_for_seconds)
                        .desired_width(140.0),
                );
            });
            let selected_device_usable = mailbox_candidates.iter().any(
                |(device_id, _, state)| {
                    device_id == &self.model.mailbox_peer_device_id
                        && *state == RuntimeIpcEndpointCandidateState::Usable
                },
            );
            let mailbox_selected = self.model.pending.is_none()
                && self.model.connection == ConnectionState::Connected
                && self.model.selected_contact_id.is_some()
                && selected_device_usable;
            let mailbox_configured = mailbox_selected
                && !self.model.mailbox_valid_for_seconds.trim().is_empty();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(mailbox_configured, egui::Button::new("Activate"))
                    .clicked()
                {
                    action = RuntimeUiAction::CreateMailbox;
                }
                if ui
                    .add_enabled(mailbox_configured, egui::Button::new("Rotate"))
                    .clicked()
                {
                    action = RuntimeUiAction::RotateMailbox;
                }
                if ui
                    .add_enabled(mailbox_selected, egui::Button::new("Revoke"))
                    .clicked()
                {
                    action = RuntimeUiAction::RevokeMailbox;
                }
                if ui
                    .add_enabled(
                        self.model.pending.is_none()
                            && self.model.connection == ConnectionState::Connected,
                        egui::Button::new("Refresh status"),
                    )
                    .clicked()
                {
                    action = RuntimeUiAction::RefreshMailboxStatus;
                }
            });
            ui.small("The service URL and store key are public connection parameters. Read/write capabilities, the Account Root and device keys stay inside the single runtime owner and never cross desktop IPC.");
            if let Some(transition) = self.model.mailbox_transition.as_ref() {
                ui.colored_label(
                    egui::Color32::from_rgb(92, 201, 137),
                    format!(
                        "Last change: {} · generation {} · update {} · {}",
                        transition.action,
                        transition.generation,
                        compact_id(&transition.update_id),
                        transition.delivery_state
                    ),
                );
            }
            if let Some(status) = self.model.mailbox_status.as_ref() {
                ui.horizontal_wrapped(|ui| {
                    metric(ui, "Local capabilities", status.local_binding_count);
                    metric(ui, "Peer capabilities", status.peer_binding_count);
                    metric(ui, "Pending uploads", status.pending_upload_count as usize);
                    metric(ui, "Stored uploads", status.stored_upload_count as usize);
                });
                ui.small(format!(
                    "Capability convergence: {}/{} local updates acknowledged · delivery {}",
                    status.local_acknowledged_update_count,
                    status.local_update_count,
                    status.delivery_state
                ));
                let selected_contact_id = self.model.selected_contact_id.as_deref();
                for capability in status
                    .capabilities
                    .iter()
                    .filter(|capability| Some(capability.contact_id.as_str()) == selected_contact_id)
                {
                    let healthy = capability.state == "active" && !capability.revoked;
                    let acknowledgement = capability.acknowledged.map_or_else(
                        || "peer-observed".to_owned(),
                        |acknowledged| {
                            if acknowledged {
                                "acknowledged".to_owned()
                            } else {
                                "awaiting-ack".to_owned()
                            }
                        },
                    );
                    ui.colored_label(
                        if healthy {
                            egui::Color32::from_rgb(92, 201, 137)
                        } else {
                            egui::Color32::from_rgb(246, 195, 93)
                        },
                        format!(
                            "{} · {} · device {} · generation {} · {} · {} · locator {} ({})",
                            capability.direction,
                            capability.capability_format,
                            compact_id(&capability.peer_device_id.to_string()),
                            capability
                                .generation
                                .map_or_else(|| "legacy".to_owned(), |value| value.to_string()),
                            capability.state,
                            acknowledgement,
                            capability.replica_set_discovery,
                            capability.replica_set_store_count,
                        ),
                    );
                }
            }
        });
        action
    }

    fn draw_identity(&self, ui: &mut egui::Ui) {
        let (Some(account_id), Some(device_id)) = (
            self.model.account_id.as_ref(),
            self.model.device_id.as_ref(),
        ) else {
            return;
        };
        ui.separator();
        ui.strong("Runtime identity");
        egui::Grid::new("runtime-identity")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Account ID");
                ui.monospace(account_id);
                ui.end_row();
                ui.label("Device ID");
                ui.monospace(device_id);
                ui.end_row();
            });
    }

    fn draw_composer(&mut self, ui: &mut egui::Ui) -> bool {
        ui.separator();
        let editable = self.model.connection == ConnectionState::Connected
            && self.model.pending.is_none()
            && self.model.selected_contact_id.is_some();
        ui.add_enabled(
            editable,
            egui::TextEdit::multiline(&mut self.model.message)
                .hint_text("Message")
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        ui.add_enabled(editable, egui::Button::new("Queue message"))
            .clicked()
    }

    fn draw_contact_onboarding(&mut self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            ui.heading("Chats");
            if ui
                .add_enabled(
                    self.model.connection == ConnectionState::Connected
                        && self.model.pending.is_none(),
                    egui::Button::new("+ Contact"),
                )
                .clicked()
            {
                self.model.show_contact_form = !self.model.show_contact_form;
            }
        });
        if !self.model.show_contact_form {
            return false;
        }
        let mut submit = false;
        ui.group(|ui| {
            ui.strong("Import signed runtime contact");
            ui.add_enabled(
                self.model.pending.is_none(),
                egui::TextEdit::singleline(&mut self.model.contact_draft.conversation)
                    .hint_text("Conversation label"),
            );
            ui.add_enabled(
                self.model.pending.is_none(),
                egui::TextEdit::singleline(&mut self.model.contact_draft.peer_account_id)
                    .hint_text("Expected peer Account ID"),
            );
            ui.add_enabled(
                self.model.pending.is_none(),
                egui::TextEdit::singleline(&mut self.model.contact_draft.descriptor_file)
                    .hint_text("Peer runtime ticket path"),
            );
            ui.small("Import another signed ticket for the same peer and conversation to add an authenticated endpoint candidate.");
            submit = ui
                .add_enabled(self.model.pending.is_none(), egui::Button::new("Add contact"))
                .clicked();
        });
        submit
    }

    fn draw_conversations(&self, ui: &mut egui::Ui) -> Option<String> {
        if self.model.conversations.is_empty() {
            ui.label("No runtime contacts yet.");
            ui.small("Use + Contact to import a signed peer runtime ticket.");
            return None;
        }
        let mut selected = None;
        egui::ScrollArea::vertical()
            .id_salt("conversation-list")
            .show(ui, |ui| {
                for conversation in &self.model.conversations {
                    let active = self.model.selected_contact_id.as_deref()
                        == Some(conversation.contact_id.as_str());
                    let title = format!(
                        "{}  ({})",
                        conversation.conversation_label, conversation.message_count
                    );
                    if ui.selectable_label(active, title).clicked() {
                        selected = Some(conversation.contact_id.clone());
                    }
                    if let Some(preview) = &conversation.latest_message {
                        let suffix = if preview.truncated { "…" } else { "" };
                        ui.small(format!("{}{}", preview.body.replace('\n', " "), suffix));
                    } else {
                        ui.small("No messages");
                    }
                    ui.separator();
                }
            });
        selected
    }

    fn draw_history(&self, ui: &mut egui::Ui) -> bool {
        let Some(selected) = self.model.selected_contact_id.as_ref() else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a chat to read local history.");
            });
            return false;
        };
        let summary = self
            .model
            .conversations
            .iter()
            .find(|summary| &summary.contact_id == selected);
        ui.horizontal(|ui| {
            ui.heading(summary.map_or(self.model.conversation.as_str(), |item| {
                item.conversation_label.as_str()
            }));
            ui.small(format!("{} messages", self.model.history_total));
        });
        if let Some(summary) = summary {
            ui.small(format!(
                "Peer {} · {} · {} endpoint(s): {} usable / {} stale / {} quarantined",
                compact_id(&summary.peer_account_id.to_string()),
                summary.route_policy.as_str(),
                summary.endpoint_candidate_count,
                summary.usable_endpoint_candidate_count,
                summary.stale_endpoint_candidate_count,
                summary.quarantined_endpoint_candidate_count
            ));
            for candidate in &summary.endpoint_candidates {
                let line = format!(
                    "{} device {} · {} · {} · publication high-water {}",
                    if candidate.primary {
                        "Primary"
                    } else {
                        "Alternate"
                    },
                    compact_id(&candidate.peer_device_id.to_string()),
                    candidate.state.as_str(),
                    candidate.detail,
                    candidate
                        .observed_publication_generation
                        .map_or_else(|| "none".to_owned(), |value| value.to_string())
                );
                if candidate.publication_conflict_proof_id.is_some() {
                    ui.colored_label(egui::Color32::LIGHT_RED, line);
                    ui.small(format!(
                        "Conflict generation {} · signed proof {} · audit the claims, rotate the peer channel, then use Security incident recovery below",
                        candidate
                            .publication_conflict_generation
                            .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
                        candidate
                            .publication_conflict_proof_id
                            .as_deref()
                            .map_or_else(|| "unknown".to_owned(), compact_id)
                    ));
                } else {
                    ui.small(line);
                }
            }
        }
        let load_older = self.model.history_next_cursor.is_some()
            && ui
                .add_enabled(
                    self.model.pending.is_none(),
                    egui::Button::new("Load older messages"),
                )
                .clicked();
        egui::ScrollArea::vertical()
            .id_salt("chat-history")
            .max_height(360.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.model.history.is_empty() {
                    ui.label("No local messages in this chat.");
                }
                for message in &self.model.history {
                    let outgoing = self.model.account_id.as_deref()
                        == Some(message.author_account_id.to_string().as_str());
                    ui.group(|ui| {
                        ui.small(if outgoing { "You" } else { "Peer" });
                        ui.label(&message.body);
                        ui.small(format!(
                            "#{} · {}",
                            message.author_sequence,
                            compact_id(&message.event_id.to_string())
                        ));
                    });
                }
            });
        load_older
    }

    fn draw_outbox(&mut self, ui: &mut egui::Ui) -> bool {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Outbox");
            ui.add_enabled(
                self.model.connection == ConnectionState::Connected && self.model.pending.is_none(),
                egui::Button::new("Refresh"),
            )
            .clicked()
        })
        .inner
    }

    fn draw_outbox_contents(&self, ui: &mut egui::Ui) {
        let Some(outbox) = self.model.outbox.as_ref() else {
            ui.label("No runtime status loaded yet.");
            return;
        };
        ui.horizontal_wrapped(|ui| {
            metric(ui, "Contacts", outbox.contact_count);
            metric(ui, "Queued", outbox.queue_count);
            metric(ui, "Pending", outbox.pending_count);
            metric(ui, "Materialized", outbox.materialized_count);
            metric(ui, "Delivered", outbox.delivered_count);
            metric(ui, "Mailbox pending", outbox.mailbox_pending_count);
            metric(ui, "Mailbox stored", outbox.mailbox_stored_count);
            metric(ui, "Mailbox expired", outbox.mailbox_expired_count);
            metric(ui, "Mailbox failed", outbox.mailbox_failed_count);
            metric(ui, "Retries", outbox.retry_state_count);
        });
        if outbox.items.is_empty() {
            ui.label("Outbox is empty.");
            return;
        }
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("outbox-items")
            .max_height(250.0)
            .show(ui, |ui| {
                egui::Grid::new("outbox-grid")
                    .striped(true)
                    .num_columns(5)
                    .spacing([14.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Queue");
                        ui.strong("Peer");
                        ui.strong("Conversation");
                        ui.strong("State");
                        ui.strong("ACK");
                        ui.end_row();
                        for item in &outbox.items {
                            ui.monospace(compact_id(&item.queue_id));
                            ui.monospace(compact_id(&item.peer_account_id));
                            ui.monospace(compact_id(&item.conversation_id));
                            ui.label(item.state);
                            ui.monospace(
                                item.acknowledgement_event_id
                                    .as_deref()
                                    .map(compact_id)
                                    .unwrap_or_else(|| "—".to_owned()),
                            );
                            ui.end_row();
                        }
                    });
            });
    }

    fn draw_feedback(&self, ui: &mut egui::Ui) {
        if let Some(notice) = self.model.notice.as_ref() {
            ui.colored_label(egui::Color32::from_rgb(92, 201, 137), notice);
        }
        if let Some(error) = self.model.error.as_ref() {
            ui.colored_label(
                egui::Color32::from_rgb(239, 112, 112),
                format!("Error: {error}"),
            );
        }
    }
}

fn required_path(value: &str, label: &str) -> Result<PathBuf> {
    let value = value.trim();
    ensure!(!value.is_empty(), "{label} path is required");
    ensure!(
        value.len() <= MAX_DESCRIPTOR_PATH_BYTES,
        "{label} path is too long"
    );
    Ok(PathBuf::from(value))
}

fn absolute_new_directory_path(value: &str, label: &str) -> Result<PathBuf> {
    let path = required_path(value, label)?;
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .context("Read current directory for account bootstrap")?
            .join(path)
    };
    ensure!(!absolute.exists(), "{label} already exists");
    let name = absolute
        .file_name()
        .context(format!("{label} path has no final component"))?;
    let parent = absolute
        .parent()
        .context(format!("{label} path has no parent"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Create {label} parent {}", parent.display()))?;
    let resolved = fs::canonicalize(parent)
        .with_context(|| format!("Resolve {label} parent {}", parent.display()))?
        .join(name);
    ensure!(!resolved.exists(), "{label} already exists");
    Ok(resolved)
}

fn reconcile_runtime_launch_profile(
    profile_path: &std::path::Path,
    descriptor_path: &std::path::Path,
    status: &RuntimeIpcDeviceDirectoryStatus,
) -> Result<RuntimeLaunchProfile> {
    ensure!(
        status.profile_convergence_status == "convergence-required",
        "Runtime does not report profile convergence as required"
    );
    ensure!(
        status.receipt_id.is_some() && status.receipt_generation.is_some(),
        "Runtime roster has no authenticated persistence receipt"
    );
    let profile_metadata = fs::symlink_metadata(profile_path)
        .with_context(|| format!("Inspect runtime profile {}", profile_path.display()))?;
    ensure!(
        profile_metadata.is_file() && !profile_metadata.file_type().is_symlink(),
        "Runtime profile must be a regular non-symlink file"
    );
    let original_bytes = fs::read(profile_path)
        .with_context(|| format!("Read runtime profile {}", profile_path.display()))?;
    let profile = RuntimeLaunchProfile::load(profile_path)?;
    ensure!(
        fs::read(profile_path)
            .with_context(|| format!("Re-read runtime profile {}", profile_path.display()))?
            == original_bytes,
        "Runtime profile changed while it was being verified"
    );
    ensure!(
        profile.settings().state_dir == status.state_dir,
        "Runtime profile state directory differs from the authenticated running runtime"
    );
    let canonical_descriptor = fs::canonicalize(descriptor_path)
        .with_context(|| format!("Resolve IPC descriptor {}", descriptor_path.display()))?;
    let canonical_profile_descriptor = fs::canonicalize(&profile.settings().ipc_file)
        .with_context(|| {
            format!(
                "Resolve profile IPC descriptor {}",
                profile.settings().ipc_file.display()
            )
        })?;
    ensure!(
        canonical_descriptor == canonical_profile_descriptor,
        "Runtime profile IPC path differs from the authenticated connected runtime"
    );
    ensure!(
        profile.settings().device_list_file == status.launch_device_list_file,
        "Runtime profile device-list path changed since runtime launch"
    );
    let applied_metadata =
        fs::symlink_metadata(&status.applied_device_list_file).with_context(|| {
            format!(
                "Inspect authenticated device list {}",
                status.applied_device_list_file.display()
            )
        })?;
    ensure!(
        applied_metadata.is_file() && !applied_metadata.file_type().is_symlink(),
        "Authenticated device list must remain a regular non-symlink file"
    );
    let canonical_applied =
        fs::canonicalize(&status.applied_device_list_file).with_context(|| {
            format!(
                "Resolve authenticated device list {}",
                status.applied_device_list_file.display()
            )
        })?;
    ensure!(
        canonical_applied == status.applied_device_list_file,
        "Authenticated device-list path changed since runtime verification"
    );
    let applied_bytes = fs::read(&canonical_applied).with_context(|| {
        format!(
            "Read authenticated device list {}",
            canonical_applied.display()
        )
    })?;
    let applied_device_list = AccountDeviceListSnapshot::decode_and_verify(&applied_bytes)
        .context("Verify authenticated device-list signature")?;
    applied_device_list
        .verify_for_account(status.account_id)
        .context("Verify authenticated device-list account")?;
    ensure!(
        applied_device_list.revision() == status.authority_revision
            && applied_device_list.devices().len() == status.active_device_count
            && applied_device_list
                .certificate_for(status.local_device_id)
                .is_some(),
        "Authenticated device list differs from the running runtime roster"
    );
    let mut digest = blake3::Hasher::new();
    digest.update(RUNTIME_DEVICE_LIST_DIGEST_DOMAIN);
    digest.update(
        &applied_device_list
            .encode()
            .context("Encode authenticated device list for digest verification")?,
    );
    ensure!(
        digest.finalize().to_hex().as_str() == status.device_list_digest,
        "Authenticated device-list digest differs from the running runtime"
    );
    let replacement = profile.with_device_list_file(canonical_applied)?;
    ensure!(
        fs::read(profile_path)
            .with_context(|| format!("Re-read runtime profile {}", profile_path.display()))?
            == original_bytes,
        "Runtime profile changed before the bounded replacement"
    );
    let final_applied_metadata = fs::symlink_metadata(&status.applied_device_list_file)
        .with_context(|| {
            format!(
                "Re-inspect authenticated device list {}",
                status.applied_device_list_file.display()
            )
        })?;
    ensure!(
        final_applied_metadata.is_file() && !final_applied_metadata.file_type().is_symlink(),
        "Authenticated device list changed before profile replacement"
    );
    ensure!(
        fs::canonicalize(&status.applied_device_list_file).with_context(|| {
            format!(
                "Re-resolve authenticated device list {}",
                status.applied_device_list_file.display()
            )
        })? == status.applied_device_list_file
            && fs::read(&status.applied_device_list_file).with_context(|| {
                format!(
                    "Re-read authenticated device list {}",
                    status.applied_device_list_file.display()
                )
            })? == applied_bytes,
        "Authenticated device list changed before profile replacement"
    );
    replacement.write_replace(profile_path)?;
    let verified = RuntimeLaunchProfile::load(profile_path)?;
    ensure!(
        verified == replacement,
        "Runtime profile verification failed after replacement"
    );
    Ok(verified)
}

fn canonical_input_path(value: &str, label: &str, require_file: bool) -> Result<PathBuf> {
    let path = required_path(value, label)?;
    let path = fs::canonicalize(&path)
        .with_context(|| format!("Resolve {label} path {}", path.display()))?;
    let metadata =
        fs::metadata(&path).with_context(|| format!("Inspect {label} path {}", path.display()))?;
    if require_file {
        ensure!(metadata.is_file(), "{label} must be a file");
    } else {
        ensure!(metadata.is_dir(), "{label} must be a directory");
    }
    Ok(path)
}

fn canonical_nonsymlink_input_file(value: &str, label: &str) -> Result<PathBuf> {
    let path = required_path(value, label)?;
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("Inspect {label} path {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "{label} must not be a symlink"
    );
    ensure!(metadata.is_file(), "{label} must be a regular file");
    fs::canonicalize(&path).with_context(|| format!("Resolve {label} path {}", path.display()))
}

fn canonical_artifact_lines(value: &str, label: &str) -> Result<Vec<PathBuf>> {
    let paths = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| canonical_nonsymlink_input_file(line, label))
        .collect::<Result<Vec<_>>>()?;
    ensure!(!paths.is_empty(), "At least one {label} path is required");
    ensure!(paths.len() <= 32, "Too many {label} paths");
    let mut distinct = BTreeSet::new();
    for path in &paths {
        ensure!(
            distinct.insert(path.clone()),
            "Duplicate {label} path: {}",
            path.display()
        );
    }
    Ok(paths)
}

fn absolute_artifact_output_path(value: &str, label: &str) -> Result<PathBuf> {
    let path = required_path(value, label)?;
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .context("Read current directory for recovery artifact")?
            .join(path)
    };
    let file_name = absolute
        .file_name()
        .context(format!("{label} path has no file name"))?;
    let parent = absolute
        .parent()
        .context(format!("{label} path has no parent"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Create {label} directory {}", parent.display()))?;
    Ok(fs::canonicalize(parent)
        .with_context(|| format!("Resolve {label} directory {}", parent.display()))?
        .join(file_name))
}

fn absolute_directory_target(value: &str, label: &str) -> Result<PathBuf> {
    let path = required_path(value, label)?;
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .context("Read current directory for recovery directory")?
            .join(path)
    };
    if absolute.exists() {
        let metadata = fs::symlink_metadata(&absolute)
            .with_context(|| format!("Inspect {label} {}", absolute.display()))?;
        ensure!(
            !metadata.file_type().is_symlink() && metadata.is_dir(),
            "{label} must be a regular directory"
        );
        return fs::canonicalize(&absolute)
            .with_context(|| format!("Resolve {label} {}", absolute.display()));
    }
    let name = absolute
        .file_name()
        .context(format!("{label} path has no final component"))?;
    let parent = absolute
        .parent()
        .context(format!("{label} path has no parent"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Create {label} parent {}", parent.display()))?;
    Ok(fs::canonicalize(parent)
        .with_context(|| format!("Resolve {label} parent {}", parent.display()))?
        .join(name))
}

fn route_policy_argument(policy: RuntimeIpcRoutePolicy) -> &'static str {
    match policy {
        RuntimeIpcRoutePolicy::Auto => "auto",
        RuntimeIpcRoutePolicy::DirectOnly => "direct-only",
        RuntimeIpcRoutePolicy::RelayOnly => "relay-only",
    }
}

fn absolute_output_path(
    value: &str,
    label: &str,
    protected_state: &std::path::Path,
) -> Result<PathBuf> {
    let path = required_path(value, label)?;
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .context("Read current directory for runtime profile")?
            .join(path)
    };
    ensure!(
        !absolute.starts_with(protected_state),
        "{label} must live outside the protected state directory"
    );
    let file_name = absolute
        .file_name()
        .context(format!("{label} path has no file name"))?;
    let parent = absolute
        .parent()
        .context(format!("{label} path has no parent"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Create {label} directory {}", parent.display()))?;
    let resolved = fs::canonicalize(parent)
        .with_context(|| format!("Resolve {label} directory {}", parent.display()))?
        .join(file_name);
    ensure!(
        !resolved.starts_with(protected_state),
        "{label} must live outside the protected state directory"
    );
    Ok(resolved)
}

fn optional_output_path(
    value: &str,
    label: &str,
    protected_state: &std::path::Path,
) -> Result<Option<PathBuf>> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        absolute_output_path(value, label, protected_state).map(Some)
    }
}

fn parse_profile_number(value: &str, label: &str) -> Result<u64> {
    value
        .trim()
        .parse::<u64>()
        .with_context(|| format!("{label} must be a non-negative integer"))
}

fn default_volunteer_storage_path(state_dir: &Path) -> Result<PathBuf> {
    let parent = state_dir
        .parent()
        .context("State directory has no parent for volunteer storage")?;
    let name = state_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("state");
    Ok(parent.join(format!("{name}-volunteer-storage")))
}

fn parse_profile_mib(value: &str, label: &str) -> Result<u64> {
    parse_profile_number(value, label)?
        .checked_mul(BYTES_PER_MIB)
        .with_context(|| format!("{label} MiB value overflows"))
}

fn bytes_to_mib(value: u64) -> String {
    value.div_ceil(BYTES_PER_MIB).to_string()
}

fn run_bootstrap_process(
    executable: &std::path::Path,
    workspace: &std::path::Path,
) -> Result<DesktopBootstrapOutput> {
    let mut command = Command::new(executable);
    command
        .arg("create")
        .arg("--workspace-dir")
        .arg(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().with_context(|| {
        format!(
            "Start bootstrap executable {} for {}",
            executable.display(),
            workspace.display()
        )
    })?;
    ensure!(
        output.stdout.len() <= MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES,
        "Bootstrap helper output is too large"
    );
    ensure!(
        output.stderr.len() <= MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES,
        "Bootstrap helper error output is too large"
    );
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Bootstrap helper exited with {}: {}",
            output.status,
            detail.trim()
        );
    }
    let stdout = Zeroizing::new(output.stdout);
    let result =
        DesktopBootstrapOutput::decode(&stdout).context("Validate bootstrap helper result")?;
    ensure!(
        result.workspace_dir() == workspace,
        "Bootstrap helper returned a different workspace"
    );
    Ok(result)
}

fn spawn_runtime_process(executable: &std::path::Path, profile: &std::path::Path) -> Result<Child> {
    let mut command = Command::new(executable);
    command
        .arg("runtime-from-profile")
        .arg("--profile-file")
        .arg(profile)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn().with_context(|| {
        format!(
            "Start runtime executable {} with profile {}",
            executable.display(),
            profile.display()
        )
    })
}

impl Drop for KilogramApp {
    fn drop(&mut self) {
        self.stop_change_subscription();
        let Some(mut child) = self.runtime_process.take() else {
            return;
        };
        if let Ok(descriptor) = self.model.descriptor()
            && let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
        {
            let _ = runtime.block_on(tokio::time::timeout(
                RUNTIME_STOP_TIMEOUT,
                kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::Shutdown),
            ));
        }
        let deadline = Instant::now() + RUNTIME_STOP_TIMEOUT;
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl eframe::App for KilogramApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.accept_dropped_descriptor(context);
        self.receive_worker_responses();
        self.receive_change_events();
        self.observe_runtime_process();
        self.enforce_guided_stop_deadline();
        self.enforce_peer_reenrollment_stop_deadline();
        self.advance_guided_canonical_activation();
        self.maybe_connect_started_runtime();

        let mut runtime_action = RuntimeUiAction::None;
        let mut bootstrap_action = BootstrapUiAction::None;
        let mut account_recovery_action = AccountRecoveryUiAction::None;
        let mut device_link_action = DeviceLinkUiAction::None;
        let mut recovery_policy_action = RecoveryPolicyUiAction::None;
        let mut recovery_action = RecoveryUiAction::None;
        let mut publication_conflict_action = PublicationConflictUiAction::None;
        let mut add_contact_clicked = false;
        let mut queue_clicked = false;
        let mut refresh_clicked = false;
        let mut selected_contact = None;
        let mut load_older_clicked = false;
        egui::CentralPanel::default().show(context, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("desktop-content")
                .show(ui, |ui| {
                    self.draw_header(ui);
                    ui.add_space(8.0);
                    bootstrap_action = self.draw_bootstrap(ui);
                    ui.add_space(4.0);
                    account_recovery_action = self.draw_account_recovery(ui);
                    ui.add_space(4.0);
                    device_link_action = self.draw_device_link(ui);
                    ui.add_space(4.0);
                    let removal_action = self.draw_device_removal(ui);
                    if removal_action != DeviceLinkUiAction::None {
                        device_link_action = removal_action;
                    }
                    ui.add_space(4.0);
                    recovery_policy_action = self.draw_recovery_policy(ui);
                    ui.add_space(4.0);
                    recovery_action = self.draw_recovery(ui);
                    ui.add_space(4.0);
                    publication_conflict_action = self.draw_publication_conflict_recovery(ui);
                    ui.add_space(4.0);
                    runtime_action = self.draw_runtime(ui);
                    self.draw_identity(ui);
                    ui.separator();
                    ui.columns(2, |columns| {
                        add_contact_clicked = self.draw_contact_onboarding(&mut columns[0]);
                        selected_contact = self.draw_conversations(&mut columns[0]);
                        load_older_clicked = self.draw_history(&mut columns[1]);
                        queue_clicked = self.draw_composer(&mut columns[1]);
                    });
                    egui::CollapsingHeader::new("Runtime outbox")
                        .default_open(false)
                        .show(ui, |ui| {
                            refresh_clicked = self.draw_outbox(ui);
                            self.draw_outbox_contents(ui);
                        });
                    ui.add_space(8.0);
                    self.draw_feedback(ui);
                });
        });

        if bootstrap_action == BootstrapUiAction::Create {
            self.start_bootstrap();
        } else if bootstrap_action == BootstrapUiAction::DismissPhrase {
            self.bootstrap_view = None;
            self.model.notice = Some(
                "Recovery phrase removed from the desktop process. Complete peer setup in the launch profile."
                    .to_owned(),
            );
        } else if account_recovery_action == AccountRecoveryUiAction::Status {
            self.start_account_recovery_status();
        } else if account_recovery_action == AccountRecoveryUiAction::Export {
            self.start_account_recovery_export();
        } else if account_recovery_action == AccountRecoveryUiAction::Inspect {
            self.start_account_recovery_inspect();
        } else if account_recovery_action == AccountRecoveryUiAction::Restore {
            self.start_account_recovery_restore();
        } else if account_recovery_action == AccountRecoveryUiAction::QuorumRequest {
            self.start_account_recovery_quorum_request();
        } else if account_recovery_action == AccountRecoveryUiAction::QuorumListen {
            self.start_account_recovery_quorum_listen();
        } else if account_recovery_action == AccountRecoveryUiAction::QuorumCollect {
            self.start_account_recovery_quorum_collect();
        } else if account_recovery_action == AccountRecoveryUiAction::QuorumVerify {
            self.start_account_recovery_quorum_verify();
        } else if device_link_action == DeviceLinkUiAction::Request {
            self.start_device_link_request();
        } else if device_link_action == DeviceLinkUiAction::Inspect {
            self.start_device_link_inspect();
        } else if device_link_action == DeviceLinkUiAction::Authorize {
            self.start_device_link_authorize();
        } else if device_link_action == DeviceLinkUiAction::Accept {
            self.start_device_link_accept();
        } else if device_link_action == DeviceLinkUiAction::Remove {
            self.start_device_remove();
        } else if recovery_policy_action == RecoveryPolicyUiAction::Request {
            self.start_recovery_policy_request();
        } else if recovery_policy_action == RecoveryPolicyUiAction::Listen {
            self.start_recovery_policy_listen();
        } else if recovery_policy_action == RecoveryPolicyUiAction::Collect {
            self.start_recovery_policy_collect();
        } else if recovery_policy_action == RecoveryPolicyUiAction::Certify {
            self.start_recovery_policy_certify();
        } else if recovery_policy_action == RecoveryPolicyUiAction::Install {
            self.start_recovery_policy_install();
        } else if recovery_action == RecoveryUiAction::Approve {
            self.start_recovery_approve();
        } else if recovery_action == RecoveryUiAction::AddExisting {
            self.add_existing_recovery_plan();
        } else if recovery_action == RecoveryUiAction::RunSelected {
            self.start_recovery_run();
        } else if recovery_action == RecoveryUiAction::CancelSelected {
            self.start_recovery_cancel();
        } else if recovery_action == RecoveryUiAction::Reconcile {
            self.start_recovery_reconcile();
        } else if publication_conflict_action == PublicationConflictUiAction::RotateOwnChannel {
            self.start_publication_channel_rotation();
        } else if publication_conflict_action == PublicationConflictUiAction::CreateRequest {
            self.start_publication_conflict_request();
        } else if publication_conflict_action == PublicationConflictUiAction::ApplyResponse {
            self.start_publication_conflict_apply();
        } else if runtime_action == RuntimeUiAction::Connect {
            self.start_connect();
        } else if runtime_action == RuntimeUiAction::Start {
            self.start_runtime();
        } else if runtime_action == RuntimeUiAction::Stop {
            self.start_shutdown();
        } else if runtime_action == RuntimeUiAction::LoadProfile {
            self.load_runtime_profile();
        } else if runtime_action == RuntimeUiAction::SaveProfile {
            self.save_runtime_profile();
        } else if runtime_action == RuntimeUiAction::ApplyDeviceDirectory {
            self.start_apply_device_directory();
        } else if runtime_action == RuntimeUiAction::ReconcileProfile {
            self.reconcile_runtime_profile();
        } else if runtime_action == RuntimeUiAction::PublishTicket {
            self.start_publish_ticket();
        } else if runtime_action == RuntimeUiAction::RefreshContactTicket {
            self.start_refresh_contact_ticket();
        } else if runtime_action == RuntimeUiAction::EnableTicketAutomation {
            self.start_configure_ticket_automation(true);
        } else if runtime_action == RuntimeUiAction::DisableTicketAutomation {
            self.start_configure_ticket_automation(false);
        } else if runtime_action == RuntimeUiAction::RefreshTicketAutomationStatus {
            self.start_ticket_automation_status();
        } else if runtime_action == RuntimeUiAction::CreateMailbox {
            self.start_mailbox_change(Operation::MailboxCreate);
        } else if runtime_action == RuntimeUiAction::RotateMailbox {
            self.start_mailbox_change(Operation::MailboxRotate);
        } else if runtime_action == RuntimeUiAction::RevokeMailbox {
            self.start_mailbox_change(Operation::MailboxRevoke);
        } else if runtime_action == RuntimeUiAction::RefreshMailboxStatus {
            self.start_mailbox_status();
        } else if runtime_action == RuntimeUiAction::RefreshCanonicalActivation {
            self.start_canonical_activation_status();
        } else if runtime_action == RuntimeUiAction::StopAndInspectCanonicalActivation {
            self.start_guided_canonical_activation();
        } else if runtime_action == RuntimeUiAction::RetryCanonicalActivationFreshStart {
            self.retry_guided_canonical_activation_fresh_start();
        } else if runtime_action == RuntimeUiAction::PlanCanonicalActivation {
            self.start_canonical_activation_plan();
        } else if runtime_action == RuntimeUiAction::ApplyCanonicalActivation {
            self.start_canonical_activation_apply();
        } else if runtime_action == RuntimeUiAction::RefreshRolloutOperatorStatus {
            self.start_rollout_operator_status();
        } else if runtime_action == RuntimeUiAction::RefreshCanonicalRuntimeStatus {
            self.start_canonical_runtime_status();
        } else if runtime_action == RuntimeUiAction::RefreshPeerCapabilityStatus {
            self.start_peer_capability_status();
        } else if runtime_action == RuntimeUiAction::RefreshPeerReenrollmentStatus {
            self.start_peer_reenrollment_status();
        } else if runtime_action == RuntimeUiAction::PlanPeerReenrollment {
            self.start_peer_reenrollment_mutation(Operation::PeerReenrollmentPlan);
        } else if runtime_action == RuntimeUiAction::ApprovePeerReenrollment {
            self.start_peer_reenrollment_mutation(Operation::PeerReenrollmentApprove);
        } else if runtime_action == RuntimeUiAction::AcceptPeerReenrollment {
            self.start_peer_reenrollment_mutation(Operation::PeerReenrollmentAccept);
        } else if runtime_action == RuntimeUiAction::RestartPeerReenrollment {
            self.start_guided_peer_reenrollment_restart();
        } else if runtime_action == RuntimeUiAction::RetryPeerReenrollmentFreshVerification {
            self.retry_peer_reenrollment_fresh_verification();
        } else if add_contact_clicked {
            self.start_add_contact();
        } else if let Some(contact_id) = selected_contact {
            if self.model.select_conversation(&contact_id) {
                self.publication_conflict_recovery = PublicationConflictRecoveryView::default();
                self.start_history(false);
            }
        } else if load_older_clicked {
            self.start_history(true);
        } else if queue_clicked {
            self.start_queue();
        } else if refresh_clicked {
            self.start_refresh();
        }
        context.request_repaint_after(Duration::from_millis(200));
    }
}

fn metric(ui: &mut egui::Ui, label: &str, value: usize) {
    ui.label(format!("{label}: {value}"));
}

fn compact_id(value: &str) -> String {
    const EDGE: usize = 6;
    if value.len() <= EDGE * 2 + 1 {
        return value.to_owned();
    }
    format!("{}…{}", &value[..EDGE], &value[value.len() - EDGE..])
}

fn valid_publication_conflict_confirmation_code(value: &str) -> bool {
    let Some(groups) = value.strip_prefix("KPC1-") else {
        return false;
    };
    let mut groups = groups.split('-');
    (0..6).all(|_| {
        groups.next().is_some_and(|group| {
            group.len() == 4 && group.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    }) && groups.next().is_none()
}

#[cfg(test)]
mod tests {
    use kilogram_identity::{AccountRootState, DeviceCapability, DeviceIdentity, DeviceState};
    use kilogram_runtime_ipc::{
        ConversationId, RuntimeIpcOutboxStatus, RuntimeIpcRoutePolicy, RuntimeIpcServer,
    };

    use super::*;

    const ACCOUNT_ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn desktop_options_keep_explicit_runtime_paths_and_reject_unknown_flags() {
        let options = DesktopOptions::from_arguments([
            OsString::from("--ipc-file"),
            OsString::from("desktop.ipc.json"),
            OsString::from("--runtime-profile"),
            OsString::from("missing-profile.json"),
            OsString::from("--runtime-exe"),
            OsString::from("runtime-test.exe"),
            OsString::from("--bootstrap-exe"),
            OsString::from("bootstrap-test.exe"),
            OsString::from("--rollout-dir"),
            OsString::from("rollout-state"),
            OsString::from("--trusted-policy-dir"),
            OsString::from("trusted-policy-state"),
            OsString::from("--release-trust-root"),
            OsString::from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ]);
        assert_eq!(options.descriptor_path, PathBuf::from("desktop.ipc.json"));
        assert_eq!(
            options.runtime_profile_path,
            PathBuf::from("missing-profile.json")
        );
        assert_eq!(
            options.runtime_executable_path,
            PathBuf::from("runtime-test.exe")
        );
        assert_eq!(
            options.bootstrap_executable_path,
            PathBuf::from("bootstrap-test.exe")
        );
        assert_eq!(
            options.rollout_directory,
            Some(PathBuf::from("rollout-state"))
        );
        assert_eq!(
            options.trusted_policy_directory,
            Some(PathBuf::from("trusted-policy-state"))
        );
        assert_eq!(
            options.release_trust_root.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert!(options.startup_error.is_none());

        let unknown = DesktopOptions::from_arguments([OsString::from("--unknown")]);
        assert!(unknown.startup_error.is_some());
        let partial = DesktopOptions::from_arguments([
            OsString::from("--rollout-dir"),
            OsString::from("rollout-state"),
        ]);
        assert!(partial.startup_error.is_some());
    }

    #[test]
    fn runtime_profile_reconciliation_is_bounded_and_rejects_path_drift()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root_path = fs::canonicalize(directory.path())?;
        let state_dir = root_path.join("state");
        fs::create_dir(&state_dir)?;
        let state_dir = fs::canonicalize(state_dir)?;
        let descriptor = root_path.join("runtime.ipc.json");
        fs::write(&descriptor, b"authenticated descriptor placeholder")?;
        let descriptor = fs::canonicalize(descriptor)?;
        let old_roster = root_path.join("old.kadl");
        let applied_roster = root_path.join("applied.kadl");
        let unexpected_roster = root_path.join("unexpected.kadl");
        fs::write(&old_roster, b"old")?;
        let root = AccountRootState::create(root_path.join("root"))?;
        let device = DeviceState::load_or_create(root_path.join("device"))?;
        let certificate = root.issue_device_certificate(
            device.identity().device_id(),
            device.encryption().public_key(),
            &DeviceCapability::MESSAGING,
        )?;
        let applied_device_list = root.publish_device_list(&[certificate])?;
        let applied_bytes = applied_device_list.encode()?;
        fs::write(&applied_roster, &applied_bytes)?;
        fs::write(&unexpected_roster, b"unexpected")?;
        let old_roster = fs::canonicalize(old_roster)?;
        let applied_roster = fs::canonicalize(applied_roster)?;
        let unexpected_roster = fs::canonicalize(unexpected_roster)?;
        let profile_path = root_path.join("runtime.launch.json");
        let profile = RuntimeLaunchProfile::new(RuntimeLaunchSettings {
            state_dir: state_dir.clone(),
            allowed_requester_account_id: root.account_id(),
            device_list_file: old_roster.clone(),
            peer_prekey_pool_files: Vec::new(),
            ticket_file: Some(root_path.join("runtime.ticket")),
            relay_wait_seconds: 15,
            route_policy: RuntimeIpcRoutePolicy::Auto,
            relay_url: None,
            poll_milliseconds: 250,
            retry_base_seconds: 1,
            retry_max_seconds: 60,
            auto_sync_seconds: 30,
            ipc_file: descriptor.clone(),
            volunteer_storage: None,
        })?;
        profile.write_new(&profile_path)?;
        let status = RuntimeIpcDeviceDirectoryStatus {
            account_id: root.account_id(),
            local_device_id: device.identity().device_id(),
            state_dir,
            authority_revision: applied_device_list.revision(),
            active_device_count: 1,
            receipt_id: Some("55".repeat(32)),
            receipt_generation: Some(1),
            device_list_digest: {
                let mut digest = blake3::Hasher::new();
                digest.update(RUNTIME_DEVICE_LIST_DIGEST_DOMAIN);
                digest.update(&applied_bytes);
                digest.finalize().to_hex().to_string()
            },
            applied_device_list_file: applied_roster.clone(),
            launch_device_list_file: old_roster,
            profile_convergence_status: "convergence-required".to_owned(),
            restart_recovery_status: "authenticated-receipt-current".to_owned(),
        };

        let reconciled = reconcile_runtime_launch_profile(&profile_path, &descriptor, &status)?;
        assert_eq!(reconciled.settings().device_list_file, applied_roster);
        assert_eq!(reconciled, RuntimeLaunchProfile::load(&profile_path)?);

        profile.write_replace(&profile_path)?;
        fs::write(&status.applied_device_list_file, b"tampered")?;
        assert!(
            reconcile_runtime_launch_profile(&profile_path, &descriptor, &status).is_err(),
            "device-list content drift must be rejected"
        );
        assert_eq!(RuntimeLaunchProfile::load(&profile_path)?, profile);
        fs::write(&status.applied_device_list_file, &applied_bytes)?;

        let drifted = reconciled.with_device_list_file(unexpected_roster)?;
        drifted.write_replace(&profile_path)?;
        assert!(
            reconcile_runtime_launch_profile(&profile_path, &descriptor, &status).is_err(),
            "a profile changed outside the exact old-path precondition must be rejected"
        );
        assert_eq!(RuntimeLaunchProfile::load(&profile_path)?, drifted);
        Ok(())
    }

    #[test]
    fn account_recovery_worker_request_redacts_phrase() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let (_, phrase) = AccountRootState::create_recoverable(directory.path().join("root"))?;
        let secret = phrase.expose_secret().to_owned();
        let request = WorkerRequest::AccountRecoveryRestore {
            executable: PathBuf::from("kilogram-bootstrap"),
            account_root_dir: directory.path().join("restored-root"),
            package_file: directory.path().join("root.karp"),
            witness_file: directory.path().join("latest.karw"),
            expected_package_id: "0101010101010101010101010101010101010101010101010101010101010101"
                .to_owned(),
            expected_authority_revision: 1,
            recovery_phrase: phrase,
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains(&secret));
        assert!(debug.contains("[REDACTED]"));
        Ok(())
    }

    #[test]
    fn configured_account_recovery_helper_process_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(executable) = std::env::var_os("KILOGRAM_TEST_BOOTSTRAP_EXE") else {
            return Ok(());
        };
        let executable = fs::canonicalize(PathBuf::from(executable))?;
        let directory = tempfile::tempdir()?;
        let directory_path = fs::canonicalize(directory.path())?;
        let workspace = directory_path.join("account");
        let mut created = run_bootstrap_process(&executable, &workspace)?;
        let initial_status: AccountRecoveryStatusOutput = run_json(
            &executable,
            "account-recovery-status",
            command_arguments([("--account-root-dir", created.account_root_dir().as_os_str())]),
        )?;
        assert_eq!(initial_status.checkpoint_state, "update-required");
        let package_file = directory_path.join("root.karp");
        let witness_file = directory_path.join("latest.karw");
        let exported: AccountRecoveryOutput = run_json(
            &executable,
            "account-recovery-export",
            command_arguments([
                ("--account-root-dir", created.account_root_dir().as_os_str()),
                ("--package-file", package_file.as_os_str()),
                ("--witness-file", witness_file.as_os_str()),
            ]),
        )?;
        exported.validate_expected_status("account-root-recovery-exported")?;
        let current_status: AccountRecoveryStatusOutput = run_json(
            &executable,
            "account-recovery-status",
            command_arguments([("--account-root-dir", created.account_root_dir().as_os_str())]),
        )?;
        assert_eq!(current_status.checkpoint_state, "current");
        assert_eq!(current_status.current_package_id, exported.package_id);
        let inspected: AccountRecoveryOutput = run_json(
            &executable,
            "account-recovery-inspect",
            command_arguments([
                ("--package-file", package_file.as_os_str()),
                ("--witness-file", witness_file.as_os_str()),
            ]),
        )?;
        inspected.validate_expected_status("account-root-recovery-verified")?;
        assert_eq!(inspected.package_id, exported.package_id);

        let restored_root = directory_path.join("restored-root");
        let phrase = Zeroizing::new(created.take_recovery_phrase());
        let mut arguments = command_arguments([
            ("--account-root-dir", restored_root.as_os_str()),
            ("--package-file", package_file.as_os_str()),
            ("--witness-file", witness_file.as_os_str()),
        ]);
        let expected_authority_revision = inspected.authority_revision.to_string();
        arguments.extend(command_arguments([
            (
                "--expected-package-id",
                std::ffi::OsStr::new(&inspected.package_id),
            ),
            (
                "--expected-authority-revision",
                std::ffi::OsStr::new(&expected_authority_revision),
            ),
        ]));
        arguments.push("--recovery-phrase-stdin".into());
        let restored: AccountRecoveryOutput = run_json_with_stdin(
            &executable,
            "account-recovery-restore",
            arguments,
            phrase.as_bytes(),
        )?;
        restored.validate_expected_status("account-root-recovery-restored")?;
        assert_eq!(restored.account_id, exported.account_id);
        assert_eq!(restored.authority_revision, exported.authority_revision);
        assert_eq!(restored.account_root_dir.as_ref(), Some(&restored_root));
        let restored_status: AccountRecoveryStatusOutput = run_json(
            &executable,
            "account-recovery-status",
            command_arguments([("--account-root-dir", restored_root.as_os_str())]),
        )?;
        assert_eq!(restored_status.checkpoint_state, "current");
        let expected_provider = if cfg!(windows) {
            "windows-dpapi-current-user"
        } else {
            "plaintext-development"
        };
        assert_eq!(
            restored.root_key_protection.as_deref(),
            Some(expected_provider)
        );
        Ok(())
    }

    #[test]
    fn profile_draft_builds_from_public_enrolled_device_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let state_dir = directory.path().join("state");
        fs::create_dir(&state_dir)?;
        let device_list = directory.path().join("device-list.bin");
        fs::write(&device_list, b"signed-device-list-placeholder")?;
        let draft = RuntimeProfileDraft {
            state_dir: state_dir.display().to_string(),
            allowed_requester_account_id: ACCOUNT_ID.to_owned(),
            device_list_file: device_list.display().to_string(),
            peer_prekey_pool_files: String::new(),
            ticket_file: directory
                .path()
                .join("runtime.ticket")
                .display()
                .to_string(),
            ipc_file: directory
                .path()
                .join("runtime.ipc.json")
                .display()
                .to_string(),
            ..RuntimeProfileDraft::default()
        };
        let profile = draft.build()?;
        assert!(profile.settings().peer_prekey_pool_files.is_empty());
        let volunteer = profile
            .settings()
            .volunteer_storage
            .as_ref()
            .context("new desktop profiles enable volunteer storage")?;
        assert_eq!(volunteer.max_total_storage_bytes, 200 * BYTES_PER_MIB);
        assert_eq!(
            volunteer.ethernet_transfer_bytes_per_30_days,
            500 * BYTES_PER_MIB
        );
        assert_eq!(
            volunteer.wifi_transfer_bytes_per_30_days,
            500 * BYTES_PER_MIB
        );
        assert_eq!(volunteer.mobile_transfer_bytes_per_30_days, 0);
        let profile_path = directory.path().join("runtime.launch.json");
        profile.write_replace(&profile_path)?;
        assert_eq!(RuntimeLaunchProfile::load(&profile_path)?, profile);
        assert_eq!(
            RuntimeProfileDraft::from_profile(&profile).route_policy,
            RuntimeIpcRoutePolicy::Auto
        );
        let mut disabled_draft = draft.clone();
        disabled_draft.volunteer_storage_enabled = false;
        assert!(
            disabled_draft
                .build()?
                .settings()
                .volunteer_storage
                .is_none()
        );
        let mut unsafe_draft = draft;
        unsafe_draft.ticket_file = state_dir.join("public.ticket").display().to_string();
        assert!(unsafe_draft.build().is_err());
        Ok(())
    }

    #[test]
    fn queue_draft_trims_routing_and_preserves_message() -> Result<(), Box<dyn std::error::Error>> {
        let draft = QueueDraft {
            conversation: "  alice-bob  ".to_owned(),
            peer_account_id: format!("  {ACCOUNT_ID}  "),
            message: " hello with intentional space ".to_owned(),
        }
        .validate()?;
        assert_eq!(draft.conversation, "alice-bob");
        assert_eq!(draft.peer_account_id.to_string(), ACCOUNT_ID);
        assert_eq!(draft.message, " hello with intentional space ");
        Ok(())
    }

    #[test]
    fn queue_draft_rejects_missing_or_invalid_fields() {
        let missing_conversation = QueueDraft {
            conversation: "  ".to_owned(),
            peer_account_id: ACCOUNT_ID.to_owned(),
            message: "hello".to_owned(),
        };
        assert!(missing_conversation.validate().is_err());

        let invalid_peer = QueueDraft {
            conversation: "chat".to_owned(),
            peer_account_id: "not-an-account".to_owned(),
            message: "hello".to_owned(),
        };
        assert!(invalid_peer.validate().is_err());

        let missing_message = QueueDraft {
            conversation: "chat".to_owned(),
            peer_account_id: ACCOUNT_ID.to_owned(),
            message: "\n \t".to_owned(),
        };
        assert!(missing_message.validate().is_err());
    }

    #[test]
    fn contact_draft_requires_exact_account_and_descriptor_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let draft = ContactDraft {
            conversation: "  alice-bob  ".to_owned(),
            peer_account_id: format!("  {ACCOUNT_ID}  "),
            descriptor_file: "  C:\\Kilogram\\bob.ticket  ".to_owned(),
        }
        .validate()?;
        assert_eq!(draft.conversation, "alice-bob");
        assert_eq!(draft.peer_account_id.to_string(), ACCOUNT_ID);
        assert_eq!(
            draft.descriptor_file,
            PathBuf::from("C:\\Kilogram\\bob.ticket")
        );

        assert!(
            ContactDraft {
                conversation: "chat".to_owned(),
                peer_account_id: "invalid".to_owned(),
                descriptor_file: "bob.ticket".to_owned(),
            }
            .validate()
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn successful_queue_clears_only_message_body() {
        let mut model = ViewModel::new(PathBuf::from("runtime.ipc.json"));
        model.connection = ConnectionState::Connected;
        model.conversation = "alice-bob".to_owned();
        model.peer_account_id = ACCOUNT_ID.to_owned();
        model.message = "hello".to_owned();
        model.begin(Operation::Queue);
        model.apply(WorkerResponse {
            operation: Operation::Queue,
            result: Ok(WorkerSuccess::Queued {
                queue_id: "a".repeat(64),
                contact_id: "b".repeat(64),
                inserted: true,
            }),
        });
        assert!(model.message.is_empty());
        assert!(model.queue_attempt.is_none());
        assert_eq!(model.conversation, "alice-bob");
        assert_eq!(model.peer_account_id, ACCOUNT_ID);
        assert!(
            model
                .notice
                .as_deref()
                .is_some_and(|value| value.contains("Queued"))
        );
    }

    #[test]
    fn conversation_list_selects_a_signed_runtime_contact() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut model = ViewModel::new(PathBuf::from("runtime.ipc.json"));
        let peer = AccountId::from_bytes([7_u8; 32]);
        let peer_device = DeviceIdentity::generate()?;
        model.apply_conversations(vec![RuntimeIpcConversationSummary {
            contact_id: "contact-a".to_owned(),
            conversation_label: "alice-bob".to_owned(),
            conversation_id: ConversationId::from_label("alice-bob"),
            peer_account_id: peer,
            peer_device_id: peer_device.device_id(),
            endpoint_candidate_count: 1,
            usable_endpoint_candidate_count: 1,
            stale_endpoint_candidate_count: 0,
            quarantined_endpoint_candidate_count: 0,
            endpoint_candidates: vec![RuntimeIpcEndpointCandidateStatus {
                peer_device_id: peer_device.device_id(),
                primary: true,
                route_policy: RuntimeIpcRoutePolicy::Auto,
                descriptor_file: PathBuf::from("peer-runtime.ticket"),
                state: RuntimeIpcEndpointCandidateState::Usable,
                authority_revision: Some(1),
                publication_channel_id: Some("44".repeat(32)),
                observed_publication_generation: Some(2),
                observed_at_unix_seconds: Some(123_000),
                publication_conflict_generation: None,
                publication_conflict_proof_id: None,
                publication_conflict_detected_at_unix_seconds: None,
                detail: "authenticated-current-authority".to_owned(),
            }],
            route_policy: RuntimeIpcRoutePolicy::Auto,
            message_count: 0,
            latest_message: None,
        }]);
        assert_eq!(model.selected_contact_id.as_deref(), Some("contact-a"));
        assert_eq!(model.conversation, "alice-bob");
        assert_eq!(model.peer_account_id, peer.to_string());
        assert_eq!(
            model.mailbox_peer_device_id,
            peer_device.device_id().to_string()
        );
        Ok(())
    }

    #[tokio::test]
    async fn conversation_refresh_collects_multiple_strict_pages()
    -> Result<(), Box<dyn std::error::Error>> {
        fn summary(index: u8, peer_device_id: DeviceId) -> RuntimeIpcConversationSummary {
            let conversation_label = format!("conversation-{index:03}");
            RuntimeIpcConversationSummary {
                contact_id: format!("{index:064x}"),
                conversation_id: ConversationId::from_label(&conversation_label),
                conversation_label,
                peer_account_id: AccountId::from_bytes([index; 32]),
                peer_device_id,
                endpoint_candidate_count: 1,
                usable_endpoint_candidate_count: 1,
                stale_endpoint_candidate_count: 0,
                quarantined_endpoint_candidate_count: 0,
                endpoint_candidates: vec![RuntimeIpcEndpointCandidateStatus {
                    peer_device_id,
                    primary: true,
                    route_policy: RuntimeIpcRoutePolicy::Auto,
                    descriptor_file: PathBuf::from("peer-runtime.ticket"),
                    state: RuntimeIpcEndpointCandidateState::Usable,
                    authority_revision: Some(1),
                    publication_channel_id: None,
                    observed_publication_generation: None,
                    observed_at_unix_seconds: None,
                    publication_conflict_generation: None,
                    publication_conflict_proof_id: None,
                    publication_conflict_detected_at_unix_seconds: None,
                    detail: "authenticated-current-authority".to_owned(),
                }],
                route_policy: RuntimeIpcRoutePolicy::Auto,
                message_count: 0,
                latest_message: None,
            }
        }

        let directory = tempfile::tempdir()?;
        let descriptor = directory.path().join("runtime.ipc.json");
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = DeviceIdentity::generate()?;
        let peer_device_id = identity.device_id();
        let (server, mut requests) =
            RuntimeIpcServer::start(descriptor.clone(), root.account_id(), &identity).await?;
        let actor = tokio::spawn(async move {
            let first = requests
                .recv()
                .await
                .context("receive first conversation page")?;
            let (command, response) = first.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::ConversationListPage {
                    cursor: None,
                    limit: MAX_RUNTIME_IPC_CONVERSATION_PAGE_SIZE,
                }
            ));
            let first_page = (1..=64)
                .map(|index| summary(index, peer_device_id))
                .collect::<Vec<_>>();
            let continuation = kilogram_runtime_ipc::RuntimeIpcConversationCursor::from_summary(
                first_page.last().context("first page is empty")?,
            )?;
            response
                .send(RuntimeIpcResponse::ConversationListPage(
                    RuntimeIpcConversationPage {
                        conversations: first_page,
                        next_cursor: Some(continuation.clone()),
                    },
                ))
                .map_err(|_| anyhow::anyhow!("send first conversation page"))?;

            let second = requests
                .recv()
                .await
                .context("receive second conversation page")?;
            let (command, response) = second.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::ConversationListPage {
                    cursor: Some(cursor),
                    limit: MAX_RUNTIME_IPC_CONVERSATION_PAGE_SIZE,
                } if cursor == continuation
            ));
            response
                .send(RuntimeIpcResponse::ConversationListPage(
                    RuntimeIpcConversationPage {
                        conversations: vec![summary(65, peer_device_id)],
                        next_cursor: None,
                    },
                ))
                .map_err(|_| anyhow::anyhow!("send second conversation page"))?;
            Ok::<_, anyhow::Error>(())
        });

        let result = execute_request(WorkerRequest::Conversations {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(
            result,
            WorkerSuccess::Conversations(conversations)
                if conversations.len() == 65
                    && conversations.first().is_some_and(|item| item.conversation_label == "conversation-001")
                    && conversations.last().is_some_and(|item| item.conversation_label == "conversation-065")
        ));
        actor.await??;
        server.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn desktop_peer_reenrollment_ceremony_uses_authenticated_ipc_for_each_stage()
    -> Result<(), Box<dyn std::error::Error>> {
        use kilogram_runtime_ipc::{
            RuntimeIpcPeerCapabilityQuarantineReason, RuntimeIpcPeerReenrollmentArtifactId,
            RuntimeIpcPeerReenrollmentDurableOutcome, RuntimeIpcPeerReenrollmentHistoryRecovery,
            RuntimeIpcPeerReenrollmentInvitationSummary, RuntimeIpcPeerReenrollmentPlanSummary,
            RuntimeIpcPeerReenrollmentQuarantineSummary, RuntimeIpcPeerReenrollmentStatus,
            RuntimeIpcPeerReenrollmentSuppressionSummary,
        };

        fn artifact(byte: u8) -> anyhow::Result<RuntimeIpcPeerReenrollmentArtifactId> {
            RuntimeIpcPeerReenrollmentArtifactId::from_bytes([byte; 32])
        }

        let directory = tempfile::tempdir()?;
        let descriptor = directory.path().join("runtime.ipc.json");
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = DeviceIdentity::generate()?;
        let contact_id: RuntimeIpcPeerReenrollmentContactId = "31".repeat(32).parse()?;
        let invitation_id = artifact(2)?;
        let quarantine_record_id = artifact(3)?;
        let plan = RuntimeIpcPeerReenrollmentPlanSummary::new(artifact(4)?, 2_000)?;
        let tombstone_id = artifact(5)?;
        let invitation = RuntimeIpcPeerReenrollmentInvitationSummary {
            invitation_id: invitation_id.clone(),
            replacement_device_fingerprint: artifact(6)?,
            required_capability_id: artifact(7)?,
            authority_revision: 9,
            publication_epoch: 4,
            issued_at_unix_seconds: 1_000,
            expires_at_unix_seconds: 2_000,
        };
        let quarantine = RuntimeIpcPeerReenrollmentQuarantineSummary {
            record_id: quarantine_record_id.clone(),
            sequence: 8,
            reason: RuntimeIpcPeerCapabilityQuarantineReason::RequiredInvitationCapabilityMissing,
        };
        let suppression = RuntimeIpcPeerReenrollmentSuppressionSummary {
            durable_quarantine_present: true,
            process_local_sync_suppressed: true,
            process_local_queue_suppressed_count: 2,
        };
        let (server, mut requests) =
            RuntimeIpcServer::start(descriptor.clone(), root.account_id(), &identity).await?;
        let runtime_descriptor = server.descriptor().clone();
        let signer = identity;
        let actor_contact = contact_id.clone();
        let actor_invitation = invitation.clone();
        let actor_quarantine = quarantine.clone();
        let actor_plan = plan.clone();
        let actor_tombstone = tombstone_id.clone();
        let actor_suppression = suppression.clone();
        let actor = tokio::spawn(async move {
            for stage in 0..4 {
                let work = requests
                    .recv()
                    .await
                    .context("receive desktop peer re-enrollment stage")?;
                let (outer, responder) = work.into_parts();
                let (encoded, mutation) = match outer {
                    RuntimeIpcCommand::PeerReenrollmentRead { request } => (request, false),
                    RuntimeIpcCommand::PeerReenrollmentMutation { request } => (request, true),
                    _ => bail!("unexpected outer desktop peer re-enrollment command"),
                };
                ensure!(mutation == (stage != 0));
                let request = RuntimeIpcPeerReenrollmentRequest::decode_and_validate(
                    &encoded,
                    &runtime_descriptor,
                )?;
                let (status, durable_outcome) = match (stage, request.command()) {
                    (0, RuntimeIpcPeerReenrollmentCommand::Status { contact_id })
                        if contact_id == &actor_contact =>
                    {
                        (
                            RuntimeIpcPeerReenrollmentStatus::Ready,
                            RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged,
                        )
                    }
                    (
                        1,
                        RuntimeIpcPeerReenrollmentCommand::Plan {
                            contact_id,
                            invitation_id,
                            quarantine_record_id,
                        },
                    ) if contact_id == &actor_contact
                        && invitation_id == &actor_invitation.invitation_id
                        && quarantine_record_id == &actor_quarantine.record_id =>
                    {
                        (
                            RuntimeIpcPeerReenrollmentStatus::ApprovalRequired,
                            RuntimeIpcPeerReenrollmentDurableOutcome::PlanPrepared {
                                plan_id: actor_plan.plan_id.clone(),
                            },
                        )
                    }
                    (
                        2,
                        RuntimeIpcPeerReenrollmentCommand::Approve {
                            contact_id,
                            plan_id,
                            confirmation_phrase,
                            explicit_approval:
                                kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed,
                        },
                    ) if contact_id == &actor_contact
                        && plan_id == &actor_plan.plan_id
                        && confirmation_phrase == &actor_plan.confirmation_phrase =>
                    {
                        (
                            RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared,
                            RuntimeIpcPeerReenrollmentDurableOutcome::ApprovalPrepared {
                                plan_id: actor_plan.plan_id.clone(),
                                tombstone_id: actor_tombstone.clone(),
                            },
                        )
                    }
                    (
                        3,
                        RuntimeIpcPeerReenrollmentCommand::Accept {
                            contact_id,
                            tombstone_id,
                            quarantine_record_id,
                        },
                    ) if contact_id == &actor_contact
                        && tombstone_id == &actor_tombstone
                        && quarantine_record_id == &actor_quarantine.record_id =>
                    {
                        (
                            RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired,
                            RuntimeIpcPeerReenrollmentDurableOutcome::AcceptanceCommitted {
                                acceptance_receipt_id: artifact(8)?,
                                runtime_transition_id: artifact(9)?,
                            },
                        )
                    }
                    _ => bail!("desktop skipped, rewound, or altered the ceremony"),
                };
                let response = RuntimeIpcPeerReenrollmentResponse {
                    contact_id: actor_contact.clone(),
                    status,
                    hazard: None,
                    invitation: Some(actor_invitation.clone()),
                    quarantine: Some(actor_quarantine.clone()),
                    plan: (stage != 0).then_some(actor_plan.clone()),
                    durable_outcome,
                    suppression: actor_suppression.clone(),
                    history_recovery:
                        RuntimeIpcPeerReenrollmentHistoryRecovery::SeparateApprovalRequired,
                    restart_required: stage == 3,
                };
                let signed = SignedRuntimeIpcPeerReenrollmentResponse::issue(
                    &request,
                    &runtime_descriptor,
                    &signer,
                    response,
                )?;
                let outer = if mutation {
                    RuntimeIpcResponse::PeerReenrollmentMutation {
                        signed_response: signed.encode()?,
                    }
                } else {
                    RuntimeIpcResponse::PeerReenrollmentRead {
                        signed_response: signed.encode()?,
                    }
                };
                responder
                    .send(outer)
                    .map_err(|_| anyhow::anyhow!("send desktop ceremony stage"))?;
            }
            Ok::<_, anyhow::Error>(())
        });

        let status = execute_request(WorkerRequest::PeerReenrollmentStatus {
            descriptor: descriptor.clone(),
            contact_id: contact_id.clone(),
        })
        .await?;
        assert!(matches!(
            status,
            WorkerSuccess::PeerReenrollmentStatus { response, .. }
                if response.status == RuntimeIpcPeerReenrollmentStatus::Ready
        ));

        let planned = execute_request(WorkerRequest::PeerReenrollmentPlan {
            descriptor: descriptor.clone(),
            command: RuntimeIpcPeerReenrollmentCommand::Plan {
                contact_id: contact_id.clone(),
                invitation_id: invitation_id.clone(),
                quarantine_record_id: quarantine_record_id.clone(),
            },
        })
        .await?;
        assert!(matches!(
            planned,
            WorkerSuccess::PeerReenrollmentStatus { response, .. }
                if response.status == RuntimeIpcPeerReenrollmentStatus::ApprovalRequired
        ));

        let approved = execute_request(WorkerRequest::PeerReenrollmentApprove {
            descriptor: descriptor.clone(),
            command: RuntimeIpcPeerReenrollmentCommand::Approve {
                contact_id: contact_id.clone(),
                plan_id: plan.plan_id.clone(),
                confirmation_phrase: plan.confirmation_phrase.clone(),
                explicit_approval:
                    kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentExplicitApproval::UserConfirmed,
            },
        })
        .await?;
        assert!(matches!(
            approved,
            WorkerSuccess::PeerReenrollmentStatus { response, .. }
                if response.status == RuntimeIpcPeerReenrollmentStatus::ApprovedPrepared
        ));

        let accepted = execute_request(WorkerRequest::PeerReenrollmentAccept {
            descriptor: descriptor.clone(),
            command: RuntimeIpcPeerReenrollmentCommand::Accept {
                contact_id,
                tombstone_id,
                quarantine_record_id,
            },
        })
        .await?;
        assert!(matches!(
            accepted,
            WorkerSuccess::PeerReenrollmentStatus { response, .. }
                if response.status == RuntimeIpcPeerReenrollmentStatus::AcceptedRestartRequired
                    && response.restart_required
        ));
        actor.await??;
        server.shutdown().await?;
        Ok(())
    }

    #[test]
    fn uncertain_queue_retry_reuses_request_id_for_unchanged_draft()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut model = ViewModel::new(PathBuf::from("runtime.ipc.json"));
        model.conversation = "alice-bob".to_owned();
        model.peer_account_id = ACCOUNT_ID.to_owned();
        model.message = "hello".to_owned();
        let first = model.prepare_queue_attempt()?;
        model.fail(Operation::Queue, "response lost".to_owned());
        let retry = model.prepare_queue_attempt()?;
        assert_eq!(retry.request_id, first.request_id);
        assert_eq!(retry.draft, first.draft);

        model.message.push('!');
        let changed = model.prepare_queue_attempt()?;
        assert_ne!(changed.request_id, first.request_id);
        Ok(())
    }

    #[test]
    fn compact_identifier_keeps_both_ends() {
        assert_eq!(compact_id("1234567890abcdef"), "123456…abcdef");
        assert_eq!(compact_id("short"), "short");
    }

    #[test]
    fn publication_conflict_confirmation_code_shape_is_exact() {
        assert!(valid_publication_conflict_confirmation_code(
            "KPC1-0123-4567-89AB-CDEF-0000-FFFF"
        ));
        assert!(!valid_publication_conflict_confirmation_code(
            "KPC1-0123-4567-89AB-CDEF-0000"
        ));
        assert!(!valid_publication_conflict_confirmation_code(
            "KPC1-0123-4567-89AB-CDEF-0000-GGGG"
        ));
        assert!(!valid_publication_conflict_confirmation_code(
            "kpc1-0123-4567-89AB-CDEF-0000-FFFF"
        ));
    }

    #[tokio::test]
    async fn desktop_adapter_uses_only_authenticated_runtime_ipc()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let descriptor = directory.path().join("runtime.ipc.json");
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = DeviceIdentity::generate()?;
        let account_id = root.account_id();
        let device_id = identity.device_id();
        let peer_account_id = AccountId::from_bytes([7_u8; 32]);
        let state_dir = fs::canonicalize(directory.path())?;
        let launch_device_list_file = state_dir.join("launch.kadl");
        let applied_device_list_file = state_dir.join("applied.kadl");
        let directory_status = RuntimeIpcDeviceDirectoryStatus {
            account_id,
            local_device_id: device_id,
            state_dir,
            authority_revision: 3,
            active_device_count: 2,
            receipt_id: Some("33".repeat(32)),
            receipt_generation: Some(1),
            device_list_digest: "44".repeat(32),
            applied_device_list_file,
            launch_device_list_file,
            profile_convergence_status: "convergence-required".to_owned(),
            restart_recovery_status: "authenticated-receipt-current".to_owned(),
        };
        let loaded_policy = RuntimeIpcCanonicalSyncV2PolicySnapshot {
            source: RuntimeIpcCanonicalSyncV2PolicySource::ImplicitDefault,
            record_id: None,
            migration_state: RuntimeIpcCanonicalSyncV2MigrationState::LegacyOnly,
            disposition: RuntimeIpcCanonicalSyncV2Disposition::Normal,
            migration_epoch: 0,
            canonical_evidence_id: None,
            permissions: RuntimeIpcCanonicalSyncV2Permissions {
                legacy_reader_enabled: true,
                publish_canonical_capability: false,
                register_canonical_alpn: false,
                accept_new_canonical_session: false,
                initiate_new_canonical_session: false,
                resume_selected_canonical_session: false,
            },
        };
        let canonical_runtime_status = RuntimeIpcCanonicalSyncV2RuntimeStatus {
            loaded: loaded_policy.clone(),
            configured: loaded_policy,
            configured_matches_loaded: true,
            restart_required: false,
            observation_capacity: 32,
            observations: Vec::new(),
        };
        let (server, mut requests) =
            RuntimeIpcServer::start(descriptor.clone(), account_id, &identity).await?;
        let peer_reenrollment_descriptor = server.descriptor().clone();
        let peer_reenrollment_identity = identity;
        let actor = tokio::spawn(async move {
            let ping = requests.recv().await.context("receive GUI ping")?;
            let (command, response) = ping.into_parts();
            ensure!(matches!(command, RuntimeIpcCommand::Ping));
            response
                .send(RuntimeIpcResponse::Pong {
                    account_id,
                    device_id,
                })
                .map_err(|_| anyhow::anyhow!("send GUI ping response"))?;

            let status = requests
                .recv()
                .await
                .context("receive GUI device-directory status")?;
            let (command, response) = status.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::OwnDeviceDirectoryStatus
            ));
            response
                .send(RuntimeIpcResponse::OwnDeviceDirectoryStatus(
                    directory_status.clone(),
                ))
                .map_err(|_| anyhow::anyhow!("send GUI device-directory status response"))?;

            let canonical_status = requests
                .recv()
                .await
                .context("receive GUI canonical runtime status")?;
            let (command, response) = canonical_status.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::CanonicalSyncV2RuntimeStatus
            ));
            response
                .send(RuntimeIpcResponse::CanonicalSyncV2RuntimeStatus(Box::new(
                    canonical_runtime_status,
                )))
                .map_err(|_| anyhow::anyhow!("send GUI canonical runtime status response"))?;

            let peer_capability_status = requests
                .recv()
                .await
                .context("receive GUI peer capability status")?;
            let (command, response) = peer_capability_status.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::PeerCapabilityQuarantinePage {
                    cursor: None,
                    limit: MAX_RUNTIME_IPC_PEER_CAPABILITY_PAGE_SIZE,
                }
            ));
            response
                .send(RuntimeIpcResponse::PeerCapabilityQuarantinePage(Box::new(
                    RuntimeIpcPeerCapabilityPage {
                        capability_store_initialized: false,
                        total_relevant_contact_count: 1,
                        durable_quarantined_contact_count: 0,
                        scheduler_suppressed_contact_count: 1,
                        scheduler_suppressed_queue_count: 1,
                        network_retry_contact_count: 0,
                        network_retry_queue_count: 0,
                        items: vec![kilogram_runtime_ipc::RuntimeIpcPeerCapabilityStatusItem {
                            contact_id: "21".repeat(32),
                            durable_quarantined_candidate_count: 0,
                            durable_reasons: Vec::new(),
                            scheduler_sync_suppressed: true,
                            scheduler_queue_suppressed_count: 1,
                            network_retry_queue_count: 0,
                            pending_queue_count: 1,
                        }],
                        next_cursor: None,
                    },
                )))
                .map_err(|_| anyhow::anyhow!("send GUI peer capability status response"))?;

            let peer_reenrollment_status = requests
                .recv()
                .await
                .context("receive GUI peer re-enrollment status")?;
            let (command, response) = peer_reenrollment_status.into_parts();
            let RuntimeIpcCommand::PeerReenrollmentRead { request } = command else {
                bail!("expected GUI peer re-enrollment read command");
            };
            let request = RuntimeIpcPeerReenrollmentRequest::decode_and_validate(
                &request,
                &peer_reenrollment_descriptor,
            )?;
            ensure!(matches!(
                request.command(),
                RuntimeIpcPeerReenrollmentCommand::Status { contact_id }
                    if contact_id.as_str() == "21".repeat(32)
            ));
            let signed = SignedRuntimeIpcPeerReenrollmentResponse::issue(
                &request,
                &peer_reenrollment_descriptor,
                &peer_reenrollment_identity,
                RuntimeIpcPeerReenrollmentResponse {
                    contact_id: "21".repeat(32).parse()?,
                    status: kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentStatus::UnsafeOrDivergent,
                    hazard: Some(
                        kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentHazard::MissingDurableQuarantine,
                    ),
                    invitation: None,
                    quarantine: None,
                    plan: None,
                    durable_outcome:
                        kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentDurableOutcome::Unchanged,
                    suppression:
                        kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentSuppressionSummary {
                            durable_quarantine_present: false,
                            process_local_sync_suppressed: true,
                            process_local_queue_suppressed_count: 1,
                        },
                    history_recovery:
                        kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentHistoryRecovery::NotEvaluated,
                    restart_required: false,
                },
            )?;
            response
                .send(RuntimeIpcResponse::PeerReenrollmentRead {
                    signed_response: signed.encode()?,
                })
                .map_err(|_| anyhow::anyhow!("send GUI peer re-enrollment status response"))?;

            let contact = requests.recv().await.context("receive GUI contact")?;
            let (command, response) = contact.into_parts();
            let RuntimeIpcCommand::AddContact {
                conversation,
                expected_peer_account_id,
                descriptor_file,
            } = command
            else {
                bail!("expected GUI add-contact command");
            };
            ensure!(conversation == "desktop-test");
            ensure!(expected_peer_account_id == peer_account_id);
            ensure!(descriptor_file == PathBuf::from("peer-runtime.ticket"));
            response
                .send(RuntimeIpcResponse::ContactAdded {
                    contact_id: "22".repeat(32),
                    peer_account_id,
                    peer_device_id: device_id,
                    endpoint_candidate_count: 1,
                    endpoint_candidate_added: true,
                    inserted: true,
                })
                .map_err(|_| anyhow::anyhow!("send GUI contact response"))?;

            let queue = requests.recv().await.context("receive GUI queue")?;
            let (command, response) = queue.into_parts();
            let RuntimeIpcCommand::QueueMessage {
                conversation,
                peer_account_id: requested_peer,
                message,
                ..
            } = command
            else {
                bail!("expected GUI queue command");
            };
            ensure!(conversation == "desktop-test");
            ensure!(requested_peer == peer_account_id);
            ensure!(message == "hello through IPC");
            response
                .send(RuntimeIpcResponse::MessageQueued {
                    queue_id: "11".repeat(32),
                    contact_id: "22".repeat(32),
                    inserted: true,
                })
                .map_err(|_| anyhow::anyhow!("send GUI queue response"))?;

            let status = requests.recv().await.context("receive GUI status")?;
            let (command, response) = status.into_parts();
            ensure!(matches!(command, RuntimeIpcCommand::OutboxStatus));
            response
                .send(RuntimeIpcResponse::OutboxStatus(RuntimeIpcOutboxStatus {
                    contact_count: 1,
                    queue_count: 1,
                    pending_count: 1,
                    materialized_count: 0,
                    delivered_count: 0,
                    mailbox_pending_count: 0,
                    mailbox_stored_count: 0,
                    mailbox_expired_count: 0,
                    mailbox_failed_count: 0,
                    retry_state_count: 0,
                    items: Vec::new(),
                }))
                .map_err(|_| anyhow::anyhow!("send GUI status response"))?;

            let conversations = requests
                .recv()
                .await
                .context("receive GUI conversation list")?;
            let (command, response) = conversations.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::ConversationListPage {
                    cursor: None,
                    limit: MAX_RUNTIME_IPC_CONVERSATION_PAGE_SIZE,
                }
            ));
            response
                .send(RuntimeIpcResponse::ConversationListPage(
                    RuntimeIpcConversationPage {
                        conversations: vec![RuntimeIpcConversationSummary {
                            contact_id: "22".repeat(32),
                            conversation_label: "desktop-test".to_owned(),
                            conversation_id: ConversationId::from_label("desktop-test"),
                            peer_account_id,
                            peer_device_id: device_id,
                            endpoint_candidate_count: 1,
                            usable_endpoint_candidate_count: 1,
                            stale_endpoint_candidate_count: 0,
                            quarantined_endpoint_candidate_count: 0,
                            endpoint_candidates: vec![RuntimeIpcEndpointCandidateStatus {
                                peer_device_id: device_id,
                                primary: true,
                                route_policy: RuntimeIpcRoutePolicy::Auto,
                                descriptor_file: PathBuf::from("peer-runtime.ticket"),
                                state: RuntimeIpcEndpointCandidateState::Usable,
                                authority_revision: Some(5),
                                publication_channel_id: Some("77".repeat(32)),
                                observed_publication_generation: Some(4),
                                observed_at_unix_seconds: Some(123_789),
                                publication_conflict_generation: None,
                                publication_conflict_proof_id: None,
                                publication_conflict_detected_at_unix_seconds: None,
                                detail: "authenticated-current-authority".to_owned(),
                            }],
                            route_policy: RuntimeIpcRoutePolicy::Auto,
                            message_count: 0,
                            latest_message: None,
                        }],
                        next_cursor: None,
                    },
                ))
                .map_err(|_| anyhow::anyhow!("send GUI conversation-list response"))?;

            let history = requests.recv().await.context("receive GUI history page")?;
            let (command, response) = history.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::HistoryPage {
                    conversation,
                    cursor: None,
                    limit: HISTORY_PAGE_SIZE,
                } if conversation == "desktop-test"
            ));
            response
                .send(RuntimeIpcResponse::HistoryPage(RuntimeIpcHistoryPage {
                    conversation_id: ConversationId::from_label("desktop-test"),
                    total_messages: 0,
                    messages: Vec::new(),
                    next_cursor: None,
                }))
                .map_err(|_| anyhow::anyhow!("send GUI history response"))?;

            let publication = requests
                .recv()
                .await
                .context("receive GUI ticket publication")?;
            let (command, response) = publication.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::PublishOwnTicket {
                    conversation,
                    peer_account_id: requested_peer,
                    service_base_url,
                    ttl_seconds: 900,
                } if conversation == "desktop-test"
                    && requested_peer == peer_account_id
                    && service_base_url == "https://publication.example"
            ));
            response
                .send(RuntimeIpcResponse::OwnTicketPublished(Box::new(
                    RuntimeIpcTicketPublication {
                        contact_id: "22".repeat(32),
                        channel_id: "55".repeat(32),
                        publication_id: "66".repeat(32),
                        publication_generation: 2,
                        expires_at_unix_seconds: 123_456,
                        recipient_device_count: 3,
                        encrypted_record_bytes: 4_096,
                        service_base_url: "https://publication.example/".to_owned(),
                        local_store_status: "Inserted".to_owned(),
                        upload_status: "confirmed-http-success".to_owned(),
                        lookup_privacy_status:
                            "opaque-hpke-recipient-slots-traffic-analysis-visible".to_owned(),
                        first_contact_freshness: "bootstrap-contact-required".to_owned(),
                    },
                )))
                .map_err(|_| anyhow::anyhow!("send GUI ticket-publication response"))?;

            let refresh = requests
                .recv()
                .await
                .context("receive GUI contact-ticket refresh")?;
            let (command, response) = refresh.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::RefreshContactTicket {
                    conversation,
                    peer_account_id: requested_peer,
                    service_base_url,
                } if conversation == "desktop-test"
                    && requested_peer == peer_account_id
                    && service_base_url == "https://publication.example"
            ));
            response
                .send(RuntimeIpcResponse::ContactTicketRefreshed(Box::new(
                    RuntimeIpcContactTicketRefresh {
                        contact_id: "22".repeat(32),
                        endpoint_candidate_count: 1,
                        refreshed_endpoint_candidate_count: 1,
                        complete: true,
                        results: vec![RuntimeIpcEndpointTicketRefresh {
                            peer_device_id: device_id,
                            primary: true,
                            route_policy: RuntimeIpcRoutePolicy::Auto,
                            descriptor_file: PathBuf::from("peer-runtime.ticket"),
                            state: RuntimeIpcEndpointCandidateState::Usable,
                            channel_id: Some("77".repeat(32)),
                            publication_id: Some("88".repeat(32)),
                            publication_generation: Some(4),
                            expires_at_unix_seconds: Some(123_789),
                            publisher_account_id: Some(peer_account_id),
                            authority_revision: Some(5),
                            active_device_count: Some(2),
                            local_observation_status: Some("Inserted".to_owned()),
                            descriptor_publish_status: Some("atomic-replace".to_owned()),
                            freshness_status: Some(
                                "signed-non-expired-local-high-water".to_owned(),
                            ),
                            first_contact_freshness: Some("local-monotonic-high-water".to_owned()),
                            detail: "refreshed-and-installed".to_owned(),
                        }],
                    },
                )))
                .map_err(|_| anyhow::anyhow!("send GUI contact-ticket response"))?;

            let configure_automation = requests
                .recv()
                .await
                .context("receive GUI ticket-automation configuration")?;
            let (command, response) = configure_automation.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::ConfigureTicketAutomation {
                    conversation,
                    peer_account_id: requested_peer,
                    enabled: true,
                    service_base_url,
                    ttl_seconds: 900,
                    refresh_before_seconds: 300,
                    retry_base_seconds: 5,
                    retry_max_seconds: 300,
                    allow_ethernet: true,
                    allow_wifi: true,
                    allow_mobile: false,
                    allow_unknown_network: false,
                } if conversation == "desktop-test"
                    && requested_peer == peer_account_id
                    && service_base_url == "https://publication.example"
            ));
            let automation_status = RuntimeIpcTicketAutomationStatus {
                contact_id: "22".repeat(32),
                conversation: "desktop-test".to_owned(),
                peer_account_id,
                enabled: true,
                policy_generation: 1,
                service_base_url: "https://publication.example/".to_owned(),
                ttl_seconds: 900,
                refresh_before_seconds: 300,
                retry_base_seconds: 5,
                retry_max_seconds: 300,
                allow_ethernet: true,
                allow_wifi: true,
                allow_mobile: false,
                allow_unknown_network: false,
                current_network: RuntimeIpcNetworkClass::Ethernet,
                network_allowed: true,
                execution_scope: "only-while-runtime-process-is-running".to_owned(),
                os_background_service_enabled: false,
                publish: RuntimeIpcTicketAutomationActionStatus {
                    action: "publish".to_owned(),
                    state: "due".to_owned(),
                    last_attempt_unix_seconds: None,
                    last_success_unix_seconds: None,
                    next_attempt_unix_seconds: None,
                    consecutive_failures: 0,
                    publication_generation: None,
                    expires_at_unix_seconds: None,
                    last_result: "never".to_owned(),
                },
                refresh: RuntimeIpcTicketAutomationActionStatus {
                    action: "refresh".to_owned(),
                    state: "due".to_owned(),
                    last_attempt_unix_seconds: None,
                    last_success_unix_seconds: None,
                    next_attempt_unix_seconds: None,
                    consecutive_failures: 0,
                    publication_generation: None,
                    expires_at_unix_seconds: None,
                    last_result: "never".to_owned(),
                },
            };
            response
                .send(RuntimeIpcResponse::TicketAutomationConfigured(Box::new(
                    automation_status.clone(),
                )))
                .map_err(|_| anyhow::anyhow!("send GUI ticket-automation response"))?;

            let automation_query = requests
                .recv()
                .await
                .context("receive GUI ticket-automation status")?;
            let (command, response) = automation_query.into_parts();
            ensure!(matches!(command, RuntimeIpcCommand::TicketAutomationStatus));
            response
                .send(RuntimeIpcResponse::TicketAutomationStatus(vec![
                    automation_status,
                ]))
                .map_err(|_| anyhow::anyhow!("send GUI ticket-automation status response"))?;

            let mailbox_create = requests
                .recv()
                .await
                .context("receive GUI mailbox activation")?;
            let (command, response) = mailbox_create.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::CreateExactMailboxCapability {
                    conversation,
                    peer_account_id: requested_peer,
                    peer_device_id: requested_device,
                    valid_for_seconds: DEFAULT_MAILBOX_VALIDITY_SECONDS,
                } if conversation == "desktop-test"
                    && requested_peer == peer_account_id
                    && requested_device == device_id
            ));
            response
                .send(RuntimeIpcResponse::MailboxCapabilityChanged(Box::new(
                    RuntimeIpcMailboxCapabilityTransition {
                        contact_id: "22".repeat(32),
                        conversation_id: ConversationId::from_label("desktop-test"),
                        peer_account_id,
                        peer_device_id: device_id,
                        binding_id: "91".repeat(32),
                        update_id: "92".repeat(32),
                        generation: 1,
                        action: "activated".to_owned(),
                        delivery_state: "queued-for-authenticated-runtime-session".to_owned(),
                    },
                )))
                .map_err(|_| anyhow::anyhow!("send GUI mailbox activation response"))?;

            let mailbox_rotate = requests
                .recv()
                .await
                .context("receive GUI mailbox rotation")?;
            let (command, response) = mailbox_rotate.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::RotateExactMailboxCapability {
                    conversation,
                    peer_account_id: requested_peer,
                    peer_device_id: requested_device,
                    valid_for_seconds: DEFAULT_MAILBOX_VALIDITY_SECONDS,
                } if conversation == "desktop-test"
                    && requested_peer == peer_account_id
                    && requested_device == device_id
            ));
            response
                .send(RuntimeIpcResponse::MailboxCapabilityChanged(Box::new(
                    RuntimeIpcMailboxCapabilityTransition {
                        contact_id: "22".repeat(32),
                        conversation_id: ConversationId::from_label("desktop-test"),
                        peer_account_id,
                        peer_device_id: device_id,
                        binding_id: "93".repeat(32),
                        update_id: "94".repeat(32),
                        generation: 2,
                        action: "rotated".to_owned(),
                        delivery_state: "queued-for-authenticated-runtime-session".to_owned(),
                    },
                )))
                .map_err(|_| anyhow::anyhow!("send GUI mailbox rotation response"))?;

            let mailbox_revoke = requests
                .recv()
                .await
                .context("receive GUI mailbox revocation")?;
            let (command, response) = mailbox_revoke.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::RevokeMailboxCapability {
                    conversation,
                    peer_account_id: requested_peer,
                    peer_device_id: requested_device,
                } if conversation == "desktop-test"
                    && requested_peer == peer_account_id
                    && requested_device == device_id
            ));
            response
                .send(RuntimeIpcResponse::MailboxCapabilityChanged(Box::new(
                    RuntimeIpcMailboxCapabilityTransition {
                        contact_id: "22".repeat(32),
                        conversation_id: ConversationId::from_label("desktop-test"),
                        peer_account_id,
                        peer_device_id: device_id,
                        binding_id: "93".repeat(32),
                        update_id: "95".repeat(32),
                        generation: 3,
                        action: "revoked".to_owned(),
                        delivery_state: "queued-for-authenticated-runtime-session".to_owned(),
                    },
                )))
                .map_err(|_| anyhow::anyhow!("send GUI mailbox revocation response"))?;

            let mailbox_status = requests
                .recv()
                .await
                .context("receive GUI mailbox status")?;
            let (command, response) = mailbox_status.into_parts();
            ensure!(matches!(command, RuntimeIpcCommand::MailboxStatus));
            response
                .send(RuntimeIpcResponse::MailboxStatus(RuntimeIpcMailboxStatus {
                    local_binding_count: 1,
                    local_usable_count: 0,
                    peer_binding_count: 0,
                    peer_usable_count: 0,
                    local_update_count: 3,
                    local_acknowledged_update_count: 2,
                    local_unacknowledged_update_count: 1,
                    peer_update_count: 0,
                    local_revoked_head_count: 1,
                    peer_revoked_head_count: 0,
                    dispatch_count: 0,
                    pending_upload_count: 0,
                    stored_upload_count: 0,
                    received_commit_count: 0,
                    deleted_inbound_count: 0,
                    expired_dispatch_count: 0,
                    failed_dispatch_count: 0,
                    delivery_state: "revoked".to_owned(),
                    capabilities: vec![kilogram_runtime_ipc::RuntimeIpcMailboxCapabilityStatus {
                        contact_id: "22".repeat(32),
                        conversation_id: ConversationId::from_label("desktop-test"),
                        peer_account_id,
                        peer_device_id: device_id,
                        direction: "receive".to_owned(),
                        capability_format: "v2-exact-volunteer".to_owned(),
                        binding_id: "93".repeat(32),
                        update_id: Some("95".repeat(32)),
                        generation: Some(3),
                        acknowledged: Some(false),
                        revoked: true,
                        state: "revocation-pending".to_owned(),
                        replica_set_discovery: "not-applicable".to_owned(),
                        replica_set_commitment_id: None,
                        replica_set_store_count: 0,
                    }],
                }))
                .map_err(|_| anyhow::anyhow!("send GUI mailbox status response"))?;

            let directory_update = requests
                .recv()
                .await
                .context("receive GUI device-directory update")?;
            let (command, response) = directory_update.into_parts();
            ensure!(matches!(
                command,
                RuntimeIpcCommand::ApplyOwnDeviceDirectory { device_list_file }
                    if device_list_file == std::path::Path::new("refreshed.kadl")
            ));
            response
                .send(RuntimeIpcResponse::OwnDeviceDirectoryApplied(Box::new(
                    RuntimeIpcDeviceDirectoryUpdate {
                        account_id,
                        local_device_id: device_id,
                        previous_authority_revision: 3,
                        authority_revision: 4,
                        active_device_count: 1,
                        removed_device_ids: vec![device_id],
                        ratchet_session_records_retired: 1,
                        prekey_observations_retired: 1,
                        pending_unmaterialized_messages: 2,
                        pending_materialized_messages: 3,
                        ticket_published: true,
                        launch_profile_update_required: true,
                        future_recipient_slot_status:
                            "removed-devices-excluded-by-refreshed-ticket".to_owned(),
                        preexisting_recipient_slot_status:
                            "immutable-cannot-be-remotely-rewritten-or-erased".to_owned(),
                        history_availability_status: "existing-copies-remain-readable".to_owned(),
                        directory_status: RuntimeIpcDeviceDirectoryStatus {
                            authority_revision: 4,
                            active_device_count: 1,
                            ..directory_status
                        },
                    },
                )))
                .map_err(|_| anyhow::anyhow!("send GUI device-directory response"))?;

            let shutdown = requests.recv().await.context("receive GUI shutdown")?;
            let (command, response) = shutdown.into_parts();
            ensure!(matches!(command, RuntimeIpcCommand::Shutdown));
            response
                .send(RuntimeIpcResponse::ShutdownAccepted)
                .map_err(|_| anyhow::anyhow!("send GUI shutdown response"))?;
            Ok::<_, anyhow::Error>(())
        });

        let connected = execute_request(WorkerRequest::Connect {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(
            connected,
            WorkerSuccess::Connected {
                account_id: connected_account,
                device_id: connected_device,
                canonical_runtime_status,
                ..
            } if connected_account == account_id.to_string()
                && connected_device == device_id.to_string()
                && canonical_runtime_status.configured_matches_loaded
                && !canonical_runtime_status.restart_required
        ));

        let peer_capability_status = execute_request(WorkerRequest::PeerCapabilityStatus {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(
            peer_capability_status,
            WorkerSuccess::PeerCapabilityStatus(status)
                if !status.capability_store_initialized
                    && status.scheduler_suppressed_contact_count == 1
                    && status.items.len() == 1
                    && status.items[0].contact_id == "21".repeat(32)
        ));

        let peer_reenrollment_status = execute_request(WorkerRequest::PeerReenrollmentStatus {
            descriptor: descriptor.clone(),
            contact_id: "21".repeat(32).parse()?,
        })
        .await?;
        assert!(matches!(
            peer_reenrollment_status,
            WorkerSuccess::PeerReenrollmentStatus { response: status, .. }
                if status.contact_id.as_str() == "21".repeat(32)
                    && status.hazard
                        == Some(kilogram_runtime_ipc::RuntimeIpcPeerReenrollmentHazard::MissingDurableQuarantine)
                    && status.suppression.process_local_sync_suppressed
                    && status.suppression.process_local_queue_suppressed_count == 1
        ));

        let contact = execute_request(WorkerRequest::AddContact {
            descriptor: descriptor.clone(),
            draft: ValidatedContactDraft {
                conversation: "desktop-test".to_owned(),
                peer_account_id,
                descriptor_file: PathBuf::from("peer-runtime.ticket"),
            },
        })
        .await?;
        assert!(matches!(
            contact,
            WorkerSuccess::ContactAdded { inserted: true, .. }
        ));

        let queued = execute_request(WorkerRequest::Queue {
            descriptor: descriptor.clone(),
            request_id: RuntimeIpcRequestId::generate()?,
            draft: ValidatedQueueDraft {
                conversation: "desktop-test".to_owned(),
                peer_account_id,
                message: "hello through IPC".to_owned(),
            },
        })
        .await?;
        assert!(matches!(
            queued,
            WorkerSuccess::Queued { inserted: true, .. }
        ));

        let refreshed = execute_request(WorkerRequest::Refresh {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(
            refreshed,
            WorkerSuccess::Refreshed(RuntimeIpcOutboxStatus {
                queue_count: 1,
                pending_count: 1,
                ..
            })
        ));

        let conversations = execute_request(WorkerRequest::Conversations {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(
            conversations,
            WorkerSuccess::Conversations(items)
                if items.len() == 1 && items[0].conversation_label == "desktop-test"
        ));

        let history = execute_request(WorkerRequest::History {
            descriptor: descriptor.clone(),
            conversation: "desktop-test".to_owned(),
            cursor: None,
            older: false,
        })
        .await?;
        assert!(matches!(
            history,
            WorkerSuccess::History { page, older: false }
                if page.total_messages == 0 && page.messages.is_empty()
        ));

        let publication = execute_request(WorkerRequest::PublishTicket {
            descriptor: descriptor.clone(),
            conversation: "desktop-test".to_owned(),
            peer_account_id,
            service_base_url: "https://publication.example".to_owned(),
        })
        .await?;
        assert!(matches!(
            publication,
            WorkerSuccess::TicketPublished(RuntimeIpcTicketPublication {
                publication_generation: 2,
                recipient_device_count: 3,
                ..
            })
        ));

        let ticket_refresh = execute_request(WorkerRequest::RefreshContactTicket {
            descriptor: descriptor.clone(),
            conversation: "desktop-test".to_owned(),
            peer_account_id,
            service_base_url: "https://publication.example".to_owned(),
        })
        .await?;
        assert!(matches!(
            ticket_refresh,
            WorkerSuccess::ContactTicketRefreshed(RuntimeIpcContactTicketRefresh {
                endpoint_candidate_count: 1,
                refreshed_endpoint_candidate_count: 1,
                complete: true,
                ..
            })
        ));

        let automation = execute_request(WorkerRequest::ConfigureTicketAutomation {
            descriptor: descriptor.clone(),
            conversation: "desktop-test".to_owned(),
            peer_account_id,
            enabled: true,
            service_base_url: "https://publication.example".to_owned(),
            allow_ethernet: true,
            allow_wifi: true,
            allow_mobile: false,
            allow_unknown_network: false,
        })
        .await?;
        assert!(matches!(
            automation,
            WorkerSuccess::TicketAutomationConfigured(RuntimeIpcTicketAutomationStatus {
                enabled: true,
                network_allowed: true,
                policy_generation: 1,
                ..
            })
        ));

        let automation_status = execute_request(WorkerRequest::TicketAutomationStatus {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(
            automation_status,
            WorkerSuccess::TicketAutomationStatus(statuses)
                if statuses.len() == 1 && statuses[0].execution_scope
                    == "only-while-runtime-process-is-running"
        ));

        let mailbox_created = execute_request(WorkerRequest::MailboxCreate {
            descriptor: descriptor.clone(),
            conversation: "desktop-test".to_owned(),
            peer_account_id,
            peer_device_id: device_id,
            valid_for_seconds: DEFAULT_MAILBOX_VALIDITY_SECONDS,
        })
        .await?;
        assert!(matches!(
            mailbox_created,
            WorkerSuccess::MailboxChanged(RuntimeIpcMailboxCapabilityTransition {
                action,
                generation: 1,
                ..
            }) if action == "activated"
        ));

        let mailbox_rotated = execute_request(WorkerRequest::MailboxRotate {
            descriptor: descriptor.clone(),
            conversation: "desktop-test".to_owned(),
            peer_account_id,
            peer_device_id: device_id,
            valid_for_seconds: DEFAULT_MAILBOX_VALIDITY_SECONDS,
        })
        .await?;
        assert!(matches!(
            mailbox_rotated,
            WorkerSuccess::MailboxChanged(RuntimeIpcMailboxCapabilityTransition {
                action,
                generation: 2,
                ..
            }) if action == "rotated"
        ));

        let mailbox_revoked = execute_request(WorkerRequest::MailboxRevoke {
            descriptor: descriptor.clone(),
            conversation: "desktop-test".to_owned(),
            peer_account_id,
            peer_device_id: device_id,
        })
        .await?;
        assert!(matches!(
            mailbox_revoked,
            WorkerSuccess::MailboxChanged(RuntimeIpcMailboxCapabilityTransition {
                action,
                generation: 3,
                ..
            }) if action == "revoked"
        ));

        let mailbox_status = execute_request(WorkerRequest::MailboxStatus {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(
            mailbox_status,
            WorkerSuccess::MailboxStatus(RuntimeIpcMailboxStatus {
                local_revoked_head_count: 1,
                capabilities,
                ..
            }) if capabilities.len() == 1
                && capabilities[0].state == "revocation-pending"
        ));

        let directory_update = execute_request(WorkerRequest::ApplyDeviceDirectory {
            descriptor: descriptor.clone(),
            device_list_file: PathBuf::from("refreshed.kadl"),
        })
        .await?;
        assert!(matches!(
            directory_update,
            WorkerSuccess::DeviceDirectoryApplied(RuntimeIpcDeviceDirectoryUpdate {
                authority_revision: 4,
                ratchet_session_records_retired: 1,
                history_availability_status,
                ..
            }) if history_availability_status == "existing-copies-remain-readable"
        ));

        let shutdown = execute_request(WorkerRequest::Shutdown {
            descriptor: descriptor.clone(),
        })
        .await?;
        assert!(matches!(shutdown, WorkerSuccess::Shutdown));

        actor.await??;
        server.shutdown().await?;
        assert!(!descriptor.exists());
        Ok(())
    }

    #[tokio::test]
    async fn desktop_change_watcher_observes_revision_without_actor_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let descriptor = directory.path().join("runtime.ipc.json");
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = DeviceIdentity::generate()?;
        let (server, mut requests) =
            RuntimeIpcServer::start(descriptor.clone(), root.account_id(), &identity).await?;
        let watcher = RuntimeChangeWatcher::spawn()?;
        watcher.subscribe(descriptor)?;
        assert_eq!(server.publish_change(), 1);

        let event = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Some(event) = watcher.try_receive() {
                    break event;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await?;
        assert!(matches!(event, ChangeWatcherEvent::Changed));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), requests.recv())
                .await
                .is_err()
        );
        watcher.stop();
        server.shutdown().await?;
        Ok(())
    }
}
