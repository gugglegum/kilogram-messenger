use crate::terminal_chat::{Frame, frame, valid_id};
use kilogram_bootstrap_contract::ORDERED2_RECOVERY_SCOPE;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, value::RawValue};
use std::io::{self, BufRead, Write};
use zeroize::Zeroizing;

pub(super) const HELP: &str = "/status /begin /open /create /recovery show /recovery saved /cancel /quit\nSave recovery words offline before confirming.";
const OUTPUT_CAP: usize = 256 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Command {
    Status,
    Begin,
    Open,
    Create,
    Show,
    Acknowledge,
    Cancel(Option<String>),
    Quit,
    Help,
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
        _ if serde_json::from_str::<Empty>(args).is_err() => Err("invalid_args"),
        "status" => Ok(Command::Status),
        "begin" => Ok(Command::Begin),
        "open" => Ok(Command::Open),
        "create" => Ok(Command::Create),
        "recovery_show" => Ok(Command::Show),
        "quit" => Ok(Command::Quit),
        _ => Err("unknown_operation"),
    }
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
        let tokens: Vec<_> = text.split_ascii_whitespace().collect();
        let command = match tokens.as_slice() {
            [] => continue,
            ["/status"] => Ok(Command::Status),
            ["/begin"] => Ok(Command::Begin),
            ["/open"] => Ok(Command::Open),
            ["/create"] => Ok(Command::Create),
            ["/recovery", "show"] => Ok(Command::Show),
            ["/recovery", "saved"] => Ok(Command::Acknowledge),
            ["/cancel"] => Ok(Command::Cancel(None)),
            ["/quit"] => Ok(Command::Quit),
            ["/help"] => Ok(Command::Help),
            _ => Err("invalid_command"),
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

pub(super) fn encode(
    jsonl: bool,
    id: Option<&str>,
    payload: Payload,
) -> io::Result<Zeroizing<Vec<u8>>> {
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
        serde_json::to_writer(&mut *bytes, &value)?;
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
                    match phase {
                        "recovery_unavailable" => bytes.write_all(
                            b"\nContinue only if you previously saved the words offline.",
                        )?,
                        "incomplete" => bytes.write_all(
                            b"\nKeep these files. Use another empty directory to start over.",
                        )?,
                        _ => {}
                    }
                } else {
                    serde_json::to_writer(&mut *bytes, &value)?;
                }
            }
            Payload::Error(code) | Payload::FrameError(code) => {
                write!(&mut *bytes, "Error: {code}.")?
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
