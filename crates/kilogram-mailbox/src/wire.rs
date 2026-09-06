use serde::{Deserialize, Serialize};

use crate::{
    DeleteOutcome, MAX_MAILBOX_ENVELOPE_BYTES, MAX_MAILBOX_PAGE_ITEMS, MailboxAddress,
    MailboxDeleteReceipt, MailboxError, MailboxId, MailboxItemId, MailboxPutOutcome,
    MailboxReadAuthorization, MailboxReadOperation, MailboxReceiptId, MailboxStoreKey,
    MailboxStoredReceipt, MailboxWriteAuthorization, StoredMailboxPage,
};

const WIRE_VERSION: u8 = 1;
const WIRE_OVERHEAD_BYTES: usize = 4 * 1024;

pub const MAX_MAILBOX_WIRE_REQUEST_BYTES: usize = MAX_MAILBOX_ENVELOPE_BYTES + WIRE_OVERHEAD_BYTES;
pub const MAX_MAILBOX_WIRE_RESPONSE_BYTES: usize = MAX_MAILBOX_PAGE_ITEMS as usize
    * (MAX_MAILBOX_ENVELOPE_BYTES + WIRE_OVERHEAD_BYTES)
    + WIRE_OVERHEAD_BYTES;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxPutRequest {
    version: u8,
    address: MailboxAddress,
    authorization: MailboxWriteAuthorization,
    envelope: Vec<u8>,
}

impl MailboxPutRequest {
    pub fn new(
        address: MailboxAddress,
        authorization: MailboxWriteAuthorization,
        envelope: Vec<u8>,
    ) -> Result<Self, MailboxError> {
        authorization.verify(address, &envelope)?;
        let request = Self {
            version: WIRE_VERSION,
            address,
            authorization,
            envelope,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        self.validate()?;
        encode_bounded(self, MAX_MAILBOX_WIRE_REQUEST_BYTES, "mailbox put request")
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, MailboxError> {
        let request: Self =
            decode_bounded(bytes, MAX_MAILBOX_WIRE_REQUEST_BYTES, "mailbox put request")?;
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), MailboxError> {
        if self.version != WIRE_VERSION {
            return Err(MailboxError::Invalid(
                "unsupported mailbox put request version",
            ));
        }
        self.authorization.verify(self.address, &self.envelope)
    }

    pub fn address(&self) -> MailboxAddress {
        self.address
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.address.mailbox_id()
    }

    pub fn item_id(&self) -> MailboxItemId {
        self.authorization.item_id()
    }

    pub fn envelope(&self) -> &[u8] {
        &self.envelope
    }

    pub fn into_parts(self) -> (MailboxAddress, MailboxWriteAuthorization, Vec<u8>) {
        (self.address, self.authorization, self.envelope)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MailboxPutResponse {
    Stored {
        version: u8,
        created: bool,
        receipt: MailboxStoredReceipt,
    },
    Conflict {
        version: u8,
    },
    Tombstoned {
        version: u8,
    },
    CapacityExceeded {
        version: u8,
    },
}

impl MailboxPutResponse {
    pub fn from_outcome(outcome: MailboxPutOutcome) -> Self {
        match outcome {
            MailboxPutOutcome::Created(receipt) => Self::Stored {
                version: WIRE_VERSION,
                created: true,
                receipt,
            },
            MailboxPutOutcome::AlreadyPresent(receipt) => Self::Stored {
                version: WIRE_VERSION,
                created: false,
                receipt,
            },
            MailboxPutOutcome::Conflict => Self::Conflict {
                version: WIRE_VERSION,
            },
            MailboxPutOutcome::Tombstoned => Self::Tombstoned {
                version: WIRE_VERSION,
            },
            MailboxPutOutcome::CapacityExceeded => Self::CapacityExceeded {
                version: WIRE_VERSION,
            },
        }
    }

    pub fn encode(
        &self,
        request: &MailboxPutRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<Vec<u8>, MailboxError> {
        self.verify(request, expected_store_key)?;
        encode_bounded(self, WIRE_OVERHEAD_BYTES, "mailbox put response")
    }

    pub fn decode_and_verify(
        bytes: &[u8],
        request: &MailboxPutRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<Self, MailboxError> {
        let response: Self = decode_bounded(bytes, WIRE_OVERHEAD_BYTES, "mailbox put response")?;
        response.verify(request, expected_store_key)?;
        Ok(response)
    }

    pub fn stored_receipt(&self) -> Option<&MailboxStoredReceipt> {
        match self {
            Self::Stored { receipt, .. } => Some(receipt),
            _ => None,
        }
    }

    fn verify(
        &self,
        request: &MailboxPutRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<(), MailboxError> {
        request.validate()?;
        let version = match self {
            Self::Stored { version, .. }
            | Self::Conflict { version }
            | Self::Tombstoned { version }
            | Self::CapacityExceeded { version } => *version,
        };
        if version != WIRE_VERSION {
            return Err(MailboxError::Invalid(
                "unsupported mailbox put response version",
            ));
        }
        if let Self::Stored { receipt, .. } = self {
            receipt.verify(request.envelope())?;
            if receipt.store_key() != expected_store_key
                || receipt.mailbox_id() != request.mailbox_id()
                || receipt.item_id() != request.item_id()
            {
                return Err(MailboxError::Invalid(
                    "mailbox put receipt does not match request or store",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxListRequest {
    version: u8,
    address: MailboxAddress,
    authorization: MailboxReadAuthorization,
}

impl MailboxListRequest {
    pub fn new(
        address: MailboxAddress,
        authorization: MailboxReadAuthorization,
    ) -> Result<Self, MailboxError> {
        let request = Self {
            version: WIRE_VERSION,
            address,
            authorization,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        self.validate()?;
        encode_bounded(self, WIRE_OVERHEAD_BYTES, "mailbox list request")
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, MailboxError> {
        let request: Self = decode_bounded(bytes, WIRE_OVERHEAD_BYTES, "mailbox list request")?;
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), MailboxError> {
        if self.version != WIRE_VERSION {
            return Err(MailboxError::Invalid(
                "unsupported mailbox list request version",
            ));
        }
        self.authorization.verify(self.address)?;
        if !matches!(
            self.authorization.operation(),
            MailboxReadOperation::List { .. }
        ) {
            return Err(MailboxError::Invalid(
                "mailbox list request has another operation",
            ));
        }
        Ok(())
    }

    pub fn address(&self) -> MailboxAddress {
        self.address
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.address.mailbox_id()
    }

    pub fn authorization(&self) -> &MailboxReadAuthorization {
        &self.authorization
    }

    fn bounds(&self) -> (Option<MailboxItemId>, u16) {
        match self.authorization.operation() {
            MailboxReadOperation::List {
                after_item_id,
                limit,
                ..
            } => (after_item_id, limit),
            MailboxReadOperation::Delete { .. } => unreachable!("validated list request"),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxListResponse {
    version: u8,
    page: StoredMailboxPage,
}

impl MailboxListResponse {
    pub fn new(page: StoredMailboxPage) -> Self {
        Self {
            version: WIRE_VERSION,
            page,
        }
    }

    pub fn encode(
        &self,
        request: &MailboxListRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<Vec<u8>, MailboxError> {
        self.verify(request, expected_store_key)?;
        encode_bounded(
            self,
            MAX_MAILBOX_WIRE_RESPONSE_BYTES,
            "mailbox list response",
        )
    }

    pub fn decode_and_verify(
        bytes: &[u8],
        request: &MailboxListRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<Self, MailboxError> {
        let response: Self = decode_bounded(
            bytes,
            MAX_MAILBOX_WIRE_RESPONSE_BYTES,
            "mailbox list response",
        )?;
        response.verify(request, expected_store_key)?;
        Ok(response)
    }

    pub fn page(&self) -> &StoredMailboxPage {
        &self.page
    }

    pub fn into_page(self) -> StoredMailboxPage {
        self.page
    }

    fn verify(
        &self,
        request: &MailboxListRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<(), MailboxError> {
        request.validate()?;
        if self.version != WIRE_VERSION {
            return Err(MailboxError::Invalid(
                "unsupported mailbox list response version",
            ));
        }
        let (after_item_id, limit) = request.bounds();
        if self.page.items.len() > limit as usize
            || self.page.more_available != self.page.next_after_item_id.is_some()
            || self.page.more_available && self.page.items.is_empty()
        {
            return Err(MailboxError::Invalid(
                "mailbox list response pagination is invalid",
            ));
        }
        if self.page.more_available
            && self.page.next_after_item_id != self.page.items.last().map(|item| item.item_id)
        {
            return Err(MailboxError::Invalid(
                "mailbox list response cursor is invalid",
            ));
        }
        let mut previous = after_item_id;
        for item in &self.page.items {
            if previous.is_some_and(|value| item.item_id <= value) {
                return Err(MailboxError::Invalid(
                    "mailbox list response item order is invalid",
                ));
            }
            item.receipt.verify(&item.envelope)?;
            if item.receipt.store_key() != expected_store_key
                || item.receipt.mailbox_id() != request.mailbox_id()
                || item.receipt.item_id() != item.item_id
                || item.receipt.expires_at_unix_seconds() != item.expires_at_unix_seconds
            {
                return Err(MailboxError::Invalid(
                    "mailbox list item does not match request or store",
                ));
            }
            previous = Some(item.item_id);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MailboxDeleteRequest {
    version: u8,
    address: MailboxAddress,
    authorization: MailboxReadAuthorization,
}

impl MailboxDeleteRequest {
    pub fn new(
        address: MailboxAddress,
        authorization: MailboxReadAuthorization,
    ) -> Result<Self, MailboxError> {
        let request = Self {
            version: WIRE_VERSION,
            address,
            authorization,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn encode(&self) -> Result<Vec<u8>, MailboxError> {
        self.validate()?;
        encode_bounded(self, WIRE_OVERHEAD_BYTES, "mailbox delete request")
    }

    pub fn decode_and_verify(bytes: &[u8]) -> Result<Self, MailboxError> {
        let request: Self = decode_bounded(bytes, WIRE_OVERHEAD_BYTES, "mailbox delete request")?;
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), MailboxError> {
        if self.version != WIRE_VERSION {
            return Err(MailboxError::Invalid(
                "unsupported mailbox delete request version",
            ));
        }
        self.authorization.verify(self.address)?;
        if !matches!(
            self.authorization.operation(),
            MailboxReadOperation::Delete { .. }
        ) {
            return Err(MailboxError::Invalid(
                "mailbox delete request has another operation",
            ));
        }
        Ok(())
    }

    pub fn address(&self) -> MailboxAddress {
        self.address
    }

    pub fn mailbox_id(&self) -> MailboxId {
        self.address.mailbox_id()
    }

    pub fn item_id(&self) -> MailboxItemId {
        match self.authorization.operation() {
            MailboxReadOperation::Delete { item_id, .. } => item_id,
            MailboxReadOperation::List { .. } => unreachable!("validated delete request"),
        }
    }

    pub fn stored_receipt_id(&self) -> MailboxReceiptId {
        match self.authorization.operation() {
            MailboxReadOperation::Delete { receipt_id, .. } => receipt_id,
            MailboxReadOperation::List { .. } => unreachable!("validated delete request"),
        }
    }

    pub fn authorization(&self) -> &MailboxReadAuthorization {
        &self.authorization
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MailboxDeleteResponse {
    Deleted {
        version: u8,
        newly_deleted: bool,
        receipt: MailboxDeleteReceipt,
    },
    Absent {
        version: u8,
    },
    ReceiptMismatch {
        version: u8,
    },
    TombstoneCapacityExceeded {
        version: u8,
    },
}

impl MailboxDeleteResponse {
    pub fn from_outcome(outcome: DeleteOutcome) -> Self {
        match outcome {
            DeleteOutcome::Deleted(receipt) => Self::Deleted {
                version: WIRE_VERSION,
                newly_deleted: true,
                receipt,
            },
            DeleteOutcome::AlreadyDeleted(receipt) => Self::Deleted {
                version: WIRE_VERSION,
                newly_deleted: false,
                receipt,
            },
            DeleteOutcome::Absent => Self::Absent {
                version: WIRE_VERSION,
            },
            DeleteOutcome::ReceiptMismatch => Self::ReceiptMismatch {
                version: WIRE_VERSION,
            },
            DeleteOutcome::TombstoneCapacityExceeded => Self::TombstoneCapacityExceeded {
                version: WIRE_VERSION,
            },
        }
    }

    pub fn encode(
        &self,
        request: &MailboxDeleteRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<Vec<u8>, MailboxError> {
        self.verify(request, expected_store_key)?;
        encode_bounded(self, WIRE_OVERHEAD_BYTES, "mailbox delete response")
    }

    pub fn decode_and_verify(
        bytes: &[u8],
        request: &MailboxDeleteRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<Self, MailboxError> {
        let response: Self = decode_bounded(bytes, WIRE_OVERHEAD_BYTES, "mailbox delete response")?;
        response.verify(request, expected_store_key)?;
        Ok(response)
    }

    pub fn delete_receipt(&self) -> Option<&MailboxDeleteReceipt> {
        match self {
            Self::Deleted { receipt, .. } => Some(receipt),
            _ => None,
        }
    }

    fn verify(
        &self,
        request: &MailboxDeleteRequest,
        expected_store_key: MailboxStoreKey,
    ) -> Result<(), MailboxError> {
        request.validate()?;
        let version = match self {
            Self::Deleted { version, .. }
            | Self::Absent { version }
            | Self::ReceiptMismatch { version }
            | Self::TombstoneCapacityExceeded { version } => *version,
        };
        if version != WIRE_VERSION {
            return Err(MailboxError::Invalid(
                "unsupported mailbox delete response version",
            ));
        }
        if let Self::Deleted { receipt, .. } = self {
            receipt.verify()?;
            if receipt.store_key() != expected_store_key
                || receipt.mailbox_id() != request.mailbox_id()
                || receipt.item_id() != request.item_id()
                || receipt.stored_receipt_id() != request.stored_receipt_id()
            {
                return Err(MailboxError::Invalid(
                    "mailbox delete receipt does not match request or store",
                ));
            }
        }
        Ok(())
    }
}

fn encode_bounded<T: Serialize>(
    value: &T,
    maximum: usize,
    kind: &'static str,
) -> Result<Vec<u8>, MailboxError> {
    let bytes = postcard::to_allocvec(value)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(MailboxError::Invalid(match kind {
            "mailbox put request" => "mailbox put request size is invalid",
            "mailbox put response" => "mailbox put response size is invalid",
            "mailbox list request" => "mailbox list request size is invalid",
            "mailbox list response" => "mailbox list response size is invalid",
            "mailbox delete request" => "mailbox delete request size is invalid",
            "mailbox delete response" => "mailbox delete response size is invalid",
            _ => "mailbox wire value size is invalid",
        }));
    }
    Ok(bytes)
}

fn decode_bounded<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    maximum: usize,
    kind: &'static str,
) -> Result<T, MailboxError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(MailboxError::Invalid(match kind {
            "mailbox put request" => "mailbox put request size is invalid",
            "mailbox put response" => "mailbox put response size is invalid",
            "mailbox list request" => "mailbox list request size is invalid",
            "mailbox list response" => "mailbox list response size is invalid",
            "mailbox delete request" => "mailbox delete request size is invalid",
            "mailbox delete response" => "mailbox delete response size is invalid",
            _ => "mailbox wire value size is invalid",
        }));
    }
    postcard::from_bytes(bytes).map_err(MailboxError::from)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use kilogram_crypto::DeviceEncryptionIdentity;

    use super::*;
    use crate::{
        BlindMailboxStore, MailboxReadCapability, MailboxRequestNonce, MailboxStoreConfig,
        MailboxStoreIdentity, MailboxWriteCapability,
    };

    #[test]
    fn paged_wire_round_trip_binds_requests_receipts_and_store() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store_identity = MailboxStoreIdentity::from_secret_bytes([9_u8; 32]);
        let store_key = store_identity.store_key();
        let store = BlindMailboxStore::open(
            MailboxStoreConfig::new(directory.path().to_path_buf()),
            store_identity,
        )?;
        let read = MailboxReadCapability::from_secret_bytes([1_u8; 32]);
        let write = MailboxWriteCapability::from_secret_bytes([2_u8; 32]);
        let address = MailboxAddress::new(read.read_key(), write.write_key());
        let recipient = DeviceEncryptionIdentity::from_secret_bytes([3_u8; 32]);

        for item_byte in [4_u8, 5_u8] {
            let item_id = MailboxItemId::from_bytes([item_byte; 32]);
            let envelope = crate::MailboxEnvelope::seal(
                address.mailbox_id(),
                item_id,
                1_000,
                1_600,
                recipient.public_key(),
                &[item_byte],
            )?
            .encode()?;
            let authorization = write.authorize(address, item_id, 600, &envelope)?;
            let request = MailboxPutRequest::new(address, authorization, envelope)?;
            let decoded = MailboxPutRequest::decode_and_verify(&request.encode()?)?;
            let (request_address, request_authorization, request_envelope) =
                decoded.clone().into_parts();
            let response = MailboxPutResponse::from_outcome(store.put(
                request_address,
                &request_authorization,
                request_envelope,
                1_000,
            )?);
            assert!(matches!(
                MailboxPutResponse::decode_and_verify(
                    &response.encode(&decoded, store_key)?,
                    &decoded,
                    store_key,
                )?,
                MailboxPutResponse::Stored { created: true, .. }
            ));
        }

        let first_list = MailboxListRequest::new(
            address,
            read.authorize_list_page(
                address,
                MailboxRequestNonce::from_bytes([6_u8; 32]),
                None,
                1,
            )?,
        )?;
        let first_page = store.list_page(address, first_list.authorization(), 1_001)?;
        let first_response = MailboxListResponse::new(first_page);
        let first_response = MailboxListResponse::decode_and_verify(
            &first_response.encode(&first_list, store_key)?,
            &first_list,
            store_key,
        )?;
        assert!(first_response.page().more_available);
        assert_eq!(first_response.page().items.len(), 1);

        let first_item = &first_response.page().items[0];
        let delete = MailboxDeleteRequest::new(
            address,
            read.authorize_delete(
                address,
                first_item.item_id,
                first_item.receipt.receipt_id()?,
            )?,
        )?;
        let delete = MailboxDeleteRequest::decode_and_verify(&delete.encode()?)?;
        let response = MailboxDeleteResponse::from_outcome(store.delete(
            address,
            delete.authorization(),
            1_002,
        )?);
        assert!(matches!(
            MailboxDeleteResponse::decode_and_verify(
                &response.encode(&delete, store_key)?,
                &delete,
                store_key,
            )?,
            MailboxDeleteResponse::Deleted {
                newly_deleted: true,
                ..
            }
        ));
        Ok(())
    }
}
