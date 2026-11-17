/// VPN Tunnel integration - bridges TUN device with UDP network
///
/// Topology: hub-and-spoke. One machine is the *hub* (reachable on a public
/// UDP port, e.g. port-forwarded). Every other machine is a *client* that
/// points at the hub's address. Clients send keepalives so the hub learns
/// their NAT-mapped address; the hub relays traffic between clients
/// (unicast by learned virtual IP, broadcast/multicast to everyone).
use crate::app::AppState;
use crate::network::crypto::{Cipher, ReplayGuard};
use crate::network::packet_handler;
use crate::system::tun_device::TunDevice;
use anyhow::Result;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

/// Packet types (carried in the authenticated header, see `crypto`).
pub const VPN_TYPE_DATA: u8 = 0x01;
pub const VPN_TYPE_KEEPALIVE: u8 = 0x02;
/// Client -> hub: payload = 8-byte client id. Asks for a virtual IP.
pub const VPN_TYPE_JOIN: u8 = 0x03;
/// Hub -> client: payload = 4-byte virtual IP (empty if the network is full).
pub const VPN_TYPE_ASSIGN: u8 = 0x04;

/// The hub's own virtual IP.
pub const HUB_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 1);

const KEEPALIVE_EVERY: Duration = Duration::from_secs(5);
const PEER_TIMEOUT: Duration = Duration::from_secs(30);

/// VPN Tunnel handler - bridges TUN and UDP
pub struct VpnTunnel {
    tun: TunDevice,
    socket: Arc<UdpSocket>,
    /// Addresses we always talk to (the hub, when we are a client)
    seeds: Vec<SocketAddr>,
    /// If true, relay traffic between peers
    hub: bool,
    state: Arc<AppState>,
    cipher: Arc<Cipher>,
    replay: ReplayGuard,
    our_ip: Ipv4Addr,
    /// Peers we have heard from -> last time
    peers: HashMap<SocketAddr, Instant>,
    /// Virtual IP -> peer address, learned from source IPs of received packets
    vip_map: HashMap<Ipv4Addr, SocketAddr>,
    /// Hub only: client id -> assigned virtual IP
    leases: HashMap<[u8; 8], Ipv4Addr>,
    /// Last time we heard from any seed (client liveness)
    last_heard_seed: Instant,
    /// Rate limit for the "rejected packet" log line
    last_reject_log: Instant,
}

impl VpnTunnel {
    /// Create a tunnel with no seeds (hub that waits for clients).
    pub async fn new(
        tun: TunDevice,
        socket: Arc<UdpSocket>,
        state: Arc<AppState>,
        our_ip: &str,
        cipher: Arc<Cipher>,
    ) -> Result<Self> {
        Self::with_peers(tun, socket, state, our_ip, Vec::new(), cipher).await
    }

    /// Create a tunnel that connects to the given seed addresses (the hub).
    pub async fn with_peers(
        tun: TunDevice,
        socket: Arc<UdpSocket>,
        state: Arc<AppState>,
        our_ip: &str,
        seeds: Vec<SocketAddr>,
        cipher: Arc<Cipher>,
    ) -> Result<Self> {
        let our_ipv4 = our_ip.split('/').next().unwrap_or(our_ip).parse::<Ipv4Addr>()?;
        state.log(format!("🌐 VPN Tunnel created: IP={}", our_ipv4));
        Ok(VpnTunnel {
            tun,
            socket,
            seeds,
            hub: false,
            state,
            cipher,
            replay: ReplayGuard::new(),
            our_ip: our_ipv4,
            peers: HashMap::new(),
            vip_map: HashMap::new(),
            leases: HashMap::new(),
            last_heard_seed: Instant::now(),
            last_reject_log: Instant::now() - Duration::from_secs(60),
        })
    }

    /// Enable relaying between peers (use on the machine everyone connects to).
    pub fn set_hub(mut self, hub: bool) -> Self {
        self.hub = hub;
        if hub {
            self.leases = crate::vpn_config::load_leases();
        }
        self
    }

    /// Main loop: TUN -> UDP, UDP -> TUN, keepalives.
    pub async fn run(&mut self) -> Result<()> {
        self.state.log(format!(
            "🔄 VPN Tunnel loop started ({})",
            if self.hub { "hub" } else { "client" }
        ));

        // TUN reads block, so they live on their own thread.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1024);
        let tun = self.tun.clone();
        let state = self.state.clone();
        std::thread::spawn(move || loop {
            if state.shutdown.load(Ordering::Relaxed) {
                break;
            }
            match tun.read_blocking() {
                Ok(pkt) if pkt.is_empty() => {}
                Ok(pkt) => {
                    if tx.blocking_send(pkt).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    if state.shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    state.log(format!("⚠️ TUN read error: {}", e));
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        });

        let mut udp_buf = vec![0u8; 65536];
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        let mut last_keepalive = Instant::now() - KEEPALIVE_EVERY;

        loop {
            tokio::select! {
                Some(pkt) = rx.recv() => self.handle_tun_packet(&pkt).await,
                res = self.socket.recv_from(&mut udp_buf) => match res {
                    Ok((n, from)) => {
                        let datagram = udp_buf[..n].to_vec();
                        self.handle_remote_packet(&datagram, from).await;
                    }
                    // Windows reports ICMP port-unreachable as a recv error
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                    Err(e) => {
                        self.state.log(format!("⚠️ UDP receive error: {}", e));
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                },
                _ = tick.tick() => {
                    if self.state.shutdown.load(Ordering::Relaxed) {
                        self.state.log("🛑 VPN Tunnel stopped".into());
                        return Ok(());
                    }
                    let now = Instant::now();
                    if now.duration_since(last_keepalive) >= KEEPALIVE_EVERY {
                        last_keepalive = now;
                        self.send_keepalives().await;
                    }
                    self.expire_peers(now);
                    self.update_status(now);
                }
            }
        }
    }

    async fn send_keepalives(&self) {
        let ka = self.cipher.seal(VPN_TYPE_KEEPALIVE, &self.our_ip.octets());
        let targets: Vec<SocketAddr> = self
            .seeds
            .iter()
            .copied()
            .chain(self.peers.keys().copied().filter(|p| !self.seeds.contains(p)))
            .collect();
        for addr in targets {
            let _ = self.socket.send_to(&ka, addr).await;
        }
    }

    fn update_status(&self, now: Instant) {
        if self.hub {
            let n = self.peers.len();
            self.state.connected.store(true, Ordering::Relaxed);
            self.state.set_status(match n {
                0 => "🏠 Hosteando: esperando jugadores".to_string(),
                1 => "🏠 Hosteando: 1 jugador conectado".to_string(),
                n => format!("🏠 Hosteando: {} jugadores conectados", n),
            });
        } else if now.duration_since(self.last_heard_seed) > Duration::from_secs(15) {
            self.state.connected.store(false, Ordering::Relaxed);
            self.state
                .set_status("⚠️ Sin respuesta del host. Reintentando…");
        } else {
            self.state.connected.store(true, Ordering::Relaxed);
            self.state
                .set_status(format!("✅ Conectado. Tu IP virtual es {}", self.our_ip));
        }
    }

    /// Hub: give `from` a virtual IP (stable per client id).
    async fn handle_join(&mut self, payload: &[u8], from: SocketAddr) {
        if payload.len() < 8 {
            return;
        }
        let mut id = [0u8; 8];
        id.copy_from_slice(&payload[..8]);

        let ip = match self.leases.get(&id) {
            Some(ip) => Some(*ip),
            None => {
                let used: std::collections::HashSet<Ipv4Addr> =
                    self.leases.values().copied().chain([self.our_ip]).collect();
                let free = (2u8..=254)
                    .map(|n| Ipv4Addr::new(10, 0, 0, n))
                    .find(|ip| !used.contains(ip));
                if let Some(ip) = free {
                    self.leases.insert(id, ip);
                    crate::vpn_config::save_leases(&self.leases);
                }
                free
            }
        };

        let reply = match ip {
            Some(ip) => {
                self.vip_map.insert(ip, from);
                self.state.log(format!("🎟️ Assigned {} to {}", ip, from));
                self.cipher.seal(VPN_TYPE_ASSIGN, &ip.octets())
            }
            None => self.cipher.seal(VPN_TYPE_ASSIGN, &[]),
        };
        let _ = self.socket.send_to(&reply, from).await;
    }

    fn expire_peers(&mut self, now: Instant) {
        let dead: Vec<SocketAddr> = self
            .peers
            .iter()
            .filter(|(a, t)| !self.seeds.contains(a) && now.duration_since(**t) > PEER_TIMEOUT)
            .map(|(a, _)| *a)
            .collect();
        for addr in dead {
            self.peers.remove(&addr);
            self.vip_map.retain(|_, a| *a != addr);
            self.state.log(format!("👋 Peer {} timed out", addr));
        }
    }

    /// Packet from the local OS (the game): send to the right peer(s).
    async fn handle_tun_packet(&mut self, packet: &[u8]) {
        let Some(ipv4) = packet_handler::get_ipv4_payload(packet) else {
            return; // not IPv4 (e.g. IPv6) - ignore
        };
        if ipv4.len() < 20 {
            return;
        }
        let dst = Ipv4Addr::new(ipv4[16], ipv4[17], ipv4[18], ipv4[19]);
        let datagram = self.cipher.seal(VPN_TYPE_DATA, ipv4);

        let is_bcast = packet_handler::is_broadcast_or_multicast(ipv4);
        if !is_bcast {
            if let Some(addr) = self.vip_map.get(&dst) {
                let _ = self.socket.send_to(&datagram, *addr).await;
                return;
            }
        }
        // Broadcast, or unicast to an unknown virtual IP: everyone we know.
        self.send_to_all(&datagram, None).await;
    }

    /// Datagram from the network.
    async fn handle_remote_packet(&mut self, datagram: &[u8], from: SocketAddr) {
        let Some((kind, payload)) = self.cipher.open(datagram, &mut self.replay) else {
            // Not ours, wrong key, tampered or replayed. Log (rate limited) so a
            // reachability test shows up even when nobody has the right key.
            if self.last_reject_log.elapsed() >= Duration::from_secs(5) {
                self.last_reject_log = Instant::now();
                self.state.log(format!(
                    "📨 Llegó un paquete UDP de {} ({} bytes) que no se pudo descifrar: clave incorrecta o no es Oxide",
                    from,
                    datagram.len()
                ));
            }
            return;
        };
        let payload = payload.as_slice();

        if self.seeds.contains(&from) {
            self.last_heard_seed = Instant::now();
        }
        if kind == VPN_TYPE_JOIN {
            if self.hub {
                self.peers.insert(from, Instant::now());
                self.handle_join(payload, from).await;
            }
            return;
        }
        if kind == VPN_TYPE_ASSIGN {
            return; // only meaningful during connect, handled in request_ip
        }

        if self.peers.insert(from, Instant::now()).is_none() {
            self.state.log(format!("🤝 Peer connected: {}", from));
        }

        if kind == VPN_TYPE_KEEPALIVE {
            if payload.len() >= 4 {
                let vip = Ipv4Addr::new(payload[0], payload[1], payload[2], payload[3]);
                if vip.octets()[..3] == [10, 0, 0]
                    && vip != self.our_ip
                    && self.vip_map.insert(vip, from) != Some(from)
                {
                    self.state.log(format!("📍 {} is at {}", vip, from));
                }
            }
            return;
        }
        if kind != VPN_TYPE_DATA || payload.len() < 20 || payload[0] >> 4 != 4 {
            return;
        }

        let src = Ipv4Addr::new(payload[12], payload[13], payload[14], payload[15]);
        let dst = Ipv4Addr::new(payload[16], payload[17], payload[18], payload[19]);

        // Only accept sources from the virtual network, never our own address
        if src.octets()[..3] != [10, 0, 0] || src == self.our_ip {
            return;
        }
        if self.vip_map.insert(src, from) != Some(from) {
            self.state.log(format!("📍 {} is at {}", src, from));
        }

        let bcast = packet_handler::is_broadcast_or_multicast(payload);

        if dst == self.our_ip || bcast {
            if let Err(e) = self.tun.write_blocking(payload) {
                self.state.log(format!("⚠️ Failed to write to TUN: {}", e));
            }
        }

        if self.hub && dst != self.our_ip {
            if bcast {
                self.send_to_all(datagram, Some(from)).await;
            } else if let Some(addr) = self.vip_map.get(&dst).copied() {
                if addr != from {
                    let _ = self.socket.send_to(datagram, addr).await;
                }
            } else {
                // Unknown destination: flood like a switch
                self.send_to_all(datagram, Some(from)).await;
            }
        }
    }

    /// Send a datagram to every seed and known peer, except `skip`.
    async fn send_to_all(&self, datagram: &[u8], skip: Option<SocketAddr>) {
        let targets: Vec<SocketAddr> = self
            .seeds
            .iter()
            .copied()
            .chain(self.peers.keys().copied())
            .filter(|a| Some(*a) != skip)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for addr in targets {
            if let Err(e) = self.socket.send_to(datagram, addr).await {
                self.state.log(format!("⚠️ Send to {} failed: {}", addr, e));
            }
        }
    }
}

/// Ask the hub for a virtual IP. Retries for ~10 s.
pub async fn request_ip(
    socket: &UdpSocket,
    hub: SocketAddr,
    client_id: [u8; 8],
    cipher: &Cipher,
) -> Result<Ipv4Addr> {
    let join = cipher.seal(VPN_TYPE_JOIN, &client_id);
    let mut replay = ReplayGuard::new();
    let mut buf = [0u8; 1500];
    for _ in 0..10 {
        socket.send_to(&join, hub).await?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match tokio::time::timeout(left, socket.recv_from(&mut buf)).await {
                Ok(Ok((n, from))) if from == hub => {
                    if let Some((VPN_TYPE_ASSIGN, p)) = cipher.open(&buf[..n], &mut replay) {
                        if p.len() == 4 {
                            return Ok(Ipv4Addr::new(p[0], p[1], p[2], p[3]));
                        }
                        anyhow::bail!("el host está lleno (máx. 253 jugadores)");
                    }
                }
                _ => {}
            }
        }
    }
    anyhow::bail!("no hay respuesta del host {}", hub)
}

/// Bind the hub socket on `[::]` accepting both IPv6 and IPv4 clients
/// (IPv4 clients appear as `::ffff:a.b.c.d`). Falls back to IPv4-only.
/// Returns the socket and whether it is dual-stack.
pub fn bind_hub_socket(port: u16) -> Result<(UdpSocket, bool)> {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::net::{IpAddr, Ipv6Addr};

    let dual = (|| -> std::io::Result<UdpSocket> {
        let s = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?;
        s.set_only_v6(false)?; // Windows defaults to v6-only
        s.set_nonblocking(true)?;
        s.bind(&SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port).into())?;
        UdpSocket::from_std(s.into())
    })();
    match dual {
        Ok(sock) => Ok((sock, true)),
        Err(e) => {
            eprintln!("IPv6 bind failed ({}), using IPv4 only", e);
            let std_sock = std::net::UdpSocket::bind(("0.0.0.0", port))?;
            std_sock.set_nonblocking(true)?;
            Ok((UdpSocket::from_std(std_sock)?, false))
        }
    }
}

/// Resolve what the user typed: `1.2.3.4`, `1.2.3.4:9000`, `2a0c::1`,
/// `[2a0c::1]:9000`, `name`, `name:9000`. Default port 9000.
pub async fn resolve_host(text: &str) -> Result<SocketAddr> {
    let text = text.trim();
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = text.parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, 9000));
    }
    let target = if text.contains(':') {
        text.to_string()
    } else {
        format!("{}:9000", text)
    };
    tokio::net::lookup_host(&target)
        .await
        .ok()
        .and_then(|mut it| it.next())
        .ok_or_else(|| anyhow::anyhow!("no se encuentra el host \"{}\"", text))
}

/// Run as the hub (host). Blocks until shutdown.
pub async fn run_host(state: Arc<AppState>, port: u16, key: &str) -> Result<()> {
    state.set_status("⏳ Iniciando host…");
    let res = async {
        let cipher = Arc::new(Cipher::new(key)?);
        let ip = HUB_IP.to_string();
        let tun = tokio::task::spawn_blocking(move || TunDevice::new(&ip)).await??;
        let (socket, dual) = bind_hub_socket(port)?;
        let socket = Arc::new(socket);
        state.log(if dual {
            format!("📡 Escuchando en UDP {} (IPv6 + IPv4)", port)
        } else {
            format!("📡 Escuchando en UDP {} (solo IPv4: no se pudo abrir IPv6)", port)
        });
        let mut tunnel = VpnTunnel::new(tun, socket, state.clone(), &HUB_IP.to_string(), cipher)
            .await?
            .set_hub(true);
        tunnel.run().await
    }
    .await;
    state.connected.store(false, Ordering::Relaxed);
    match &res {
        Ok(_) => state.set_status("Desconectado"),
        Err(e) => state.set_status(format!("❌ Error al hostear: {}", e)),
    }
    res
}

/// Run as a client: get an IP from the hub, create the adapter, tunnel. Blocks until shutdown.
pub async fn run_client(state: Arc<AppState>, hub: SocketAddr, key: &str) -> Result<()> {
    state.set_status("⏳ Conectando con el host…");
    let res = async {
        let cipher = Arc::new(Cipher::new(key)?);
        let bind_addr = if hub.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
        let socket = Arc::new(UdpSocket::bind(bind_addr).await?);
        let ip = request_ip(&socket, hub, crate::vpn_config::client_id(), &cipher)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "{}. Revisa la dirección, que el host tenga UDP abierto y que la clave sea la misma",
                    e
                )
            })?;
        state.log(format!("🎟️ El host te asignó {}", ip));
        let ip_s = ip.to_string();
        let tun = tokio::task::spawn_blocking(move || TunDevice::new(&ip_s))
            .await?
            .map_err(|e| {
                anyhow::anyhow!("no se pudo crear el adaptador de red ({}). ¿Administrador?", e)
            })?;
        let mut tunnel =
            VpnTunnel::with_peers(tun, socket, state.clone(), &ip.to_string(), vec![hub], cipher)
                .await?;
        tunnel.run().await
    }
    .await;
    state.connected.store(false, Ordering::Relaxed);
    match &res {
        Ok(_) => state.set_status("Desconectado"),
        Err(e) => state.set_status(format!("❌ {}", e)),
    }
    res
}
