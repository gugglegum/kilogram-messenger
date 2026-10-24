//! A single Work's authenticated values; never shared across transactions.
use super::{Error, Id, Result, Summary, tree};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

pub(super) const ENTRY_CAP: usize = 128;
pub(super) const BYTE_CAP: usize = 1024 * 1024;
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Version {
    pub hash: Id,
    pub length: u64,
    pub reserved: u64,
    pub owner: u8,
}
impl Version {
    pub fn of(node: &tree::Node) -> Result<Self> {
        match node.body {
            tree::Body::Leaf {
                hash,
                length,
                reserved,
                owner,
                ..
            } => Ok(Self {
                hash,
                length,
                reserved,
                owner,
            }),
            _ => Err(Error::Integrity("cached value leaf")),
        }
    }
}
struct Entry {
    root: Summary,
    version: Option<Version>,
    plain: Option<Zeroizing<Vec<u8>>>,
}
#[derive(Default)]
pub(super) struct VerifiedValues {
    entries: BTreeMap<(u8, Id), Entry>,
    bytes: usize,
}
impl VerifiedValues {
    pub fn stamp(&self, scope: u8, key: Id) -> Option<(Summary, Option<Version>)> {
        self.entries.get(&(scope, key)).map(|e| (e.root, e.version))
    }
    pub fn copy(&mut self, scope: u8, key: Id, root: Summary) -> Result<Option<Vec<u8>>> {
        let entry = self
            .entries
            .get_mut(&(scope, key))
            .ok_or(Error::Integrity("value cache entry"))?;
        entry.root = root;
        Ok(entry.plain.as_ref().map(|p| p.to_vec()))
    }
    pub fn remove(&mut self, scope: u8, key: Id) {
        if let Some(entry) = self.entries.remove(&(scope, key)) {
            self.bytes -= entry.plain.as_ref().map_or(0, |p| p.capacity());
        }
    }
    pub fn insert(
        &mut self,
        scope: u8,
        key: Id,
        root: Summary,
        version: Option<Version>,
        plain: Option<Zeroizing<Vec<u8>>>,
    ) {
        self.remove(scope, key);
        let bytes = plain.as_ref().map_or(0, |p| p.capacity());
        if bytes > BYTE_CAP {
            return;
        }
        if self.entries.len() >= ENTRY_CAP || self.bytes + bytes > BYTE_CAP {
            self.entries.clear();
            self.bytes = 0;
        }
        self.bytes += bytes;
        self.entries.insert(
            (scope, key),
            Entry {
                root,
                version,
                plain,
            },
        );
    }
    pub fn buffer_bound_bytes(&self) -> usize {
        self.bytes
            + (self.entries.len() + 11) * 3 * (std::mem::size_of::<((u8, Id), Entry)>() + 128)
    }
    #[cfg(test)]
    pub fn sizes(&self) -> (usize, usize) {
        (self.entries.len(), self.bytes)
    }
}
