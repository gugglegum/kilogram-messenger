use std::{
    fmt, fs,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use kilogram_identity::{AccountId, DeviceId, DeviceIdentity};
use kilogram_protocol::{ConversationId, EventId};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tempfile::NamedTempFile;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};

const IPC_VERSION: u8 = 1;
const MAX_DESCRIPTOR_BYTES: u64 = 16 * 1024;
const MAX_FRAME_BYTES: usize = 256 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_CHANNEL_CAPACITY: usize = 64;
const DESCRIPTOR_SIGNATURE_DOMAIN: &[u8] = b"kilogram:runtime-ipc-descriptor:v1\0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct RuntimeIpcDescriptorContent {
    version: u8,
    address: String,
    token: String,
    account_id: AccountId,
    device_id: DeviceId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeIpcDescriptor {
    content: RuntimeIpcDescriptorContent,
    signature: Vec<u8>,
}

impl RuntimeIpcDescriptor {
    pub fn load(path: &Path) -> Result<Self> {
        load_descriptor(path)
    }

    pub fn account_id(&self) -> AccountId {
        self.content.account_id
    }

    pub fn device_id(&self) -> DeviceId {
        self.content.device_id
    }

    fn new(
        address: SocketAddr,
        token: [u8; 32],
        account_id: AccountId,
        identity: &DeviceIdentity,
    ) -> Result<Self> {
        let content = RuntimeIpcDescriptorContent {
            version: IPC_VERSION,
            address: address.to_string(),
            token: URL_SAFE_NO_PAD.encode(token),
            account_id,
            device_id: identity.device_id(),
        };
        let mut signing_bytes = Vec::with_capacity(DESCRIPTOR_SIGNATURE_DOMAIN.len() + 256);
        signing_bytes.extend_from_slice(DESCRIPTOR_SIGNATURE_DOMAIN);
        signing_bytes.extend_from_slice(
            &postcard::to_allocvec(&content).context("encode runtime IPC descriptor content")?,
        );
        let signature = identity.sign(&signing_bytes).to_vec();
        Ok(Self { content, signature })
    }

    fn verify(&self) -> Result<(SocketAddr, [u8; 32])> {
        ensure!(
            self.content.version == IPC_VERSION,
            "unsupported runtime IPC version"
        );
        let address: SocketAddr = self
            .content
            .address
            .parse()
            .context("runtime IPC descriptor has an invalid address")?;
        ensure!(
            address.ip().is_loopback(),
            "runtime IPC descriptor does not name a loopback address"
        );
        let encoded_token = URL_SAFE_NO_PAD
            .decode(&self.content.token)
            .context("runtime IPC descriptor has an invalid token")?;
        let token: [u8; 32] = encoded_token
            .try_into()
            .map_err(|_| anyhow::anyhow!("runtime IPC descriptor token has an invalid length"))?;
        let mut signing_bytes = Vec::with_capacity(DESCRIPTOR_SIGNATURE_DOMAIN.len() + 256);
        signing_bytes.extend_from_slice(DESCRIPTOR_SIGNATURE_DOMAIN);
        signing_bytes.extend_from_slice(
            &postcard::to_allocvec(&self.content)
                .context("encode runtime IPC descriptor for verification")?,
        );
        self.content
            .device_id
            .verify(&signing_bytes, &self.signature)
            .context("verify runtime IPC descriptor signature")?;
        Ok((address, token))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcCommand {
    Ping,
    QueueMessage {
        request_id: RuntimeIpcRequestId,
        conversation: String,
        peer_account_id: AccountId,
        message: String,
    },
    OutboxStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeIpcRequestId([u8; 32]);

impl RuntimeIpcRequestId {
    pub fn generate() -> Result<Self> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).context("generate runtime IPC request ID")?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for RuntimeIpcRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for RuntimeIpcRequestId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        ensure!(
            value.len() == 64 && value.is_ascii(),
            "runtime IPC request ID must be 64 hexadecimal characters"
        );
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let start = index * 2;
            *byte = u8::from_str_radix(&value[start..start + 2], 16)
                .context("runtime IPC request ID contains non-hexadecimal characters")?;
        }
        Ok(Self(bytes))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcQueueState {
    Queued,
    Materialized,
    Delivered,
}

impl RuntimeIpcQueueState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Materialized => "materialized",
            Self::Delivered => "delivered",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeIpcQueueItem {
    pub queue_id: String,
    pub peer_account_id: AccountId,
    pub conversation_id: ConversationId,
    pub state: RuntimeIpcQueueState,
    pub acknowledgement_event_id: Option<EventId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeIpcOutboxStatus {
    pub contact_count: usize,
    pub queue_count: usize,
    pub pending_count: usize,
    pub materialized_count: usize,
    pub delivered_count: usize,
    pub retry_state_count: usize,
    pub items: Vec<RuntimeIpcQueueItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeIpcResponse {
    Pong {
        account_id: AccountId,
        device_id: DeviceId,
    },
    MessageQueued {
        queue_id: String,
        contact_id: String,
        inserted: bool,
    },
    OutboxStatus(RuntimeIpcOutboxStatus),
    Error {
        message: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct RuntimeIpcRequest {
    version: u8,
    token: [u8; 32],
    command: RuntimeIpcCommand,
}

pub struct RuntimeIpcWork {
    command: RuntimeIpcCommand,
    response: oneshot::Sender<RuntimeIpcResponse>,
}

impl RuntimeIpcWork {
    pub fn into_parts(self) -> (RuntimeIpcCommand, oneshot::Sender<RuntimeIpcResponse>) {
        (self.command, self.response)
    }
}

pub struct RuntimeIpcServer {
    descriptor_path: PathBuf,
    descriptor: RuntimeIpcDescriptor,
    accept_task: JoinHandle<()>,
}

impl RuntimeIpcServer {
    pub async fn start(
        descriptor_path: PathBuf,
        account_id: AccountId,
        identity: &DeviceIdentity,
    ) -> Result<(Self, mpsc::Receiver<RuntimeIpcWork>)> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .context("bind authenticated runtime IPC loopback listener")?;
        let address = listener
            .local_addr()
            .context("read runtime IPC loopback address")?;
        let mut token = [0_u8; 32];
        getrandom::fill(&mut token).context("generate runtime IPC bearer token")?;
        let descriptor = RuntimeIpcDescriptor::new(address, token, account_id, identity)?;
        publish_descriptor(&descriptor_path, &descriptor)?;
        let (sender, receiver) = mpsc::channel(REQUEST_CHANNEL_CAPACITY);
        let accept_task = tokio::spawn(run_accept_loop(listener, token, sender));
        Ok((
            Self {
                descriptor_path,
                descriptor,
                accept_task,
            },
            receiver,
        ))
    }

    pub fn address(&self) -> &str {
        &self.descriptor.content.address
    }

    pub async fn shutdown(self) -> Result<()> {
        self.accept_task.abort();
        let current = load_descriptor(&self.descriptor_path);
        if current
            .as_ref()
            .is_ok_and(|value| value == &self.descriptor)
        {
            fs::remove_file(&self.descriptor_path).with_context(|| {
                format!(
                    "remove stopped runtime IPC descriptor {}",
                    self.descriptor_path.display()
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for RuntimeIpcServer {
    fn drop(&mut self) {
        self.accept_task.abort();
        let current = load_descriptor(&self.descriptor_path);
        if current
            .as_ref()
            .is_ok_and(|value| value == &self.descriptor)
        {
            let _ = fs::remove_file(&self.descriptor_path);
        }
    }
}

pub async fn call(
    descriptor_path: &Path,
    command: RuntimeIpcCommand,
) -> Result<RuntimeIpcResponse> {
    let descriptor = load_descriptor(descriptor_path)?;
    let (address, token) = descriptor.verify()?;
    let mut stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
        .await
        .context("runtime IPC connect timed out")?
        .context("connect to runtime IPC")?;
    let request = RuntimeIpcRequest {
        version: IPC_VERSION,
        token,
        command,
    };
    timeout(IO_TIMEOUT, write_frame(&mut stream, &request))
        .await
        .context("runtime IPC request timed out")??;
    timeout(IO_TIMEOUT, read_frame(&mut stream))
        .await
        .context("runtime IPC response timed out")?
}

async fn run_accept_loop(
    listener: TcpListener,
    expected_token: [u8; 32],
    sender: mpsc::Sender<RuntimeIpcWork>,
) {
    loop {
        let Ok((stream, remote)) = listener.accept().await else {
            return;
        };
        if !remote.ip().is_loopback() {
            continue;
        }
        let connection_sender = sender.clone();
        tokio::spawn(async move {
            let _ = serve_connection(stream, expected_token, connection_sender).await;
        });
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    expected_token: [u8; 32],
    sender: mpsc::Sender<RuntimeIpcWork>,
) -> Result<()> {
    let request: RuntimeIpcRequest = timeout(IO_TIMEOUT, read_frame(&mut stream))
        .await
        .context("runtime IPC client request timed out")??;
    ensure!(
        request.version == IPC_VERSION,
        "unsupported runtime IPC request"
    );
    ensure!(
        request.token == expected_token,
        "runtime IPC authentication failed"
    );
    let (response_sender, response_receiver) = oneshot::channel();
    timeout(
        IO_TIMEOUT,
        sender.send(RuntimeIpcWork {
            command: request.command,
            response: response_sender,
        }),
    )
    .await
    .context("runtime IPC actor queue timed out")?
    .context("runtime IPC actor stopped")?;
    let response = timeout(IO_TIMEOUT, response_receiver)
        .await
        .context("runtime IPC actor response timed out")?
        .context("runtime IPC actor dropped its response")?;
    timeout(IO_TIMEOUT, write_frame(&mut stream, &response))
        .await
        .context("runtime IPC client response timed out")??;
    Ok(())
}

fn publish_descriptor(path: &Path, descriptor: &RuntimeIpcDescriptor) -> Result<()> {
    descriptor.verify()?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "create runtime IPC descriptor directory {}",
                parent.display()
            )
        })?;
    }
    let temporary_directory = parent.unwrap_or_else(|| Path::new("."));
    let mut temporary =
        NamedTempFile::new_in(temporary_directory).context("create runtime IPC descriptor")?;
    serde_json::to_writer(&mut temporary, descriptor).context("encode runtime IPC descriptor")?;
    temporary
        .write_all(b"\n")
        .context("finish runtime IPC descriptor")?;
    temporary
        .as_file()
        .sync_all()
        .context("sync runtime IPC descriptor")?;
    let persisted = temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("publish runtime IPC descriptor to {}", path.display()))?;
    persisted
        .sync_all()
        .with_context(|| format!("sync runtime IPC descriptor at {}", path.display()))?;
    Ok(())
}

fn load_descriptor(path: &Path) -> Result<RuntimeIpcDescriptor> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("inspect runtime IPC descriptor {}", path.display()))?;
    ensure!(
        metadata.len() <= MAX_DESCRIPTOR_BYTES,
        "runtime IPC descriptor is too large"
    );
    let descriptor: RuntimeIpcDescriptor = serde_json::from_slice(
        &fs::read(path)
            .with_context(|| format!("read runtime IPC descriptor {}", path.display()))?,
    )
    .context("decode runtime IPC descriptor")?;
    descriptor.verify()?;
    Ok(descriptor)
}

async fn write_frame<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<()> {
    let encoded = postcard::to_allocvec(value).context("encode runtime IPC frame")?;
    ensure!(
        encoded.len() <= MAX_FRAME_BYTES,
        "runtime IPC frame is too large"
    );
    let length = u32::try_from(encoded.len()).context("runtime IPC frame length overflow")?;
    stream
        .write_all(&length.to_be_bytes())
        .await
        .context("write runtime IPC frame length")?;
    stream
        .write_all(&encoded)
        .await
        .context("write runtime IPC frame body")?;
    stream.flush().await.context("flush runtime IPC frame")
}

async fn read_frame<T: DeserializeOwned>(stream: &mut TcpStream) -> Result<T> {
    let mut encoded_length = [0_u8; 4];
    stream
        .read_exact(&mut encoded_length)
        .await
        .context("read runtime IPC frame length")?;
    let length = usize::try_from(u32::from_be_bytes(encoded_length))
        .context("runtime IPC frame length conversion")?;
    if length > MAX_FRAME_BYTES {
        bail!("runtime IPC frame is too large");
    }
    let mut encoded = vec![0_u8; length];
    stream
        .read_exact(&mut encoded)
        .await
        .context("read runtime IPC frame body")?;
    postcard::from_bytes(&encoded).context("decode runtime IPC frame")
}

#[cfg(test)]
mod tests {
    use std::{error::Error, net::IpAddr};

    use kilogram_identity::AccountRootState;

    use super::*;

    #[tokio::test]
    async fn authenticated_loopback_round_trip_and_cleanup() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let descriptor_path = directory.path().join("runtime.ipc.json");
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = kilogram_identity::DeviceIdentity::generate()?;
        let (server, mut receiver) =
            RuntimeIpcServer::start(descriptor_path.clone(), root.account_id(), &identity).await?;
        let client = tokio::spawn({
            let descriptor_path = descriptor_path.clone();
            async move { call(&descriptor_path, RuntimeIpcCommand::Ping).await }
        });
        let work = receiver.recv().await.context("receive loopback IPC work")?;
        let (command, response) = work.into_parts();
        assert_eq!(command, RuntimeIpcCommand::Ping);
        response
            .send(RuntimeIpcResponse::Pong {
                account_id: root.account_id(),
                device_id: identity.device_id(),
            })
            .map_err(|_| anyhow::anyhow!("send test IPC response"))?;
        assert!(matches!(client.await??, RuntimeIpcResponse::Pong { .. }));
        server.shutdown().await?;
        assert!(!descriptor_path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn wrong_token_is_rejected_before_actor_dispatch() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let descriptor_path = directory.path().join("runtime.ipc.json");
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = kilogram_identity::DeviceIdentity::generate()?;
        let (server, mut receiver) =
            RuntimeIpcServer::start(descriptor_path.clone(), root.account_id(), &identity).await?;
        let descriptor = load_descriptor(&descriptor_path)?;
        let (address, mut token) = descriptor.verify()?;
        token[0] ^= 1;
        let mut stream = TcpStream::connect(address).await?;
        write_frame(
            &mut stream,
            &RuntimeIpcRequest {
                version: IPC_VERSION,
                token,
                command: RuntimeIpcCommand::Ping,
            },
        )
        .await?;
        assert!(read_frame::<RuntimeIpcResponse>(&mut stream).await.is_err());
        assert!(
            timeout(Duration::from_millis(100), receiver.recv())
                .await
                .is_err()
        );
        server.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn old_instance_does_not_remove_replacement_descriptor() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let descriptor_path = directory.path().join("runtime.ipc.json");
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = kilogram_identity::DeviceIdentity::generate()?;
        let (first, _first_receiver) =
            RuntimeIpcServer::start(descriptor_path.clone(), root.account_id(), &identity).await?;
        let first_descriptor = load_descriptor(&descriptor_path)?;
        let (second, _second_receiver) =
            RuntimeIpcServer::start(descriptor_path.clone(), root.account_id(), &identity).await?;
        let second_descriptor = load_descriptor(&descriptor_path)?;
        assert_ne!(first_descriptor, second_descriptor);
        first.shutdown().await?;
        assert_eq!(load_descriptor(&descriptor_path)?, second_descriptor);
        second.shutdown().await?;
        assert!(!descriptor_path.exists());
        Ok(())
    }

    #[test]
    fn descriptor_rejects_non_loopback_addresses() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let root = AccountRootState::create(directory.path().join("root"))?;
        let identity = kilogram_identity::DeviceIdentity::generate()?;
        let descriptor = RuntimeIpcDescriptor::new(
            SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 1),
            [7_u8; 32],
            root.account_id(),
            &identity,
        )?;
        assert!(descriptor.verify().is_ok());
        let mut tampered = descriptor.clone();
        tampered.content.address = SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 2).to_string();
        assert!(tampered.verify().is_err());
        let mut remote = descriptor;
        remote.content.address = SocketAddr::new(IpAddr::from([192, 0, 2, 1]), 1).to_string();
        assert!(remote.verify().is_err());
        Ok(())
    }

    #[test]
    fn request_id_text_round_trips() -> Result<(), Box<dyn Error>> {
        let request_id = RuntimeIpcRequestId::generate()?;
        assert_eq!(
            request_id.to_string().parse::<RuntimeIpcRequestId>()?,
            request_id
        );
        assert!("00".parse::<RuntimeIpcRequestId>().is_err());
        assert!(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
                .parse::<RuntimeIpcRequestId>()
                .is_err()
        );
        Ok(())
    }
}
