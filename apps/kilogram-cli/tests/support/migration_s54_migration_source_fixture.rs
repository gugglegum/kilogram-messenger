//! Unequal-ID unit records only: no real state growth, crypto, child or I/O.
use super::*;

pub(in super::super) struct Fixture {
    pub contexts: [String; 2],
    pub witnesses: [Option<Legacy>; 2],
    pub rows: BTreeMap<String, Value>,
    pub endpoints: [Option<intervals::Endpoint>; 2],
    pub genesis: [old::Id; 2],
}
impl Fixture {
    pub fn new(c1_slot: u8, frontiers: [[u64; 2]; 2]) -> Self {
        let contexts = [[91; 32], [92; 32]];
        let witnesses = std::array::from_fn(|owner| {
            Some(Legacy {
                state: PathBuf::from(format!("synthetic-no-io-{owner}")),
                signing: Zeroizing::new([0; 32]),
                encryption: Zeroizing::new([0; 32]),
                source_id: [31 + owner as u8; 32],
                contexts: contexts
                    .iter()
                    .enumerate()
                    .map(|(c, id)| {
                        let next = if c == 0 { frontiers[owner] } else { [1, 1] };
                        let queue = if owner == 0 && c == 0 {
                            [([1; 16], vec![1]), ([2; 16], vec![2])].into()
                        } else {
                            BTreeMap::new()
                        };
                        SavedContext {
                            c: *id,
                            role: owner as u8,
                            spec: vec![],
                            head: vec![],
                            native: vec![],
                            keys: vec![],
                            history: BTreeMap::new(),
                            queue,
                            receipts: BTreeMap::new(),
                            events: (0..2u8)
                                .flat_map(|role| {
                                    (0..next[role as usize]).map(move |n| ((role, n), vec![role]))
                                })
                                .collect(),
                        }
                    })
                    .collect(),
            })
        });
        let endpoints = std::array::from_fn(|owner| {
            Some(intervals::Endpoint {
                source: [31 + owner as u8; 32],
                lanes: std::array::from_fn(|c| intervals::Lane {
                    context: contexts[c],
                    role: owner as u8,
                    slot: if c == 0 { c1_slot } else { 1 - c1_slot },
                    next: if c == 0 { frontiers[owner] } else { [1, 1] },
                }),
            })
        });
        let rows = (0..2).map(|owner| {
            let mut physical = [Value::Null,Value::Null];
            for (c,id) in contexts.iter().enumerate() {
                let slot=if c==0 {c1_slot}else{1-c1_slot};
                physical[slot as usize]=json!({"context":state::hex(id),"role":owner,
                    "readable":if c==0 {frontiers[owner]}else{[1,1]},"queue":if c==0&&owner==0 {2}else{0},
                    "undelivered":if owner==1&&frontiers[owner]==[25,25]&&c==0 {1}else{0}});
            }
            (format!("ready-{owner}"),json!({"source_id":state::hex(&[31+owner as u8;32]),
                "storage_stage":"Ready","zero_migration_native_calls":true,"contexts":physical}))
        }).collect();
        Self {
            contexts: contexts.map(|c| state::hex(&c)),
            witnesses,
            rows,
            endpoints,
            genesis: [[11; 32], [12; 32]],
        }
    }
    pub fn corrupt_witness(&mut self, mutation: u8) -> Result<()> {
        let w = self.witnesses[0].as_mut().context("synthetic witness")?;
        match mutation {
            0 => {
                w.contexts[0].events.remove(&(0, 1));
            }
            1 => {
                w.contexts[0].events.insert((2, 0), vec![]);
            }
            2 => {
                w.contexts[1].c = w.contexts[0].c;
            }
            3 => {
                w.contexts[0].role = 1;
            }
            _ => unreachable!("fixed synthetic mutations"),
        }
        Ok(())
    }
}
