//! One lease-local debug retry. Every source and Complete still uses production code.
use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub incarnation: String,
    pub lease_generation: u64,
    pub from: u64,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        let nonce = state::parse_id(&self.incarnation)?;
        ensure!(
            nonce != [0; 32]
                && state::hex(&nonce) == self.incarnation
                && self.lease_generation > 0
                && (1..512).contains(&self.from),
            "S54 retry nonce/generation/old ordinal bounds"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ordered_dialog::runtime7) struct Ticket {
    pub request: Request,
    pub binding: Id,
    pub context: Id,
    pub first_id: Id,
    pub normal_first: u64,
}
pub(in crate::ordered_dialog::runtime7) struct Gate {
    pub nonce: Id,
    pub binding: Id,
    pub context: Id,
    pub generation: u64,
    pub role: u8,
    pub active: bool,
    pub received: u64,
    pub normal_first: u64,
    pub debt: Option<(u64, Id)>,
    pub held: bool,
}
impl Gate {
    pub fn ticket(self, request: &Request) -> Result<Ticket> {
        request.validate()?;
        ensure!(
            self.role == 1
                && self.active
                && state::parse_id(&request.incarnation)? == self.nonce
                && request.lease_generation == self.generation,
            "S54 retry actual B Active/current incarnation/lease"
        );
        let (n, first_id) = self
            .debt
            .context("S54 real first undelivered source required")?;
        ensure!(
            !self.held && n == request.from && n == self.received && n < self.normal_first,
            "S54 retry lowest unheld debt equals actual peer frontier behind cursor"
        );
        Ok(Ticket {
            request: request.clone(),
            binding: self.binding,
            context: self.context,
            first_id,
            normal_first: self.normal_first,
        })
    }
}
#[derive(Clone, Default)]
pub(in crate::ordered_dialog::runtime7) enum Once {
    #[default]
    Fresh,
    InFlight(Ticket),
    Completed(Ticket),
}
impl Once {
    pub fn choose(
        &self,
        request: Option<&Request>,
        binding: Id,
        validate: impl FnOnce(&Request) -> Result<Ticket>,
    ) -> Result<Option<Ticket>> {
        match self {
            Self::Completed(ticket) => {
                ensure!(
                    ticket.binding == binding,
                    "S54 completed retry cannot move to another binding"
                );
                // This check MUST precede all mutable debt/frontier predicates.
                // Original B16 may now be readable, ACKed or absent from debt.
                ensure!(
                    request.is_none_or(|r| r == &ticket.request),
                    "S54 consumed lease cannot re-arm a replacement retry"
                );
                Ok(None)
            }
            Self::InFlight(_) => anyhow::bail!("S54 retry already in flight"),
            Self::Fresh => request.map(validate).transpose(),
        }
    }
    pub fn begin(&mut self, ticket: Ticket) -> Result<()> {
        ensure!(
            matches!(self, Self::Fresh),
            "S54 one captured retry per lease"
        );
        *self = Self::InFlight(ticket);
        Ok(())
    }
    pub fn finish(&mut self, ticket: &Ticket, ids: &[Id], next: u64) -> Result<()> {
        ensure!(
            matches!(self,Self::InFlight(current) if current==ticket),
            "S54 matching in-flight token required"
        );
        completed_ids(ticket, ids, next)?;
        *self = Self::Completed(ticket.clone());
        Ok(())
    }
}
pub(in crate::ordered_dialog::runtime7) fn completed_ids(
    ticket: &Ticket,
    ids: &[Id],
    next: u64,
) -> Result<()> {
    ensure!(
        !ids.is_empty()
            && ids.len() <= 16
            && ids.first() == Some(&ticket.first_id)
            && ids.iter().collect::<BTreeSet<_>>().len() == ids.len()
            && next > ticket.request.from
            && next <= ticket.request.from + 16,
        "S54 retry must actually transmit original first ID within normal wire cap"
    );
    Ok(())
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
