//! Durable responder-side journal for dormant canonical sync v2.
//!
//! Responder checkpoints use their own signed protocol format and their own
//! redb tables. They must never be decoded as initiator checkpoints: the two
//! state machines have different actions and different signature domains.

use kilogram_protocol::{
    canonical_sync_responder_session_v2::{
        CanonicalSignedSyncResponderCheckpointV2, CanonicalSyncResponderNextActionV2,
        decode_and_verify_signed_sync_responder_checkpoint_v2,
        encode_signed_sync_responder_checkpoint_v2, sync_responder_checkpoint_id_v2,
        verify_signed_sync_responder_checkpoint_v2, verify_sync_responder_checkpoint_successor_v2,
    },
    canonical_sync_v2::{
        CanonicalSyncServerFrameV2, decode_and_verify_sync_server_frame_v2,
        encode_sync_server_frame_v2,
    },
};

use super::*;

pub(super) const CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("canonical-sync-v2-responder-checkpoints-v1");
pub(super) const CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_HEAD_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("canonical-sync-v2-responder-checkpoint-heads-v1");
pub(super) const CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("canonical-sync-v2-responder-runtime-v1");

const RESPONDER_CHECKPOINT_RECORD_VERSION: u8 = 1;
const RESPONDER_CHECKPOINT_RECORD_HEADER_BYTES: usize = 65;
const RESPONDER_RUNTIME_RECORD_VERSION: u8 = 1;
const RESPONDER_RUNTIME_RECORD_HEADER_BYTES: usize = 34;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2StoredResponderCheckpoint {
    import_plan_id: [u8; 32],
    checkpoint_id: [u8; 32],
    checkpoint: CanonicalSignedSyncResponderCheckpointV2,
}

impl CanonicalSyncV2StoredResponderCheckpoint {
    pub fn import_plan_id(&self) -> [u8; 32] {
        self.import_plan_id
    }

    pub fn checkpoint_id(&self) -> [u8; 32] {
        self.checkpoint_id
    }

    pub fn checkpoint(&self) -> &CanonicalSignedSyncResponderCheckpointV2 {
        &self.checkpoint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSyncV2StoredResponderRuntime {
    checkpoint: CanonicalSyncV2StoredResponderCheckpoint,
    pending_server_frame: Option<CanonicalSyncServerFrameV2>,
}

impl CanonicalSyncV2StoredResponderRuntime {
    pub fn checkpoint(&self) -> &CanonicalSyncV2StoredResponderCheckpoint {
        &self.checkpoint
    }

    pub fn pending_server_frame(&self) -> Option<&CanonicalSyncServerFrameV2> {
        self.pending_server_frame.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResponderRuntimeRecord {
    checkpoint_id: [u8; 32],
    pending_server_frame: Option<CanonicalSyncServerFrameV2>,
}

impl CanonicalSyncV2CarrierStore {
    pub fn responder_checkpoint_chain(
        &self,
        conversation_id: [u8; 32],
        responder_device_id: [u8; 32],
        requester_device_id: [u8; 32],
        session_binding: [u8; 32],
    ) -> Result<Vec<CanonicalSyncV2StoredResponderCheckpoint>, StoreError> {
        let identity_key = canonical_sync_v2_session_identity_key(
            conversation_id,
            responder_device_id,
            requester_device_id,
            session_binding,
        );
        let read = self
            .database
            .begin_read()
            .map_err(canonical_sync_v2_database_error)?;
        let head = {
            let table = read
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_session_checkpoint_head(value.value()))
                .transpose()?
        };
        let Some(head) = head else {
            return Ok(Vec::new());
        };
        if head.checkpoint_sequence > MAX_CANONICAL_SYNC_V2_SESSION_CHECKPOINTS {
            return Err(StoreError::CanonicalSyncV2SessionCheckpointChainTooLong {
                actual: head.checkpoint_sequence,
                maximum: MAX_CANONICAL_SYNC_V2_SESSION_CHECKPOINTS,
            });
        }

        let table = read
            .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE)
            .map_err(canonical_sync_v2_database_error)?;
        let mut chain: Vec<CanonicalSyncV2StoredResponderCheckpoint> =
            Vec::with_capacity(head.checkpoint_sequence as usize);
        for checkpoint_sequence in 1..=head.checkpoint_sequence {
            let record_key =
                canonical_sync_v2_session_checkpoint_key(identity_key, checkpoint_sequence);
            let record = table
                .get(record_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_responder_checkpoint_record(value.value()))
                .transpose()?
                .ok_or(
                    StoreError::CanonicalSyncV2SessionCheckpointChainMissingRecord {
                        checkpoint_sequence,
                    },
                )?;
            let record_head = CanonicalSyncV2SessionCheckpointHead {
                import_plan_id: head.import_plan_id,
                checkpoint_sequence,
                checkpoint_id: record.checkpoint_id,
            };
            validate_responder_checkpoint_record(&record, identity_key, record_head)?;
            if let Some(previous) = chain.last() {
                verify_sync_responder_checkpoint_successor_v2(
                    previous.checkpoint(),
                    record.checkpoint(),
                )?;
            } else {
                validate_initial_responder_checkpoint(record.checkpoint())?;
            }
            chain.push(record);
        }
        let restored = chain
            .last()
            .ok_or(StoreError::CanonicalSyncV2SessionCheckpointHeadMissingRecord)?;
        if restored.import_plan_id != head.import_plan_id
            || restored.checkpoint_id != head.checkpoint_id
            || restored.checkpoint.content.checkpoint_sequence != head.checkpoint_sequence
        {
            return Err(StoreError::CanonicalSyncV2SessionCheckpointRecordMismatch);
        }
        Ok(chain)
    }

    pub fn responder_runtime_state(
        &self,
        conversation_id: [u8; 32],
        responder_device_id: [u8; 32],
        requester_device_id: [u8; 32],
        session_binding: [u8; 32],
    ) -> Result<Option<CanonicalSyncV2StoredResponderRuntime>, StoreError> {
        let mut chain = self.responder_checkpoint_chain(
            conversation_id,
            responder_device_id,
            requester_device_id,
            session_binding,
        )?;
        let identity_key = canonical_sync_v2_session_identity_key(
            conversation_id,
            responder_device_id,
            requester_device_id,
            session_binding,
        );
        let read = self
            .database
            .begin_read()
            .map_err(canonical_sync_v2_database_error)?;
        let runtime = {
            let table = read
                .open_table(CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_responder_runtime_record(value.value()))
                .transpose()?
        };
        match (chain.pop(), runtime) {
            (None, None) => Ok(None),
            (None, Some(_)) => Err(StoreError::CanonicalSyncV2SessionRuntimeOrphanRecord),
            (Some(_), None) => Err(StoreError::CanonicalSyncV2SessionRuntimeMissing),
            (Some(checkpoint), Some(runtime)) => {
                validate_responder_runtime_record(&runtime, &checkpoint)?;
                Ok(Some(CanonicalSyncV2StoredResponderRuntime {
                    checkpoint,
                    pending_server_frame: runtime.pending_server_frame,
                }))
            }
        }
    }

    pub fn install_legacy_import_responder_bootstrap(
        &self,
        plan: &CanonicalSyncV2LegacyImportPlan,
        checkpoint: &CanonicalSignedSyncResponderCheckpointV2,
    ) -> Result<CanonicalSyncV2StoredResponderCheckpoint, StoreError> {
        plan.verify()?;
        validate_initial_responder_checkpoint(checkpoint)?;
        if checkpoint.content.conversation_id != *plan.conversation_id.as_bytes() {
            return Err(StoreError::CanonicalSyncV2SessionBootstrapConversationMismatch);
        }
        let checkpoint_id = sync_responder_checkpoint_id_v2(checkpoint)?;
        let stored = CanonicalSyncV2StoredResponderCheckpoint {
            import_plan_id: plan.plan_id,
            checkpoint_id,
            checkpoint: checkpoint.clone(),
        };
        let encoded_record = encode_responder_checkpoint_record(&stored)?;
        let content = &checkpoint.content;
        let identity_key = canonical_sync_v2_session_identity_key(
            content.conversation_id,
            content.responder_device_id,
            content.requester_device_id,
            content.session_binding,
        );
        let record_key = canonical_sync_v2_session_checkpoint_key(identity_key, 1);
        let head = CanonicalSyncV2SessionCheckpointHead {
            import_plan_id: plan.plan_id,
            checkpoint_sequence: 1,
            checkpoint_id,
        };
        let encoded_head = encode_canonical_sync_v2_session_checkpoint_head(head);
        let runtime = ResponderRuntimeRecord {
            checkpoint_id,
            pending_server_frame: None,
        };
        let encoded_runtime = encode_responder_runtime_record(&runtime)?;

        let mut write = self
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        let import_checkpoint = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_LEGACY_IMPORT_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            let import_key = canonical_sync_v2_legacy_import_checkpoint_key(plan);
            table
                .get(import_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_legacy_import_checkpoint(value.value()))
                .transpose()?
                .ok_or(StoreError::CanonicalSyncV2SessionBootstrapImportMissing)?
        };
        import_checkpoint.verify_for_plan(plan)?;
        if !import_checkpoint.is_complete() {
            return Err(
                StoreError::CanonicalSyncV2SessionBootstrapImportIncomplete {
                    next_index: import_checkpoint.next_index,
                    event_count: import_checkpoint.total_event_count,
                },
            );
        }

        let existing_head = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_session_checkpoint_head(value.value()))
                .transpose()?
        };
        if let Some(existing_head) = existing_head {
            if existing_head != head {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointHeadConflict);
            }
            let existing = {
                let table = write
                    .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE)
                    .map_err(canonical_sync_v2_database_error)?;
                table
                    .get(record_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| decode_responder_checkpoint_record(value.value()))
                    .transpose()?
                    .ok_or(StoreError::CanonicalSyncV2SessionCheckpointHeadMissingRecord)?
            };
            validate_responder_checkpoint_record(&existing, identity_key, existing_head)?;
            if existing != stored {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointConflict);
            }
            let existing_runtime = {
                let table = write
                    .open_table(CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE)
                    .map_err(canonical_sync_v2_database_error)?;
                table
                    .get(identity_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| decode_responder_runtime_record(value.value()))
                    .transpose()?
                    .ok_or(StoreError::CanonicalSyncV2SessionRuntimeMissing)?
            };
            validate_responder_runtime_record(&existing_runtime, &existing)?;
            return Ok(existing);
        }

        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            if table
                .get(record_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .is_some()
            {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointOrphanRecord);
            }
            table
                .insert(record_key.as_slice(), encoded_record.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(identity_key.as_slice(), encoded_head.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            if table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .is_some()
            {
                return Err(StoreError::CanonicalSyncV2SessionRuntimeOrphanRecord);
            }
            table
                .insert(identity_key.as_slice(), encoded_runtime.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)?;
        Ok(stored)
    }

    pub fn commit_responder_runtime_transition(
        &self,
        expected_runtime: &CanonicalSyncV2StoredResponderRuntime,
        successor: &CanonicalSignedSyncResponderCheckpointV2,
        pending_server_frame: Option<&CanonicalSyncServerFrameV2>,
        received_events: &[CanonicalSyncEventCarrierV2],
    ) -> Result<CanonicalSyncV2StoredResponderRuntime, StoreError> {
        verify_signed_sync_responder_checkpoint_v2(successor)?;
        let previous = expected_runtime.checkpoint();
        let previous_content = &previous.checkpoint.content;
        let identity_key = canonical_sync_v2_session_identity_key(
            previous_content.conversation_id,
            previous_content.responder_device_id,
            previous_content.requester_device_id,
            previous_content.session_binding,
        );
        let expected_head = CanonicalSyncV2SessionCheckpointHead {
            import_plan_id: previous.import_plan_id,
            checkpoint_sequence: previous_content.checkpoint_sequence,
            checkpoint_id: previous.checkpoint_id,
        };
        validate_responder_checkpoint_record(previous, identity_key, expected_head)?;
        let expected_runtime_record = ResponderRuntimeRecord {
            checkpoint_id: previous.checkpoint_id,
            pending_server_frame: expected_runtime.pending_server_frame.clone(),
        };
        validate_responder_runtime_record(&expected_runtime_record, previous)?;
        verify_sync_responder_checkpoint_successor_v2(previous.checkpoint(), successor)?;
        let content = &successor.content;
        if content.checkpoint_sequence > MAX_CANONICAL_SYNC_V2_SESSION_CHECKPOINTS {
            return Err(StoreError::CanonicalSyncV2SessionCheckpointChainTooLong {
                actual: content.checkpoint_sequence,
                maximum: MAX_CANONICAL_SYNC_V2_SESSION_CHECKPOINTS,
            });
        }
        let successor_id = sync_responder_checkpoint_id_v2(successor)?;
        let stored = CanonicalSyncV2StoredResponderCheckpoint {
            import_plan_id: previous.import_plan_id,
            checkpoint_id: successor_id,
            checkpoint: successor.clone(),
        };
        let next_head = CanonicalSyncV2SessionCheckpointHead {
            import_plan_id: previous.import_plan_id,
            checkpoint_sequence: content.checkpoint_sequence,
            checkpoint_id: successor_id,
        };
        let next_runtime = ResponderRuntimeRecord {
            checkpoint_id: successor_id,
            pending_server_frame: pending_server_frame.cloned(),
        };
        validate_responder_runtime_record(&next_runtime, &stored)?;

        let prepared =
            prepare_canonical_sync_v2_carriers(content.conversation_id, received_events)?;
        let expected_received_keys = content
            .received_event_keys
            .iter()
            .copied()
            .filter(|key| {
                previous_content
                    .received_event_keys
                    .binary_search(key)
                    .is_err()
            })
            .collect::<Vec<_>>();
        let actual_received_keys = prepared.iter().map(|(key, _)| *key).collect::<Vec<_>>();
        if actual_received_keys != expected_received_keys {
            return Err(StoreError::CanonicalSyncV2SessionRuntimeReceivedCarrierMismatch);
        }

        let encoded_record = encode_responder_checkpoint_record(&stored)?;
        let encoded_head = encode_canonical_sync_v2_session_checkpoint_head(next_head);
        let encoded_runtime = encode_responder_runtime_record(&next_runtime)?;
        let previous_key = canonical_sync_v2_session_checkpoint_key(
            identity_key,
            previous_content.checkpoint_sequence,
        );
        let successor_key =
            canonical_sync_v2_session_checkpoint_key(identity_key, content.checkpoint_sequence);

        let mut write = self
            .database
            .begin_write()
            .map_err(canonical_sync_v2_database_error)?;
        write
            .set_durability(Durability::Immediate)
            .map_err(canonical_sync_v2_database_error)?;
        let current_head = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_canonical_sync_v2_session_checkpoint_head(value.value()))
                .transpose()?
                .ok_or(StoreError::CanonicalSyncV2SessionCheckpointHeadMissing)?
        };

        if current_head == next_head {
            let existing = {
                let table = write
                    .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE)
                    .map_err(canonical_sync_v2_database_error)?;
                table
                    .get(successor_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| decode_responder_checkpoint_record(value.value()))
                    .transpose()?
                    .ok_or(StoreError::CanonicalSyncV2SessionCheckpointHeadMissingRecord)?
            };
            validate_responder_checkpoint_record(&existing, identity_key, current_head)?;
            if existing != stored {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointConflict);
            }
            let current_runtime = {
                let table = write
                    .open_table(CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE)
                    .map_err(canonical_sync_v2_database_error)?;
                table
                    .get(identity_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| decode_responder_runtime_record(value.value()))
                    .transpose()?
                    .ok_or(StoreError::CanonicalSyncV2SessionRuntimeMissing)?
            };
            validate_responder_runtime_record(&current_runtime, &existing)?;
            if current_runtime != next_runtime {
                return Err(StoreError::CanonicalSyncV2SessionRuntimeTransitionConflict);
            }
            verify_prepared_carriers_in_table(&write, content.conversation_id, &prepared)?;
            return Ok(CanonicalSyncV2StoredResponderRuntime {
                checkpoint: existing,
                pending_server_frame: next_runtime.pending_server_frame,
            });
        }
        if current_head != expected_head {
            return Err(StoreError::CanonicalSyncV2SessionRuntimeStaleCheckpoint);
        }

        let durable_previous = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(previous_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_responder_checkpoint_record(value.value()))
                .transpose()?
                .ok_or(StoreError::CanonicalSyncV2SessionCheckpointHeadMissingRecord)?
        };
        validate_responder_checkpoint_record(&durable_previous, identity_key, current_head)?;
        if durable_previous != *previous {
            return Err(StoreError::CanonicalSyncV2SessionRuntimeExpectationMismatch);
        }
        let current_runtime = {
            let table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .get(identity_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .map(|value| decode_responder_runtime_record(value.value()))
                .transpose()?
                .ok_or(StoreError::CanonicalSyncV2SessionRuntimeMissing)?
        };
        validate_responder_runtime_record(&current_runtime, &durable_previous)?;
        if current_runtime != expected_runtime_record {
            return Err(StoreError::CanonicalSyncV2SessionRuntimeExpectationMismatch);
        }

        let mut missing = Vec::new();
        {
            let table = write
                .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            for (index, (event_key, encoded)) in prepared.iter().enumerate() {
                let storage_key =
                    canonical_sync_v2_storage_key(content.conversation_id, *event_key);
                let existing = table
                    .get(storage_key.as_slice())
                    .map_err(canonical_sync_v2_database_error)?
                    .map(|value| value.value().to_vec());
                match existing {
                    Some(value) => {
                        validate_canonical_sync_v2_stored_carrier(
                            content.conversation_id,
                            *event_key,
                            &value,
                        )?;
                        if value != *encoded {
                            return Err(StoreError::CanonicalSyncV2CarrierConflict {
                                conversation_id: content.conversation_id,
                                event_key: *event_key,
                            });
                        }
                    }
                    None => missing.push(index),
                }
            }
        }
        {
            let table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            if table
                .get(successor_key.as_slice())
                .map_err(canonical_sync_v2_database_error)?
                .is_some()
            {
                return Err(StoreError::CanonicalSyncV2SessionCheckpointOrphanRecord);
            }
        }

        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            for index in missing {
                let (event_key, encoded) = &prepared[index];
                let storage_key =
                    canonical_sync_v2_storage_key(content.conversation_id, *event_key);
                table
                    .insert(storage_key.as_slice(), encoded.as_slice())
                    .map_err(canonical_sync_v2_database_error)?;
            }
        }
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(successor_key.as_slice(), encoded_record.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_CHECKPOINT_HEAD_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(identity_key.as_slice(), encoded_head.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        {
            let mut table = write
                .open_table(CANONICAL_SYNC_V2_RESPONDER_RUNTIME_TABLE)
                .map_err(canonical_sync_v2_database_error)?;
            table
                .insert(identity_key.as_slice(), encoded_runtime.as_slice())
                .map_err(canonical_sync_v2_database_error)?;
        }
        write.commit().map_err(canonical_sync_v2_database_error)?;
        Ok(CanonicalSyncV2StoredResponderRuntime {
            checkpoint: stored,
            pending_server_frame: next_runtime.pending_server_frame,
        })
    }
}

fn validate_initial_responder_checkpoint(
    checkpoint: &CanonicalSignedSyncResponderCheckpointV2,
) -> Result<(), StoreError> {
    verify_signed_sync_responder_checkpoint_v2(checkpoint)?;
    let content = &checkpoint.content;
    if content.checkpoint_sequence != 1
        || content.sync_round != 1
        || content.predecessor_checkpoint_id.is_some()
        || !content.received_event_keys.is_empty()
        || !content.sent_event_keys.is_empty()
        || content.continuation_required
        || content.next_action != CanonicalSyncResponderNextActionV2::ReceiveInventory
        || content.paused
    {
        return Err(StoreError::CanonicalSyncV2SessionBootstrapShape);
    }
    Ok(())
}

fn encode_responder_checkpoint_record(
    record: &CanonicalSyncV2StoredResponderCheckpoint,
) -> Result<Vec<u8>, StoreError> {
    verify_signed_sync_responder_checkpoint_v2(&record.checkpoint)?;
    if sync_responder_checkpoint_id_v2(&record.checkpoint)? != record.checkpoint_id {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointIdMismatch);
    }
    let checkpoint = encode_signed_sync_responder_checkpoint_v2(&record.checkpoint)?;
    let mut encoded =
        Vec::with_capacity(RESPONDER_CHECKPOINT_RECORD_HEADER_BYTES + checkpoint.len());
    encoded.push(RESPONDER_CHECKPOINT_RECORD_VERSION);
    encoded.extend_from_slice(&record.import_plan_id);
    encoded.extend_from_slice(&record.checkpoint_id);
    encoded.extend_from_slice(&checkpoint);
    Ok(encoded)
}

fn decode_responder_checkpoint_record(
    encoded: &[u8],
) -> Result<CanonicalSyncV2StoredResponderCheckpoint, StoreError> {
    let maximum =
        RESPONDER_CHECKPOINT_RECORD_HEADER_BYTES + MAX_CANONICAL_SYNC_SESSION_CHECKPOINT_BYTES_V2;
    if encoded.len() <= RESPONDER_CHECKPOINT_RECORD_HEADER_BYTES || encoded.len() > maximum {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointRecordLength {
            actual: encoded.len(),
            maximum,
        });
    }
    if encoded[0] != RESPONDER_CHECKPOINT_RECORD_VERSION {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointRecordVersion(
            encoded[0],
        ));
    }
    let import_plan_id = encoded[1..33].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionCheckpointRecordLength {
            actual: encoded.len(),
            maximum,
        }
    })?;
    let checkpoint_id = encoded[33..65].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionCheckpointRecordLength {
            actual: encoded.len(),
            maximum,
        }
    })?;
    let checkpoint = decode_and_verify_signed_sync_responder_checkpoint_v2(&encoded[65..])?;
    let record = CanonicalSyncV2StoredResponderCheckpoint {
        import_plan_id,
        checkpoint_id,
        checkpoint,
    };
    if sync_responder_checkpoint_id_v2(record.checkpoint())? != record.checkpoint_id {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointIdMismatch);
    }
    Ok(record)
}

fn encode_responder_runtime_record(
    runtime: &ResponderRuntimeRecord,
) -> Result<Vec<u8>, StoreError> {
    let frame = runtime
        .pending_server_frame
        .as_ref()
        .map(encode_sync_server_frame_v2)
        .transpose()?;
    let mut encoded = Vec::with_capacity(
        RESPONDER_RUNTIME_RECORD_HEADER_BYTES + frame.as_ref().map_or(0, Vec::len),
    );
    encoded.push(RESPONDER_RUNTIME_RECORD_VERSION);
    encoded.extend_from_slice(&runtime.checkpoint_id);
    encoded.push(u8::from(frame.is_some()));
    if let Some(frame) = frame {
        encoded.extend_from_slice(&frame);
    }
    Ok(encoded)
}

fn decode_responder_runtime_record(encoded: &[u8]) -> Result<ResponderRuntimeRecord, StoreError> {
    let maximum = RESPONDER_RUNTIME_RECORD_HEADER_BYTES + MAX_CANONICAL_SYNC_V2_OBJECT_BYTES;
    if encoded.len() < RESPONDER_RUNTIME_RECORD_HEADER_BYTES || encoded.len() > maximum {
        return Err(StoreError::CanonicalSyncV2SessionRuntimeRecordLength {
            actual: encoded.len(),
            maximum,
        });
    }
    if encoded[0] != RESPONDER_RUNTIME_RECORD_VERSION {
        return Err(StoreError::CanonicalSyncV2SessionRuntimeRecordVersion(
            encoded[0],
        ));
    }
    let checkpoint_id = encoded[1..33].try_into().map_err(|_| {
        StoreError::CanonicalSyncV2SessionRuntimeRecordLength {
            actual: encoded.len(),
            maximum,
        }
    })?;
    let pending_server_frame = match encoded[33] {
        0 if encoded.len() == RESPONDER_RUNTIME_RECORD_HEADER_BYTES => None,
        1 if encoded.len() > RESPONDER_RUNTIME_RECORD_HEADER_BYTES => {
            Some(decode_and_verify_sync_server_frame_v2(
                &encoded[RESPONDER_RUNTIME_RECORD_HEADER_BYTES..],
            )?)
        }
        value => {
            return Err(StoreError::CanonicalSyncV2SessionRuntimeFramePresence(
                value,
            ));
        }
    };
    Ok(ResponderRuntimeRecord {
        checkpoint_id,
        pending_server_frame,
    })
}

fn validate_responder_checkpoint_record(
    record: &CanonicalSyncV2StoredResponderCheckpoint,
    expected_identity_key: [u8; CANONICAL_SYNC_V2_SESSION_IDENTITY_KEY_BYTES],
    expected_head: CanonicalSyncV2SessionCheckpointHead,
) -> Result<(), StoreError> {
    verify_signed_sync_responder_checkpoint_v2(record.checkpoint())?;
    let content = &record.checkpoint.content;
    let actual_identity_key = canonical_sync_v2_session_identity_key(
        content.conversation_id,
        content.responder_device_id,
        content.requester_device_id,
        content.session_binding,
    );
    if actual_identity_key != expected_identity_key
        || content.checkpoint_sequence != expected_head.checkpoint_sequence
        || record.import_plan_id != expected_head.import_plan_id
        || record.checkpoint_id != expected_head.checkpoint_id
        || sync_responder_checkpoint_id_v2(record.checkpoint())? != record.checkpoint_id
    {
        return Err(StoreError::CanonicalSyncV2SessionCheckpointRecordMismatch);
    }
    Ok(())
}

fn validate_responder_runtime_record(
    runtime: &ResponderRuntimeRecord,
    checkpoint: &CanonicalSyncV2StoredResponderCheckpoint,
) -> Result<(), StoreError> {
    if runtime.checkpoint_id != checkpoint.checkpoint_id {
        return Err(StoreError::CanonicalSyncV2SessionRuntimeCheckpointMismatch);
    }
    let content = &checkpoint.checkpoint.content;
    let frame_matches_action = matches!(
        (content.next_action, &runtime.pending_server_frame),
        (CanonicalSyncResponderNextActionV2::ReceiveInventory, None)
            | (CanonicalSyncResponderNextActionV2::Complete, None)
            | (
                CanonicalSyncResponderNextActionV2::SendDiff
                    | CanonicalSyncResponderNextActionV2::ReceiveEventBatch,
                Some(CanonicalSyncServerFrameV2::Diff(_))
            )
            | (
                CanonicalSyncResponderNextActionV2::SendComplete,
                Some(CanonicalSyncServerFrameV2::Complete(_))
            )
            | (
                CanonicalSyncResponderNextActionV2::Rejected,
                Some(CanonicalSyncServerFrameV2::Rejected(_))
            )
    );
    if !frame_matches_action {
        return Err(StoreError::CanonicalSyncV2SessionRuntimeFrameConflict);
    }
    if let Some(frame) = &runtime.pending_server_frame {
        validate_pending_server_frame(frame, checkpoint)?;
    }
    Ok(())
}

fn validate_pending_server_frame(
    frame: &CanonicalSyncServerFrameV2,
    checkpoint: &CanonicalSyncV2StoredResponderCheckpoint,
) -> Result<(), StoreError> {
    encode_sync_server_frame_v2(frame)?;
    let content = &checkpoint.checkpoint.content;
    let valid = match frame {
        CanonicalSyncServerFrameV2::Diff(diff) => {
            diff.content.conversation_id == content.conversation_id
                && diff.content.responder_device_id == content.responder_device_id
                && diff.content.session_binding == content.session_binding
        }
        CanonicalSyncServerFrameV2::Complete(complete) => {
            complete.conversation_id == content.conversation_id
        }
        CanonicalSyncServerFrameV2::Rejected(rejected) => {
            rejected.conversation_id == content.conversation_id
        }
        CanonicalSyncServerFrameV2::Paused(paused) => {
            paused.conversation_id == content.conversation_id
        }
    };
    if valid {
        Ok(())
    } else {
        Err(StoreError::CanonicalSyncV2SessionRuntimeFrameSessionMismatch)
    }
}

fn verify_prepared_carriers_in_table(
    write: &redb::WriteTransaction,
    conversation_id: [u8; 32],
    prepared: &[(CanonicalSyncEventKeyV2, Vec<u8>)],
) -> Result<(), StoreError> {
    let table = write
        .open_table(CANONICAL_SYNC_V2_CARRIER_TABLE)
        .map_err(canonical_sync_v2_database_error)?;
    for (event_key, encoded) in prepared {
        let storage_key = canonical_sync_v2_storage_key(conversation_id, *event_key);
        let existing = table
            .get(storage_key.as_slice())
            .map_err(canonical_sync_v2_database_error)?
            .map(|value| value.value().to_vec())
            .ok_or(StoreError::CanonicalSyncV2CarrierMissing {
                conversation_id,
                event_key: *event_key,
            })?;
        validate_canonical_sync_v2_stored_carrier(conversation_id, *event_key, &existing)?;
        if existing != *encoded {
            return Err(StoreError::CanonicalSyncV2CarrierConflict {
                conversation_id,
                event_key: *event_key,
            });
        }
    }
    Ok(())
}
