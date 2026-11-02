//! Numeric authenticated root summaries only; independent of runtime/status schema.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct L1SourceCounts {
    pub lanes: [u64; 2],
    pub projections: u64,
    pub requests: u64,
}
fn read(roots: &[Summary], c: u8) -> Result<L1SourceCounts> {
    let count = |ns| -> Result<u64> {
        let slot = slot(c, ns)?;
        Ok(roots
            .get(slot)
            .ok_or(Error::Bound("source root slot"))?
            .count)
    };
    Ok(L1SourceCounts {
        lanes: [count(Namespace::LaneA)?, count(Namespace::LaneB)?],
        projections: count(Namespace::Projection)?,
        requests: count(Namespace::Request)?,
    })
}
fn slot(c: u8, ns: Namespace) -> Result<usize> {
    if ns.numeric() {
        Key::ordinal(c, ns, 0)?.slot()
    } else {
        Key::hash(c, ns, [0; 32])?.slot()
    }
}
impl L1Repository {
    pub fn source_counts(&self, c: u8) -> Result<L1SourceCounts> {
        self.occupied(c)?;
        read(&self.core.root.roots, c)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s54_p11a5_source_counts_use_actual_namespace_root_slots() -> Result<()> {
        let mut roots = [Summary::default(); ROOTS];
        // Seed the actual format5 Root summary layout independently of read/slot:
        // context stride24, fixed namespaces LaneA1/LaneB2/Projection3/Request4.
        for (index, value) in [
            (1, 641),
            (2, 642),
            (3, 1280),
            (4, 640),
            (25, 642),
            (26, 643),
            (27, 1281),
            (28, 641),
            (0, 9999),
            (24, 9999),
        ] {
            roots[index].count = value;
        }
        assert_eq!(
            read(&roots, 0)?,
            L1SourceCounts {
                lanes: [641, 642],
                projections: 1280,
                requests: 640
            }
        );
        assert_eq!(
            read(&roots, 1)?,
            L1SourceCounts {
                lanes: [642, 643],
                projections: 1281,
                requests: 641
            }
        );
        assert!(read(&roots, 2).is_err());
        assert!(read(&roots[..2], 1).is_err());
        Ok(())
    }
}
