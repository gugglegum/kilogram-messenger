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
use kilogram_identity::AccountId;
use kilogram_runtime_ipc::{
    RuntimeIpcCommand, RuntimeIpcConversationSummary, RuntimeIpcHistoryCursor,
    RuntimeIpcHistoryMessage, RuntimeIpcHistoryPage, RuntimeIpcOutboxStatus, RuntimeIpcQueueState,
    RuntimeIpcRequestId, RuntimeIpcResponse, RuntimeIpcRoutePolicy, RuntimeLaunchProfile,
    RuntimeLaunchSettings,
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
    startup_error: Option<String>,
}

impl DesktopOptions {
    fn from_arguments(arguments: impl IntoIterator<Item = OsString>) -> Self {
        let mut descriptor_path = None;
        let mut runtime_profile_path = None;
        let mut runtime_executable_path = None;
        let mut startup_error = None;
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let target = if argument == "--ipc-file" {
                &mut descriptor_path
            } else if argument == "--runtime-profile" {
                &mut runtime_profile_path
            } else if argument == "--runtime-exe" {
                &mut runtime_executable_path
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
            startup_error,
        }
    }
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
            peer_prekey_pool_files: "peer-prekeys.bin".to_owned(),
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
        ensure!(
            !peer_prekey_pool_files.is_empty(),
            "At least one peer prekey pool is required"
        );
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
            Err(message) => self.fail(response.operation, message),
        }
    }
}

#[derive(Debug)]
enum WorkerRequest {
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

struct KilogramApp {
    model: ViewModel,
    worker: Option<RuntimeWorker>,
    change_watcher: Option<RuntimeChangeWatcher>,
    pending_change_refresh: bool,
    runtime_profile_path: String,
    runtime_profile_draft: RuntimeProfileDraft,
    show_profile_editor: bool,
    runtime_executable_path: String,
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
            runtime_process: None,
            runtime_start_deadline: None,
            runtime_next_connect_attempt: Instant::now(),
            runtime_start_error: None,
        }
    }

    fn receive_worker_responses(&mut self) {
        while let Some(response) = self.worker.as_ref().and_then(RuntimeWorker::try_receive) {
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
            if self.model.show_contact_form {
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
            ui.label(egui::RichText::new("M0.9.15").color(egui::Color32::from_rgb(88, 166, 255)));
        });
        ui.label("Desktop client · authenticated local runtime IPC");
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
                ui.small("It configures an already enrolled device; account/device bootstrap remains separate.");
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

        if runtime_action == RuntimeUiAction::Connect {
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
        assert!(options.startup_error.is_none());

        let unknown = DesktopOptions::from_arguments([OsString::from("--unknown")]);
        assert!(unknown.startup_error.is_some());
    }

    #[test]
    fn profile_draft_builds_from_public_enrolled_device_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let state_dir = directory.path().join("state");
        fs::create_dir(&state_dir)?;
        let device_list = directory.path().join("device-list.bin");
        let prekeys = directory.path().join("peer-prekeys.bin");
        fs::write(&device_list, b"signed-device-list-placeholder")?;
        fs::write(&prekeys, b"signed-prekeys-placeholder")?;
        let draft = RuntimeProfileDraft {
            state_dir: state_dir.display().to_string(),
            allowed_requester_account_id: ACCOUNT_ID.to_owned(),
            device_list_file: device_list.display().to_string(),
            peer_prekey_pool_files: prekeys.display().to_string(),
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
