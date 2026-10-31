//! Small genuine L1 fixture: reviewed Pair construction/admission/activation
//! reused from ordered_dialog/repair_tests.rs; two texts, no exhausted fixture.
use super::*;
use kilogram_cli::ordered_dialog::{
    admin,
    engine::{self, Engine, Lease},
    handshake,
};
use kilogram_identity::{AccountId, AccountRootState};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Frame, Id, a, b, u};
use kilogram_state::{OrderedVault, StateDirectoryLock};
pub(super) struct Pair {
    pub(super) _tmp: tempfile::TempDir,
    pub(super) accounts: [PathBuf; 2],
    pub(super) a: Engine,
    pub(super) b: Engine,
    pub(super) c: Id,
    pub(super) binding: Id,
}
impl Pair {
    pub(super) fn new_in(parent: &std::path::Path) -> Result<Self> {
        Self::from_temp(
            tempfile::Builder::new()
                .prefix("genuine-l1-")
                .tempdir_in(parent)?,
        )
    }
    fn from_temp(tmp: tempfile::TempDir) -> Result<Self> {
        let mut stores = Vec::new();
        for i in 0..2 {
            let owner = tmp.path().join(format!("owner{i}"));
            AccountRootState::create(&owner)?;
            let root = tmp.path().join(format!("device{i}"));
            admin::enroll(StateDirectoryLock::acquire(&root)?, &owner, None)?;
            let store = OrderedVault::open(StateDirectoryLock::acquire(root)?)?;
            stores.push((admin::credentials(&store)?.cert.device_id(), owner, store));
        }
        stores.sort_by_key(|(id, _, _)| *id);
        let (_, owner_b, mut sb) = stores.pop().context("B")?;
        let (_, owner_a, mut sa) = stores.pop().context("A")?;
        let ca = admin::credentials(&sa)?;
        let cb = admin::credentials(&sb)?;
        let now = engine::now()?;
        let (c, invitation) =
            admin::create_context(&sa, &owner_a, &cb.raw, cb.cert.account_id(), now)?;
        let (_, complete) = admin::import_context(
            &mut sb,
            Some(&owner_b),
            &invitation,
            ca.cert.account_id(),
            now,
        )?;
        admin::import_context(&mut sa, None, &complete, cb.cert.account_id(), now)?;
        Ok(Self {
            _tmp: tmp,
            accounts: [owner_a, owner_b],
            a: Engine::new(sa)?,
            b: Engine::new(sb)?,
            c,
            binding: [0; 32],
        })
    }
    fn pair_with_refs(&self, refs: [Id; 2], binding: Id) -> Result<Vec<u8>> {
        let mut hellos = Vec::new();
        for (role, engine) in [&self.a, &self.b].into_iter().enumerate() {
            let spec = engine.spec(self.c)?;
            let body = a([
                b(self.c),
                u(role as u64),
                b([role as u8; 32]),
                b([1 - role as u8; 32]),
                b(admin::random::<32>()?),
                a([u(1)]),
                a([u(1)]),
                b(admin::credentials(&engine.store)?.raw),
                b(&spec.membership),
                b(refs[role]),
                b([7; 32]),
            ]);
            hellos.push(
                Frame::signed(
                    1,
                    binding,
                    admin::random::<16>()?,
                    body,
                    engine.device.identity(),
                )?
                .raw()?,
            );
        }
        let t = handshake::transcript(binding, &hellos[0], &hellos[1])?;
        let certs = self.a.spec(self.c)?.cert_refs()?;
        let proofs = [&self.a, &self.b]
            .into_iter()
            .map(|e| {
                Frame::signed(
                    2,
                    binding,
                    [1; 16],
                    a([b(t), b(self.c), u(1), u(1), b(certs[0]), b(certs[1])]),
                    e.device.identity(),
                )
                .and_then(|f| f.raw())
            })
            .collect::<wire::Result<Vec<_>>>()?;
        handshake::initial_pair(
            t,
            hellos[0].clone(),
            hellos[1].clone(),
            proofs[0].clone(),
            proofs[1].clone(),
        )
    }
    pub(super) fn admit(&mut self) -> Result<Vec<u8>> {
        self.binding = admin::random()?;
        for e in [&mut self.a, &mut self.b] {
            e.policy(self.c)?;
            e.leases.insert(
                self.c,
                Lease {
                    binding: self.binding,
                    generation: 1,
                    deadline: engine::now()? + 300,
                },
            );
        }
        let consent = admin::required(&self.a.store, &admin::consent_path(self.c))?;
        self.pair_with_refs([wire::hash(handshake::CONSENT, &consent); 2], self.binding)
    }
    pub(super) fn activate(&mut self, pair: &[u8]) -> Result<()> {
        let offer = self.a.ensure_offer(self.c, pair, self.binding)?;
        let grant = self.b.grant(self.c, &offer, pair, self.binding)?;
        let init = self.a.init(self.c, &grant, self.binding)?;
        let confirm = self.b.confirm(self.c, &init, self.binding)?;
        let ready_a = self.a.receive_confirm(self.c, &confirm, self.binding)?;
        let ready_b = self.b.ready(self.c, &ready_a, self.binding)?;
        self.a.ready(self.c, &ready_b, self.binding)?;
        Ok(())
    }
}

pub struct Small {
    pub repositories: Vec<store::L1Repository>,
    pub profiles: Vec<Vec<u8>>,
    pub configs: Vec<Vec<u8>>,
    pub expected: Vec<fixed::Expected>,
    pub proofs: [Vec<u8>; 2],
}
pub fn small() -> Result<Small> {
    let parent = Path::new(fixed::WORKSPACE).join(".tmp/C-s53-cold-b1-synthetic");
    std::fs::create_dir_all(&parent)?;
    let mut pair = Pair::new_in(&parent)?;
    let initial = pair.admit()?;
    pair.activate(&initial)?;
    for role in 0..2 {
        let mut id = [0; 16];
        id[..8].copy_from_slice(&(if role == 0 { 2u64 } else { 1 }).to_be_bytes());
        id[8..].copy_from_slice(&1u64.to_be_bytes());
        let text = format!("OD1/{}/1/Привет", if role == 0 { "two" } else { "one" });
        let (sender, receiver) = if role == 0 {
            (&mut pair.a, &mut pair.b)
        } else {
            (&mut pair.b, &mut pair.a)
        };
        sender.queue(pair.c, id, &text)?;
        let event = sender
            .materialize_one(pair.c, pair.binding)?
            .context("small actual Text")?;
        let frame = sender.deliver_frame(pair.c, &[event], pair.binding, id)?;
        let result = receiver.accept_data_frame(pair.c, &frame)?;
        ensure!(
            result.error.is_none() && result.readable == 1,
            "small actual receive"
        );
        let (ack, ordinals) = receiver
            .ack_intents(pair.c, pair.binding)?
            .context("small actual ACK")?;
        sender.accept_ack_frame(pair.c, &ack)?;
        receiver.mark_ack_intents(pair.c, pair.binding, &ordinals, 2)?;
    }
    let proofs = [
        AccountRootState::load(&pair.accounts[0])?
            .authority_snapshot()?
            .encode()?,
        AccountRootState::load(&pair.accounts[1])?
            .authority_snapshot()?
            .encode()?,
    ];
    let c = pair.c;
    let base = pair._tmp.keep();
    let mut repositories = Vec::new();
    let mut profiles = Vec::new();
    let mut configs = Vec::new();
    let mut expected = Vec::new();
    for (local, engine) in [pair.a, pair.b].into_iter().enumerate() {
        let root = engine.store.root().to_path_buf();
        let cred = admin::credentials(&engine.store)?;
        let account = AccountId::from_bytes(*cred.cert.account_id().as_bytes());
        let device = cred.cert.device_id();
        drop(engine);
        let path = base.join(format!("{local}.profile"));
        let config = db::migration7::MigrationConfig {
            logical_quota: [512 * 1024 * 1024; 2],
            physical_capacity: 2 * 1024 * 1024 * 1024,
            output_profile: path.clone(),
            control_ipc_file: base.join(format!("{local}.ipc")),
            locator_directory: base.join(format!("{local}.locators")),
        };
        kilogram_cli::ordered_dialog::storage_migration::upgrade(
            StateDirectoryLock::acquire(&root)?,
            account,
            device,
            &config,
            false,
            None,
        )?;
        let profile = Profile3::load(&path)?;
        let mut repo = store::L1Repository::open_ready(
            StateDirectoryLock::acquire(&root)?,
            db::ExpectedOwner {
                account: state::parse_id(&profile.account)?,
                claim: state::parse_id(&profile.claim)?,
            },
            state::parse_id(&profile.device)?,
            db::migration7::MigrationConfig {
                output_profile: path.canonicalize()?,
                ..config.clone()
            },
        )?;
        // Fresh synthetic migration only: mirror Owner7::new's saved initialization
        // without constructing an owner/device secret service or starting a network.
        let spec = kilogram_cli::ordered_dialog::handshake::Spec::parse(&repo.spec(0)?)?;
        let accounts = spec
            .peers
            .each_ref()
            .map(|p| *p.cert.account_id().as_bytes());
        let mut pins = Vec::new();
        for account in accounts {
            let raw = repo.authority(account)?;
            let authority = kilogram_identity::AccountAuthoritySnapshot::decode_and_verify(&raw)?;
            pins.push(store::L1Authority {
                account,
                revision: authority.revision(),
                raw,
            });
        }
        repo.initialize(vec![accounts], pins)?;
        for _ in 0..4 {
            if repo.metadata(0)?.request_index_ready {
                break;
            }
            repo.commit(store::L1Operation::PrepareRequests { context: 0 }, None)?;
        }
        ensure!(
            repo.metadata(0)?.request_index_ready,
            "small saved request index ready"
        );
        repo.commit(store::L1Operation::InitializeHistory { context: 0 }, None)?;
        let scope = readers::scope(&mut repo, 0, local)?;
        let snapshot = repo.snapshot(0)?;
        expected.push(fixed::Expected {
            local,
            c,
            accounts: scope
                .verified
                .peers()
                .each_ref()
                .map(|p| *p.certificate().account_id().as_bytes()),
            devices: scope
                .verified
                .peers()
                .each_ref()
                .map(|p| *p.certificate().device_id().as_bytes()),
            sid: scope.verified.sid().into(),
            next: [2, 2],
            native_generation: scope.head.native_generation,
            native_hash: state::hex(&wire::hash(
                state::NATIVE_TAG,
                &snapshot.native.context("small opaque Native")?,
            )),
        });
        profiles.push(std::fs::read(path)?);
        configs.push(serde_json::to_vec(&config)?);
        repositories.push(repo);
    }
    Ok(Small {
        repositories,
        profiles,
        configs,
        expected,
        proofs,
    })
}
