use super::Binding;
use crate::PacketInfo;
use anyhow::{Context, Result, bail, ensure};
use kilogram_protocol::ordered_dialog_v2::lan_discovery::{DATAGRAM_BYTES, GROUP, PORT};
use std::{
    io,
    mem::{offset_of, size_of},
    net::{Ipv4Addr, SocketAddrV4},
    ptr,
    sync::Arc,
    time::Instant,
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Networking::WinSock::{
            AF_INET, CMSGHDR, FD_READ, FD_READ_BIT, IN_ADDR, IN_ADDR_0, IN_PKTINFO,
            IP_ADD_MEMBERSHIP, IP_DONTFRAGMENT, IP_MULTICAST_IF, IP_MULTICAST_LOOP,
            IP_MULTICAST_TTL, IP_PKTINFO, IPPROTO_IP, IPPROTO_UDP, LPFN_WSARECVMSG, MSG_CTRUNC,
            MSG_TRUNC, SIO_GET_EXTENSION_FUNCTION_POINTER, SO_EXCLUSIVEADDRUSE, SOCK_DGRAM,
            SOCKADDR_IN, SOCKET, SOCKET_ERROR, SOL_SOCKET, WSA_FLAG_NO_HANDLE_INHERIT,
            WSA_WAIT_TIMEOUT, WSABUF, WSACleanup, WSACreateEvent, WSADATA, WSAEMSGSIZE, WSAEVENT,
            WSAEWOULDBLOCK, WSAEnumNetworkEvents, WSAEventSelect, WSAGetLastError,
            WSAID_WSARECVMSG, WSAIoctl, WSAMSG, WSANETWORKEVENTS, WSASetEvent, WSASocketW,
            WSAStartup, WSAWaitForMultipleEvents, bind, sendto, setsockopt,
        },
    },
    core::{Owned, PSTR},
};

fn socket_error() -> io::Error {
    // SAFETY: Reads the current thread's Winsock error state.
    io::Error::from_raw_os_error(unsafe { WSAGetLastError().0 })
}

fn check(result: i32) -> io::Result<i32> {
    if result == SOCKET_ERROR {
        Err(socket_error())
    } else {
        Ok(result)
    }
}

struct Winsock;

impl Winsock {
    fn start() -> io::Result<Self> {
        let mut data = WSADATA::default();
        // SAFETY: data is writable, properly aligned WSADATA.
        let result = unsafe { WSAStartup(0x0202, &mut data) };
        if result == 0 {
            Ok(Self)
        } else {
            Err(io::Error::from_raw_os_error(result))
        }
    }
}

impl Drop for Winsock {
    fn drop(&mut self) {
        // SAFETY: This guard represents one successful WSAStartup call.
        unsafe { WSACleanup() };
    }
}

struct Event {
    raw: Owned<WSAEVENT>,
    _winsock: Arc<Winsock>,
}

impl Event {
    fn new(winsock: Arc<Winsock>) -> Result<Self> {
        // SAFETY: Winsock is initialized and the event is newly owned.
        let raw = unsafe { Owned::new(WSACreateEvent()?) };
        Ok(Self {
            raw,
            _winsock: winsock,
        })
    }

    fn handle(&self) -> HANDLE {
        HANDLE(self.raw.0 as _)
    }
}

/// A one-shot, waitable cancellation event for the socket's dedicated worker.
#[derive(Clone)]
pub struct Cancellation(Arc<Event>);

impl Cancellation {
    pub fn cancel(&self) -> Result<()> {
        // SAFETY: The shared Event keeps the event and Winsock alive.
        unsafe { WSASetEvent(*self.0.raw) }?;
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Receive {
    Packet(PacketInfo),
    Truncated,
    WrongLength,
    OutsideInterface,
    Cancelled,
    TimedOut,
}

/// A fixed, exclusive IPv4 listener. This is a blocking-worker API, never a
/// public runtime listener or a peer authority check. Revalidate the binding
/// before use. Packet source still needs the exact peer predicate.
pub struct DiscoverySocket {
    socket: Owned<SOCKET>,
    readable: Event,
    cancellation: Cancellation,
    recv_msg: LPFN_WSARECVMSG,
    binding: Binding,
}

fn address(ip: Ipv4Addr, port: u16) -> SOCKADDR_IN {
    SOCKADDR_IN {
        sin_family: AF_INET,
        sin_port: port.to_be(),
        sin_addr: IN_ADDR {
            S_un: IN_ADDR_0 {
                S_addr: u32::from_ne_bytes(ip.octets()),
            },
        },
        sin_zero: [0; 8],
    }
}

fn option(socket: SOCKET, level: i32, name: i32, bytes: &[u8]) -> io::Result<()> {
    // SAFETY: The socket is owned by the caller, all option buffers live
    // through the call and their lengths are supplied by the slice.
    check(unsafe { setsockopt(socket, level, name, Some(bytes)) }).map(|_| ())
}

impl DiscoverySocket {
    pub fn open(binding: Binding) -> Result<Self> {
        Self::open_observed(binding.refresh()?)
    }

    /// Open the exact socket following a controller's fresh, checked observation.
    /// The controller must discard this socket if that observation expires.
    pub fn open_observed(binding: Binding) -> Result<Self> {
        let winsock = Arc::new(Winsock::start()?);
        // SAFETY: Winsock is initialized and the new IPv4 datagram socket is
        // returned with unique ownership and inheritance disabled.
        let socket = unsafe {
            Owned::new(WSASocketW(
                i32::from(AF_INET.0),
                SOCK_DGRAM.0,
                IPPROTO_UDP.0,
                None,
                0,
                WSA_FLAG_NO_HANDLE_INHERIT,
            )?)
        };
        let level = IPPROTO_IP.0;
        let enabled = 1_u32.to_ne_bytes();
        option(*socket, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, &enabled).context("exclusive LAN UDP")?;
        // Wildcard bind is for multicast reception. Advertisements and joins
        // select the exact adapter separately; receive verifies packet info.
        let local = address(Ipv4Addr::UNSPECIFIED, PORT);
        // SAFETY: local is an IPv4 SOCKADDR_IN of the given length.
        check(unsafe {
            bind(
                *socket,
                ptr::from_ref(&local).cast(),
                size_of::<SOCKADDR_IN>() as i32,
            )
        })
        .context("bind discovery UDP port")?;
        option(*socket, level, IP_PKTINFO, &enabled).context("enable IPv4 packet info")?;
        option(*socket, level, IP_DONTFRAGMENT, &enabled).context("disable IPv4 fragmentation")?;
        option(*socket, level, IP_MULTICAST_TTL, &enabled).context("set multicast TTL 1")?;
        option(*socket, level, IP_MULTICAST_IF, &binding.local().octets())
            .context("select multicast egress IPv4")?;
        option(*socket, level, IP_MULTICAST_LOOP, &enabled).context("set multicast loopback")?;
        let [a, b, c, d] = GROUP.octets();
        let [e, f, g, h] = binding.local().octets();
        // Winsock ip_mreq is two consecutive IN_ADDR values in network order.
        option(*socket, level, IP_ADD_MEMBERSHIP, &[a, b, c, d, e, f, g, h])
            .context("join discovery multicast on selected IPv4")?;

        let mut recv_msg: LPFN_WSARECVMSG = None;
        let mut returned = 0;
        // SAFETY: SIO_GET_EXTENSION_FUNCTION_POINTER reads the documented
        // GUID and writes the pointer into a live LPFN_WSARECVMSG slot.
        check(unsafe {
            WSAIoctl(
                *socket,
                SIO_GET_EXTENSION_FUNCTION_POINTER,
                Some(ptr::from_ref(&WSAID_WSARECVMSG).cast()),
                size_of_val(&WSAID_WSARECVMSG) as u32,
                Some(ptr::from_mut(&mut recv_msg).cast()),
                size_of::<LPFN_WSARECVMSG>() as u32,
                &mut returned,
                None,
                None,
            )
        })
        .context("get WSARecvMsg")?;
        ensure!(
            returned as usize == size_of::<LPFN_WSARECVMSG>() && recv_msg.is_some(),
            "missing WSARecvMsg extension"
        );
        let readable = Event::new(winsock.clone())?;
        let cancellation = Cancellation(Arc::new(Event::new(winsock)?));
        // SAFETY: The socket outlives the registration and closes before the
        // associated event. Event selection also sets non-blocking mode.
        check(unsafe { WSAEventSelect(*socket, Some(*readable.raw), FD_READ as i32) })
            .context("watch discovery UDP readiness")?;
        Ok(Self {
            socket,
            readable,
            cancellation,
            recv_msg,
            binding,
        })
    }

    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    pub fn send(&self, frame: &[u8; DATAGRAM_BYTES]) -> Result<()> {
        self.send_to_group(frame)
    }

    fn send_to_group(&self, frame: &[u8]) -> Result<()> {
        let destination = address(GROUP, PORT);
        // SAFETY: The frame and address live through this non-blocking call.
        let sent = check(unsafe {
            sendto(
                *self.socket,
                frame,
                0,
                ptr::from_ref(&destination).cast(),
                size_of::<SOCKADDR_IN>() as i32,
            )
        })
        .context("send discovery multicast")?;
        ensure!(
            sent as usize == frame.len(),
            "incomplete discovery datagram"
        );
        Ok(())
    }

    pub fn receive(
        &mut self,
        frame: &mut [u8; DATAGRAM_BYTES],
        deadline: Instant,
    ) -> Result<Receive> {
        let events = [self.cancellation.0.handle(), self.readable.handle()];
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(Receive::TimedOut);
            }
            let wait_ms = remaining.as_millis().clamp(1, u128::from(u32::MAX - 1)) as u32;
            // SAFETY: Both handles live across the call. Cancellation is first
            // so it wins if both events are signaled.
            let signaled = unsafe { WSAWaitForMultipleEvents(&events, false, wait_ms, false) };
            match signaled.0 {
                0 => return Ok(Receive::Cancelled),
                1 => {
                    let mut pending = WSANETWORKEVENTS::default();
                    // SAFETY: This is the exact event registered for the live socket.
                    check(unsafe {
                        WSAEnumNetworkEvents(*self.socket, *self.readable.raw, &mut pending)
                    })?;
                    let error = pending.iErrorCode[FD_READ_BIT as usize];
                    if error != 0 {
                        return Err(io::Error::from_raw_os_error(error))
                            .context("discovery FD_READ");
                    }
                    if let Some(packet) = self.receive_ready(frame)? {
                        return Ok(packet);
                    }
                    if Instant::now() >= deadline {
                        return Ok(Receive::TimedOut);
                    }
                }
                WSA_WAIT_TIMEOUT => return Ok(Receive::TimedOut),
                _ => return Err(socket_error()).context("wait for discovery events"),
            }
        }
    }

    fn receive_ready(&self, frame: &mut [u8; DATAGRAM_BYTES]) -> Result<Option<Receive>> {
        #[repr(C)]
        #[derive(Default)]
        struct Control {
            header: CMSGHDR,
            info: IN_PKTINFO,
        }
        const CONTROL_LEN: usize = offset_of!(Control, info) + size_of::<IN_PKTINFO>();
        let mut source = SOCKADDR_IN::default();
        let mut ancillary = Control::default();
        let mut data = WSABUF {
            len: DATAGRAM_BYTES as u32,
            buf: PSTR(frame.as_mut_ptr()),
        };
        let mut message = WSAMSG {
            name: ptr::from_mut(&mut source).cast(),
            namelen: size_of::<SOCKADDR_IN>() as i32,
            lpBuffers: &mut data,
            dwBufferCount: 1,
            Control: WSABUF {
                len: size_of::<Control>() as u32,
                buf: PSTR(ptr::from_mut(&mut ancillary).cast()),
            },
            dwFlags: 0,
        };
        let mut transferred = 0;
        let recv = self
            .recv_msg
            .context("missing discovery receive function")?;
        // SAFETY: The buffers are live, properly aligned and sized as passed.
        // No OVERLAPPED or completion routine is supplied.
        match check(unsafe {
            recv(
                *self.socket,
                &mut message,
                &mut transferred,
                ptr::null_mut(),
                None,
            )
        }) {
            Ok(_) => {}
            Err(error) if error.raw_os_error() == Some(WSAEWOULDBLOCK.0) => return Ok(None),
            Err(error) if error.raw_os_error() == Some(WSAEMSGSIZE.0) => {
                return Ok(Some(Receive::Truncated));
            }
            Err(error) => return Err(error).context("receive discovery datagram"),
        }
        if message.dwFlags & (MSG_TRUNC | MSG_CTRUNC) != 0 {
            return Ok(Some(Receive::Truncated));
        }
        if transferred as usize != DATAGRAM_BYTES {
            return Ok(Some(Receive::WrongLength));
        }
        if message.namelen as usize != size_of::<SOCKADDR_IN>()
            || source.sin_family != AF_INET
            || (message.Control.len as usize) < CONTROL_LEN
            || ancillary.header.cmsg_len != CONTROL_LEN
            || ancillary.header.cmsg_level != IPPROTO_IP.0
            || ancillary.header.cmsg_type != IP_PKTINFO
            || ancillary.info.ipi_ifindex == 0
        {
            bail!("discovery IPv4 packet info missing or invalid");
        }
        // SAFETY: Valid AF_INET sockaddr and IP_PKTINFO put IN_ADDR values
        // into these unions. Source and destination are distinct fields.
        let (source_ip, destination) = unsafe {
            (
                Ipv4Addr::from(source.sin_addr.S_un.S_addr.to_ne_bytes()),
                Ipv4Addr::from(ancillary.info.ipi_addr.S_un.S_addr.to_ne_bytes()),
            )
        };
        let if_index = ancillary.info.ipi_ifindex;
        if if_index != self.binding.if_index() || destination != GROUP {
            return Ok(Some(Receive::OutsideInterface));
        }
        Ok(Some(Receive::Packet(PacketInfo {
            source: SocketAddrV4::new(source_ip, u16::from_be(source.sin_port)),
            destination,
            if_index,
        })))
    }
}

#[cfg(all(test, feature = "native-feasibility"))]
mod feasibility {
    use super::*;
    use crate::{inspect, is_peer_datagram};
    use kilogram_protocol::ordered_dialog_v2::DirectLan2;
    use std::{env, net::UdpSocket, sync::mpsc, thread, time::Duration};
    use windows::Win32::{
        NetworkManagement::IpHelper::IF_TYPE_IEEE80211,
        Networking::WinSock::{WSAEACCES, WSAEADDRINUSE, getsockopt},
    };

    fn read_option(socket: &DiscoverySocket, name: i32) -> Result<[u8; 4]> {
        let mut value = [0_u8; 4];
        let mut size = size_of_val(&value) as i32;
        // SAFETY: The socket is live and the buffer holds the requested DWORD.
        check(unsafe {
            getsockopt(
                *socket.socket,
                IPPROTO_IP.0,
                name,
                PSTR(value.as_mut_ptr()),
                &mut size,
            )
        })?;
        ensure!(
            size as usize == value.len(),
            "unexpected discovery option size"
        );
        Ok(value)
    }

    #[test]
    fn lan_native_private_wifi_feasibility() -> Result<()> {
        let local = env::var("KILOGRAM_T13B_NATIVE_LOCAL")?.parse()?;
        let peer = env::var("KILOGRAM_T13B_NATIVE_PEER")?.parse()?;
        let route = DirectLan2::new(local, peer)?;
        let binding = inspect(route, None)?;
        assert_eq!(binding.interface_type(), IF_TYPE_IEEE80211);
        let scope = binding.scope();
        assert_eq!(inspect(route, Some(scope))?, binding);
        assert!(
            inspect(
                route,
                Some(crate::NetworkScope {
                    network_id: windows::core::GUID::zeroed(),
                    ..scope
                })
            )
            .is_err()
        );
        println!(
            "native_nlm=Private local={local} prefix={} ifindex={} mtu={} adapter={:?} network={:?}",
            scope.prefix,
            binding.if_index(),
            binding.mtu(),
            scope.adapter_id,
            scope.network_id
        );

        let competing = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, PORT))
            .context("bind owned competing discovery socket")?;
        let failure = match DiscoverySocket::open(binding) {
            Ok(_) => bail!("unexpected discovery listener despite owned UDP port conflict"),
            Err(error) => error,
        };
        let code = failure
            .downcast_ref::<io::Error>()
            .and_then(io::Error::raw_os_error);
        assert!(
            code == Some(WSAEACCES.0) || code == Some(WSAEADDRINUSE.0),
            "{failure:#}"
        );
        println!("native_owned_port_conflict={code:?}");
        drop(competing);

        let mut socket = DiscoverySocket::open(binding)?;
        let ttl = u32::from_ne_bytes(read_option(&socket, IP_MULTICAST_TTL)?);
        let dont_fragment = u32::from_ne_bytes(read_option(&socket, IP_DONTFRAGMENT)?);
        let egress = Ipv4Addr::from(read_option(&socket, IP_MULTICAST_IF)?);
        assert_eq!(ttl, 1);
        assert_eq!(dont_fragment, 1);
        assert_eq!(egress, local);
        let payload = [0x69_u8; DATAGRAM_BYTES];
        let mut output = [0; DATAGRAM_BYTES];
        socket.send(&payload)?;
        let recv = socket.receive(&mut output, Instant::now() + Duration::from_secs(3))?;
        let Receive::Packet(info) = recv else {
            bail!("no packet-info self-receive: {recv:?}");
        };
        assert!(output == payload, "synthetic self-receive payload mismatch");
        assert_eq!(*info.source.ip(), local);
        assert_eq!(info.destination, GROUP);
        assert_eq!(info.if_index, binding.if_index());
        assert!(!is_peer_datagram(info, peer, binding.if_index()));
        println!(
            "native_self_receive=true bytes={} source={} destination={} ifindex={} ttl={ttl} egress={egress} df={dont_fragment} remote_peer_qualified=false",
            output.len(),
            info.source,
            info.destination,
            info.if_index
        );

        socket.send_to_group(&[0_u8; DATAGRAM_BYTES + 1])?;
        socket.send_to_group(&[0_u8; 8])?;
        let length_deadline = Instant::now() + Duration::from_secs(3);
        let mut truncated = false;
        let mut short = false;
        let mut duplicate_self_receive = 0;
        while !truncated || !short {
            match socket.receive(&mut output, length_deadline)? {
                Receive::Truncated => truncated = true,
                Receive::WrongLength => short = true,
                Receive::Packet(copy) if copy == info && output == payload => {
                    duplicate_self_receive += 1;
                }
                other => bail!("missing synthetic length observations: {other:?}"),
            }
        }
        // Timeout and cancellation test an idle socket independently of any
        // queued or duplicated datagrams from the send-side probes above.
        drop(socket);
        let mut socket = DiscoverySocket::open(binding)?;
        assert_eq!(
            socket.receive(&mut output, Instant::now() + Duration::from_millis(25))?,
            Receive::TimedOut
        );
        let cancellation = socket.cancellation();
        let (started, ready) = mpsc::sync_channel(0);
        let worker = thread::spawn(move || {
            started.send(())?;
            socket.receive(&mut output, Instant::now() + Duration::from_secs(5))
        });
        ready.recv_timeout(Duration::from_secs(1))?;
        thread::sleep(Duration::from_millis(40));
        let cancel_at = Instant::now();
        cancellation.cancel()?;
        assert_eq!(
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("discovery worker panicked"))??,
            Receive::Cancelled
        );
        let latency = cancel_at.elapsed();
        assert!(latency < Duration::from_secs(1));
        drop(cancellation);
        drop(DiscoverySocket::open(binding)?);
        println!(
            "native_truncation=true short_frame_rejected=true duplicate_self_receive={duplicate_self_receive} idle_timeout=true cancel_ms={} own_port_reopened=true",
            latency.as_millis()
        );
        Ok(())
    }
}
