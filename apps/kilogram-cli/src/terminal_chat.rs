use kilogram_first_dialog::chat::{Chat, ChatStatus, Mode, SessionStop};
use kilogram_first_dialog::connection::LocatorCommand;
use kilogram_first_dialog::operation::{OperationControl, RequestIdentity as RequestKey};
use kilogram_first_dialog::{send_intent::valid_text, send_recovery::RequestOperation};
use kilogram_runtime_ipc::ordered2::{HistoryCursor2, HistoryPage2, MAX_HISTORY_ROWS};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::Arc,
};
use zeroize::Zeroizing;

mod coordinator;
pub use coordinator::run;
pub use coordinator::run_product_owner;
pub use coordinator::run_product_owned;

const LINE_CAP: usize = 32 * 1024;
const OUTPUT_CAP: usize = 256 * 1024;
const HELP: &str = "/help /connect start|attach /status [request_id] /chats /use 1|2 /send <text> /retry <request_id> /history [limit] /history more [limit] /locator export|import <path>|retry /cancel /disconnect /quit";
type CommandResult<T> = Result<T, &'static str>;
type Prepared<T> = CommandResult<(Value, Option<T>)>;

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Locator(LocatorCommand),
    Connect {
        observer: bool,
        instance: Option<String>,
    },
    Status,
    Chats,
    Use(u8),
    Message {
        context: Option<String>,
        request: Option<[u8; 16]>,
        operation: RequestOperation,
    },
    History {
        context: Option<String>,
        limit: usize,
        cursor: Option<String>,
        more: bool,
    },
    Cancel(Option<String>),
    Disconnect,
    Quit,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    v: u8,
    id: String,
    op: String,
    args: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectArgs {
    mode: String,
    runtime_instance: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryArgs {
    context: String,
    #[serde(default = "page_limit")]
    limit: usize,
    cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendArgs {
    context: String,
    request_id: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestArgs {
    context: String,
    request_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelArgs {
    target_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocatorImportArgs {
    path: String,
}

fn locator_import(path: &str) -> CommandResult<Command> {
    if path.is_empty() || path.contains('\0') {
        return Err("invalid_args");
    }
    Ok(Command::Locator(LocatorCommand::Import(PathBuf::from(
        path,
    ))))
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.is_ascii()
}

fn request_id(text: &str) -> CommandResult<[u8; 16]> {
    if text.len() != 32 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid_request_id");
    }
    let mut id = [0; 16];
    for (index, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * index..2 * index + 2], 16)
            .map_err(|_| "invalid_request_id")?;
    }
    Ok(id)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn send(
    context: Option<String>,
    request: Option<[u8; 16]>,
    text: String,
) -> CommandResult<Command> {
    let text = Zeroizing::new(text);
    if !valid_text(&text) {
        return Err("invalid_text");
    }
    Ok(Command::Message {
        context,
        request,
        operation: RequestOperation::Send(text),
    })
}

fn page_limit() -> usize {
    16
}

fn parse_wire(request: Request) -> CommandResult<Command> {
    if request.v != 1 {
        return Err("unsupported_version");
    }
    let empty = request.args.as_object().is_some_and(|args| args.is_empty());
    match request.op.as_str() {
        "connect" => {
            let args: ConnectArgs =
                serde_json::from_value(request.args).map_err(|_| "invalid_args")?;
            match args.mode.as_str() {
                "start" if args.runtime_instance.is_none() => Ok(Command::Connect {
                    observer: false,
                    instance: None,
                }),
                "attach" => Ok(Command::Connect {
                    observer: true,
                    instance: args.runtime_instance,
                }),
                _ => Err("invalid_args"),
            }
        }
        "history" => {
            let args: HistoryArgs =
                serde_json::from_value(request.args).map_err(|_| "invalid_args")?;
            Ok(Command::History {
                context: Some(args.context),
                limit: args.limit,
                cursor: args.cursor,
                more: false,
            })
        }
        "send" => {
            let args: SendArgs =
                serde_json::from_value(request.args).map_err(|_| "invalid_args")?;
            send(
                Some(args.context),
                Some(request_id(&args.request_id)?),
                args.text,
            )
        }
        "request_status" | "retry" => {
            let args: RequestArgs =
                serde_json::from_value(request.args).map_err(|_| "invalid_args")?;
            Ok(Command::Message {
                context: Some(args.context),
                request: Some(request_id(&args.request_id)?),
                operation: if request.op == "retry" {
                    RequestOperation::Retry
                } else {
                    RequestOperation::Status
                },
            })
        }
        "cancel" => {
            let args: CancelArgs =
                serde_json::from_value(request.args).map_err(|_| "invalid_args")?;
            if !valid_id(&args.target_id) || args.target_id == request.id {
                return Err("invalid_args");
            }
            Ok(Command::Cancel(Some(args.target_id)))
        }
        "status" if empty => Ok(Command::Status),
        "chats" if empty => Ok(Command::Chats),
        "disconnect" if empty => Ok(Command::Disconnect),
        "quit" if empty => Ok(Command::Quit),
        "status" | "chats" | "disconnect" | "quit" => Err("invalid_args"),
        "locator_export" if empty => Ok(Command::Locator(LocatorCommand::Export)),
        "locator_retry" if empty => Ok(Command::Locator(LocatorCommand::Retry)),
        "locator_import" | "locator_import_latest" => {
            let args: LocatorImportArgs =
                serde_json::from_value(request.args).map_err(|_| "invalid_args")?;
            let command=locator_import(&args.path)?;
            if request.op=="locator_import_latest" {
                Ok(Command::Locator(LocatorCommand::ImportLatest(PathBuf::from(args.path))))
            }else {Ok(command)}
        }
        "locator_export" | "locator_retry" => Err("invalid_args"),
        _ => Err("unknown_operation"),
    }
}

fn parse_tty(text: &str) -> CommandResult<Command> {
    if !text.starts_with('/') {
        return send(None, None, text.into());
    }
    if text.starts_with("//") {
        return send(None, None, text[1..].into());
    }
    if let Some(tail) = text.strip_prefix("/send")
        && let Some(body) = tail.strip_prefix(|c: char| c.is_ascii_whitespace())
    {
        return send(None, None, body.into());
    }
    if let Some(tail) = text.strip_prefix("/locator")
        && let Some(args) = tail.strip_prefix(|c: char| c.is_ascii_whitespace())
    {
        let args = args.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if let Some(path) = args
            .strip_prefix("import")
            .and_then(|tail| tail.strip_prefix(|c: char| c.is_ascii_whitespace()))
        {
            return locator_import(path);
        }
        return match args.trim_ascii() {
            "export" => Ok(Command::Locator(LocatorCommand::Export)),
            "retry" => Ok(Command::Locator(LocatorCommand::Retry)),
            "import" => Err("invalid_args"),
            _ => Err("invalid_command"),
        };
    }
    let args: Vec<_> = text.split_ascii_whitespace().take(4).collect();
    match args.as_slice() {
        ["/help"] => Ok(Command::Help),
        ["/connect", "start"] => Ok(Command::Connect {
            observer: false,
            instance: None,
        }),
        ["/connect", "attach"] => Ok(Command::Connect {
            observer: true,
            instance: None,
        }),
        ["/status"] => Ok(Command::Status),
        [command @ ("/status" | "/retry"), id] => Ok(Command::Message {
            context: None,
            request: Some(request_id(id)?),
            operation: if *command == "/retry" {
                RequestOperation::Retry
            } else {
                RequestOperation::Status
            },
        }),
        ["/chats"] => Ok(Command::Chats),
        ["/use", "1"] => Ok(Command::Use(0)),
        ["/use", "2"] => Ok(Command::Use(1)),
        ["/history", tail @ ..] => {
            let (more, limit) = match tail {
                [] => (false, None),
                ["more"] => (true, None),
                ["more", limit] => (true, Some(limit)),
                [limit] => (false, Some(limit)),
                _ => return Err("invalid_command"),
            };
            let limit = match limit {
                Some(limit) => limit.parse().map_err(|_| "invalid_args")?,
                None => page_limit(),
            };
            Ok(Command::History {
                context: None,
                cursor: None,
                limit,
                more,
            })
        }
        ["/cancel"] => Ok(Command::Cancel(None)),
        ["/disconnect"] => Ok(Command::Disconnect),
        ["/quit"] => Ok(Command::Quit),
        ["/send"] => Err("invalid_text"),
        _ => Err("invalid_command"),
    }
}

struct Continuation {
    token: String,
    cursor: HistoryCursor2,
    view: [u8; 32],
    frontiers: [u64; 2],
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CommandScope {
    generation: u64,
    connection: Option<u64>,
}

struct HistoryAdvance {
    scope: CommandScope,
    gap: Option<u64>,
    slot: usize,
    context: String,
    from: Option<HistoryCursor2>,
    view: [u8; 32],
    frontiers: [u64; 2],
    cursor: HistoryCursor2,
    next_token: Option<String>,
}

impl HistoryAdvance {
    fn apply(self, frontend: &mut Frontend, connection: Option<u64>) {
        if self.scope.connection.is_none()
            || self.scope.connection != connection
            || frontend
                .status
                .as_ref()
                .is_none_or(|status| status.contexts[self.slot] != self.context)
        {
            return;
        }
        let current = &mut frontend.continuation[self.slot];
        if let Some(input) = self.from
            && current.as_ref().is_none_or(|previous| {
                previous.cursor != input
                    || previous.view != self.view
                    || previous.frontiers != self.frontiers
            })
        {
            return;
        }
        *current = self.next_token.map(|token| Continuation {
            token,
            cursor: self.cursor,
            view: self.view,
            frontiers: self.frontiers,
        });
    }
}

struct Frontend {
    service: Chat,
    status: Option<ChatStatus>,
    selected: u8,
    continuation: [Option<Continuation>; 2],
}

impl Frontend {
    fn execute(
        &mut self,
        command: Command,
        scope: CommandScope,
        control: &Arc<OperationControl>,
        stop: &SessionStop,
        announce: impl FnOnce(&RequestKey) -> bool,
    ) -> Prepared<HistoryAdvance> {
        let result: CommandResult<Value> = match command {
            Command::Help => Ok(json!({ "help": HELP, "sending": true })),
            Command::Connect { observer, instance } => {
                if self.status.is_some() {
                    return Err("already_connected");
                }
                let mode = if observer {
                    Mode::Observer
                } else {
                    Mode::Owner
                };
                let status = self
                    .service
                    .connect_controlled(mode, instance.as_deref(), Some(control), stop)
                    .map_err(|_| "connect_failed")?;
                let value = json!(status);
                self.status = Some(status);
                self.continuation = [None, None];
                Ok(value)
            }
            Command::Status => {
                if self.status.is_none() {
                    return Ok((json!({ "connected": false }), None));
                }
                let status = self
                    .service
                    .status_controlled(Some(control))
                    .map_err(|_| "session_unavailable")?;
                let pinned = self.status.as_ref().ok_or("not_connected")?;
                if pinned.runtime_instance != status.runtime_instance
                    || pinned.contexts != status.contexts
                    || pinned.own_roles != status.own_roles
                {
                    return Err("session_changed");
                }
                Ok(json!({ "connected": true, "status": status }))
            }
            Command::Chats => {
                let contexts = &self.status.as_ref().ok_or("not_connected")?.contexts;
                Ok(
                    json!({ "chats": contexts.iter().enumerate().map(|(i, context)| json!({
                    "number": i+1, "context": context,
                })).collect::<Vec<_>>() }),
                )
            }
            Command::Use(slot) => {
                self.selected = slot;
                Ok(json!({ "selected": slot + 1 }))
            }
            Command::Message {
                context,
                request,
                operation,
            } => {
                let status = self.status.as_ref().ok_or("not_connected")?;
                let slot = match context {
                    Some(context) => status
                        .contexts
                        .iter()
                        .position(|c| c == &context)
                        .ok_or("invalid_context")?,
                    None => usize::from(self.selected),
                };
                let request = match request {
                    Some(request) => request,
                    None => {
                        let mut request = [0; 16];
                        getrandom::fill(&mut request).map_err(|_| "request_id_unavailable")?;
                        request
                    }
                };
                let resolved = RequestKey {
                    context: status.contexts[slot].clone(),
                    request_id: hex(&request),
                };
                control.identify(resolved.clone());
                if status.mode == Mode::Observer && !matches!(operation, RequestOperation::Status) {
                    return Err("observer_read_only");
                }
                if matches!(&operation, RequestOperation::Send(_)) && !announce(&resolved) {
                    return Err("stdout_unavailable");
                }
                let report = self.service.request_controlled(
                    slot as u8,
                    request,
                    operation,
                    Some(control),
                )?;
                Ok(json!(report))
            }
            Command::History {
                context,
                limit,
                cursor,
                more,
            } => return self.history(context, limit, cursor, more, scope, control),
            Command::Locator(command) => Ok(json!(self.service.locator(command, control)?)),
            Command::Cancel(_) | Command::Disconnect | Command::Quit => {
                unreachable!("control command")
            }
        };
        Ok((result?, None))
    }

    fn history(
        &mut self,
        context: Option<String>,
        limit: usize,
        token: Option<String>,
        more: bool,
        scope: CommandScope,
        control: &Arc<OperationControl>,
    ) -> Prepared<HistoryAdvance> {
        if !(1..=MAX_HISTORY_ROWS).contains(&limit) {
            return Err("invalid_args");
        }
        let contexts = &self.status.as_ref().ok_or("not_connected")?.contexts;
        let slot = match context {
            Some(context) => contexts
                .iter()
                .position(|c| c == &context)
                .ok_or("invalid_context")?,
            None => usize::from(self.selected),
        };
        let cursor = if token.is_some() || more {
            Some(
                self.continuation[slot]
                    .as_ref()
                    .filter(|current| token.as_ref().is_none_or(|t| t == &current.token))
                    .ok_or("invalid_cursor")?
                    .cursor
                    .clone(),
            )
        } else {
            None
        };
        let page = self
            .service
            .history_controlled(slot as u8, cursor.clone(), limit, Some(control))
            .map_err(|_| "history_unavailable")?;
        let next = if page.finished {
            None
        } else {
            let mut nonce = [0u8; 16];
            getrandom::fill(&mut nonce).map_err(|_| "cursor_unavailable")?;
            Some(hex(&nonce))
        };
        let result = page_value(&contexts[slot], &page, next.as_deref());
        Ok((
            result,
            Some(HistoryAdvance {
                scope,
                gap: None,
                slot,
                context: contexts[slot].clone(),
                from: cursor,
                view: page.view,
                frontiers: page.frontiers,
                cursor: page.cursor,
                next_token: next,
            }),
        ))
    }
}

fn page_value(context: &str, page: &HistoryPage2, cursor: Option<&str>) -> Value {
    let rows: Vec<_> = page
        .items
        .iter()
        .map(|row| {
            json!({
                "event_id": hex(&row.event),
                "role": row.role, "ordinal": row.ordinal, "text": row.text.as_str(),
            })
        })
        .collect();
    json!({ "context": context, "frontiers": page.frontiers, "finished": page.finished, "cursor": cursor, "items": rows })
}

pub(crate) fn terminal_escaped(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

pub(crate) fn terminal_text(text: &str) -> String {
    let mut safe = String::with_capacity(text.len());
    for c in text.chars() {
        if terminal_escaped(c) {
            safe.extend(c.escape_default());
        } else {
            safe.push(c);
        }
    }
    safe
}

fn write_tty(mut out: impl Write, result: &CommandResult<Value>) -> io::Result<()> {
    match result {
        Ok(value) if value.get("help").is_some() => {
            writeln!(
                out,
                "{HELP}\nExample: /locator import C:\\Pair Files\\peer.json"
            )
        }
        Ok(value) if value.get("path").and_then(Value::as_str).is_some() => {
            let path = terminal_text(value["path"].as_str().unwrap_or_default());
            let state = value["state"].as_str().unwrap_or("unconfirmed");
            writeln!(out, "Locator {state}: {path}")
        }
        Ok(value) if value.get("items").is_some() => {
            if let Some(rows) = value["items"].as_array() {
                for row in rows {
                    let text = row["text"].as_str().unwrap_or_default();
                    let safe = terminal_text(text);
                    writeln!(out, "{}: {safe}", row["role"])?;
                }
            }
            if value["finished"] == true {
                writeln!(out, "End of history.")
            } else {
                writeln!(out, "Use /history more for the next page.")
            }
        }
        Ok(value) if value.get("lan_discovery").and_then(Value::as_str).is_some() => {
            let message = match value["lan_discovery"].as_str() {
                Some("waiting_network") => {
                    "Local runtime ready. Waiting for the saved Private LAN."
                }
                Some("waiting_policy") => {
                    "Local runtime ready. Waiting for current permission and locators."
                }
                Some("discovering") => "Local runtime ready. Discovering the agreed LAN peer.",
                Some("retrying") => "Local runtime ready. Authenticating or retrying the peer.",
                Some("connected") if value["live_admitted"] == true => {
                    "Connected on both LAN contexts."
                }
                _ => "Local runtime ready. Awaiting a current connection check.",
            };
            writeln!(out, "{message}")
        }
        Ok(value) => writeln!(out, "{value}"),
        Err("unsupported") => {
            writeln!(out, "Operation unavailable in this version. Use /help.")
        }
        Err("response_too_large") => writeln!(
            out,
            "Error: response_too_large. Retry /history or /history more with a smaller limit."
        ),
        Err("locator_requires_manual_profile") => writeln!(
            out,
            "Locator import and retry require a Manual profile. Quit chat before changing the route in Setup."
        ),
        Err(code) => writeln!(out, "Error: {code}. Use /help."),
    }
}

#[derive(Serialize)]
struct WireResult<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    id: Option<&'a str>,
    ok: bool,
    #[serde(flatten)]
    request: Option<&'a RequestKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitor: Option<&'a Value>,
}

#[derive(Default)]
struct ResultBuffer(Vec<u8>);

impl Write for ResultBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > OUTPUT_CAP - self.0.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Result exceeds output cap",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
fn encode_result(
    jsonl: bool,
    id: Option<&str>,
    request: Option<&RequestKey>,
    result: &CommandResult<Value>,
) -> io::Result<Vec<u8>> {
    encode_reply(jsonl, id, request, None, result, None)
}

fn encode_reply(
    jsonl: bool,
    id: Option<&str>,
    request: Option<&RequestKey>,
    target_id: Option<&str>,
    result: &CommandResult<Value>,
    monitor: Option<&Value>,
) -> io::Result<Vec<u8>> {
    let mut output = ResultBuffer::default();
    if jsonl {
        let message = WireResult {
            v: 1,
            kind: "result",
            id,
            ok: result.is_ok(),
            request,
            target_id,
            value: result.as_ref().ok(),
            code: result.as_ref().err().copied(),
            monitor,
        };
        serde_json::to_writer(&mut output, &message).map_err(io::Error::other)?;
        output.write_all(b"\n")?;
    } else {
        if let Some(request) = request {
            writeln!(
                output,
                "Request {} ({})",
                request.request_id, request.context
            )?;
        }
        match (monitor, result) {
            (Some(monitor), Ok(value)) => {
                let mut value = value.clone();
                value["monitor"] = monitor.clone();
                write_tty(&mut output, &Ok(value))?;
            }
            (Some(monitor), Err(_)) => {
                write_tty(&mut output, result)?;
                writeln!(output, "Monitor {monitor}")?;
            }
            _ => write_tty(&mut output, result)?,
        }
    }
    Ok(output.0)
}

#[cfg(test)]
fn prepare_result<T>(
    jsonl: bool,
    id: Option<&str>,
    request: Option<&RequestKey>,
    result: Prepared<T>,
) -> io::Result<(Vec<u8>, Option<T>)> {
    prepare_result_with_monitor(jsonl, id, request, result, None)
}

fn prepare_result_with_monitor<T>(
    jsonl: bool,
    id: Option<&str>,
    request: Option<&RequestKey>,
    result: Prepared<T>,
    monitor: Option<&Value>,
) -> io::Result<(Vec<u8>, Option<T>)> {
    let (value, advance) = match result {
        Ok((value, advance)) => (Ok(value), advance),
        Err(code) => (Err(code), None),
    };
    match encode_reply(jsonl, id, request, None, &value, monitor) {
        Ok(bytes) => Ok((bytes, advance)),
        // No history position changes when the rendered page will not be published.
        Err(_) => Ok((
            encode_reply(
                jsonl,
                id,
                request,
                None,
                &Err("response_too_large"),
                monitor,
            )?,
            None,
        )),
    }
}

pub(crate) enum Frame {
    Line(Vec<u8>),
    TooLong,
}

pub(crate) fn frame(input: &mut impl BufRead) -> io::Result<Option<Frame>> {
    let mut line = Vec::new();
    let mut oversized = false;
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return Ok(if oversized {
                Some(Frame::TooLong)
            } else if line.is_empty() {
                None
            } else {
                Some(Frame::Line(line))
            });
        }
        let end = chunk.iter().position(|b| *b == b'\n');
        let len = end.unwrap_or(chunk.len());
        if !oversized && line.len() + len <= LINE_CAP {
            line.extend_from_slice(&chunk[..len]);
        } else {
            oversized = true;
        }
        input.consume(len + usize::from(end.is_some()));
        if end.is_some() {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok(Some(if oversized {
                Frame::TooLong
            } else {
                Frame::Line(line)
            }));
        }
    }
}

enum Input {
    Command {
        id: Option<String>,
        command: CommandResult<Command>,
    },
    FrameError(&'static str),
    ReadError,
}

fn read_commands(
    mut reader: impl BufRead,
    jsonl: bool,
    mut consume: impl FnMut(Input) -> bool,
) -> bool {
    loop {
        let input = match frame(&mut reader) {
            Ok(None) => return true,
            Ok(Some(Frame::TooLong)) => Input::FrameError("line_too_long"),
            Ok(Some(Frame::Line(line))) => match std::str::from_utf8(&line) {
                Ok(text) if jsonl => match serde_json::from_str::<Request>(text) {
                    Ok(request) if valid_id(&request.id) => Input::Command {
                        id: Some(request.id.clone()),
                        command: parse_wire(request),
                    },
                    _ => Input::FrameError("invalid_request"),
                },
                Ok("") => continue,
                Ok(text) => Input::Command {
                    id: None,
                    command: parse_tty(text),
                },
                Err(_) => Input::FrameError("invalid_utf8"),
            },
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => Input::ReadError,
        };
        let failed = matches!(input, Input::ReadError);
        if !consume(input) || failed {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn t11a_framing_and_utf8() -> io::Result<()> {
        let mut data = vec![b'X'; LINE_CAP + 1];
        data.extend_from_slice("\n/чаты\r\n".as_bytes());
        let mut input = Cursor::new(data);
        assert!(matches!(frame(&mut input)?, Some(Frame::TooLong)));
        assert!(
            matches!(frame(&mut input)?, Some(Frame::Line(line)) if line == "/чаты".as_bytes())
        );
        assert!(frame(&mut input)?.is_none());
        Ok(())
    }

    #[test]
    fn t11a_command_parity() -> anyhow::Result<()> {
        let request = serde_json::from_value(
            json!({"v":1,"id":"a","op":"connect","args":{"mode":"attach"}}),
        )?;
        assert_eq!(parse_wire(request), parse_tty("/connect attach"));
        assert_eq!(parse_tty("/connect start extra"), Err("invalid_command"));
        assert_eq!(
            parse_tty("/history more"),
            Ok(Command::History {
                context: None,
                limit: 16,
                cursor: None,
                more: true
            })
        );
        let request =
            serde_json::from_value(json!({"v":1,"id":"b","op":"send","args":{"text":"test"}}))?;
        assert_eq!(parse_wire(request), Err("invalid_args"));
        Ok(())
    }

    #[test]
    fn t11d_locator_paths_are_literal_and_safe_in_tty() -> anyhow::Result<()> {
        let path = r"C:\Тест Software\peer.json";
        let command = Ok(Command::Locator(LocatorCommand::Import(PathBuf::from(
            path,
        ))));
        assert_eq!(parse_tty(&format!("/locator import {path}")), command);
        let wire = serde_json::from_value(
            json!({"v":1,"id":"loc","op":"locator_import","args":{"path":path}}),
        )?;
        assert_eq!(parse_wire(wire), command);
        assert_eq!(
            parse_tty("/locator export"),
            Ok(Command::Locator(LocatorCommand::Export))
        );
        assert_eq!(
            parse_tty("/locator retry"),
            Ok(Command::Locator(LocatorCommand::Retry))
        );
        assert_eq!(parse_tty("/locator import"), Err("invalid_args"));
        assert_eq!(parse_tty("/locator import "), Err("invalid_args"));
        assert_eq!(parse_tty("/locator import a\0b"), Err("invalid_args"));
        let mut output = Vec::new();
        write_tty(
            &mut output,
            &Ok(json!({"state":"exported","path":"public-\u{202e}\u{1b}.json"})),
        )?;
        let text = String::from_utf8(output)?;
        assert!(text.contains(r"public-\u{202e}\u{1b}.json"));
        assert!(!text.contains('\u{202e}') && !text.contains('\u{1b}'));
        Ok(())
    }

    #[test]
    fn t11b_send_text_and_ids_are_independent() -> anyhow::Result<()> {
        for (input, expected) in [
            ("  привет  ", "  привет  "),
            ("//quit", "/quit"),
            ("/send  привет  ", " привет  "),
        ] {
            assert!(
                matches!(parse_tty(input).map_err(anyhow::Error::msg)?, Command::Message {
                context: None, request: None, operation: RequestOperation::Send(text),
            } if text.as_str() == expected)
            );
        }
        assert_eq!(parse_tty("/send "), Err("invalid_text"));
        assert_eq!(parse_tty("/retry атака"), Err("invalid_request_id"));
        assert_eq!(parse_tty(&"Ж".repeat(2049)), Err("invalid_text"));
        let durable = "aB".repeat(16);
        let request: Request = serde_json::from_value(json!({
            "v":1,"id":"correlation","op":"send",
            "args":{"context":"ctx","request_id":durable,"text":"a\n b"}
        }))?;
        assert_eq!(request.id, "correlation");
        assert!(
            matches!(parse_wire(request).map_err(anyhow::Error::msg)?, Command::Message {
            context: Some(context), request: Some(id), operation: RequestOperation::Send(text),
        } if context == "ctx" && id == [0xab; 16] && text.as_str() == "a\n b")
        );
        let key = RequestKey {
            context: "ctx".into(),
            request_id: durable.to_ascii_lowercase(),
        };
        let bytes = encode_result(true, Some("correlation"), Some(&key), &Err("text_conflict"))?;
        let value: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            value,
            json!({"v":1,"type":"result","id":"correlation","ok":false,
            "context":"ctx","request_id":key.request_id,"code":"text_conflict"})
        );
        Ok(())
    }

    #[test]
    fn t11a_input_keeps_frames_distinct() -> anyhow::Result<()> {
        let mut input = vec![0xff, b'\n'];
        input.extend_from_slice(b"{\"v\":1,\"id\":\"same\",\"op\":\"status\",\"args\":{}}\n");
        let mut entries = Vec::new();
        assert!(read_commands(Cursor::new(input), true, |frame| {
            entries.push(frame);
            true
        }));
        assert!(matches!(entries[0], Input::FrameError("invalid_utf8")));
        assert!(
            matches!(&entries[1], Input::Command { id: Some(id), command: Ok(Command::Status) } if id == "same")
        );
        Ok(())
    }

    #[test]
    fn t11a_output_cap_counts_result_envelope_and_newline() -> anyhow::Result<()> {
        for jsonl in [true, false] {
            let id = Some("quoted\"id");
            let envelope = encode_result(jsonl, id, None, &Ok(json!("")))?.len();
            let maximum = OUTPUT_CAP - envelope;
            let (exact, advance) =
                prepare_result(jsonl, id, None, Ok((json!("x".repeat(maximum)), Some(1))))?;
            assert_eq!(exact.len(), OUTPUT_CAP);
            assert_eq!(exact.last(), Some(&b'\n'));
            assert_eq!(advance, Some(1));
            let (error, advance) = prepare_result(
                jsonl,
                id,
                None,
                Ok((json!("x".repeat(maximum + 1)), Some(2))),
            )?;
            assert!(error.len() < 512);
            assert!(advance.is_none());
        }
        Ok(())
    }

    #[test]
    fn t11a_oversized_history_can_retry_same_cursor_without_losing_rows() -> anyhow::Result<()> {
        let all_rows: Vec<Value> = (0..17)
            .map(|ordinal| {
                let prefix = format!("row{ordinal:02} ");
                let text = format!("{prefix}{}", "\u{1}".repeat(4096 - prefix.len()));
                json!({"event_id": format!("{ordinal:064x}"), "role":0, "ordinal":ordinal, "text":text})
            })
            .collect();
        assert_eq!(
            parse_tty("/history more 4"),
            Ok(Command::History {
                context: None,
                limit: 4,
                cursor: None,
                more: true
            })
        );

        for jsonl in [true, false] {
            // An offset stands in for the backend's opaque cursor. Only the
            // staged advance returned by prepare_result can replace it.
            let mut position = 1;
            let page = |from: usize, to: usize| {
                json!({
                    "context": "synthetic", "frontiers":[17,0], "finished":to == all_rows.len(),
                    "cursor": format!("cursor-{to}"), "items": &all_rows[from..to],
                })
            };
            let mut output = Vec::new();
            let (bytes, advance) = prepare_result(
                jsonl,
                Some("history"),
                None,
                Ok((page(position, all_rows.len()), Some(all_rows.len()))),
            )?;
            output.extend_from_slice(&bytes);
            assert_eq!(advance, None);
            assert_eq!(position, 1);
            assert_eq!(output.iter().filter(|&&b| b == b'\n').count(), 1);
            if jsonl {
                assert_eq!(
                    serde_json::from_slice::<Value>(&output)?,
                    json!({
                        "v":1,"type":"result","id":"history","ok":false,"code":"response_too_large",
                    })
                );
            } else {
                assert!(std::str::from_utf8(&output)?.contains("response_too_large"));
            }

            let mut seen = vec![0usize];
            while position < all_rows.len() {
                let end = (position + 4).min(all_rows.len());
                let (bytes, advance) = prepare_result(
                    jsonl,
                    Some("history"),
                    None,
                    Ok((page(position, end), Some(end))),
                )?;
                output.clear();
                output.extend_from_slice(&bytes);
                assert!(!output.contains(&1));
                if jsonl {
                    assert_eq!(output.iter().filter(|&&b| b == b'\n').count(), 1);
                    let response: Value = serde_json::from_slice(&output)?;
                    assert_eq!(response["id"], "history");
                    assert_eq!(response["ok"], true);
                    assert_eq!(response["value"]["items"], json!(&all_rows[position..end]));
                    for row in response["value"]["items"]
                        .as_array()
                        .ok_or_else(|| anyhow::anyhow!("missing rows"))?
                    {
                        seen.push(
                            row["ordinal"]
                                .as_u64()
                                .ok_or_else(|| anyhow::anyhow!("missing ordinal"))?
                                as usize,
                        );
                    }
                } else {
                    let display = std::str::from_utf8(&output)?;
                    let mut lines = display.lines();
                    for ordinal in position..end {
                        let line = lines.next().ok_or_else(|| anyhow::anyhow!("missing row"))?;
                        let expected = format!("0: row{ordinal:02} ");
                        assert!(line.starts_with(&expected));
                        assert_eq!(
                            line[expected.len()..].matches("\\u{1}").count(),
                            4096 - "row00 ".len()
                        );
                        seen.push(ordinal);
                    }
                    assert!(lines.next().is_some());
                    assert!(lines.next().is_none());
                }
                position =
                    advance.ok_or_else(|| anyhow::anyhow!("successful page lost its cursor"))?;
            }
            assert_eq!(seen, (0..all_rows.len()).collect::<Vec<_>>());
        }
        Ok(())
    }
}
