//! Format5 compressed binary radix tree. Digests cover canonical plaintext; AAD covers addresses.
use super::{Crypto, Error, Id, NODE_CAP, Reader, Result, Summary};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy)]
pub(super) enum Body {
    Leaf {
        hash: Id,
        length: u64,
        charge: u64,
        reserved: u64,
        owner: u8,
    },
    Branch {
        left: Summary,
        right: Summary,
    },
}
#[derive(Clone)]
pub(super) struct Node {
    pub prefix: Id,
    pub bits: u16,
    pub body: Body,
}
pub(super) fn bit(key: Id, at: u16) -> bool {
    key[usize::from(at / 8)] & (0x80 >> (at % 8)) != 0
}
pub(super) fn prefix(mut key: Id, bits: u16) -> Id {
    profiled!(Prefix, {
        for at in bits..256 {
            key[usize::from(at / 8)] &= !(0x80 >> (at % 8));
        }
        key
    })
}
fn common(a: Id, b: Id) -> u16 {
    profiled!(
        Common,
        (0..256)
            .find(|at| bit(a, *at) != bit(b, *at))
            .unwrap_or(256)
    )
}
pub(super) fn child(parent: &Node, right: bool) -> (Id, u16) {
    let mut p = parent.prefix;
    if right {
        p[usize::from(parent.bits / 8)] |= 0x80 >> (parent.bits % 8);
    }
    (p, parent.bits + 1)
}
impl Node {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(200);
        out.extend(self.bits.to_be_bytes());
        out.extend(self.prefix);
        match self.body {
            Body::Leaf {
                hash,
                length,
                charge,
                reserved,
                owner,
            } => {
                out.push(0);
                out.extend(hash);
                out.extend(length.to_be_bytes());
                out.extend(charge.to_be_bytes());
                out.extend(reserved.to_be_bytes());
                out.push(owner);
            }
            Body::Branch { left, right } => {
                out.push(1);
                left.encode(&mut out);
                right.encode(&mut out);
            }
        }
        out
    }
    fn decode(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw);
        let bits = r.u16()?;
        let p = r.id()?;
        if bits > 256 || prefix(p, bits) != p {
            return Err(Error::Integrity("node prefix"));
        }
        let body = match r.byte()? {
            0 if bits == 256 => {
                let hash = r.id()?;
                let length = r.u64()?;
                let charge = r.u64()?;
                let reserved = r.u64()?;
                let owner = r.byte()?;
                if owner > 1
                    || length > super::MAX_VALUE as u64
                    || charge != length + 41 + 32 + 2 * NODE_CAP as u64
                {
                    return Err(Error::Integrity("leaf bounds"));
                }
                Body::Leaf {
                    hash,
                    length,
                    charge,
                    reserved,
                    owner,
                }
            }
            1 if bits < 256 => {
                let left = Summary::decode(&mut r)?;
                let right = Summary::decode(&mut r)?;
                if left.count == 0 || right.count == 0 {
                    return Err(Error::Integrity("empty branch"));
                }
                Body::Branch { left, right }
            }
            _ => return Err(Error::Integrity("node kind")),
        };
        r.end()?;
        Ok(Self {
            prefix: p,
            bits,
            body,
        })
    }
    fn summary(&self, crypto: &Crypto, scope: u8) -> Result<Summary> {
        let mut s = Summary {
            digest: crypto.hash(scope, b"node", &self.encode()),
            ..Summary::default()
        };
        match self.body {
            Body::Leaf {
                charge,
                reserved,
                owner,
                ..
            } => {
                s.count = 1;
                s.used[usize::from(owner)] = charge;
                s.reserved[usize::from(owner)] = reserved;
            }
            Body::Branch { left, right } => {
                s.count = left
                    .count
                    .checked_add(right.count)
                    .ok_or(Error::Bound("node count"))?;
                for c in 0..2 {
                    s.used[c] = left.used[c]
                        .checked_add(right.used[c])
                        .ok_or(Error::Bound("used"))?;
                    s.reserved[c] = left.reserved[c]
                        .checked_add(right.reserved[c])
                        .ok_or(Error::Bound("reserved"))?;
                }
            }
        }
        Ok(s)
    }
}

/// A bounded operation overlay, never a cache of the whole persistent tree.
pub(super) struct Tree<'a, T: redb::ReadableTable<&'static [u8], &'static [u8]>> {
    pub table: &'a T,
    pub crypto: &'a Crypto,
    pub changed: BTreeMap<Id, Option<Vec<u8>>>,
    // Verified nodes only, scoped to this operation/read transaction. No history cache.
    decoded: BTreeMap<Id, (Node, Summary)>,
    // Only a short history read enables this fixed FIFO. Full candidate proofs
    // still clear; a reused node must match scope and the expected proof version.
    read_reuse: bool,
    reused: BTreeMap<Id, (u8, Node, Summary)>,
    reuse_fifo: VecDeque<Id>,
    // Trusted new nodes already live in decoded. The changed map reserves their
    // exact envelope lengths; only final versions are sealed before publication.
    pending_scopes: BTreeMap<Id, u8>,
    pub reads: u64,
    pub plan_bytes: usize,
}
impl<'a, T: redb::ReadableTable<&'static [u8], &'static [u8]>> Tree<'a, T> {
    // Read-only history pages discard proofs between candidates; write overlays remain intact.
    pub fn clear_read_proofs(&mut self) {
        self.decoded
            .retain(|address, _| self.pending_scopes.contains_key(address));
    }
    pub fn enable_read_reuse(&mut self) {
        self.read_reuse = true;
    }
    #[cfg(test)]
    pub fn read_reuse_sizes(&self) -> (usize, usize) {
        (self.reused.len(), self.reuse_bound_bytes())
    }
    fn reuse_bound_bytes(&self) -> usize {
        (self.reused.len() + 11) * 3 * (std::mem::size_of::<(Id, (u8, Node, Summary))>() + 128)
            + self.reuse_fifo.capacity() * 32
    }
    fn reuse(&mut self, address: Id, scope: u8, node: Node, summary: Summary) {
        if !self.read_reuse || self.reused.contains_key(&address) {
            return;
        }
        while self.reused.len() >= 512 || self.reuse_bound_bytes() + 4096 > 1024 * 1024 {
            if let Some(old) = self.reuse_fifo.pop_front() {
                self.reused.remove(&old);
            } else {
                break;
            }
        }
        self.reused.insert(address, (scope, node, summary));
        self.reuse_fifo.push_back(address);
    }
    /// Conservative bound from live lengths/capacities; includes sparse BTree nodes.
    pub fn buffer_bound_bytes(&self) -> usize {
        let decoded =
            (self.decoded.len() + 11) * 3 * (std::mem::size_of::<(Id, (Node, Summary))>() + 128);
        let changed =
            (self.changed.len() + 11) * 3 * (std::mem::size_of::<(Id, Option<Vec<u8>>)>() + 128)
                + self
                    .changed
                    .values()
                    .flatten()
                    .map(Vec::capacity)
                    .sum::<usize>();
        decoded + changed + self.reuse_bound_bytes() + (self.pending_scopes.len() + 11) * 3 * 192
    }
    pub fn new(table: &'a T, crypto: &'a Crypto) -> Self {
        Self {
            table,
            crypto,
            changed: BTreeMap::new(),
            decoded: BTreeMap::new(),
            read_reuse: false,
            reused: BTreeMap::new(),
            reuse_fifo: VecDeque::new(),
            pending_scopes: BTreeMap::new(),
            reads: 0,
            plan_bytes: 0,
        }
    }
    /// Isolated scratch overlay for Root7's digest excluding its own checkpoints.
    /// Normal append/range/allocator paths do not use or mutate this fork.
    pub fn fork_for_digest(&self) -> Self {
        let mut result = Self::new(self.table, self.crypto);
        result.changed = self.changed.clone();
        result.decoded = self.decoded.clone();
        result.pending_scopes = self.pending_scopes.clone();
        result.plan_bytes = self.plan_bytes;
        result
    }
    fn address(&self, scope: u8, p: Id, bits: u16) -> Id {
        let mut bytes = Vec::with_capacity(34);
        bytes.extend(bits.to_be_bytes());
        bytes.extend(p);
        self.crypto.address(scope, b"node", &bytes)
    }
    pub fn load(&mut self, scope: u8, entrance: (Id, u16), expected: Summary) -> Result<Node> {
        if expected.count == 0 {
            return Err(Error::Integrity("empty expected node"));
        }
        let address = self.address(scope, entrance.0, entrance.1);
        if let Some((node, summary)) = self.decoded.get(&address) {
            if *summary != expected {
                return Err(Error::Integrity("cached node proof"));
            }
            return Ok(node.clone());
        }
        if let Some((proved_scope, node, summary)) = self.reused.get(&address).cloned() {
            if proved_scope != scope || summary != expected {
                return Err(Error::Integrity("reused node proof/scope"));
            }
            self.remember(address, node.clone(), summary)?;
            return Ok(node);
        }
        let raw = if let Some(c) = self.changed.get(&address) {
            c.clone().ok_or(Error::Integrity("removed expected node"))?
        } else {
            self.reads += 1;
            let guard = self
                .table
                .get(address.as_slice())
                .map_err(super::db)?
                .ok_or(Error::Integrity("missing expected node"))?;
            if guard.value().len() > NODE_CAP {
                return Err(Error::Integrity("node envelope cap"));
            }
            guard.value().to_vec()
        };
        let plain = self.crypto.open_node(scope, address, &raw)?;
        let n = profiled!(
            NodeProof,
            (|| {
                let n = Node::decode(&plain)?;
                if n.bits < entrance.1
                    || prefix(n.prefix, entrance.1) != entrance.0
                    || n.summary(self.crypto, scope)? != expected
                {
                    return Err(Error::Integrity("node proof"));
                }
                Ok(n)
            })()
        )?;
        self.remember(address, n.clone(), expected)?;
        self.reuse(address, scope, n.clone(), expected);
        Ok(n)
    }
    fn remember(&mut self, address: Id, node: Node, summary: Summary) -> Result<()> {
        let per_entry = std::mem::size_of::<(Id, Node, Summary)>() + 64;
        if !self.decoded.contains_key(&address)
            && self
                .decoded
                .len()
                .checked_add(1)
                .and_then(|n| n.checked_mul(per_entry))
                .is_none_or(|v| v > super::MAX_PLAN)
        {
            return Err(Error::Bound("operation proof cache"));
        }
        self.decoded.insert(address, (node, summary));
        Ok(())
    }
    fn change(&mut self, address: Id, value: Option<Vec<u8>>) -> Result<()> {
        self.read_reuse = false;
        self.reused.clear();
        self.reuse_fifo.clear();
        self.pending_scopes.remove(&address);
        self.decoded.remove(&address);
        if !self.changed.contains_key(&address) {
            let old = self.table.get(address.as_slice()).map_err(super::db)?;
            self.plan_bytes = self
                .plan_bytes
                .checked_add(old.as_ref().map_or(0, |v| v.value().len()) + 32)
                .ok_or(Error::Bound("plan"))?;
        } else if let Some(Some(old)) = self.changed.get(&address) {
            self.plan_bytes = self.plan_bytes.saturating_sub(old.len());
        }
        self.plan_bytes = self
            .plan_bytes
            .checked_add(value.as_ref().map_or(0, Vec::len))
            .ok_or(Error::Bound("plan"))?;
        if self.plan_bytes > super::MAX_PLAN {
            return Err(Error::Bound("node plan"));
        }
        self.changed.insert(address, value);
        Ok(())
    }
    fn store(&mut self, scope: u8, entrance: (Id, u16), node: Node) -> Result<Summary> {
        let summary = node.summary(self.crypto, scope)?;
        let address = self.address(scope, entrance.0, entrance.1);
        let envelope_len = node.encode().len() + 41;
        if envelope_len > NODE_CAP {
            return Err(Error::Bound("node envelope"));
        }
        self.change(address, Some(vec![0; envelope_len]))?;
        self.pending_scopes.insert(address, scope);
        self.remember(address, node, summary)?;
        Ok(summary)
    }
    /// No placeholder can reach a database/plan: the common engine calls this
    /// before reading changed node bytes. Persisted/old nodes always use AEAD open.
    pub fn seal_changes(&mut self) -> Result<()> {
        for (address, scope) in &self.pending_scopes {
            let (node, _) = self
                .decoded
                .get(address)
                .ok_or(Error::Integrity("pending node missing"))?;
            let raw = self.crypto.seal_node(*scope, *address, &node.encode())?;
            let slot = self
                .changed
                .get_mut(address)
                .ok_or(Error::Integrity("pending node change missing"))?;
            if slot.as_ref().is_none_or(|old| old.len() != raw.len()) || raw.len() > NODE_CAP {
                return Err(Error::Integrity("pending node envelope bound"));
            }
            *slot = Some(raw);
        }
        Ok(())
    }
    pub fn find(&mut self, scope: u8, root: Summary, key: Id) -> Result<Option<Node>> {
        profiled!(TreeFind, self.find_inner(scope, root, key))
    }
    fn find_inner(&mut self, scope: u8, root: Summary, key: Id) -> Result<Option<Node>> {
        let mut cursor = (([0; 32], 0), root);
        while cursor.1.count != 0 {
            let n = self.load(scope, cursor.0, cursor.1)?;
            if prefix(key, n.bits) != n.prefix {
                return Ok(None);
            }
            match n.body {
                Body::Leaf { .. } => return Ok(Some(n)),
                Body::Branch { left, right } => {
                    let b = bit(key, n.bits);
                    cursor = (child(&n, b), if b { right } else { left });
                }
            }
        }
        if cursor.1 != Summary::default() {
            return Err(Error::Integrity("noncanonical empty root"));
        }
        Ok(None)
    }
    pub fn put(&mut self, scope: u8, root: Summary, leaf: Node) -> Result<Summary> {
        self.insert(scope, ([0; 32], 0), root, leaf, 0)
    }
    fn insert(
        &mut self,
        scope: u8,
        ent: (Id, u16),
        summary: Summary,
        leaf: Node,
        depth: u16,
    ) -> Result<Summary> {
        if depth > 256 {
            return Err(Error::Integrity("tree depth"));
        }
        if summary.count == 0 {
            return self.store(scope, ent, leaf);
        }
        let mut old = self.load(scope, ent, summary)?;
        let common = common(old.prefix, leaf.prefix).min(old.bits);
        if common < old.bits {
            let parent = Node {
                prefix: prefix(leaf.prefix, common),
                bits: common,
                body: Body::Branch {
                    left: Summary::default(),
                    right: Summary::default(),
                },
            };
            let new_right = bit(leaf.prefix, common);
            let a = self.store(scope, child(&parent, !new_right), old)?;
            let b = self.store(scope, child(&parent, new_right), leaf)?;
            return self.store(
                scope,
                ent,
                Node {
                    body: Body::Branch {
                        left: if new_right { a } else { b },
                        right: if new_right { b } else { a },
                    },
                    ..parent
                },
            );
        }
        if old.bits == 256 {
            return self.store(scope, ent, leaf);
        }
        let Body::Branch {
            mut left,
            mut right,
        } = old.body
        else {
            return Err(Error::Integrity("branch"));
        };
        let b = bit(leaf.prefix, old.bits);
        let next = self.insert(
            scope,
            child(&old, b),
            if b { right } else { left },
            leaf,
            depth + 1,
        )?;
        if b {
            right = next
        } else {
            left = next
        };
        old.body = Body::Branch { left, right };
        self.store(scope, ent, old)
    }
    pub fn remove(&mut self, scope: u8, root: Summary, key: Id) -> Result<Summary> {
        self.delete(scope, ([0; 32], 0), root, key, 0)
    }
    fn delete(
        &mut self,
        scope: u8,
        ent: (Id, u16),
        summary: Summary,
        key: Id,
        depth: u16,
    ) -> Result<Summary> {
        if depth > 256 {
            return Err(Error::Integrity("tree depth"));
        }
        let mut n = self.load(scope, ent, summary)?;
        if prefix(key, n.bits) != n.prefix {
            return Err(Error::Integrity("retire missing leaf"));
        }
        if n.bits == 256 {
            self.change(self.address(scope, ent.0, ent.1), None)?;
            return Ok(Summary::default());
        }
        let Body::Branch {
            mut left,
            mut right,
        } = n.body
        else {
            return Err(Error::Integrity("delete branch"));
        };
        let b = bit(key, n.bits);
        let changed = self.delete(
            scope,
            child(&n, b),
            if b { right } else { left },
            key,
            depth + 1,
        )?;
        if changed.count == 0 {
            let survivor_ent = child(&n, !b);
            let survivor = if b { left } else { right };
            let promoted = self.load(scope, survivor_ent, survivor)?;
            self.change(self.address(scope, survivor_ent.0, survivor_ent.1), None)?;
            return self.store(scope, ent, promoted);
        }
        if b {
            right = changed
        } else {
            left = changed
        };
        n.body = Body::Branch { left, right };
        self.store(scope, ent, n)
    }
    pub fn range(
        &mut self,
        scope: u8,
        root: Summary,
        start: Id,
        limit: usize,
    ) -> Result<Vec<Node>> {
        let mut out = Vec::with_capacity(limit);
        let mut stack = vec![(([0; 32], 0), root)];
        while let Some((ent, s)) = stack.pop() {
            if s.count == 0 {
                continue;
            }
            let n = self.load(scope, ent, s)?;
            // Skip only after authenticating this subtree's complete key interval.
            let mut max = n.prefix;
            for at in n.bits..256 {
                max[usize::from(at / 8)] |= 0x80 >> (at % 8);
            }
            if max < start {
                continue;
            }
            match n.body {
                Body::Leaf { .. } => {
                    out.push(n);
                    if out.len() == limit {
                        break;
                    }
                }
                Body::Branch { left, right } => {
                    stack.push((child(&n, true), right));
                    stack.push((child(&n, false), left));
                }
            }
        }
        Ok(out)
    }
}

pub(super) fn leaf(key: Id, hash: Id, length: u64, owner: u8, reserved: u64) -> Node {
    Node {
        prefix: key,
        bits: 256,
        body: Body::Leaf {
            hash,
            length,
            charge: length + 41 + 32 + 2 * NODE_CAP as u64,
            reserved,
            owner,
        },
    }
}
