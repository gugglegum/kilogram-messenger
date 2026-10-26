use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct L1Global {
    pub accounts: Vec<[Id; 2]>,
    pub next_author: u64,
}
#[derive(Clone, Serialize)]
pub struct L1Initialization {
    pub(super) accounts: Vec<[Id; 2]>,
    pub(super) next_author: u64,
    pub(super) authorities: Vec<L1Authority>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct L1Metadata {
    pub policy_generation: u64,
    pub wall_high_water: u64,
    pub next: [u64; 2],
    pub tails: [Option<Id>; 2],
    pub next_queue: u64,
    pub request_index_after: Option<u64>,
    pub request_index_ready: bool,
    pub ack_next: u64,
    pub peer_source_next: u64,
    pub peer_source_last: Option<Id>,
}
#[derive(Clone)]
pub struct L1Snapshot {
    pub head: Vec<u8>,
    pub native: Option<Vec<u8>>,
    pub meta: L1Metadata,
}
pub struct L1OwnerMaterial {
    pub account: Id,
    pub nonce: Id,
    pub incarnation: Id,
    pub signing: Zeroizing<Id>,
    pub encryption: Zeroizing<Id>,
    pub certificate: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct L1Authority {
    pub account: Id,
    pub revision: u64,
    pub raw: Vec<u8>,
}
#[derive(Clone, Copy, Serialize)]
pub enum L1Control {
    Consent,
    InitialPair,
    Ready(u8),
    Locator(bool),
    CurrentAuth,
    BlockedFork,
}
impl L1Control {
    pub(super) fn name(self) -> Result<String> {
        Ok(match self {
            Self::Consent => "consent-current".into(),
            Self::InitialPair => "initial-pair".into(),
            Self::Ready(role) if role < 2 => format!("ready/{role}"),
            Self::Locator(true) => "local-locator".into(),
            Self::Locator(false) => "peer-locator".into(),
            Self::CurrentAuth => "current-auth".into(),
            Self::BlockedFork => "blocked-fork".into(),
            _ => return Err(Error::Bound("L1 control kind")),
        })
    }
}
#[derive(Clone)]
pub struct L1Queue {
    pub sequence: u64,
    pub request: [u8; 16],
    pub sealed: Vec<u8>,
}
#[derive(Clone)]
pub struct L1Pending {
    pub ordinal: u64,
    pub raw: Vec<u8>,
    pub state: Vec<u8>,
    pub head: bool,
    pub quarantined: bool,
    pub original: Id,
    pub admission: Id,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct L1Request {
    pub queue_sequence: Option<u64>,
    pub event: Option<Id>,
    pub text_hash: Id,
}
#[derive(Clone, Serialize)]
pub struct L1ProofRefs {
    pub original: Id,
    pub admission: Option<Id>,
    pub final_auth: Option<Id>,
    pub receiver: Option<Id>,
    pub receipt: Option<Id>,
    pub receipt_overlay: Option<Id>,
    pub receiver_overlay: Option<Id>,
}
#[derive(Clone, Serialize)]
pub struct L1NativeChange {
    pub context: u8,
    pub expected_head: Id,
    pub policy_generation: u64,
    pub wall_time: u64,
    pub head: Vec<u8>,
    pub native: Option<Vec<u8>>,
}
#[derive(Clone, Serialize)]
pub struct L1Readable {
    pub author: Id,
    pub source: Vec<u8>,
    pub text: Option<String>,
    pub original: HistoryAuth,
    pub admission: Option<HistoryAuth>,
    pub final_auth: Option<HistoryAuth>,
    pub receiver: Option<HistoryAuth>,
    pub receipt: Option<Vec<u8>>,
    pub queue: Option<u64>,
}
#[derive(Clone, Copy, Serialize)]
pub enum L1Bootstrap {
    Offer,
    Grant,
    Init,
    Confirm,
    ReceiveConfirm,
    Ready,
}
#[derive(Clone, Serialize)]
pub enum L1Operation {
    Initialize(L1Initialization),
    PrepareRequests {
        context: u8,
    },
    InitializeHistory {
        context: u8,
    },
    PinAuthority {
        authority: L1Authority,
        now: u64,
    },
    RenewConsent {
        context: u8,
        expected: Option<Id>,
        raw: Vec<u8>,
        now: u64,
    },
    Locator {
        context: u8,
        local: bool,
        revision: u64,
        raw: Vec<u8>,
        now: u64,
    },
    AdmissionPair {
        context: u8,
        pair: Vec<u8>,
    },
    Bootstrap {
        kind: L1Bootstrap,
        change: L1NativeChange,
        pair: Option<Vec<u8>>,
        controls: Vec<Vec<u8>>,
        rows: Vec<L1Readable>,
        ready: Vec<(u8, Vec<u8>)>,
    },
    Queue {
        context: u8,
        request: [u8; 16],
        text_hash: Id,
        sealed: Vec<u8>,
        now: u64,
    },
    Materialize {
        change: L1NativeChange,
        rows: Vec<L1Readable>,
    },
    Pending {
        context: u8,
        author: Id,
        raw: Vec<u8>,
        original: HistoryAuth,
        admission: HistoryAuth,
        head: bool,
    },
    Drain {
        change: L1NativeChange,
        rows: Vec<L1Readable>,
    },
    Quarantine {
        context: u8,
        ordinal: u64,
    },
    Acknowledge {
        context: u8,
        event: Id,
        receipt: Vec<u8>,
        receiver: HistoryAuth,
    },
    AckProgress {
        context: u8,
        next: u64,
    },
    ObserveHead {
        context: u8,
        next: u64,
        last: Option<Id>,
    },
    Block {
        context: u8,
        proof: Vec<u8>,
    },
}
impl L1Operation {
    pub(super) fn context(&self) -> Option<u8> {
        match self {
            Self::Initialize(_) | Self::PinAuthority { .. } => None,
            Self::Bootstrap { change, .. }
            | Self::Materialize { change, .. }
            | Self::Drain { change, .. } => Some(change.context),
            Self::PrepareRequests { context }
            | Self::InitializeHistory { context }
            | Self::RenewConsent { context, .. }
            | Self::Locator { context, .. }
            | Self::AdmissionPair { context, .. }
            | Self::Queue { context, .. }
            | Self::Pending { context, .. }
            | Self::Quarantine { context, .. }
            | Self::Acknowledge { context, .. }
            | Self::AckProgress { context, .. }
            | Self::ObserveHead { context, .. }
            | Self::Block { context, .. } => Some(*context),
        }
    }
    pub(super) fn bound(&self) -> Result<()> {
        if self.context().is_some_and(|c| c > 1) {
            return Err(Error::Bound("L1 context"));
        }
        match self {
            Self::Initialize(init)
                if init.accounts.len() > 2
                    || init.authorities.len() > 3
                    || init.authorities.iter().any(|a| a.raw.len() > 16 * 1024) =>
            {
                return Err(Error::Bound("L1 account membership"));
            }
            Self::Bootstrap {
                change,
                pair,
                controls,
                rows,
                ready,
                ..
            } => {
                change.bound()?;
                if pair.as_ref().is_some_and(|p| p.len() > 192 * 1024)
                    || controls.len() > 2
                    || controls.iter().any(|p| p.len() > 8192)
                    || rows.len() > 2
                    || ready.len() > 2
                    || ready.iter().any(|(r, p)| *r > 1 || p.len() > 8192)
                {
                    return Err(Error::Bound("L1 bootstrap bytes"));
                }
                for row in rows {
                    row.bound()?;
                }
            }
            Self::Materialize { change, rows } | Self::Drain { change, rows } => {
                change.bound()?;
                if rows.is_empty() || rows.len() > 4 {
                    return Err(Error::Bound("L1 native batch"));
                }
                for row in rows {
                    row.bound()?;
                }
            }
            Self::PinAuthority { authority, .. } if authority.raw.len() > 16 * 1024 => {
                return Err(Error::Bound("L1 authority"));
            }
            Self::RenewConsent { raw, .. } if raw.len() > 4096 => {
                return Err(Error::Bound("L1 consent"));
            }
            Self::Locator { raw, .. } if raw.len() > 8192 => {
                return Err(Error::Bound("L1 locator"));
            }
            Self::Queue { sealed, .. } if sealed.len() > 8192 => {
                return Err(Error::Bound("L1 queue"));
            }
            Self::AdmissionPair { pair, .. } if pair.len() > 192 * 1024 => {
                return Err(Error::Bound("L1 admission pair"));
            }
            Self::Pending {
                raw,
                original,
                admission,
                ..
            } => {
                if raw.len() > 16 * 1024 {
                    return Err(Error::Bound("L1 pending"));
                }
                original.check_bound()?;
                admission.check_bound()?;
            }
            Self::Acknowledge {
                receipt, receiver, ..
            } => {
                if receipt.len() > 1024 {
                    return Err(Error::Bound("L1 receipt"));
                }
                receiver.check_bound()?;
            }
            Self::Block { proof, .. } if proof.len() > 64 * 1024 => {
                return Err(Error::Bound("L1 conflict proof"));
            }
            _ => (),
        }
        Ok(())
    }
}
impl L1NativeChange {
    fn bound(&self) -> Result<()> {
        if self.context > 1
            || self.head.len() > 2048
            || self
                .native
                .as_ref()
                .is_some_and(|n| n.is_empty() || n.len() > 128 * 1024)
        {
            return Err(Error::Bound("L1 native change"));
        }
        Ok(())
    }
}
impl L1Readable {
    fn bound(&self) -> Result<()> {
        if self.final_auth.is_none() {
            return Err(Error::Integrity("post-Ready final capture missing"));
        }
        if self.source.len() > 16 * 1024
            || self.text.as_ref().is_some_and(|t| t.len() > 4096)
            || self.receipt.as_ref().is_some_and(|r| r.len() > 1024)
        {
            return Err(Error::Bound("L1 readable input"));
        }
        self.original.check_bound()?;
        for auth in [&self.admission, &self.final_auth, &self.receiver]
            .into_iter()
            .flatten()
        {
            auth.check_bound()?;
        }
        Ok(())
    }
}
/// L1 keeps this independent bound until an actual later protocol handoff.
pub const L1_MAX_UNACKED: u64 = 128;

pub(super) fn check_capture_shape(own: bool, text: bool, captures: [bool; 4]) -> Result<()> {
    let [admission, final_auth, receiver, receipt] = captures;
    if !final_auth
        || if own {
            admission || receiver || receipt
        } else {
            !admission || receiver != text || receipt != text
        }
    {
        return Err(Error::Integrity("full L1 applicable capture shape"));
    }
    Ok(())
}
