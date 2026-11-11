//! Verify the interface that owns the configured IPv4, using Windows' prefix.
use super::*;

#[cfg(windows)]
pub(super) fn verify_interface(lan: wire::DirectLan2) -> Result<()> {
    use std::net::Ipv4Addr;
    use windows::Networking::{Connectivity::NetworkInformation, HostNameType};

    let addresses = NetworkInformation::GetHostNames().context("Enumerate local LAN addresses")?;
    let mut prefix = None;
    for i in 0..addresses.Size()? {
        let address = addresses.GetAt(i)?;
        if address.Type()? != HostNameType::Ipv4
            || address
                .CanonicalName()?
                .to_string()
                .parse::<Ipv4Addr>()
                .ok()
                != Some(lan.local())
        {
            continue;
        }
        let info = address
            .IPInformation()
            .context("LAN IPv4 has no interface information")?;
        let _adapter = info
            .NetworkAdapter()
            .context("LAN IPv4 has no assigned adapter")?;
        let length = info
            .PrefixLength()
            .context("LAN interface prefix unavailable")?
            .Value()?;
        ensure!(
            prefix.replace(length).is_none(),
            "LAN IPv4 is assigned ambiguously"
        );
    }
    lan.check_on_link(prefix.context("LAN IPv4 is not assigned to a local interface")?)?;
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn verify_interface(_lan: wire::DirectLan2) -> Result<()> {
    anyhow::bail!("Fresh Direct LAN requires Windows interface verification")
}
