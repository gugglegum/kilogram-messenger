use crate::MIN_MTU;
use anyhow::{Context, Result, bail, ensure};
use kilogram_protocol::ordered_dialog_v2::DirectLan2;
use std::{mem::size_of, net::Ipv4Addr, thread};
use windows::{
    Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS},
        NetworkManagement::{
            IpHelper::{
                ConvertInterfaceLuidToGuid, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
                GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, GetIpInterfaceEntry,
                IP_ADAPTER_ADDRESSES_LH, InitializeIpInterfaceEntry, MIB_IPINTERFACE_ROW,
            },
            Ndis::IfOperStatusUp,
        },
        Networking::{
            NetworkListManager::{
                INetworkListManager, NLM_NETWORK_CATEGORY_PRIVATE, NetworkListManager,
            },
            WinSock::{AF_INET, IpDadStatePreferred, SOCKADDR_IN},
        },
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
    },
    core::GUID,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkScope {
    pub adapter_id: GUID,
    pub network_id: GUID,
    pub prefix: u8,
}

/// A current, unambiguous snapshot associated with the selected two IPv4 addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    route: DirectLan2,
    scope: NetworkScope,
    if_index: u32,
    mtu: u32,
    interface_type: u32,
}

impl Binding {
    pub fn local(&self) -> Ipv4Addr {
        self.route.local()
    }

    pub fn peer(&self) -> Ipv4Addr {
        self.route.peer()
    }

    pub fn scope(&self) -> NetworkScope {
        self.scope
    }

    pub fn if_index(&self) -> u32 {
        self.if_index
    }

    pub fn mtu(&self) -> u32 {
        self.mtu
    }

    pub fn interface_type(&self) -> u32 {
        self.interface_type
    }

    /// Re-read the exact route and selected scope before opening network resources.
    pub fn refresh(&self) -> Result<Self> {
        let current = inspect(self.route, Some(self.scope))?;
        ensure!(current.if_index == self.if_index, "LAN ifIndex changed");
        Ok(current)
    }
}

/// `expected = None` samples the current scope for initial explicit selection.
/// Starts and joins a private COM thread. Call from outside database transactions
/// and interactive handlers. No COM interfaces leave that thread.
pub fn inspect(route: DirectLan2, expected: Option<NetworkScope>) -> Result<Binding> {
    thread::Builder::new()
        .name("kilogram-lan-nlm".into())
        .spawn(move || inspect_on_com_thread(route, expected))?
        .join()
        .map_err(|_| anyhow::anyhow!("LAN metadata worker failed"))?
}

struct ComApartment;

impl ComApartment {
    fn enter() -> Result<Self> {
        // SAFETY: Called on the freshly spawned thread. Every successful
        // initialization, including S_FALSE, is balanced by Drop.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: This private guard never leaves the thread which initialized COM.
        unsafe { CoUninitialize() };
    }
}

fn inspect_on_com_thread(route: DirectLan2, expected: Option<NetworkScope>) -> Result<Binding> {
    let adapter = adapter_for(route.local())?;
    route.check_on_link(adapter.prefix)?;
    let _apartment = ComApartment::enter().context("initialize NLM apartment")?;
    let scope = NetworkScope {
        adapter_id: adapter.id,
        network_id: private_network(adapter.id)?,
        prefix: adapter.prefix,
    };
    ensure!(
        expected.is_none_or(|value| value == scope),
        "LAN network scope changed"
    );
    // A disconnect or reassignment during the NLM call must not return the old tuple.
    ensure!(
        adapter_for(route.local())? == adapter,
        "LAN interface changed during inspection"
    );
    Ok(Binding {
        route,
        scope,
        if_index: adapter.index,
        mtu: adapter.mtu,
        interface_type: adapter.interface_type,
    })
}

fn private_network(adapter_id: GUID) -> Result<GUID> {
    // SAFETY: The calling thread owns an initialized COM apartment. Windows
    // provides the typed interfaces and their reference counts are scoped here.
    unsafe {
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_INPROC_SERVER)
                .context("open Network List Manager")?;
        let connections = manager
            .GetNetworkConnections()
            .context("enumerate NLM connections")?;
        let mut selection = None;
        loop {
            let mut item = [None];
            let mut count = 0;
            connections.Next(&mut item, Some(&mut count))?;
            if count == 0 {
                break;
            }
            let connection = item[0].take().context("missing NLM connection")?;
            if connection.GetAdapterId()? != adapter_id || !connection.IsConnected()?.as_bool() {
                continue;
            }
            ensure!(
                selection.is_none(),
                "ambiguous active NLM network for LAN adapter"
            );
            let network = connection.GetNetwork()?;
            selection = Some((network.GetNetworkId()?, network.GetCategory()?));
        }
        let (network_id, category) = selection.context("no active NLM network for LAN adapter")?;
        ensure!(
            category == NLM_NETWORK_CATEGORY_PRIVATE,
            "LAN network is not Private"
        );
        ensure!(network_id != GUID::zeroed(), "missing NLM network GUID");
        Ok(network_id)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Adapter {
    id: GUID,
    index: u32,
    prefix: u8,
    mtu: u32,
    interface_type: u32,
}

fn adapter_for(local: Ipv4Addr) -> Result<Adapter> {
    let mut bytes = 15 * 1024;
    for _ in 0..3 {
        ensure!(bytes <= 1024 * 1024, "LAN adapter list bound");
        // The returned list includes u64-aligned unions and pointers.
        let mut storage = vec![0_u64; (bytes as usize).div_ceil(size_of::<u64>())];
        let head = storage.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: Aligned storage holds at least bytes writable bytes. A
        // successful result populates linked adapter/address records in it.
        let result = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_INET.0),
                GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                None,
                Some(head),
                &mut bytes,
            )
        };
        if result == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if result != ERROR_SUCCESS.0 {
            return Err(std::io::Error::from_raw_os_error(result as i32))
                .context("read IPv4 adapters");
        }
        let mut selection = None;
        // SAFETY: Success supplies these list records and sockaddr pointers
        // within the live buffer. Address length and family are checked.
        unsafe {
            let mut cursor = head.cast_const();
            while let Some(adapter) = cursor.as_ref() {
                cursor = adapter.Next.cast_const();
                let mut next = adapter.FirstUnicastAddress.cast_const();
                while let Some(address) = next.as_ref() {
                    next = address.Next.cast_const();
                    let socket = address.Address;
                    if socket.lpSockaddr.is_null()
                        || socket.iSockaddrLength < size_of::<SOCKADDR_IN>() as i32
                        || (*socket.lpSockaddr).sa_family != AF_INET
                    {
                        continue;
                    }
                    let ipv4 = (*socket.lpSockaddr.cast::<SOCKADDR_IN>()).sin_addr;
                    if Ipv4Addr::from(ipv4.S_un.S_addr.to_ne_bytes()) != local {
                        continue;
                    }
                    ensure!(selection.is_none(), "LAN IPv4 assigned ambiguously");
                    ensure!(adapter.OperStatus == IfOperStatusUp, "LAN adapter is down");
                    ensure!(
                        address.DadState == IpDadStatePreferred,
                        "LAN IPv4 is not preferred"
                    );
                    let index = adapter.Anonymous1.Anonymous.IfIndex;
                    ensure!(index != 0, "LAN adapter has no IPv4 ifIndex");
                    let mut id = GUID::zeroed();
                    ConvertInterfaceLuidToGuid(&adapter.Luid, &mut id).ok()?;
                    ensure!(id != GUID::zeroed(), "missing LAN adapter GUID");
                    let mut ip = MIB_IPINTERFACE_ROW::default();
                    InitializeIpInterfaceEntry(&mut ip);
                    ip.Family = AF_INET;
                    ip.InterfaceIndex = index;
                    GetIpInterfaceEntry(&mut ip).ok()?;
                    let mtu = adapter.Mtu.min(ip.NlMtu);
                    ensure!(mtu >= MIN_MTU, "LAN IPv4 MTU is below discovery size");
                    selection = Some(Adapter {
                        id,
                        index,
                        prefix: address.OnLinkPrefixLength,
                        mtu,
                        interface_type: adapter.IfType,
                    });
                }
            }
        }
        return selection.context("selected LAN IPv4 is not assigned");
    }
    bail!("LAN adapter list changed during inspection")
}
