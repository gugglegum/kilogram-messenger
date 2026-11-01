//! Read-only validation of an actual normally finalized decoder/ledger stream.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct FinalObservation {
    pub key: oracle::Incarnation,
    pub sequence: u64,
    pub counts: oracle::Counts,
}
impl Decoder {
    pub fn normal_identity(&self) -> Result<oracle::Incarnation> {
        self.key
            .context("actual started normal stream, including its retained final")
    }
    pub fn normal_final(
        &self,
        ops: &oracle::Operations,
        key: oracle::Incarnation,
        eof: u64,
    ) -> Result<FinalObservation> {
        let final_record = self
            .final_record
            .context("actual normal final record is required")?;
        ensure!(
            self.final_seen
                && self.key == Some(key)
                && final_record.key == key
                && final_record.sequence == self.sequence
                && self.fragment.is_empty()
                && self.offset == eof,
            "normal final owned identity/sequence and complete stable post-wait EOF"
        );
        ops.closed(key, final_record.sequence, final_record.counts)?;
        Ok(final_record)
    }
    pub fn close_normal(
        &self,
        ops: &oracle::Operations,
        key: oracle::Incarnation,
        start: (u64, u64),
        end: &source::End,
        eof: u64,
    ) -> Result<Value> {
        let actual = self.normal_final(ops, key, eof)?;
        ensure!(
            end.point
                == (
                    start
                        .0
                        .checked_add(actual.counts.encrypt)
                        .context("normal own source overflow")?,
                    start
                        .1
                        .checked_add(actual.counts.decrypt)
                        .context("normal peer source overflow")?
                ),
            "normal source frontier must exactly match actual committed final successes"
        );
        if let Some(packet) = self.last_packet {
            ensure!(
                packet.sequence == actual.sequence
                    && end.generation >= packet.generation
                    && (end.generation != packet.generation || end.operation == packet.operation),
                "stopped root compatible with last native operation, later non-native commits allowed"
            );
        } else {
            ensure!(
                actual.sequence == 0 && actual.counts.encrypt == 0 && actual.counts.decrypt == 0,
                "zero-work normal stream has no fabricated packet"
            );
        }
        Ok(
            json!({"pid":key.pid,"source":state::hex(&key.owner),"incarnation":state::hex(&key.nonce),"sequence":actual.sequence,"eof":eof,"source_point":[end.point.0,end.point.1],"root_generation":end.generation,"last_commit":state::hex(&end.operation),"last_native":self.last_packet.map(|p|json!({"context":p.context,"operation":state::hex(&p.operation),"generation":p.generation,"sequence":p.sequence,"encrypt_delta":p.counts.encrypt,"decrypt_delta":p.counts.decrypt,"attempts_delta":p.counts.attempts,"abort_delta":p.counts.aborted})),"final_counts":{"encrypt":actual.counts.encrypt,"decrypt":actual.counts.decrypt,"attempts":actual.counts.attempts,"aborted":actual.counts.aborted,"ambiguous":actual.counts.ambiguous,"accounts":actual.counts.accounts,"outbound":actual.counts.outbound,"inbound":actual.counts.inbound},"classification":"read-only normal final/source reconciliation, no second finish"}),
        )
    }
}

#[cfg(test)]
#[path = "migration_s54_normal_decoder_tests.rs"]
mod tests;
