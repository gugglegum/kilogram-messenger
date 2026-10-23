//! Filesystem-free, byte-oriented Olm trials for explicitly negotiated OD1.
//! Callers own ordering, authentication and the atomic persistence boundary.
use thiserror::Error;
use vodozemac::{
    Curve25519PublicKey,
    olm::{
        Account, AccountPickle, DecryptionError, OlmMessage, Session, SessionConfig,
        SessionCreationError, SessionPickle,
    },
};

pub const MAX_INNER: usize = 8 * 1024;
pub const MAX_OLM: usize = 12 * 1024;
const MAX_PICKLE: usize = 128 * 1024;

#[derive(Debug, Error)]
pub enum NativeFailure {
    #[error("invalid OD1 native input: {0}")]
    Input(&'static str),
    #[error("OD1 native local state failure: {0}")]
    Operational(&'static str),
}

pub struct OrderedAccount(Account);
pub struct OrderedSession(Session);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Packet {
    pub kind: u8,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Header {
    pub key: [u8; 32],
    pub index: u64,
    pub prekey_sid: Option<String>,
    pub prekey_keys: Option<[[u8; 32]; 3]>,
}

impl Packet {
    fn decode(&self) -> Result<OlmMessage, NativeFailure> {
        if self.kind > 1 || self.bytes.is_empty() || self.bytes.len() > MAX_OLM {
            return Err(NativeFailure::Input("kind/length"));
        }
        OlmMessage::from_parts(usize::from(self.kind), &self.bytes)
            .map_err(|_| NativeFailure::Input("Olm encoding"))
    }

    pub fn header(&self) -> Result<Header, NativeFailure> {
        let message = self.decode()?;
        let (inner, sid, keys) = match &message {
            OlmMessage::Normal(m) => (m, None, None),
            OlmMessage::PreKey(m) => {
                let keys = m.session_keys();
                (
                    m.message(),
                    Some(m.session_id()),
                    Some([
                        keys.identity_key.to_bytes(),
                        keys.base_key.to_bytes(),
                        keys.one_time_key.to_bytes(),
                    ]),
                )
            }
        };
        Ok(Header {
            key: inner.ratchet_key().to_bytes(),
            index: inner.chain_index(),
            prekey_sid: sid,
            prekey_keys: keys,
        })
    }
}

impl OrderedAccount {
    /// Only fresh construction can generate an OTK; there is no refill API.
    pub fn fresh(receiver: bool) -> (Self, Option<[u8; 32]>) {
        let mut account = Account::new();
        let otk = if receiver {
            account.generate_one_time_keys(1);
            account
                .one_time_keys()
                .values()
                .next()
                .map(Curve25519PublicKey::to_bytes)
        } else {
            None
        };
        (Self(account), otk)
    }

    pub fn curve_key(&self) -> [u8; 32] {
        self.0.curve25519_key().to_bytes()
    }

    pub fn fork(&self) -> Self {
        Self(Account::from_pickle(self.0.pickle()))
    }

    pub fn snapshot(&self, key: &[u8; 32]) -> Vec<u8> {
        self.0.pickle().encrypt(key).into_bytes()
    }

    pub fn restore(raw: &[u8], key: &[u8; 32]) -> Result<Self, NativeFailure> {
        let raw = pickle_text(raw)?;
        AccountPickle::from_encrypted(raw, key)
            .map(Account::from_pickle)
            .map(Self)
            .map_err(|_| NativeFailure::Operational("account pickle"))
    }

    pub fn outbound(&self, peer: [u8; 32], otk: [u8; 32]) -> Result<OrderedSession, NativeFailure> {
        self.0
            .create_outbound_session(
                SessionConfig::version_1(),
                Curve25519PublicKey::from_bytes(peer),
                Curve25519PublicKey::from_bytes(otk),
            )
            .map(OrderedSession)
            .map_err(|_| NativeFailure::Operational("outbound session"))
    }

    /// Account changes are assigned only after successful native MAC verification.
    /// The caller still discards this whole trial if the inner binding fails.
    pub fn inbound(
        &mut self,
        peer: [u8; 32],
        packet: &Packet,
    ) -> Result<(OrderedSession, Vec<u8>), NativeFailure> {
        let OlmMessage::PreKey(prekey) = packet.decode()? else {
            return Err(NativeFailure::Input("Init must be PreKey"));
        };
        let mut trial = self.fork();
        let result = trial
            .0
            .create_inbound_session(
                SessionConfig::version_1(),
                Curve25519PublicKey::from_bytes(peer),
                &prekey,
            )
            .map_err(|error| match error {
                SessionCreationError::MissingOneTimeKey(_) => {
                    NativeFailure::Operational("reserved OTK missing")
                }
                SessionCreationError::Decryption(error) => classify_decrypt(error),
                _ => NativeFailure::Input("prekey/session binding"),
            })?;
        if result.plaintext.len() > MAX_INNER {
            return Err(NativeFailure::Input("inner length"));
        }
        self.0 = trial.0;
        Ok((OrderedSession(result.session), result.plaintext))
    }
}

impl OrderedSession {
    pub fn fork(&self) -> Self {
        Self(Session::from_pickle(self.0.pickle()))
    }
    pub fn sid(&self) -> String {
        self.0.session_id()
    }
    pub fn keys(&self) -> [[u8; 32]; 3] {
        let keys = self.0.session_keys();
        [
            keys.identity_key.to_bytes(),
            keys.base_key.to_bytes(),
            keys.one_time_key.to_bytes(),
        ]
    }
    pub fn snapshot(&self, key: &[u8; 32]) -> Vec<u8> {
        self.0.pickle().encrypt(key).into_bytes()
    }
    pub fn restore(raw: &[u8], key: &[u8; 32]) -> Result<Self, NativeFailure> {
        let session = SessionPickle::from_encrypted(pickle_text(raw)?, key)
            .map(Session::from_pickle)
            .map_err(|_| NativeFailure::Operational("session pickle"))?;
        if session.session_config() != SessionConfig::version_1() {
            return Err(NativeFailure::Operational(
                "unsupported native session config",
            ));
        }
        Ok(Self(session))
    }
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Packet, NativeFailure> {
        if plaintext.len() > MAX_INNER {
            return Err(NativeFailure::Operational("inner length"));
        }
        let message = self
            .0
            .encrypt(plaintext)
            .map_err(|_| NativeFailure::Operational("native encrypt"))?;
        let (kind, bytes) = message.to_parts();
        if bytes.len() > MAX_OLM || kind > 1 {
            return Err(NativeFailure::Operational("Olm output kind/length"));
        }
        let kind = u8::try_from(kind).map_err(|_| NativeFailure::Operational("Olm kind"))?;
        Ok(Packet { kind, bytes })
    }
    pub fn decrypt(&mut self, packet: &Packet) -> Result<Vec<u8>, NativeFailure> {
        if packet.kind != 1 {
            return Err(NativeFailure::Input("selected session requires Normal"));
        }
        let message = packet.decode()?;
        let mut trial = self.fork();
        let plaintext = trial.0.decrypt(&message).map_err(classify_decrypt)?;
        if plaintext.len() > MAX_INNER {
            return Err(NativeFailure::Input("inner length"));
        }
        self.0 = trial.0;
        Ok(plaintext)
    }
}

fn pickle_text(raw: &[u8]) -> Result<&str, NativeFailure> {
    if raw.len() > MAX_PICKLE {
        return Err(NativeFailure::Operational("pickle length"));
    }
    std::str::from_utf8(raw).map_err(|_| NativeFailure::Operational("pickle encoding"))
}

fn classify_decrypt(error: DecryptionError) -> NativeFailure {
    match error {
        DecryptionError::InvalidMAC(_)
        | DecryptionError::InvalidMACLength(_, _)
        | DecryptionError::InvalidPadding(_)
        | DecryptionError::NonContributoryKey => NativeFailure::Input("ciphertext authentication"),
        DecryptionError::MissingMessageKey(_) | DecryptionError::TooBigMessageGap(_, _) => {
            NativeFailure::Operational("ordered guard/native cursor disagreement")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_reopen_before_session_and_binary_duplex_survives_eight_dh_changes()
    -> Result<(), NativeFailure> {
        let key = [17; 32];
        let (a, _) = OrderedAccount::fresh(false);
        let (b, otk) = OrderedAccount::fresh(true);
        let a_key = a.curve_key();
        let b_key = b.curve_key();
        let a = OrderedAccount::restore(&a.snapshot(&key), &key)?;
        let mut b = OrderedAccount::restore(&b.snapshot(&key), &key)?;
        assert_eq!(a.curve_key(), a_key);
        let mut sa = a.outbound(b_key, otk.ok_or(NativeFailure::Operational("test OTK"))?)?;
        let init = sa.encrypt(b"\0\xffinit")?;
        let (mut sb, init_text) = b.inbound(a_key, &init)?;
        assert_eq!(init_text, b"\0\xffinit");
        assert_eq!(sa.sid(), sb.sid());
        let confirm = sb.encrypt(b"confirm")?;
        assert_eq!(sa.decrypt(&confirm)?, b"confirm");
        let mut old_a = init.header()?.key;
        let mut old_b = confirm.header()?.key;
        let mut last_b = confirm;
        for n in 0..8 {
            let text = [0, 255, n];
            let a_message = sa.encrypt(&text)?;
            assert_ne!(a_message.header()?.key, old_a);
            assert_eq!(a_message.header()?.index, 0);
            old_a = a_message.header()?.key;
            assert_eq!(sb.decrypt(&a_message)?, text);
            let b_message = sb.encrypt(&text)?;
            assert_ne!(b_message.header()?.key, old_b);
            old_b = b_message.header()?.key;
            assert_eq!(sa.decrypt(&b_message)?, text);
            last_b = b_message;
            sa = OrderedSession::restore(&sa.snapshot(&key), &key)?;
            sb = OrderedSession::restore(&sb.snapshot(&key), &key)?;
        }
        // A duplicate cannot silently become input quarantine: with a correct
        // application guard it must have been removed before this native call.
        assert!(matches!(
            sa.decrypt(&last_b),
            Err(NativeFailure::Operational(_))
        ));
        Ok(())
    }
}
