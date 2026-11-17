use super::*;
use kilogram_runtime_ipc::ordered2::LanDiscovery2;
use kilogram_windows_lan::{Binding, NetworkScope, inspect_on_com_thread};
use std::{thread, time::Instant};
use tokio::sync::oneshot;
use windows::core::GUID;

pub struct Inspection {
    pub started: Instant,
    reply: oneshot::Receiver<Result<Binding>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Inspection {
    pub fn start(route: wire::DirectLan2, expected: Option<NetworkScope>) -> Result<Self> {
        Self::spawn(move || inspect_on_com_thread(route, expected))
    }

    pub(super) fn spawn(
        inspect: impl FnOnce() -> Result<Binding> + Send + 'static,
    ) -> Result<Self> {
        let started = Instant::now();
        let (send, reply) = oneshot::channel();
        let worker = thread::Builder::new()
            .name("kilogram-lan-metadata".into())
            .spawn(move || {
                let _ = send.send(inspect());
            })?;
        Ok(Self {
            started,
            reply,
            worker: Some(worker),
        })
    }

    /// Call after the result arrives. No COM waits are left on the worker.
    fn complete(&mut self, result: Result<Binding>) -> Result<Binding> {
        self.worker
            .take()
            .context("LAN metadata result consumed")?
            .join()
            .map_err(|_| anyhow::anyhow!("LAN metadata worker panicked"))?;
        result
    }

    pub async fn result(&mut self) -> Result<Binding> {
        let result = (&mut self.reply)
            .await
            .context("LAN metadata result lost")
            .and_then(|r| r);
        self.complete(result)
    }

    pub fn try_result(&mut self) -> Option<Result<Binding>> {
        match self.reply.try_recv() {
            Ok(result) => Some(self.complete(result)),
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => {
                Some(self.complete(Err(anyhow::anyhow!("LAN metadata result lost"))))
            }
        }
    }
}

pub fn setting(binding: Binding) -> LanDiscovery2 {
    let scope = binding.scope();
    LanDiscovery2::Auto {
        adapter_id: format!("{:032x}", scope.adapter_id.to_u128()),
        network_id: format!("{:032x}", scope.network_id.to_u128()),
        prefix: scope.prefix,
    }
}

pub(super) fn scope(setting: &LanDiscovery2) -> Result<NetworkScope> {
    let LanDiscovery2::Auto {
        adapter_id,
        network_id,
        prefix,
    } = setting
    else {
        anyhow::bail!("LAN Auto scope required");
    };
    Ok(NetworkScope {
        adapter_id: GUID::from_u128(u128::from_str_radix(adapter_id, 16)?),
        network_id: GUID::from_u128(u128::from_str_radix(network_id, 16)?),
        prefix: *prefix,
    })
}
