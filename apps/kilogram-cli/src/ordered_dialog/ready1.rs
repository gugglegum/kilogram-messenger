//! Pure Ready1 scope validation. Genesis parsing and fork classification stay
//! with each runtime, which deliberately retains its existing evaluation order.
use super::state::Head;
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v1::Value;

pub(super) fn scoped_fields<'a>(head: &Head, peer: u64, body: &'a Value) -> Result<&'a [Value]> {
    let f = body.fields(8)?;
    ensure!(
        Some(f[0].fixed::<32>()?) == head.d
            && Some(f[1].fixed::<32>()?) == head.e
            && f[2].text()? == head.sid.as_deref().context("SID")?
            && f[5].uint()? == peer
            && f[6].uint()? == 1
            && f[7].uint()? == 1,
        "Ready scope/role/suite"
    );
    Ok(f)
}
