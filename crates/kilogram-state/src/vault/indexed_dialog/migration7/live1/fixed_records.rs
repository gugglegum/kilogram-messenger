//! Bounded identities for six fixed public metadata/routing records. No raw API.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L1RecordFingerprint7 {
    pub length: u64,
    pub digest: Id,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L1FixedRecords7 {
    pub head: L1RecordFingerprint7,
    pub legacy_budget: L1RecordFingerprint7,
    pub own_locator: Option<L1RecordFingerprint7>,
    pub peer_locator: Option<L1RecordFingerprint7>,
    pub existing: Option<L1RecordFingerprint7>,
    pub metadata: Option<L1RecordFingerprint7>,
}
fn fingerprint(raw: Option<Vec<u8>>, cap: usize) -> Result<Option<L1RecordFingerprint7>> {
    raw.map(|raw| {
        if raw.len() > cap {
            return Err(Error::Bound("fixed L1 record cap"));
        }
        Ok(L1RecordFingerprint7 {
            length: raw.len() as u64,
            digest: *blake3::hash(&raw).as_bytes(),
        })
    })
    .transpose()
}
fn read(c: u8, mut get: impl FnMut(Key) -> Result<Option<Vec<u8>>>) -> Result<L1FixedRecords7> {
    if c >= 2 {
        return Err(Error::Bound("unoccupied L1 slot"));
    }
    Ok(L1FixedRecords7 {
        head: fingerprint(get(head_key(c)?)?, 8192)?
            .ok_or(Error::Integrity("fixed Head7 absent"))?,
        legacy_budget: fingerprint(get(budget_key(c)?)?, 4096)?
            .ok_or(Error::Integrity("fixed LegacyBudget absent"))?,
        own_locator: fingerprint(get(key(c, b"locator2-own")?)?, 8192)?,
        peer_locator: fingerprint(get(key(c, b"locator2-peer")?)?, 8192)?,
        existing: fingerprint(get(key(c, b"existing")?)?, 24 * 1024)?,
        metadata: fingerprint(get(key(c, b"meta")?)?, META)?,
    })
}
impl L1Repository {
    /// Authenticated fixed-record lengths/digests for one occupied context.
    /// Head/budget are required; absent optional routing/current leaves remain None.
    /// No Native, grant/key material, caller-selected namespace or raw bytes escape.
    pub fn fixed_records(&mut self, c: u8) -> Result<L1FixedRecords7> {
        self.occupied(c)?;
        read(c, |key| self.core.get(key))
    }
}

#[cfg(test)]
#[path = "g3_records_tests.rs"]
mod tests;
