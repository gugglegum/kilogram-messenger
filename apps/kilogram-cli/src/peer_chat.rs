//! One-invitation product path; existing signed consent, runtime owner and durable
//! send/history APIs remain authoritative. The control channel carries PUBLIC
//! setup/locator artifacts, never chat plaintext, account keys or recovery words.
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode, SecretKey, endpoint::presets};
use kilogram_first_dialog::{
    artifact::{self, Artifact, Kind},
    connection, consent,
    consent_artifact::Envelope,
    helper,
    model::{self, Setup},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::time::timeout;
use zeroize::Zeroizing;
#[path = "peer_chat/reader.rs"]
mod reader;
#[path = "peer_chat/terminal.rs"]
mod terminal;
const ALPN: &[u8] = b"kilogram/peer-setup/1";
const CAP: usize = 1024 * 1024;
fn cli_exe() -> Result<PathBuf> {
    Ok(std::env::current_exe()?.with_file_name("kilogram-cli.exe"))
}
fn terminal_text(text: &str) -> String {
    text.chars().flat_map(|c|{
        if c.is_control() || matches!(c,'\u{061c}'|'\u{200e}'|'\u{200f}'|'\u{2028}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
            c.escape_default().collect::<Vec<_>>()
        }else{vec![c]}
    }).collect()
}

#[derive(Clone)]
struct Ui {
    jsonl: bool,
    input: Arc<Mutex<mpsc::Receiver<String>>>,
    stop: Arc<AtomicBool>,
}
#[derive(Debug)]
struct UserClosed;
impl std::fmt::Display for UserClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Closed by user")
    }
}
impl std::error::Error for UserClosed {}
fn user_result(result: Result<()>) -> Result<()> {
    match result {
        Err(error)
            if error.is::<UserClosed>()
                || error.downcast_ref::<helper::HelperError>()
                    == Some(&helper::HelperError::Cancelled) =>
        {
            Ok(())
        }
        other => other,
    }
}
impl Ui {
    fn new(jsonl: bool) -> Self {
        let (tx, rx) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let input_stop = stop.clone();
        std::thread::spawn(move || {
            let stdin = io::stdin();
            let mut input = stdin.lock();
            loop {
                let mut line = String::new();
                match input.by_ref().take(32 * 1024 + 1).read_line(&mut line) {
                    Ok(0) | Err(_) => {
                        input_stop.store(true, Ordering::Release);
                        break;
                    }
                    Ok(_) if line.len() > 32 * 1024 => {
                        input_stop.store(true, Ordering::Release);
                        break;
                    }
                    Ok(_) => {
                        if line.trim_end_matches(['\r', '\n']) == "/quit" {
                            input_stop.store(true, Ordering::Release);
                            break;
                        }
                        if tx
                            .send(line.trim_end_matches(['\r', '\n']).to_owned())
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        Self {
            jsonl,
            input: Arc::new(Mutex::new(rx)),
            stop,
        }
    }
    fn emit(&self, kind: &str, text: &str, value: Value) {
        self.emitted(kind, text, value);
    }
    fn emitted(&self, kind: &str, text: &str, value: Value) -> bool {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let result = if self.jsonl {
            writeln!(out, "{}", json!({"type":kind,"text":text,"value":value}))
        } else {
            writeln!(out, "{}", terminal_text(text))
        };
        let written = result.and_then(|()| out.flush()).is_ok();
        if !written {
            self.stop.store(true, Ordering::Release);
        }
        written
    }
    fn confirm(&self, text: &str, command: &str) -> Result<()> {
        let input = self
            .input
            .lock()
            .map_err(|_| anyhow::anyhow!("Input unavailable"))?;
        self.check_open()?;
        self.emit(
            "confirm",
            &format!("{text} Введите {command} для подтверждения или /quit."),
            json!({"command":command}),
        );
        loop {
            self.check_open()?;
            match input.recv_timeout(Duration::from_millis(100)) {
                Ok(line) if line == command => {
                    self.check_open()?;
                    return Ok(());
                }
                Ok(line) if line == "/quit" => {
                    self.stop.store(true, Ordering::Release);
                    bail!(UserClosed)
                }
                Ok(_) => self.emit("notice", "Подтверждение не принято.", Value::Null),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => {
                    self.stop.store(true, Ordering::Release);
                    bail!(UserClosed)
                }
            }
        }
    }
    fn check_open(&self) -> Result<()> {
        if self.stop.load(Ordering::Acquire) {
            bail!(UserClosed)
        }
        Ok(())
    }
    async fn network<T>(&self, future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        tokio::select! {
            result=future=>result,
            _=async {while !self.stop.load(Ordering::Acquire){tokio::time::sleep(Duration::from_millis(50)).await;}}=>bail!(UserClosed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usable_peer_invitation_is_bounded_and_exact() -> Result<()> {
        let invitation = Invitation {
            version: 1,
            address: EndpointAddr::new(SecretKey::from_bytes(&[7; 32]).public()),
            token: "aa".repeat(32),
            digest: "bb".repeat(32),
            relay: "https://relay.example/".into(),
        };
        let encoded = invitation.encode()?;
        let decoded = Invitation::decode(&encoded)?;
        ensure!(
            decoded.address == invitation.address
                && decoded.token == invitation.token
                && decoded.digest == invitation.digest
        );
        for bad in [
            "not-an-invitation".to_owned(),
            encoded.clone() + " ",
            "x".repeat(16385),
        ] {
            ensure!(Invitation::decode(&bad).is_err());
        }
        for field in ["version", "token", "relay", "unknown"] {
            let mut value = serde_json::to_value(&invitation)?;
            match field {
                "version" => value[field] = 2.into(),
                "token" => value[field] = "0".into(),
                "relay" => value[field] = "http://127.0.0.1/".into(),
                _ => value[field] = true.into(),
            }
            let raw = format!(
                "kilogram://peer/{}",
                URL_SAFE_NO_PAD.encode(serde_json::to_vec(&value)?)
            );
            ensure!(Invitation::decode(&raw).is_err(), "accepted {field}");
        }
        Ok(())
    }
    #[test]
    fn usable_peer_capability_and_cancellation() -> Result<()> {
        ensure!(same_token(&"a".repeat(64), &"a".repeat(64)));
        for bad in [
            "a".repeat(63),
            "b".repeat(64),
            format!("{}b", "a".repeat(63)),
        ] {
            ensure!(!same_token(&"a".repeat(64), &bad));
        }
        let (tx, rx) = mpsc::channel();
        tx.send("/quit".to_owned())?;
        let ui = Ui {
            jsonl: true,
            input: Arc::new(Mutex::new(rx)),
            stop: Arc::new(AtomicBool::new(false)),
        };
        ensure!(
            ui.confirm("Fixture consent", "/accept").is_err() && ui.stop.load(Ordering::Acquire)
        );
        Ok(())
    }
    #[test]
    fn usable_peer_confirmation_eof_and_stop_refuse_late_acceptance() -> Result<()> {
        for stopped in [false, true] {
            let (tx, rx) = mpsc::channel();
            if stopped {
                tx.send("/accept".into())?;
            }
            drop(tx);
            let ui = Ui {
                jsonl: true,
                input: Arc::new(Mutex::new(rx)),
                stop: Arc::new(AtomicBool::new(stopped)),
            };
            ensure!(ui.confirm("Fixture waiting", "/accept").is_err());
            ensure!(ui.stop.load(Ordering::Acquire));
        }
        Ok(())
    }
    #[tokio::test]
    async fn usable_peer_user_close_cancels_pending_network_without_late_action() -> Result<()> {
        let (_tx, rx) = mpsc::channel();
        let ui = Ui {
            jsonl: true,
            input: Arc::new(Mutex::new(rx)),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let stop = ui.stop.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            stop.store(true, Ordering::Release);
        });
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            ui.network(std::future::pending::<Result<()>>()),
        )
        .await?;
        ensure!(result.as_ref().err().is_some_and(|e| e.is::<UserClosed>()));
        user_result(result)?;
        ensure!(user_result(Err(anyhow::anyhow!("real failure"))).is_err());
        ensure!(user_result(Err(helper::HelperError::CleanupUnconfirmed.into())).is_err());
        Ok(())
    }
    #[cfg(windows)]
    #[test]
    fn usable_peer_transport_secret_uses_current_user_dpapi() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let state = NetworkState {
            version: 2,
            initiator: false,
            key: "12".repeat(32),
            token: "34".repeat(32),
            relay: "https://relay.example/".into(),
            peer: None,
            peer_address: None,
        };
        state.save(temp.path())?;
        let raw = std::fs::read(temp.path().join("peer-network.sealed"))?;
        ensure!(
            !raw.windows(32)
                .any(|s| s == state.key.as_bytes().get(..32).unwrap_or_default())
        );
        let opened = NetworkState::load(temp.path())?;
        ensure!(opened.key == state.key && opened.token == state.token);
        let mut bad = raw;
        bad[0] ^= 1;
        std::fs::write(temp.path().join("peer-network.sealed"), bad)?;
        ensure!(NetworkState::load(temp.path()).is_err());
        Ok(())
    }
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut out = tempfile::NamedTempFile::new_in(path.parent().context("Profile parent")?)?;
    out.write_all(bytes)?;
    out.as_file().sync_all()?;
    out.persist(path)?;
    Ok(())
}
fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
fn check_address(address: &EndpointAddr, relay: &str) -> Result<()> {
    ensure!(address.addrs.len() <= 16, "Peer address cap");
    for transport in &address.addrs {
        match transport {
            iroh::TransportAddr::Relay(url) => ensure!(
                url.as_str() == relay,
                "Peer relay differs from the explicitly selected relay"
            ),
            iroh::TransportAddr::Ip(ip) => ensure!(
                ip.port() != 0 && !ip.ip().is_unspecified() && !ip.ip().is_multicast(),
                "Invalid peer IP address"
            ),
            _ => bail!("Unsupported peer transport"),
        }
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkState {
    version: u8,
    initiator: bool,
    key: String,
    token: String,
    relay: String,
    peer: Option<String>,
    peer_address: Option<EndpointAddr>,
}
impl Drop for NetworkState {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key.zeroize();
        self.token.zeroize();
    }
}
impl NetworkState {
    fn save(&self, home: &Path) -> Result<()> {
        let raw = Zeroizing::new(serde_json::to_vec(self)?);
        #[cfg(windows)]
        let bytes = stellar_agent_windows_identity::dpapi_protect(&raw)?;
        #[cfg(not(windows))]
        let bytes = raw.to_vec();
        atomic(&home.join("peer-network.sealed"), &bytes)
    }
    fn load(home: &Path) -> Result<Self> {
        let bytes = model::read(&home.join("peer-network.sealed"), 16 * 1024)?;
        #[cfg(windows)]
        let raw = Zeroizing::new(stellar_agent_windows_identity::dpapi_unprotect(&bytes)?);
        #[cfg(not(windows))]
        let raw = Zeroizing::new(bytes);
        let state: Self = serde_json::from_slice(&raw)?;
        ensure!(
            state.version == 2,
            "Unsupported peer network state; preserve the profile"
        );
        kilogram_runtime_ipc::ordered2::id(&state.key)?;
        kilogram_runtime_ipc::ordered2::id(&state.token)?;
        if let Some(peer) = &state.peer {
            let _: EndpointId = peer.parse()?;
        }
        if let Some(address) = &state.peer_address {
            check_address(address, &state.relay)?;
        }
        ensure!(
            state
                .peer_address
                .as_ref()
                .is_none_or(|a| state.peer.as_deref() == Some(a.id.to_string().as_str())
                    && a.addrs.len() <= 16),
            "Saved peer address binding"
        );
        kilogram_protocol::ordered_dialog_v2::canonical_runtime_relay(&state.relay)?;
        Ok(state)
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invitation {
    version: u8,
    address: EndpointAddr,
    token: String,
    digest: String,
    relay: String,
}
impl Invitation {
    fn decode(text: &str) -> Result<Self> {
        ensure!(text.len() < 16 * 1024, "Invitation too large");
        let raw = URL_SAFE_NO_PAD.decode(
            text.strip_prefix("kilogram://peer/")
                .context("Expected kilogram invitation")?,
        )?;
        let invite: Self = serde_json::from_slice(&raw)?;
        ensure!(
            invite.version == 1 && invite.address.addrs.len() <= 16,
            "Invitation version/addresses"
        );
        kilogram_runtime_ipc::ordered2::id(&invite.token)?;
        kilogram_runtime_ipc::ordered2::id(&invite.digest)?;
        kilogram_protocol::ordered_dialog_v2::canonical_runtime_relay(&invite.relay)?;
        ensure!(
            invite.relay.starts_with("https://"),
            "Invitation relay requires HTTPS"
        );
        check_address(&invite.address, &invite.relay)?;
        Ok(invite)
    }
    fn encode(&self) -> Result<String> {
        Ok(format!(
            "kilogram://peer/{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(self)?)
        ))
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Public {
    artifact: Artifact,
    consent: Option<Envelope>,
    locators: Option<String>,
    address: Option<EndpointAddr>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    token: String,
    public: Option<Public>,
}
struct Local {
    _ownership: model::SetupOwnership,
    home: PathBuf,
    setup: Option<Setup>,
    state: NetworkState,
    public: Public,
    ui: Ui,
    ready: bool,
    chat_started: bool,
    incoming: Option<String>,
    peer_identity: Option<kilogram_first_dialog::artifact::Identity>,
    approval: Option<(EndpointId, Artifact)>,
}
fn create_account(
    home: &Path,
    selected: Option<Setup>,
    invite: Option<Artifact>,
    ui: &Ui,
) -> Result<Setup> {
    ui.check_open()?;
    let mut setup = if let Some(setup) = selected {
        Ok(setup)
    } else if let Some(invite) = invite {
        Setup::join_terminal(home, invite)
    } else {
        Setup::begin_terminal(home)
    }
    .map_err(|e| anyhow::anyhow!("Account unavailable: {e:?}; preserve existing profile"))?;
    if setup.incomplete {
        ui.emit(
            "notice",
            "Создаётся защищённый локальный аккаунт…",
            Value::Null,
        );
        let generation = setup.record.local_id.clone();
        let output = helper::run(
            &std::env::current_exe()?.with_file_name("kilogram-bootstrap.exe"),
            &setup.record.workspace,
            &setup.record.contexts,
            ui.stop.clone(),
        )?;
        setup.accept(output, &generation)?;
    }
    if !setup.record.acknowledged {
        if let Some(words) = &setup.phrase {
            ui.emit("recovery", words.as_str(), Value::Null);
        }
        ui.confirm(
            "Сохраните recovery words в безопасном месте. Они нужны для восстановления аккаунта.",
            "/saved",
        )?;
        ui.check_open()?;
        setup.acknowledge(true)?;
    }
    Ok(setup)
}
impl Local {
    fn request_approval(&mut self, peer: EndpointId, artifact: &Artifact) -> Result<()> {
        if let Some((selected, candidate)) = &self.approval {
            ensure!(
                *selected == peer && candidate == artifact,
                "Another peer confirmation is pending"
            );
        } else {
            self.approval = Some((peer, artifact.clone()));
        }
        Ok(())
    }
    fn approve(&mut self, peer: EndpointId, candidate: &Artifact) -> Result<()> {
        ensure!(!self.ui.stop.load(Ordering::Acquire), "Closing");
        ensure!(
            self.approval
                .as_ref()
                .is_some_and(|(p, a)| *p == peer && a == candidate),
            "Peer confirmation changed"
        );
        if !self.ready && !self.chat_started {
            let setup = self.setup.as_mut().context("Account setup unavailable")?;
            if !setup.record.joined {
                setup.select_reply(candidate.clone(), true)?;
            }
        }
        self.state.peer = Some(peer.to_string());
        self.state.save(&self.home)?;
        self.approval = None;
        Ok(())
    }
    fn update(&mut self, peer: EndpointId, public: Public) -> Result<()> {
        ensure!(!self.ui.stop.load(Ordering::Acquire), "Closing");
        ensure!(
            self.state
                .peer
                .as_deref()
                .is_none_or(|id| id == peer.to_string()),
            "Unexpected peer endpoint"
        );
        if let Some(address) = &public.address {
            ensure!(
                address.id == peer && address.addrs.len() <= 16,
                "Peer transport address binding"
            );
            check_address(address, &self.state.relay)?;
        }
        if self.chat_started || self.ready {
            ensure!(
                self.public.artifact.invitation == public.artifact.invitation
                    && self.public.artifact.contexts == public.artifact.contexts
                    && self.peer_identity.as_ref() == Some(&public.artifact.sender),
                "Peer identity/context changed"
            );
            if self.state.peer.is_none() {
                self.request_approval(peer, &public.artifact)?;
                return Ok(());
            }
            if public.address.is_some() && self.state.peer_address != public.address {
                self.state.peer_address = public.address.clone();
                self.state.save(&self.home)?;
            }
            self.incoming = public.locators;
            return Ok(());
        }
        let setup = self.setup.as_mut().context("Account setup unavailable")?;
        public.artifact.validate()?;
        if !setup.record.joined {
            setup.check_reply(&public.artifact)?;
            if self.state.peer.is_none() {
                self.request_approval(peer, &public.artifact)?;
                return Ok(());
            }
        } else {
            ensure!(
                public.artifact.kind == Kind::Invite
                    && public.artifact.invitation == setup.record.invitation
                    && Some(&public.artifact.sender) == setup.record.peer.as_ref(),
                "Invitation identity changed"
            );
        }
        if self.state.peer.is_none() {
            self.state.peer = Some(peer.to_string());
            self.state.save(&self.home)?;
        }
        self.peer_identity = setup.record.peer.clone();
        if public.address.is_some() && self.state.peer_address != public.address {
            self.state.peer_address = public.address.clone();
            self.state.save(&self.home)?;
        }
        if let Some(envelope) = public.consent {
            consent::preview(setup, &envelope)?;
            consent::select(setup, &envelope)?;
        }
        if consent::role(setup)? == 0
            && setup
                .record
                .consent
                .as_ref()
                .is_none_or(|s| s.slots.iter().any(|s| s.output.is_none()))
        {
            consent::execute(
                setup,
                &cli_exe()?,
                consent::Action::Propose,
                self.ui.stop.clone(),
            )?;
        }
        if consent::role(setup)? == 1
            && setup
                .record
                .consent
                .as_ref()
                .is_some_and(|s| s.proposal.is_some() && s.final_artifact.is_none())
        {
            consent::execute(
                setup,
                &cli_exe()?,
                consent::Action::Cosign,
                self.ui.stop.clone(),
            )?;
        }
        if setup
            .record
            .consent
            .as_ref()
            .is_some_and(|s| s.final_artifact.is_some())
        {
            consent::execute(
                setup,
                &cli_exe()?,
                consent::Action::Import,
                self.ui.stop.clone(),
            )?;
            self.ready = true;
        }
        let envelope = if let Some(reference) = setup
            .record
            .consent
            .as_ref()
            .and_then(|s| s.final_artifact.as_ref())
        {
            Some(Envelope::decode(&model::read(
                &setup.home.join("consent").join(&reference.name),
                artifact::FILE_CAP,
            )?)?)
        } else if setup.record.consent.as_ref().is_some_and(|s| {
            consent::role(setup).ok() == Some(0) && s.slots.iter().all(|s| s.output.is_some())
        }) {
            Some(Envelope::decode(&model::read(
                &consent::export(setup)?,
                artifact::FILE_CAP,
            )?)?)
        } else {
            None
        };
        self.public.consent = envelope;
        self.incoming = public.locators;
        Ok(())
    }
}
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |v, (a, b)| v | (a ^ b)) == 0
}
async fn receive(stream: &mut iroh::endpoint::RecvStream) -> Result<Vec<u8>> {
    Ok(timeout(Duration::from_secs(120), stream.read_to_end(CAP)).await??)
}
async fn respond(stream: &mut iroh::endpoint::SendStream, value: &Public) -> Result<()> {
    let raw = serde_json::to_vec(value)?;
    ensure!(raw.len() <= CAP, "Public exchange bound");
    stream.write_all(&raw).await?;
    stream.finish()?;
    Ok(())
}
async fn serve(endpoint: Endpoint, local: Arc<Mutex<Local>>, ui: Ui) {
    // A stalled/failed TLS attempt must not head-of-line block its successful
    // alternative path. At most two bounded exchanges; Local still serializes
    // account changes and admits only the selected peer/capability.
    let mut active = tokio::task::JoinSet::new();
    let last_notice = Arc::new(Mutex::new(None::<std::time::Instant>));
    while !ui.stop.load(Ordering::Acquire) {
        let available = active.len() < 2;
        let incoming = tokio::select! {v=endpoint.accept(),if available=>v,_=active.join_next(),if !active.is_empty()=>continue,_=tokio::time::sleep(Duration::from_millis(100))=>continue};
        let Some(incoming) = incoming else { break };
        let endpoint = endpoint.clone();
        let local = local.clone();
        let ui = ui.clone();
        let last_notice = last_notice.clone();
        active.spawn(async move {
            let operation = async {
                let connection = timeout(Duration::from_secs(10), incoming).await??;
                let peer = connection.remote_id();
                let (mut send, mut recv) =
                    timeout(Duration::from_secs(10), connection.accept_bi()).await??;
                let raw = receive(&mut recv).await?;
                let request: Request = serde_json::from_slice(&raw)?;
                let address = endpoint.addr();
                let response = tokio::task::spawn_blocking({
                    let local = local.clone();
                    move || -> Result<Public> {
                        let mut local = local
                            .try_lock()
                            .map_err(|_| anyhow::anyhow!("Peer state unavailable"))?;
                        ensure!(!local.ui.stop.load(Ordering::Acquire), "Closing");
                        ensure!(
                            request.version == 1 && same_token(&request.token, &local.state.token),
                            "Invalid invitation capability"
                        );
                        local.public.address = Some(address);
                        if let Some(public) = request.public {
                            local.update(peer, public)?;
                        } else {
                            ensure!(
                                local
                                    .state
                                    .peer
                                    .as_deref()
                                    .is_none_or(|id| id == peer.to_string()),
                                "Invitation already used"
                            );
                        }
                        Ok(local.public.clone())
                    }
                })
                .await??;
                timeout(Duration::from_secs(10), respond(&mut send, &response)).await??;
                let _ = timeout(Duration::from_secs(5), send.stopped()).await;
                connection.close(0u32.into(), b"exchange-complete");
                Ok::<_, anyhow::Error>(())
            };
            // Human confirmation is owned by the UI, never by this request.
            // Transport awaits have their own bounds; accepted account work
            // is joined, not detached by a timeout around spawn_blocking.
            if let Err(error) = operation.await {
                if std::env::var("KILOGRAM_TEST_OD2_TIMINGS").is_ok_and(|s| s == "1") {
                    eprintln!("peer_control_attempt_failed {error}");
                }
                let notify = last_notice.lock().is_ok_and(|mut last| {
                    if last.is_none_or(|at| at.elapsed() > Duration::from_secs(30)) {
                        *last = Some(std::time::Instant::now());
                        true
                    } else {
                        false
                    }
                });
                if notify && !ui.stop.load(Ordering::Acquire) {
                    ui.emit(
                        "network_notice",
                        "Подключение ещё не завершено; повторяю автоматически. /quit — выход.",
                        Value::Null,
                    );
                }
            }
        });
    }
}
async fn exchange(
    endpoint: &Endpoint,
    address: EndpointAddr,
    token: String,
    public: Option<Public>,
) -> Result<Public> {
    let connection = timeout(Duration::from_secs(15), endpoint.connect(address, ALPN)).await??;
    let (mut send, mut recv) = timeout(Duration::from_secs(10), connection.open_bi()).await??;
    let raw = serde_json::to_vec(&Request {
        version: 1,
        token,
        public,
    })?;
    ensure!(raw.len() <= CAP, "Public request bound");
    timeout(Duration::from_secs(10), send.write_all(&raw)).await??;
    send.finish()?;
    let reply: Public = serde_json::from_slice(&receive(&mut recv).await?)?;
    // The stream's EOF is required before interpreting the bounded response.
    Ok(reply)
}

pub async fn run(
    home: PathBuf,
    join: Option<String>,
    relay: Option<String>,
    jsonl: bool,
) -> Result<()> {
    #[cfg(windows)]
    let job = {
        let mut info = win32job::ExtendedLimitInfo::new();
        info.limit_kill_on_job_close();
        let job = win32job::Job::create_with_limit_info(&info)?;
        job.assign_current_process()?;
        job
    };
    let result = run_inner(home, join, relay, jsonl).await;
    #[cfg(windows)]
    {
        let until = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let children: Vec<_> = job
                .query_process_id_list()?
                .into_iter()
                .filter(|pid| *pid != std::process::id() as usize)
                .collect();
            if children.is_empty() {
                break;
            }
            if std::time::Instant::now() >= until {
                eprintln!(
                    "peer_cleanup_unconfirmed owned_descendants={children:?}; own Job closes with process"
                );
                bail!("Owned peer descendants remain; normal cleanup was not confirmed");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        job.set_extended_limit_info(&win32job::ExtendedLimitInfo::new())?;
    }
    user_result(result)
}
async fn run_inner(
    home: PathBuf,
    join: Option<String>,
    relay: Option<String>,
    jsonl: bool,
) -> Result<()> {
    let ui = Ui::new(jsonl);
    ensure!(
        cfg!(windows),
        "This account implementation currently requires ordinary Windows user context"
    );
    let interrupt_ui = ui.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            interrupt_ui.stop.store(true, Ordering::Release);
        }
    });
    let home = if home.is_absolute() {
        home
    } else {
        std::env::current_dir()?.join(home)
    };
    let existing = home.join("setup.json").exists();
    let selected = if existing {
        // Refuse locked/reparse/conflicting personal profiles before opening a
        // technical endpoint or reading its separate protected network state.
        let selected = Setup::open_terminal(&home).map_err(|e| {
            anyhow::anyhow!("Close the existing account window first; profile preserved: {e:?}")
        })?;
        Some(selected)
    } else {
        None
    };
    // Keep the same file object locked throughout existing-profile network/UI
    // work, and beyond a failed/closed chat-owner handoff too.
    let mut ownership = selected.as_ref().map(Setup::retain_ownership);
    let existing_record = selected.as_ref().map(|s| &s.record);
    let saved_network = home.join("peer-network.sealed").exists();
    let join = if (!existing || !saved_network) && join.is_none() {
        ui.emit("welcome",if existing {"Существующий аккаунт сохранён. Enter — создать ссылку для прежнего собеседника; или вставьте его приглашение."}else{"Kilogram. Нажмите Enter, чтобы создать приглашение, или вставьте приглашение собеседника."},Value::Null);
        let line = {
            let input = ui.input.lock().map_err(|_| anyhow::anyhow!("Input"))?;
            loop {
                if ui.stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                match input.recv_timeout(Duration::from_millis(100)) {
                    Ok(line) => break line,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => return Ok(()),
                }
            }
        };
        if line == "/quit" {
            return Ok(());
        } else if line.is_empty() {
            None
        } else {
            Some(line)
        }
    } else {
        join
    };
    let invitation = join.as_deref().map(Invitation::decode).transpose()?;
    let mut state = if saved_network {
        NetworkState::load(&home)?
    } else {
        NetworkState {
            version: 2,
            initiator: invitation.is_some(),
            key: artifact::id()?,
            token: invitation
                .as_ref()
                .map_or_else(artifact::id, |i| Ok(i.token.clone()))?,
            relay: relay
                .clone()
                .or_else(|| invitation.as_ref().map(|i| i.relay.clone()))
                .unwrap_or_else(|| format!("https://{}/", iroh::defaults::prod::EU_RELAY_HOSTNAME)),
            peer: invitation.as_ref().map(|i| i.address.id.to_string()),
            peer_address: invitation.as_ref().map(|i| i.address.clone()),
        }
    };
    if let Some(invite) = &invitation {
        ensure!(
            same_token(&state.token, &invite.token)
                && state
                    .peer
                    .as_deref()
                    .is_none_or(|id| id == invite.address.id.to_string()),
            "Saved peer differs from invitation; preserve account"
        );
        state.peer = Some(invite.address.id.to_string());
        state.peer_address = Some(invite.address.clone());
    }
    state.relay = state.relay.parse::<iroh::RelayUrl>()?.to_string();
    if let Some(selected) = relay {
        ensure!(
            selected.parse::<iroh::RelayUrl>()?.as_str() == state.relay,
            "Relay differs from saved selection; profile preserved"
        );
    }
    if let Some(invite) = &invitation {
        ensure!(
            invite.relay == state.relay,
            "Invitation relay differs from selected relay"
        );
    }
    kilogram_protocol::ordered_dialog_v2::canonical_runtime_relay(&state.relay)?;
    ensure!(
        state.relay.starts_with("https://"),
        "Peer rendezvous/relay requires HTTPS"
    );
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(SecretKey::from_bytes(&kilogram_runtime_ipc::ordered2::id(
            &state.key,
        )?))
        .relay_mode(RelayMode::custom([state.relay.parse()?]))
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await?;
    let invite_artifact = if let Some(invite) = &invitation {
        let public = ui
            .network(exchange(
                &endpoint,
                invite.address.clone(),
                invite.token.clone(),
                None,
            ))
            .await?;
        ensure!(
            hash(&public.artifact.encode()?) == invite.digest,
            "Invitation fingerprint mismatch"
        );
        if let Some(record) = existing_record {
            ensure!(
                record.peer.as_ref() == Some(&public.artifact.sender)
                    && record.contexts == public.artifact.contexts
                    && record.invitation == public.artifact.invitation,
                "This invitation is not from the saved peer; account preserved"
            );
        } else {
            ensure!(
                public.artifact.kind == Kind::Invite,
                "An existing conversation link requires its original account; no account was replaced"
            );
        }
        ui.confirm(&format!("Приглашение аккаунта {}. Получите ссылку по доверенному каналу; она не удостоверяет имя человека. Принять?",public.artifact.sender.account),"/accept")?;
        Some(public.artifact)
    } else {
        None
    };
    let home_copy = home.clone();
    let account_ui = ui.clone();
    let mut setup = tokio::task::spawn_blocking(move || {
        create_account(&home_copy, selected, invite_artifact, &account_ui)
    })
    .await??;
    ui.check_open()?;
    if ownership.is_none() {
        ownership = Some(setup.retain_ownership());
    }
    state.save(&home)?;
    let (_, artifact) = setup.export_terminal().map_err(anyhow::Error::from)?;
    let ready = setup.record.consent.as_ref().is_some_and(|s| {
        s.final_artifact.is_some()
            && s.slots
                .iter()
                .all(|s| s.importing == consent::Outcome::Accepted)
    });
    let final_envelope = setup
        .record
        .consent
        .as_ref()
        .and_then(|s| s.final_artifact.as_ref())
        .map(|r| {
            Envelope::decode(&model::read(
                &home.join("consent").join(&r.name),
                artifact::FILE_CAP,
            )?)
        })
        .transpose()?;
    connection::select_internet(&mut setup, &state.relay)?;
    let peer_identity = setup.record.peer.clone();
    let local = Arc::new(Mutex::new(Local {
        _ownership: ownership.context("Profile ownership unavailable")?,
        home: home.clone(),
        setup: Some(setup),
        state,
        public: Public {
            artifact,
            consent: final_envelope,
            locators: None,
            address: Some(endpoint.addr()),
        },
        ui: ui.clone(),
        ready,
        chat_started: false,
        incoming: None,
        peer_identity,
        approval: None,
    }));
    let server = tokio::spawn(serve(endpoint.clone(), local.clone(), ui.clone()));
    if local
        .lock()
        .map_err(|_| anyhow::anyhow!("State"))?
        .state
        .peer
        .is_none()
    {
        ui.network(async {
            let _ = timeout(Duration::from_secs(15), endpoint.online()).await;
            Ok(())
        })
        .await?;
        let link = {
            let l = local.lock().map_err(|_| anyhow::anyhow!("State"))?;
            Invitation {
                version: 1,
                address: endpoint.addr(),
                token: l.state.token.clone(),
                digest: hash(&l.public.artifact.encode()?),
                relay: l.state.relay.clone(),
            }
            .encode()?
        };
        ui.emit(
            "notice",
            "Передайте собеседнику одно приглашение:",
            Value::Null,
        );
        ui.emit("invitation", &link, json!({"invitation":link}));
        ui.emit("notice", "Ожидаю подключение…", Value::Null);
    }
    let client = tokio::spawn({
        let local = local.clone();
        let endpoint = endpoint.clone();
        let ui = ui.clone();
        async move {
            while !ui.stop.load(Ordering::Acquire) {
                let outgoing = local.try_lock().ok().and_then(|l| {
                    if !l.state.initiator {
                        return None;
                    }
                    l.state
                        .peer
                        .clone()
                        .map(|peer| (peer, l.state.token.clone(), l.public.clone()))
                });
                if let Some((peer, token, public)) = outgoing {
                    let result = async {
                        let id: EndpointId = peer.parse()?;
                        let address = {
                            let mut l = local
                                .try_lock()
                                .map_err(|_| anyhow::anyhow!("Peer exchange busy"))?;
                            l.public.address = Some(endpoint.addr());
                            l.state.peer_address.clone().unwrap_or(
                                EndpointAddr::new(id).with_relay_url(l.state.relay.parse()?),
                            )
                        };
                        let public = exchange(&endpoint, address, token, Some(public)).await?;
                        tokio::task::spawn_blocking({
                            let local = local.clone();
                            move || {
                                local
                                    .try_lock()
                                    .map_err(|_| anyhow::anyhow!("State"))?
                                    .update(id, public)
                            }
                        })
                        .await??;
                        Ok::<_, anyhow::Error>(())
                    }
                    .await;
                    if result.is_err() { /* Saved pending messages and selected peer remain intact; retry bounded transport. */
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });
    let result = terminal::run(home, &local, &ui).await;
    ui.stop.store(true, Ordering::Release);
    endpoint.close().await;
    client.abort();
    server.abort();
    result
}
