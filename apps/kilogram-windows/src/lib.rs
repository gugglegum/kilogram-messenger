use std::{
    path::PathBuf,
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
    RuntimeIpcRequestId, RuntimeIpcResponse,
};

const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_CONVERSATION_BYTES: usize = 4_096;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const HISTORY_PAGE_SIZE: u16 = 50;

pub fn run() -> eframe::Result {
    let descriptor_path = descriptor_path_from_args();
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
        Box::new(move |creation_context| {
            Ok(Box::new(KilogramApp::new(
                creation_context,
                descriptor_path,
            )))
        }),
    )
}

fn descriptor_path_from_args() -> PathBuf {
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--ipc-file" {
            if let Some(path) = arguments.next() {
                return PathBuf::from(path);
            }
            break;
        }
    }
    PathBuf::from("runtime.ipc.json")
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
    Queue,
    Refresh,
    Conversations,
    History,
    HistoryOlder,
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
            Err(message) => self.fail(response.operation, message),
        }
    }
}

#[derive(Debug)]
enum WorkerRequest {
    Connect {
        descriptor: PathBuf,
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
}

impl WorkerRequest {
    fn operation(&self) -> Operation {
        match self {
            Self::Connect { .. } => Operation::Connect,
            Self::Queue { .. } => Operation::Queue,
            Self::Refresh { .. } => Operation::Refresh,
            Self::Conversations { .. } => Operation::Conversations,
            Self::History { older: false, .. } => Operation::History,
            Self::History { older: true, .. } => Operation::HistoryOlder,
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
    Refreshed(RuntimeIpcOutboxStatus),
    Conversations(Vec<RuntimeIpcConversationSummary>),
    History {
        page: RuntimeIpcHistoryPage,
        older: bool,
    },
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
    }
}

struct KilogramApp {
    model: ViewModel,
    worker: Option<RuntimeWorker>,
    last_poll_started: Instant,
}

impl KilogramApp {
    fn new(creation_context: &eframe::CreationContext<'_>, descriptor_path: PathBuf) -> Self {
        creation_context.egui_ctx.set_visuals(egui::Visuals::dark());
        creation_context.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(14.0, 7.0);
            style.interaction.selectable_labels = true;
        });
        let mut model = ViewModel::new(descriptor_path);
        let worker = match RuntimeWorker::spawn() {
            Ok(worker) => Some(worker),
            Err(error) => {
                model.error = Some(format!("Start IPC worker: {error:#}"));
                None
            }
        };
        Self {
            model,
            worker,
            last_poll_started: Instant::now(),
        }
    }

    fn receive_worker_responses(&mut self) {
        while let Some(response) = self.worker.as_ref().and_then(RuntimeWorker::try_receive) {
            let connected = matches!(response.result, Ok(WorkerSuccess::Connected { .. }));
            let queued = matches!(response.result, Ok(WorkerSuccess::Queued { .. }));
            let conversations = matches!(response.result, Ok(WorkerSuccess::Conversations(_)));
            let initial_history = matches!(
                response.result,
                Ok(WorkerSuccess::History { older: false, .. })
            );
            self.model.apply(response);
            if connected || queued {
                self.start_conversations();
            } else if conversations && self.model.selected_contact_id.is_some() {
                self.start_history(false);
            } else if conversations || initial_history {
                self.start_refresh();
            }
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
                self.last_poll_started = Instant::now();
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
                self.last_poll_started = Instant::now();
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

    fn maybe_poll(&mut self) {
        if self.model.connection == ConnectionState::Connected
            && self.model.pending.is_none()
            && self.last_poll_started.elapsed() >= STATUS_POLL_INTERVAL
        {
            self.start_conversations();
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
            self.model.descriptor_path = path.display().to_string();
            self.model.connection = ConnectionState::Disconnected;
            self.model.notice = Some("Runtime descriptor path updated".to_owned());
        }
    }

    fn draw_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Kilogram").size(28.0).strong());
            ui.label(egui::RichText::new("M0.9.13").color(egui::Color32::from_rgb(88, 166, 255)));
        });
        ui.label("Desktop client · authenticated local runtime IPC");
    }

    fn draw_runtime(&mut self, ui: &mut egui::Ui) -> bool {
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
                    self.model.pending.is_none(),
                    egui::TextEdit::singleline(&mut self.model.descriptor_path)
                        .desired_width(f32::INFINITY),
                );
            });
            ui.small("Pass --ipc-file PATH or drop runtime.ipc.json onto this window.");
        });
        ui.add_enabled(
            self.model.pending.is_none(),
            egui::Button::new(if self.model.connection == ConnectionState::Connected {
                "Reconnect"
            } else {
                "Connect"
            }),
        )
        .clicked()
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

    fn draw_conversations(&self, ui: &mut egui::Ui) -> Option<String> {
        ui.heading("Chats");
        if self.model.conversations.is_empty() {
            ui.label("No runtime contacts yet.");
            ui.small("Add a signed contact with kilogram-cli, then refresh.");
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

impl eframe::App for KilogramApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.accept_dropped_descriptor(context);
        self.receive_worker_responses();
        self.maybe_poll();

        let mut connect_clicked = false;
        let mut queue_clicked = false;
        let mut refresh_clicked = false;
        let mut selected_contact = None;
        let mut load_older_clicked = false;
        egui::CentralPanel::default().show(context, |ui| {
            self.draw_header(ui);
            ui.add_space(8.0);
            connect_clicked = self.draw_runtime(ui);
            self.draw_identity(ui);
            ui.separator();
            ui.columns(2, |columns| {
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

        if connect_clicked {
            self.start_connect();
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

        actor.await??;
        server.shutdown().await?;
        assert!(!descriptor.exists());
        Ok(())
    }
}
