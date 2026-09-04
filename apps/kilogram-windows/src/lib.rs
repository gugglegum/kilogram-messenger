use std::{
    ffi::OsString,
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use eframe::egui;
use kilogram_bootstrap_contract::{DesktopBootstrapOutput, MAX_DESKTOP_BOOTSTRAP_OUTPUT_BYTES};
use kilogram_identity::{AccountId, AccountRecoveryPhrase};
use kilogram_runtime_ipc::{
    RuntimeIpcCommand, RuntimeIpcConversationSummary, RuntimeIpcHistoryCursor,
    RuntimeIpcHistoryMessage, RuntimeIpcHistoryPage, RuntimeIpcOutboxStatus, RuntimeIpcQueueState,
    RuntimeIpcRequestId, RuntimeIpcResponse, RuntimeIpcRoutePolicy, RuntimeLaunchProfile,
    RuntimeLaunchSettings,
};
use zeroize::{Zeroize as _, Zeroizing};

mod wizard;

use wizard::{
    AccountRecoveryOutput, AccountRecoveryStatusOutput, DeviceLinkAcceptOutput,
    DeviceLinkAuthorizeOutput, DeviceLinkInspectOutput, DeviceLinkRequestOutput,
    RecoveryCommandOutput, command_arguments, run_json, run_json_with_stdin, run_recovery_command,
};

const CHANGE_WAIT_MILLISECONDS: u32 = 20_000;
const CHANGE_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const MAX_CONVERSATION_BYTES: usize = 4_096;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const HISTORY_PAGE_SIZE: u16 = 50;
const RUNTIME_START_TIMEOUT: Duration = Duration::from_secs(60);
const RUNTIME_START_RETRY_INTERVAL: Duration = Duration::from_millis(500);
const RUNTIME_STOP_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_DESCRIPTOR_PATH_BYTES: usize = 32 * 1024;

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
    startup_error: Option<String>,
}

impl DesktopOptions {
    fn from_arguments(arguments: impl IntoIterator<Item = OsString>) -> Self {
        let mut descriptor_path = None;
        let mut runtime_profile_path = None;
        let mut runtime_executable_path = None;
        let mut bootstrap_executable_path = None;
        let mut startup_error = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let target = if argument == "--ipc-file" {
                &mut descriptor_path
            } else if argument == "--runtime-profile" {
                &mut runtime_profile_path
            } else if argument == "--runtime-exe" {
                &mut runtime_executable_path
            } else if argument == "--bootstrap-exe" {
                &mut bootstrap_executable_path
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
            startup_error,
        }
    }
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
    DeviceLinkRequest,
    DeviceLinkInspect,
    DeviceLinkAuthorize,
    DeviceLinkAccept,
    RecoveryApprove,
    RecoveryRun,
    RecoveryCancel,
    RecoveryReconcile,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccountRecoveryUiAction {
    None,
    Status,
    Export,
    Inspect,
    Restore,
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
        }
    }
}

impl RuntimeProfileDraft {
    fn from_profile(profile: &RuntimeLaunchProfile) -> Self {
        let settings = profile.settings();
        Self {
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
        }
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
        self.selected_contact_id = Some(contact_id.to_owned());
        self.conversation.clone_from(&summary.conversation_label);
        self.peer_account_id = summary.peer_account_id.to_string();
        if changed {
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
            }) => {
                self.connection = ConnectionState::Connected;
                self.account_id = Some(account_id);
                self.device_id = Some(device_id);
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
                inserted,
            }) => {
                self.contact_draft = ContactDraft::default();
                self.show_contact_form = false;
                let action = if inserted {
                    "Contact added"
                } else {
                    "Contact already present"
                };
                self.notice = Some(format!(
                    "{action}: {} · peer {} / device {}",
                    compact_id(&contact_id),
                    compact_id(&peer_account_id),
                    compact_id(&peer_device_id)
                ));
                self.error = None;
            }
            Ok(WorkerSuccess::Refreshed(status)) => {
                self.connection = ConnectionState::Connected;
                self.outbox = Some(status.into());
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
                self.notice = Some("Runtime stopped cleanly".to_owned());
                self.error = None;
            }
            Ok(
                WorkerSuccess::Bootstrapped(_)
                | WorkerSuccess::AccountRecoveryStatus(_)
                | WorkerSuccess::AccountRecoveryExported(_)
                | WorkerSuccess::AccountRecoveryInspected { .. }
                | WorkerSuccess::AccountRecoveryRestored(_)
                | WorkerSuccess::DeviceLinkRequested(_)
                | WorkerSuccess::DeviceLinkInspected { .. }
                | WorkerSuccess::DeviceLinkAuthorized(_)
                | WorkerSuccess::DeviceLinkAccepted(_)
                | WorkerSuccess::RecoveryApproved { .. }
                | WorkerSuccess::RecoveryRan { .. }
                | WorkerSuccess::RecoveryCancelled { .. }
                | WorkerSuccess::RecoveryReconciled(_),
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
    Connect {
        descriptor: PathBuf,
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
            Self::DeviceLinkRequest { .. } => Operation::DeviceLinkRequest,
            Self::DeviceLinkInspect { .. } => Operation::DeviceLinkInspect,
            Self::DeviceLinkAuthorize { .. } => Operation::DeviceLinkAuthorize,
            Self::DeviceLinkAccept { .. } => Operation::DeviceLinkAccept,
            Self::RecoveryApprove { .. } => Operation::RecoveryApprove,
            Self::RecoveryRun { .. } => Operation::RecoveryRun,
            Self::RecoveryCancel { .. } => Operation::RecoveryCancel,
            Self::RecoveryReconcile { .. } => Operation::RecoveryReconcile,
            Self::Connect { .. } => Operation::Connect,
            Self::AddContact { .. } => Operation::AddContact,
            Self::Queue { .. } => Operation::Queue,
            Self::Refresh { .. } => Operation::Refresh,
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
    DeviceLinkRequested(Box<DeviceLinkRequestOutput>),
    DeviceLinkInspected {
        request_file: PathBuf,
        output: Box<DeviceLinkInspectOutput>,
    },
    DeviceLinkAuthorized(Box<DeviceLinkAuthorizeOutput>),
    DeviceLinkAccepted(Box<DeviceLinkAcceptOutput>),
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
    Connected {
        account_id: String,
        device_id: String,
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
        inserted: bool,
    },
    Refreshed(RuntimeIpcOutboxStatus),
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
        WorkerRequest::Connect { descriptor } => {
            match kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::Ping).await? {
                RuntimeIpcResponse::Pong {
                    account_id,
                    device_id,
                } => Ok(WorkerSuccess::Connected {
                    account_id: account_id.to_string(),
                    device_id: device_id.to_string(),
                }),
                RuntimeIpcResponse::Error { message } => bail!("Runtime rejected ping: {message}"),
                _ => bail!("Runtime returned an unexpected ping response"),
            }
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
                    inserted,
                } => Ok(WorkerSuccess::ContactAdded {
                    contact_id,
                    peer_account_id: peer_account_id.to_string(),
                    peer_device_id: peer_device_id.to_string(),
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
        WorkerRequest::Conversations { descriptor } => {
            match kilogram_runtime_ipc::call(&descriptor, RuntimeIpcCommand::ConversationList)
                .await?
            {
                RuntimeIpcResponse::ConversationList(conversations) => {
                    Ok(WorkerSuccess::Conversations(conversations))
                }
                RuntimeIpcResponse::Error { message } => {
                    bail!("Runtime rejected conversation list: {message}")
                }
                _ => bail!("Runtime returned an unexpected conversation-list response"),
            }
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
    drop_target: Option<DropTarget>,
    runtime_process: Option<Child>,
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
            drop_target: None,
            runtime_process: None,
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
                    | Operation::DeviceLinkRequest
                    | Operation::DeviceLinkInspect
                    | Operation::DeviceLinkAuthorize
                    | Operation::DeviceLinkAccept
                    | Operation::RecoveryApprove
                    | Operation::RecoveryRun
                    | Operation::RecoveryCancel
                    | Operation::RecoveryReconcile
            ) {
                self.apply_wizard_response(response);
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
            let connected = matches!(response.result, Ok(WorkerSuccess::Connected { .. }));
            let shutdown = matches!(response.result, Ok(WorkerSuccess::Shutdown));
            let conversations = matches!(response.result, Ok(WorkerSuccess::Conversations(_)));
            let initial_history = matches!(
                response.result,
                Ok(WorkerSuccess::History { older: false, .. })
            );
            self.model.apply(response);
            if connected {
                self.runtime_start_deadline = None;
                self.runtime_start_error = None;
                self.start_change_subscription();
            }
            if shutdown {
                self.stop_change_subscription();
            }
            if connected {
                self.start_conversations();
            } else if conversations && self.model.selected_contact_id.is_some() {
                self.start_history(false);
            } else if conversations || initial_history {
                self.start_refresh();
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
                self.device_link.account_root_dir = account_root_dir;
                self.account_recovery.restored = Some(*output);
                self.model.notice = Some(
                    "Account Root restored. Use the device-link ceremony below to enroll a new device, then recover message history separately."
                        .to_owned(),
                );
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
                self.account_recovery.checkpoint_status = None;
                self.account_recovery.exported = None;
                self.account_recovery.inspected = None;
                self.account_recovery.confirm_latest_witness = false;
                self.device_link.response_input_file = output.response_file.display().to_string();
                self.device_link.authorization = Some(*output);
                self.model.notice = Some(
                    "Exact device enrolled; return the recipient-encrypted response and updated public device list."
                        .to_owned(),
                );
            }
            WorkerSuccess::DeviceLinkAccepted(output) => {
                let workspace = output.workspace_dir.clone();
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
                self.runtime_profile_draft.peer_prekey_pool_files.clear();
                self.show_profile_editor = true;
                self.recovery.state_dir = output.state_dir.display().to_string();
                self.device_link.accepted = Some(*output);
                self.model.notice = Some(
                    "Device link accepted. Runtime profile paths are filled; add peer routing data, save the profile, then recover history from one or more devices."
                        .to_owned(),
                );
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
        self.runtime_profile_draft.state_dir = output.state_dir().display().to_string();
        self.runtime_profile_draft
            .allowed_requester_account_id
            .clear();
        self.runtime_profile_draft.device_list_file =
            output.device_list_file().display().to_string();
        self.runtime_profile_draft.peer_prekey_pool_files.clear();
        self.runtime_profile_draft.ticket_file = runtime_ticket.display().to_string();
        self.runtime_profile_draft.ipc_file = runtime_ipc.display().to_string();
        self.show_profile_editor = true;
        self.bootstrap_workspace_path = workspace.display().to_string();
        self.account_recovery.account_root_dir = output.account_root_dir().display().to_string();
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
            ensure!(
                self.account_recovery.confirm_latest_witness,
                "Confirm that the independently retained witness is the newest known checkpoint"
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

    fn load_runtime_profile(&mut self) {
        let result: Result<RuntimeLaunchProfile> = (|| {
            let path = required_path(&self.runtime_profile_path, "Runtime profile")?;
            RuntimeLaunchProfile::load(&path)
        })();
        match result {
            Ok(profile) => {
                self.model.descriptor_path = profile.settings().ipc_file.display().to_string();
                self.runtime_profile_draft = RuntimeProfileDraft::from_profile(&profile);
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
                self.model.notice = Some("Runtime launch profile saved atomically".to_owned());
                self.model.error = None;
            }
            Err(error) => self.model.error = Some(format!("Save runtime profile: {error:#}")),
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
                self.runtime_process = Some(child);
                self.runtime_start_deadline = Some(Instant::now() + RUNTIME_START_TIMEOUT);
                self.runtime_next_connect_attempt = Instant::now();
                self.runtime_start_error = None;
                self.model.connection = ConnectionState::Connecting;
                self.model.notice = Some("Runtime process started; waiting for IPC".to_owned());
                self.model.error = None;
            }
            Err(error) => self.model.fail(Operation::Connect, format!("{error:#}")),
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
                let was_starting = self.runtime_start_deadline.take().is_some();
                self.model.pending = None;
                self.model.connection = ConnectionState::Disconnected;
                self.model.account_id = None;
                self.model.device_id = None;
                self.stop_change_subscription();
                if was_starting {
                    let detail = self
                        .runtime_start_error
                        .take()
                        .unwrap_or_else(|| format!("runtime exited with {status}"));
                    self.model.error = Some(format!("Runtime failed to start: {detail}"));
                } else {
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
            self.model.fail(
                Operation::Connect,
                self.runtime_start_error
                    .take()
                    .unwrap_or_else(|| "Runtime startup timed out".to_owned()),
            );
            return;
        }
        if self.model.pending.is_none() && Instant::now() >= self.runtime_next_connect_attempt {
            self.start_connect();
        }
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
                self.model.notice = Some("Runtime descriptor path updated".to_owned());
            }
        }
    }

    fn draw_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Kilogram").size(28.0).strong());
            ui.label(egui::RichText::new("M0.9.21").color(egui::Color32::from_rgb(88, 166, 255)));
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
                    ui.checkbox(
                        &mut self.account_recovery.confirm_latest_witness,
                        "I independently verified that this is my newest known witness",
                    );
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
                                && self.account_recovery.confirm_latest_witness
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
                ui.add_enabled(
                    self.model.pending.is_none() && self.runtime_process.is_none(),
                    egui::TextEdit::singleline(&mut self.model.descriptor_path)
                        .desired_width(f32::INFINITY),
                );
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
            ui.small("The runtime verifies membership, account, device authorization and route before persisting.");
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
                "Peer {} · {}",
                compact_id(&summary.peer_account_id.to_string()),
                summary.route_policy.as_str()
            ));
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
        self.maybe_connect_started_runtime();

        let mut runtime_action = RuntimeUiAction::None;
        let mut bootstrap_action = BootstrapUiAction::None;
        let mut account_recovery_action = AccountRecoveryUiAction::None;
        let mut device_link_action = DeviceLinkUiAction::None;
        let mut recovery_action = RecoveryUiAction::None;
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
                    recovery_action = self.draw_recovery(ui);
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
        } else if device_link_action == DeviceLinkUiAction::Request {
            self.start_device_link_request();
        } else if device_link_action == DeviceLinkUiAction::Inspect {
            self.start_device_link_inspect();
        } else if device_link_action == DeviceLinkUiAction::Authorize {
            self.start_device_link_authorize();
        } else if device_link_action == DeviceLinkUiAction::Accept {
            self.start_device_link_accept();
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
        } else if add_contact_clicked {
            self.start_add_contact();
        } else if let Some(contact_id) = selected_contact {
            if self.model.select_conversation(&contact_id) {
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

#[cfg(test)]
mod tests {
    use kilogram_identity::{AccountRootState, DeviceIdentity};
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
        assert!(options.startup_error.is_none());

        let unknown = DesktopOptions::from_arguments([OsString::from("--unknown")]);
        assert!(unknown.startup_error.is_some());
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
        let profile_path = directory.path().join("runtime.launch.json");
        profile.write_replace(&profile_path)?;
        assert_eq!(RuntimeLaunchProfile::load(&profile_path)?, profile);
        assert_eq!(
            RuntimeProfileDraft::from_profile(&profile).route_policy,
            RuntimeIpcRoutePolicy::Auto
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
            route_policy: RuntimeIpcRoutePolicy::Auto,
            message_count: 0,
            latest_message: None,
        }]);
        assert_eq!(model.selected_contact_id.as_deref(), Some("contact-a"));
        assert_eq!(model.conversation, "alice-bob");
        assert_eq!(model.peer_account_id, peer.to_string());
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
        let (server, mut requests) =
            RuntimeIpcServer::start(descriptor.clone(), account_id, &identity).await?;
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
                    retry_state_count: 0,
                    items: Vec::new(),
                }))
                .map_err(|_| anyhow::anyhow!("send GUI status response"))?;

            let conversations = requests
                .recv()
                .await
                .context("receive GUI conversation list")?;
            let (command, response) = conversations.into_parts();
            ensure!(matches!(command, RuntimeIpcCommand::ConversationList));
            response
                .send(RuntimeIpcResponse::ConversationList(vec![
                    RuntimeIpcConversationSummary {
                        contact_id: "22".repeat(32),
                        conversation_label: "desktop-test".to_owned(),
                        conversation_id: ConversationId::from_label("desktop-test"),
                        peer_account_id,
                        peer_device_id: device_id,
                        route_policy: RuntimeIpcRoutePolicy::Auto,
                        message_count: 0,
                        latest_message: None,
                    },
                ]))
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
            } if connected_account == account_id.to_string()
                && connected_device == device_id.to_string()
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
