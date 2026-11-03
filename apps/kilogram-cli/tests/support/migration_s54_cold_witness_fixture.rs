//! Component lease pattern from repair_tests; real public identity, Engine,
//! Native handshake/Text/outbox APIs and full provenance-checked migration.
use super::*;
use kilogram_cli::ordered_dialog::{
    admin,
    engine::{Engine, Lease},
    handshake, storage_migration,
};
use kilogram_identity::AccountRootState;
use kilogram_protocol::ordered_dialog_v1::{Frame, a, b, u};
use kilogram_ratchet::ordered::native_call_counts;
use kilogram_state::{OrderedVault, StateDirectoryLock};

pub(super) struct Fresh {
    pub _dir: tempfile::TempDir,
    pub nodes: [Node; 2],
    pub contexts: [String; 2],
    pub ready: [Value; 2],
    pub purpose: Purpose,
}
impl Fresh {
    pub fn legacy(purpose: Purpose) -> Result<Self> {
        Self::staged(purpose, 1, false).map(|(fresh, _)| fresh)
    }
    pub fn f_with_genesis(slot: u8) -> Result<(Self, [Legacy; 2])> {
        let (fresh, genesis) = Self::staged(Purpose::FeasibilityF, slot, true)?;
        Ok((fresh, genesis.context("actual early genesis witnesses")?))
    }
    fn staged(purpose: Purpose, slot: u8, early: bool) -> Result<(Self, Option<[Legacy; 2]>)> {
        ensure!(slot < 2, "fixture C1 slot");
        let base = workspace().join(if early {
            ".tmp/s54-block4ao-deferred-memory-01/fixtures"
        } else {
            ".tmp/s54-block4af-cold-witness-01/fixtures"
        });
        std::fs::create_dir_all(&base)?;
        let dir = tempfile::Builder::new().prefix("fresh-").tempdir_in(base)?;
        let mut stores = Vec::new();
        for i in 0..2 {
            let account = dir.path().join(format!("account{i}"));
            AccountRootState::create(&account)?;
            let state = dir.path().join(format!("device{i}"));
            admin::enroll(StateDirectoryLock::acquire(&state)?, &account, None)?;
            let store = OrderedVault::open(StateDirectoryLock::acquire(state)?)?;
            stores.push((admin::credentials(&store)?.cert.device_id(), account, store));
        }
        stores.sort_by_key(|(id, _, _)| *id);
        let (_, account_b, mut sb) = stores.pop().context("owner B")?;
        let (_, account_a, mut sa) = stores.pop().context("owner A")?;
        let ca = admin::credentials(&sa)?;
        let cb = admin::credentials(&sb)?;
        let mut contexts = Vec::new();
        for _ in 0..2 {
            let (c, invitation) = admin::create_context(
                &sa,
                &account_a,
                &cb.raw,
                cb.cert.account_id(),
                engine::now()?,
            )?;
            let (_, complete) = admin::import_context(
                &mut sb,
                Some(&account_b),
                &invitation,
                ca.cert.account_id(),
                engine::now()?,
            )?;
            admin::import_context(
                &mut sa,
                None,
                &complete,
                cb.cert.account_id(),
                engine::now()?,
            )?;
            contexts.push(c);
        }
        // Preserve AF's default C1-last mapping; AO also exercises C1-first.
        contexts.sort();
        if slot == 1 {
            contexts.reverse();
        }
        let mut a_engine = Engine::new(sa)?;
        let mut b_engine = Engine::new(sb)?;
        let mut bindings = Vec::new();
        for c in &contexts {
            let binding = admin::random()?;
            activate(&mut a_engine, &mut b_engine, *c, binding)?;
            bindings.push(binding);
        }
        let early_nodes = if early {
            Some(nodes(dir.path(), [&a_engine, &b_engine], &contexts, slot)?)
        } else {
            None
        };
        let genesis = if let Some(nodes) = &early_nodes {
            // Capture only after both real vault leases are quiescent. Restoring
            // transient Engine leases below does not redo the Native handshake.
            drop(a_engine);
            drop(b_engine);
            let mut saved = Vec::new();
            let logical = [state::hex(&contexts[0]), state::hex(&contexts[1])];
            for (owner, node) in nodes.iter().enumerate() {
                let source = db::migration7::V4Source::open(
                    StateDirectoryLock::acquire(&node.branch.state)?,
                    db::Config::default(),
                )?;
                let witness = Legacy::capture_source(node, &logical, purpose, &source)?;
                witness.role(owner)?;
                ensure!(
                    witness.contexts.iter().all(|c| c.events.len() == 2
                        && c.events.contains_key(&(0, 0))
                        && c.events.contains_key(&(1, 0))
                        && c.history.is_empty()
                        && c.queue.is_empty()),
                    "actual zero Text genesis before traffic"
                );
                saved.push(witness);
            }
            a_engine = Engine::new(OrderedVault::open(StateDirectoryLock::acquire(
                &nodes[0].branch.state,
            )?)?)?;
            b_engine = Engine::new(OrderedVault::open(StateDirectoryLock::acquire(
                &nodes[1].branch.state,
            )?)?)?;
            for e in [&mut a_engine, &mut b_engine] {
                for (c, binding) in contexts.iter().zip(&bindings) {
                    let generation = e.policy(*c)?;
                    e.leases.insert(
                        *c,
                        Lease {
                            binding: *binding,
                            generation,
                            deadline: engine::now()? + 300,
                        },
                    );
                }
            }
            Some(
                saved
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("two genesis owners"))?,
            )
        } else {
            None
        };
        let last = match purpose {
            Purpose::FeasibilityF => 2,
            Purpose::WholeP => 16,
        };
        for n in 1..=last {
            send(
                &mut a_engine,
                &mut b_engine,
                contexts[0],
                bindings[0],
                0,
                n,
                true,
            )?;
            send(
                &mut b_engine,
                &mut a_engine,
                contexts[0],
                bindings[0],
                1,
                n,
                purpose == Purpose::FeasibilityF || n < last,
            )?;
        }
        for n in [last + 1, last + 2] {
            let request = oracle::Request::new(0, 0, n)?;
            a_engine.queue(
                contexts[0],
                parse_request_id(&request_id(request))?,
                &request.body(),
            )?;
        }
        let nodes = if let Some(nodes) = early_nodes {
            nodes
        } else {
            nodes(dir.path(), [&a_engine, &b_engine], &contexts, slot)?
        };
        drop(a_engine);
        drop(b_engine);
        Ok((
            Self {
                _dir: dir,
                nodes,
                contexts: [state::hex(&contexts[0]), state::hex(&contexts[1])],
                ready: [Value::Null, Value::Null],
                purpose,
            },
            genesis,
        ))
    }
    pub fn next(&self, owner: usize) -> [[u64; 2]; 2] {
        let c1 = match self.purpose {
            Purpose::FeasibilityF => [3, 3],
            Purpose::WholeP => {
                if owner == 0 {
                    [17, 16]
                } else {
                    [17, 17]
                }
            }
        };
        [c1, [1, 1]]
    }
    pub fn migrate(&mut self) -> Result<()> {
        let calls = native_call_counts();
        for owner in 0..2 {
            let node = &self.nodes[owner];
            let config = db::migration7::MigrationConfig {
                logical_quota: [512 * 1024 * 1024; 2],
                physical_capacity: 2 * 1024 * 1024 * 1024,
                output_profile: node.profile.clone(),
                control_ipc_file: node.ipc.clone(),
                locator_directory: node.locators.clone(),
            };
            self.ready[owner] = storage_migration::upgrade(
                StateDirectoryLock::acquire(&node.branch.state)?,
                node.branch.account.parse()?,
                node.branch.device.parse()?,
                &config,
                false,
                None,
            )?;
            ensure!(
                self.ready[owner]["storage_stage"] == "Ready"
                    && self.ready[owner]["zero_migration_native_calls"] == true,
                "actual Ready publication"
            );
            self.nodes[owner].indexed = true;
        }
        ensure!(
            native_call_counts() == calls,
            "migration performs zero Native work"
        );
        Ok(())
    }
    pub fn open(&self, owner: usize) -> Result<db::migration7::live1::L1Repository> {
        let n = &self.nodes[owner];
        Profile3::load(&n.profile)?.open(&n.profile)
    }
    pub fn capture(&self, owner: usize) -> Result<Legacy> {
        let n = &self.nodes[owner];
        let source = db::migration7::V4Source::open(
            StateDirectoryLock::acquire(&n.branch.state)?,
            db::Config::default(),
        )?;
        Legacy::capture_source(n, &self.contexts, self.purpose, &source)
    }
}
fn nodes(dir: &Path, engines: [&Engine; 2], contexts: &[old::Id], slot: u8) -> Result<[Node; 2]> {
    engines
        .iter()
        .enumerate()
        .map(|(owner, e)| {
            let state = e.store.root().to_path_buf();
            let (_, account, claim) = admin::claim(&e.store)?;
            let base = dir.join(format!("public{owner}"));
            std::fs::create_dir(&base)?;
            Ok(Node {
                branch: Branch {
                    base: base.clone(),
                    state,
                    account: account.to_string(),
                    device: e.device.identity().device_id().to_string(),
                    config: base.join("config.json"),
                    context: state::hex(&contexts[0]),
                    claim: state::hex(&claim),
                },
                profile: base.join("indexed.profile"),
                ipc: base.join("indexed.ipc"),
                locators: base.join("locators"),
                indexed: false,
                slot,
            })
        })
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| anyhow::anyhow!("two original owners"))
}
fn activate(
    a_engine: &mut Engine,
    b_engine: &mut Engine,
    c: old::Id,
    binding: old::Id,
) -> Result<()> {
    for e in [&mut *a_engine, &mut *b_engine] {
        e.policy(c)?;
        e.leases.insert(
            c,
            Lease {
                binding,
                generation: 1,
                deadline: engine::now()? + 300,
            },
        );
    }
    let consent = admin::required(&a_engine.store, &admin::consent_path(c))?;
    let reference = old::hash(handshake::CONSENT, &consent);
    let certs = a_engine.spec(c)?.cert_refs()?;
    let mut hellos = Vec::new();
    for (role, e) in [&*a_engine, &*b_engine].into_iter().enumerate() {
        let spec = e.spec(c)?;
        hellos.push(
            Frame::signed(
                1,
                binding,
                admin::random()?,
                a([
                    b(c),
                    u(role as u64),
                    b([role as u8; 32]),
                    b([1 - role as u8; 32]),
                    b(admin::random::<32>()?),
                    a([u(1)]),
                    a([u(1)]),
                    b(admin::credentials(&e.store)?.raw),
                    b(&spec.membership),
                    b(reference),
                    b([7; 32]),
                ]),
                e.device.identity(),
            )?
            .raw()?,
        );
    }
    let t = handshake::transcript(binding, &hellos[0], &hellos[1])?;
    let proofs = [&*a_engine, &*b_engine]
        .into_iter()
        .map(|e| {
            Frame::signed(
                2,
                binding,
                [1; 16],
                a([b(t), b(c), u(1), u(1), b(certs[0]), b(certs[1])]),
                e.device.identity(),
            )
            .and_then(|f| f.raw())
        })
        .collect::<old::Result<Vec<_>>>()?;
    let pair = handshake::initial_pair(
        t,
        hellos[0].clone(),
        hellos[1].clone(),
        proofs[0].clone(),
        proofs[1].clone(),
    )?;
    let offer = a_engine.ensure_offer(c, &pair, binding)?;
    let grant = b_engine.grant(c, &offer, &pair, binding)?;
    let init = a_engine.init(c, &grant, binding)?;
    let confirm = b_engine.confirm(c, &init, binding)?;
    let ready_a = a_engine.receive_confirm(c, &confirm, binding)?;
    let ready_b = b_engine.ready(c, &ready_a, binding)?;
    a_engine.ready(c, &ready_b, binding)?;
    Ok(())
}
fn send(
    sender: &mut Engine,
    receiver: &mut Engine,
    c: old::Id,
    binding: old::Id,
    role: u8,
    n: u16,
    deliver: bool,
) -> Result<()> {
    let request = oracle::Request::new(0, role, n)?;
    let id = parse_request_id(&request_id(request))?;
    sender.queue(c, id, &request.body())?;
    let event = sender
        .materialize_one(c, binding)?
        .context("genuine Text materialization")?;
    ensure!(
        event.core.n == u64::from(n) && event.core.role == u64::from(role),
        "real Text ordinal"
    );
    if deliver {
        let frame = sender.deliver_frame(c, &[event], binding, id)?;
        let batch = receiver.accept_data_frame(c, &frame)?;
        ensure!(
            batch.error.is_none() && batch.readable == 1,
            "genuine Native Text receive"
        );
        let (ack, ordinals) = receiver.ack_intents(c, binding)?.context("actual ACK")?;
        sender.accept_ack_frame(c, &ack)?;
        receiver.mark_ack_intents(c, binding, &ordinals, 2)?;
    }
    Ok(())
}
