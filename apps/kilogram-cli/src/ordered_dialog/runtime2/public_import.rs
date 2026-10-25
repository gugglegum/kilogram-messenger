//! One bounded local public-artifact transfer. Each authenticated IPC request fits 32 KiB.
use super::*;
use owner::Owner2;
use std::time::{Duration, Instant};
pub(super) struct PublicImport2 {
    pub id: Id,
    expected: Id,
    length: usize,
    bytes: Vec<u8>,
    deadline: Instant,
}
impl Owner2 {
    pub(super) fn expire_public_import(&mut self) {
        if self
            .public_import
            .as_ref()
            .is_some_and(|p| Instant::now() >= p.deadline)
        {
            self.public_import = None;
        }
    }
    pub(super) fn public_import_begin(&mut self, length: usize, expected: Id) -> Result<Id> {
        ensure!((1..=96 * 1024).contains(&length), "context artifact cap");
        ensure!(
            self.public_import
                .as_ref()
                .is_none_or(|p| Instant::now() >= p.deadline),
            "public artifact transfer busy"
        );
        let id = shared_admin::random()?;
        self.public_import = Some(PublicImport2 {
            id,
            expected,
            length,
            bytes: Vec::with_capacity(length),
            deadline: Instant::now() + Duration::from_secs(30),
        });
        Ok(id)
    }
    pub(super) fn public_import_part(&mut self, id: Id, offset: usize, bytes: &[u8]) -> Result<()> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= 16 * 1024,
            "public artifact chunk cap"
        );
        let p = self
            .public_import
            .as_mut()
            .context("public artifact transfer missing")?;
        ensure!(
            p.id == id
                && Instant::now() < p.deadline
                && p.bytes.len() == offset
                && offset
                    .checked_add(bytes.len())
                    .is_some_and(|n| n <= p.length),
            "public artifact chunk order/deadline"
        );
        p.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub(super) fn public_import_finish(&mut self, id: Id) -> Result<()> {
        let p = self
            .public_import
            .take()
            .context("public artifact transfer missing")?;
        ensure!(
            p.id == id
                && Instant::now() < p.deadline
                && p.bytes.len() == p.length
                && wire::hash(b"", &p.bytes) == p.expected,
            "public artifact complete/hash/deadline"
        );
        let before = self.repository.generation();
        admin::install(&mut self.repository, &p.bytes)?;
        if self.repository.generation() != before {
            self.invalidate();
        }
        Ok(())
    }
}
