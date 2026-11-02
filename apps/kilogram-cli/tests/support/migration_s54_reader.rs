//! Dormant stopped cut reader; no invocation, Native checkpoint or phase composition.
use super::*;
use db::migration7::live1 as store;
use kilogram_cli::ordered_dialog::{handshake::Spec, state::Head};
use kilogram_protocol::ordered_dialog_v2 as v2;
use zeroize::Zeroizing;
#[path = "migration_s54_reader_gates.rs"]
mod gates;
#[path = "migration_s54_reader_owner.rs"]
mod owner;
#[cfg(test)]
#[path = "migration_s54_reader_tests.rs"]
mod tests;
fn digest(raw: &[u8]) -> old::Id {
    *blake3::hash(raw).as_bytes()
}
fn limit(context: usize) -> u64 {
    if context == 0 { 640 } else { 32 }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    source: old::Id,
    migration: old::Id,
    account: old::Id,
    claim: old::Id,
    device: old::Id,
    slots: [u8; 2],
    profile: old::Id,
    config_revision: u64,
}
pub(super) struct Owner {
    identity: Identity,
    pub graph: graph::Terminal,
    pub closure: control::Closure,
    pub generation: u64,
    pub last_commit: old::Id,
    pub history: [old::Id; 2],
    pub witnesses: old::Id,
}
pub(super) struct Cut {
    pub owners: [Owner; 2],
}
impl Cut {
    pub fn same_content(&self, other: &Self) -> Result<()> {
        for (a, b) in self.owners.iter().zip(&other.owners) {
            ensure!(
                a.identity == b.identity && a.history == b.history && a.witnesses == b.witnesses,
                "same original owner identity/mapping and immutable cut content"
            );
            a.graph.same(&b.graph)?;
        }
        Ok(())
    }
}
fn expected(previous: &relay::Continuation) -> Result<[[BTreeMap<String, Value>; 2]; 2]> {
    let c2 = &previous.previous.previous.previous.c2_history;
    let mut maps = std::array::from_fn(|o| {
        [
            previous.previous.previous.histories[o].clone(),
            c2[o].clone(),
        ]
    });
    let held = &previous.delivery["original_commitment"];
    let id = held["id"].as_str().context("retained original B640")?;
    let row = maps[1][0]
        .get(id)
        .context("actual original B640 history association")?
        .clone();
    ensure!(
        row["request"] == held["request"]
            && row["ordinal"] == 640
            && row["role"] == 1
            && row["format"] == 2
            && maps[0][0].insert(id.into(), row).is_none()
            && maps[0] == maps[1],
        "original complete P9/C2 maps plus sole retained B640 delivery"
    );
    for (o, contexts) in maps.iter().enumerate() {
        for (c, map) in contexts.iter().enumerate() {
            let retained = previous
                .histories
                .as_array()
                .context("retained P10 final history digests")?
                .iter()
                .filter(|r| r["owner"] == o && r["context"] == c)
                .collect::<Vec<_>>();
            ensure!(
                retained.len() == 1
                    && retained[0]["rows"] == limit(c) * 2
                    && retained[0]["digest"] == state::hex(&digest(&serde_json::to_vec(map)?)),
                "actual same-run final history commitment"
            );
        }
    }
    Ok(maps)
}
pub(super) fn read(previous: &mut relay::Continuation) -> Result<Cut> {
    let before = kilogram_ratchet::ordered::native_call_counts();
    let result = (|| {
        let maps = expected(previous)?;
        let immutable = previous.previous.previous.previous.previous.immutable()?;
        let ack = previous.previous.previous.ack_replayed.clone();
        ensure!(
            previous.previous.previous.ack_lost.receipts == ack.receipts
                && previous.previous.previous.ack_lost.digest == ack.digest,
            "original lost/replayed ACK immutable tuple/batch commitments"
        );
        let public = previous.relay["final"].clone();
        let profiles = previous.relay["profiles"].clone();
        let held = previous.delivery["original_commitment"].clone();
        let relay_pid = previous.relay_pid;
        let s = &mut previous.previous.previous.previous.previous.controller;
        let _phase = s53_deadline::Phase::until(s.until)?;
        let closures = s.normal_pair()?;
        ensure!(
            s.phases.next == 11
                && !s.phases.complete()
                && s.relay
                    .as_ref()
                    .is_some_and(|r| r.child.0.id() == relay_pid),
            "original incomplete P11 and same relay handle"
        );
        let mut owners = Vec::new();
        for o in 0..2 {
            ensure!(
                s.normal_pair()? == closures,
                "same reconciled pair before either read"
            );
            owners.push(owner::read(
                s,
                owner::Input {
                    owner: o,
                    closure: &closures[o],
                    maps: &maps[o],
                    public: &public[o],
                    profiles: &profiles,
                    immutable: &immutable,
                    held: &held,
                    ack: &ack,
                },
            )?);
            ensure!(
                s.normal_pair()? == closures,
                "same pair after owner repository dropped"
            );
        }
        let owners: [Owner; 2] = owners
            .try_into()
            .map_err(|_| anyhow::anyhow!("two independently closed owner reads"))?;
        owners[0].graph.same(&owners[1].graph)?;
        ensure!(
            owners[0].history == owners[1].history && owners[0].witnesses == owners[1].witnesses,
            "both actual immutable owner contents equal, excluding owner-specific IDs/generations"
        );
        Ok(Cut { owners })
    })();
    ensure!(
        before == kilogram_ratchet::ordered::native_call_counts(),
        "stopped reader has zero Native constructors/packet calls, including failures"
    );
    result
}
