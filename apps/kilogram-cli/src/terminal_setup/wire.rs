use crate::terminal_chat::{Frame, frame, terminal_escaped, terminal_text, valid_id};
use kilogram_bootstrap_contract::ORDERED2_RECOVERY_SCOPE;
use kilogram_first_dialog::{
    artifact::Kind,
    consent::{self, Action},
    consent_artifact::Kind as ConsentKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, value::RawValue};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};
use zeroize::Zeroizing;

pub(super) const HELP: &str = "/status /begin /open /create /recovery show /recovery saved /cancel /quit\n/invite export|inspect <path>|join <preview_id>\n/reply export|inspect <path>|accept <preview_id>\n/consent propose|export|cosign|import\n/consent inspect <proposal|final> <path>\n/consent accept <proposal|final> <preview_id>\n/preview clear\nSave recovery words offline before confirming.";
const OUTPUT_CAP: usize = 256 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Command {
    Status,
    Begin,
    Open,
    Create,
    Show,
    Acknowledge,
    Export(Kind),
    Inspect(Kind, PathBuf),
    Confirm(Kind, String),
    ConsentInspect(ConsentKind, PathBuf),
    ConsentAccept(ConsentKind, String),
    ConsentExport,
    ConsentAction(Action),
    ClearPreview,
    Cancel(Option<String>),
    Quit,
    Help,
}

impl Command {
    pub fn cancellable(&self) -> bool {
        matches!(self, Self::Create | Self::ConsentAction(_))
    }
}

pub(super) struct Request {
    pub id: Option<String>,
    pub command: Result<Command, &'static str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest<'a> {
    v: u8,
    id: String,
    op: String,
    #[serde(borrow)]
    args: &'a RawValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryAck {
    saved: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancel {
    target_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspect {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Confirm {
    preview_id: String,
    confirmed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsentInspect {
    kind: ConsentKind,
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsentAccept {
    kind: ConsentKind,
    preview_id: String,
    confirmed: bool,
}

fn path(path: &str) -> Result<PathBuf, &'static str> {
    if path.is_empty() || path.contains('\0') {
        return Err("invalid_args");
    }
    Ok(PathBuf::from(path))
}

fn preview_id(id: String) -> Result<String, &'static str> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err("invalid_args");
    }
    Ok(id)
}

fn consent_kind(kind: &str) -> Result<ConsentKind, &'static str> {
    match kind {
        "proposal" => Ok(ConsentKind::Proposal),
        "final" => Ok(ConsentKind::Final),
        _ => Err("invalid_args"),
    }
}

fn parse(request: &WireRequest<'_>) -> Result<Command, &'static str> {
    if request.v != 1 {
        return Err("unsupported_version");
    }
    let args = request.args.get();
    match request.op.as_str() {
        "recovery_ack" => serde_json::from_str::<RecoveryAck>(args)
            .ok()
            .filter(|confirmation| confirmation.saved)
            .map(|_| Command::Acknowledge)
            .ok_or("invalid_args"),
        "cancel" => serde_json::from_str::<Cancel>(args)
            .ok()
            .filter(|cancel| valid_id(&cancel.target_id))
            .map(|cancel| Command::Cancel(Some(cancel.target_id)))
            .ok_or("invalid_args"),
        "invite_inspect" | "reply_inspect" => {
            let kind = if request.op == "invite_inspect" {
                Kind::Invite
            } else {
                Kind::Reply
            };
            let input: Inspect = serde_json::from_str(args).map_err(|_| "invalid_args")?;
            Ok(Command::Inspect(kind, path(&input.path)?))
        }
        "invite_join" | "reply_accept" => {
            let kind = if request.op == "invite_join" {
                Kind::Invite
            } else {
                Kind::Reply
            };
            let input: Confirm = serde_json::from_str(args).map_err(|_| "invalid_args")?;
            if !input.confirmed {
                return Err("invalid_args");
            }
            Ok(Command::Confirm(kind, preview_id(input.preview_id)?))
        }
        "consent_inspect" => {
            let input: ConsentInspect = serde_json::from_str(args).map_err(|_| "invalid_args")?;
            Ok(Command::ConsentInspect(input.kind, path(&input.path)?))
        }
        "consent_accept" => {
            let input: ConsentAccept = serde_json::from_str(args).map_err(|_| "invalid_args")?;
            if !input.confirmed {
                return Err("invalid_args");
            }
            Ok(Command::ConsentAccept(
                input.kind,
                preview_id(input.preview_id)?,
            ))
        }
        _ if serde_json::from_str::<Empty>(args).is_err() => Err("invalid_args"),
        "status" => Ok(Command::Status),
        "begin" => Ok(Command::Begin),
        "open" => Ok(Command::Open),
        "create" => Ok(Command::Create),
        "recovery_show" => Ok(Command::Show),
        "invite_export" => Ok(Command::Export(Kind::Invite)),
        "reply_export" => Ok(Command::Export(Kind::Reply)),
        "consent_propose" => Ok(Command::ConsentAction(Action::Propose)),
        "consent_cosign" => Ok(Command::ConsentAction(Action::Cosign)),
        "consent_import" => Ok(Command::ConsentAction(Action::Import)),
        "consent_export" => Ok(Command::ConsentExport),
        "preview_clear" => Ok(Command::ClearPreview),
        "quit" => Ok(Command::Quit),
        _ => Err("unknown_operation"),
    }
}

fn parse_tty(command: &str) -> Option<Result<Command, &'static str>> {
    let command = command.trim_ascii();
    if let Some((topic, tail)) = command.split_once(|c: char| c.is_ascii_whitespace())
        && let Some((action, args)) = split(tail)
        && action == "inspect"
    {
        match topic {
            "/invite" => return Some(path(args).map(|p| Command::Inspect(Kind::Invite, p))),
            "/reply" => return Some(path(args).map(|p| Command::Inspect(Kind::Reply, p))),
            "/consent" => {
                return Some((|| {
                    let (kind, literal_path) = split(args).ok_or("invalid_args")?;
                    Ok(Command::ConsentInspect(
                        consent_kind(kind)?,
                        path(literal_path)?,
                    ))
                })());
            }
            _ => {}
        }
    }
    let tokens: Vec<_> = command.split_ascii_whitespace().collect();
    Some(match tokens.as_slice() {
        [] => return None,
        ["/status"] => Ok(Command::Status),
        ["/begin"] => Ok(Command::Begin),
        ["/open"] => Ok(Command::Open),
        ["/create"] => Ok(Command::Create),
        ["/recovery", "show"] => Ok(Command::Show),
        ["/recovery", "saved"] => Ok(Command::Acknowledge),
        ["/invite", "export"] => Ok(Command::Export(Kind::Invite)),
        ["/reply", "export"] => Ok(Command::Export(Kind::Reply)),
        ["/invite", "join", id] => {
            preview_id((*id).into()).map(|id| Command::Confirm(Kind::Invite, id))
        }
        ["/reply", "accept", id] => {
            preview_id((*id).into()).map(|id| Command::Confirm(Kind::Reply, id))
        }
        ["/consent", "propose"] => Ok(Command::ConsentAction(Action::Propose)),
        ["/consent", "cosign"] => Ok(Command::ConsentAction(Action::Cosign)),
        ["/consent", "import"] => Ok(Command::ConsentAction(Action::Import)),
        ["/consent", "export"] => Ok(Command::ConsentExport),
        ["/consent", "accept", kind, id] => (|| {
            Ok(Command::ConsentAccept(
                consent_kind(kind)?,
                preview_id((*id).into())?,
            ))
        })(),
        ["/preview", "clear"] => Ok(Command::ClearPreview),
        ["/invite" | "/reply" | "/preview" | "/consent", ..] => Err("invalid_args"),
        ["/cancel"] => Ok(Command::Cancel(None)),
        ["/quit"] => Ok(Command::Quit),
        ["/help"] => Ok(Command::Help),
        _ => Err("invalid_command"),
    })
}

fn split(input: &str) -> Option<(&str, &str)> {
    let (head, tail) = input
        .trim_ascii_start()
        .split_once(|c: char| c.is_ascii_whitespace())?;
    Some((head, tail.trim_ascii_start()))
}

pub(super) fn read(
    input: &mut impl BufRead,
    jsonl: bool,
) -> io::Result<Option<Result<Request, &'static str>>> {
    loop {
        let Some(line) = frame(input)? else {
            return Ok(None);
        };
        let Frame::Line(bytes) = line else {
            return Ok(Some(Err("line_too_long")));
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return Ok(Some(Err("invalid_utf8")));
        };
        if jsonl {
            let Ok(request) = serde_json::from_str::<WireRequest>(text) else {
                return Ok(Some(Err("invalid_request")));
            };
            if !valid_id(&request.id) {
                return Ok(Some(Err("invalid_request")));
            }
            let command = parse(&request);
            return Ok(Some(Ok(Request {
                id: Some(request.id),
                command,
            })));
        }
        let Some(command) = parse_tty(text) else {
            continue;
        };
        return Ok(Some(Ok(Request { id: None, command })));
    }
}

pub(super) enum Payload {
    Public(Value),
    Secret(Zeroizing<String>),
    Error(&'static str),
    FrameError(&'static str),
    Ready,
    Help,
}

#[derive(Serialize)]
struct SecretReply<'a> {
    v: u8,
    #[serde(rename = "type")]
    kind: &'static str,
    id: &'a str,
    ok: bool,
    value: SecretValue<'a>,
}

#[derive(Serialize)]
struct SecretValue<'a> {
    recovery_words: &'a str,
    recovery_scope: &'static str,
}

fn identity(out: &mut impl Write, label: &str, pins: &Value) -> io::Result<()> {
    writeln!(
        out,
        "{label} account {}",
        pins["account_id"].as_str().unwrap_or_default()
    )?;
    write!(
        out,
        "{label} device {}",
        pins["device_id"].as_str().unwrap_or_default()
    )
}

fn contexts(out: &mut impl Write, value: &Value) -> io::Result<()> {
    write!(
        out,
        "\nContext 1 {}\nContext 2 {}",
        value[0].as_str().unwrap_or_default(),
        value[1].as_str().unwrap_or_default()
    )
}

fn artifact_tty(out: &mut impl Write, value: &Value, kind: &str) -> io::Result<()> {
    if let Some(path) = value["path"].as_str() {
        writeln!(out, "Exported {kind}: {}", terminal_text(path))?;
    } else {
        writeln!(
            out,
            "Preview {kind} {}",
            value["preview_id"].as_str().unwrap_or_default()
        )?;
    }
    identity(out, "Sender", &value["sender"])?;
    if !value["recipient"].is_null() {
        writeln!(out)?;
        identity(out, "Recipient", &value["recipient"])?;
    }
    write!(
        out,
        "\nInvitation {}",
        value["invitation_id"].as_str().unwrap_or_default()
    )?;
    contexts(out, &value["contexts"])?;
    if let Some(digest) = value["artifact_digest"].as_str() {
        write!(
            out,
            "\nValidity {} to {} (Unix seconds)\nArtifact digest {digest}",
            value["not_before"], value["expires_at"]
        )?;
        if value.get("preview_id").is_some() {
            write!(
                out,
                "\nBoth signed payloads verified against the selected peer and contexts. Accept this preview to keep the document."
            )?;
        }
    } else if value.get("preview_id").is_some() {
        write!(
            out,
            "\nCredential and authority signatures verified. The envelope and contexts are unsigned.\n\
                     The contact's identity still requires your verification.\n\
                     Compare account, device, invitation and ordered contexts with your contact over an independent channel before confirming.\n\
                     An authenticated connection is not established yet."
        )?;
    }
    Ok(())
}

fn json_public(out: &mut impl Write, value: &Value) -> io::Result<()> {
    let line = serde_json::to_string(value)?;
    // serde already escapes JSON control syntax. Escape remaining display
    // controls inside strings without changing machine-readable path values.
    let mut start = 0;
    for (index, c) in line.char_indices().filter(|(_, c)| terminal_escaped(*c)) {
        out.write_all(&line.as_bytes()[start..index])?;
        write!(out, "\\u{:04x}", u32::from(c))?;
        start = index + c.len_utf8();
    }
    out.write_all(&line.as_bytes()[start..])
}

pub(super) fn encode(
    jsonl: bool,
    id: Option<&str>,
    payload: Payload,
) -> io::Result<Zeroizing<Vec<u8>>> {
    encode_at(jsonl, id, payload, consent::now().ok())
}

pub(super) fn encode_at(
    jsonl: bool,
    id: Option<&str>,
    mut payload: Payload,
    wall: Option<u64>,
) -> io::Result<Zeroizing<Vec<u8>>> {
    if let Payload::Public(value) = &mut payload
        && let Some(view) = value.get_mut("consent")
    {
        let period = view["not_before"].as_u64().zip(view["expires_at"].as_u64());
        let current = period
            .zip(wall)
            .is_some_and(|((start, end), wall)| consent::valid_interval(start, end, wall).is_ok());
        view["validity"] = json!(match (period, current) {
            (None, _) => "unset",
            (Some(_), true) => "current",
            (Some(_), false) => "not_current",
        });
        if !current && view["local_installation"] == "confirmed_current_session" {
            view["local_installation"] = json!("previously_accepted");
        }
    }
    // Fixed capacity for the sensitive response prevents reallocation of any
    // serialized phrase. The only serializer input is a borrowed str.
    let mut bytes = Zeroizing::new(Vec::with_capacity(8192));
    if let Payload::Secret(phrase) = payload {
        if phrase.len() > 1024 || !phrase.bytes().all(|b| b.is_ascii_lowercase() || b == b' ') {
            return Err(io::Error::other("invalid recovery output"));
        }
        if jsonl {
            serde_json::to_writer(
                &mut *bytes,
                &SecretReply {
                    v: 1,
                    kind: "result",
                    id: id.ok_or_else(|| io::Error::other("missing id"))?,
                    ok: true,
                    value: SecretValue {
                        recovery_words: phrase.as_str(),
                        recovery_scope: ORDERED2_RECOVERY_SCOPE,
                    },
                },
            )?;
        } else {
            bytes.write_all(phrase.as_bytes())?;
        }
    } else if jsonl {
        let value = match payload {
            Payload::Ready => json!({"v":1,"type":"ready"}),
            Payload::Public(value) => {
                json!({"v":1,"type":"result","id":id,"ok":true,"value":value})
            }
            Payload::Error(code) => json!({"v":1,"type":"result","id":id,"ok":false,"code":code}),
            Payload::FrameError(code) => json!({"v":1,"type":"frame_error","code":code}),
            _ => return Err(io::Error::other("invalid JSONL output")),
        };
        json_public(&mut *bytes, &value)?;
    } else {
        match payload {
            Payload::Ready => bytes.write_all(b"First-dialog setup. Use /help.")?,
            Payload::Help => bytes.write_all(HELP.as_bytes())?,
            Payload::Public(value) => {
                if let Some(phase) = value["state"].as_str() {
                    write!(&mut *bytes, "Setup {phase}")?;
                    if let (Some(account), Some(device)) =
                        (value["account_id"].as_str(), value["device_id"].as_str())
                    {
                        write!(&mut *bytes, "\nAccount {account}\nDevice {device}")?;
                    }
                    if let Some(peer) = value.get("peer_selection") {
                        writeln!(
                            &mut *bytes,
                            "\nPairing {}",
                            peer["role"].as_str().unwrap_or_default()
                        )?;
                        identity(&mut *bytes, "Peer", &peer["peer"])?;
                        write!(
                            &mut *bytes,
                            "\nInvitation {}",
                            peer["invitation_id"].as_str().unwrap_or_default()
                        )?;
                        contexts(&mut *bytes, &value["contexts"])?;
                    }
                    if let Some(view) = value.get("consent") {
                        consent_tty(&mut *bytes, view)?;
                    }
                    match phase {
                        "recovery_unavailable" => bytes.write_all(
                            b"\nContinue only if you previously saved the words offline.",
                        )?,
                        "incomplete" => bytes.write_all(
                            b"\nKeep these files. Use another empty directory to start over.",
                        )?,
                        _ => {}
                    }
                } else if let Some(kind) = value["kind"].as_str() {
                    artifact_tty(&mut *bytes, &value, kind)?;
                } else {
                    serde_json::to_writer(&mut *bytes, &value)?;
                }
            }
            Payload::Error(code) | Payload::FrameError(code) => {
                write!(&mut *bytes, "Error: {code}.")?;
                if code == "consent_import_failed" {
                    write!(
                        &mut *bytes,
                        " If the messaging runtime holds this account, close it and retry the same finals."
                    )?;
                }
            }
            Payload::Secret(_) => unreachable!(),
        }
    }
    bytes.push(b'\n');
    if bytes.len() > OUTPUT_CAP {
        return Err(io::Error::other("output bound"));
    }
    Ok(bytes)
}

fn consent_tty(out: &mut impl Write, view: &Value) -> io::Result<()> {
    let name = |field: &str| view[field].as_str().unwrap_or_default();
    write!(
        out,
        "\nConsent {} {} {}",
        name("role"),
        name("stage"),
        name("validity")
    )?;
    if let (Some(start), Some(end)) = (view["not_before"].as_u64(), view["expires_at"].as_u64()) {
        write!(out, "\nValidity {start} to {end} (Unix seconds)")?;
    }
    for (label, field) in [
        ("Selected proposal", "selected_proposal_digest"),
        ("Final", "final_digest"),
    ] {
        if let Some(digest) = view[field].as_str() {
            write!(out, "\n{label} digest {digest}")?;
        }
    }
    for (slot, value) in view["slots"].as_array().into_iter().flatten().enumerate() {
        let name = |field: &str| value[field].as_str().unwrap_or_default();
        write!(
            out,
            "\nConsent slot {} {} signing={} importing={}",
            slot + 1,
            name("context_id"),
            name("signing"),
            name("importing")
        )?;
    }
    write!(out, "\nLocal installation {}", name("local_installation"))
}
