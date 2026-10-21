//! Tools menu: TCP connect port scanner and network scanner (no raw packets, no root).
//!
//! Ported from PlexusTerm v0.2.15 `src-tauri/src/tools/scan.rs`: results stream through a
//! [`ScanSink`] callback instead of a Tauri channel, and [`ScanManager`] runs scans on the
//! current Tokio runtime.

use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::task::JoinSet;

use crate::lock;

pub const MAX_PROBES: usize = 65536;
pub const CONCURRENCY: usize = 256;
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(1);
pub const NETWORK_PORTS: [u16; 8] = [21, 22, 23, 80, 443, 445, 3389, 5900];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortState {
    Open,
    Closed,
    Filtered,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub host: IpAddr,
    pub port: u16,
    pub state: PortState,
}

/// What a running scan reports, in order: `Started`, then `Open`/`Host` and `Progress` as
/// probes complete, and always `Done` last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanEvent {
    Started {
        total: usize,
    },
    Open {
        host: String,
        port: u16,
        service: Option<String>,
    },
    Progress {
        done: usize,
        total: usize,
    },
    Host {
        host: String,
        name: Option<String>,
        mac: Option<String>,
        ports: Vec<u16>,
    },
    Done {
        done: usize,
        total: usize,
        cancelled: bool,
    },
}

/// Receives the events of a scan, from the scan's task.
pub type ScanSink = Arc<dyn Fn(ScanEvent) + Send + Sync>;

/// Cancellation shared by a running scan and [`ScanManager::stop`]; wakes the scan immediately.
#[derive(Default)]
pub struct CancelFlag {
    cancelled: AtomicBool,
    notify: Notify,
}

impl CancelFlag {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    async fn cancelled(&self) {
        loop {
            // Created before the check, so a cancel() in between is not missed.
            let notified = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

const IPV4_TARGETS_HINT: &str = "Enter an IPv4 address, range (192.168.1.10-20) or CIDR (192.168.1.0/24)";

/// Parses `a.b.c.d`, `a.b.c.d-e` (last-octet range) or `a.b.c.d/n` with n in 24..=32
/// (network and broadcast addresses skipped up to /30). Never more than 256 hosts.
pub fn parse_ipv4_targets(text: &str) -> Result<Vec<Ipv4Addr>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(IPV4_TARGETS_HINT.to_string());
    }
    let address = |part: &str| {
        part.parse::<Ipv4Addr>()
            .map_err(|_| format!("Invalid IPv4 address: {part}"))
    };
    if let Some((base, prefix)) = text.split_once('/') {
        let base = address(base)?;
        let prefix = prefix
            .parse::<u32>()
            .ok()
            .filter(|p| (24..=32).contains(p))
            .ok_or_else(|| "CIDR prefix must be between /24 and /32 (at most 256 hosts)".to_string())?;
        let size = 1u32 << (32 - prefix);
        let network = u32::from(base) & !(size - 1);
        let (first, last) = if prefix <= 30 {
            (network + 1, network + size - 2)
        } else {
            (network, network + size - 1)
        };
        return Ok((first..=last).map(Ipv4Addr::from).collect());
    }
    if let Some((start, end)) = text.split_once('-') {
        let start = address(start)?;
        let [a, b, c, d] = start.octets();
        let end = end
            .parse::<u8>()
            .ok()
            .filter(|e| *e >= d)
            .ok_or_else(|| format!("Invalid range: {text} (use a.b.c.d-e with e between d and 255)"))?;
        return Ok((d..=end).map(|last| Ipv4Addr::new(a, b, c, last)).collect());
    }
    Ok(vec![address(text)?])
}

/// Parses a host name (resolved now) or any IPv4 form of [`parse_ipv4_targets`].
pub async fn parse_targets(text: &str) -> Result<Vec<IpAddr>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Enter a host, IPv4 address or range".to_string());
    }
    if text.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '/')) {
        return Ok(parse_ipv4_targets(text)?.into_iter().map(IpAddr::V4).collect());
    }
    if let Ok(address) = text.parse::<IpAddr>() {
        return Ok(vec![address]);
    }
    if !text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(format!("Invalid host: {text}"));
    }
    let unresolved = || format!("Could not resolve host: {text}");
    let mut addresses: Vec<IpAddr> = tokio::net::lookup_host((text, 0))
        .await
        .map_err(|_| unresolved())?
        .map(|a| a.ip())
        .collect();
    // Prefer IPv4, which is what most LAN services listen on.
    addresses.sort_by_key(|a| !a.is_ipv4());
    addresses.first().map(|a| vec![*a]).ok_or_else(unresolved)
}

/// Parses `22,80,8000-8100` into sorted, unique ports in 1..=65535.
pub fn parse_ports(text: &str) -> Result<Vec<u16>, String> {
    let mut ports = BTreeSet::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let port = |p: &str| p.trim().parse::<u16>().ok().filter(|p| *p >= 1);
        if let Some((start, end)) = part.split_once('-') {
            match (port(start), port(end)) {
                (Some(start), Some(end)) if start <= end => ports.extend(start..=end),
                _ => return Err(format!("Invalid port range: {part}")),
            }
        } else {
            ports.insert(port(part).ok_or_else(|| format!("Invalid port: {part} (use numbers from 1 to 65535)"))?);
        }
    }
    if ports.is_empty() {
        return Err("Enter at least one port".to_string());
    }
    Ok(ports.into_iter().collect())
}

pub fn check_probe_count(hosts: usize, ports: usize) -> Result<usize, String> {
    let total = hosts.saturating_mul(ports);
    if total > MAX_PROBES {
        return Err(format!(
            "Too many probes: {total} (the limit is {MAX_PROBES}; scan fewer hosts or ports)"
        ));
    }
    Ok(total)
}

pub fn service_name(port: u16) -> Option<&'static str> {
    Some(match port {
        20 => "ftp-data",
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "dns",
        69 => "tftp",
        80 => "http",
        110 => "pop3",
        111 => "rpcbind",
        123 => "ntp",
        135 => "msrpc",
        139 => "netbios-ssn",
        143 => "imap",
        161 => "snmp",
        389 => "ldap",
        443 => "https",
        445 => "smb",
        465 => "smtps",
        514 => "syslog",
        587 => "submission",
        631 => "ipp",
        636 => "ldaps",
        873 => "rsync",
        993 => "imaps",
        995 => "pop3s",
        1433 => "mssql",
        1521 => "oracle",
        1883 => "mqtt",
        2049 => "nfs",
        2375 => "docker",
        3000 => "http-dev",
        3306 => "mysql",
        3389 => "rdp",
        5000 => "upnp",
        5432 => "postgresql",
        5672 => "amqp",
        5900 => "vnc",
        6379 => "redis",
        8000 => "http-alt",
        8080 => "http-alt",
        8443 => "https-alt",
        9000 => "http-alt",
        9090 => "http-alt",
        9200 => "elasticsearch",
        11211 => "memcached",
        27017 => "mongodb",
        _ => return None,
    })
}

/// Parses Linux `/proc/net/arp`, keeping only complete entries (MAC lowercased).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_arp_table(text: &str) -> HashMap<Ipv4Addr, String> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (address, flags, mac) = (fields.first()?, fields.get(2)?, fields.get(3)?);
            let complete = u32::from_str_radix(flags.trim_start_matches("0x"), 16).ok()? & 0x2 != 0;
            (complete && *mac != "00:00:00:00:00:00").then(|| Some((address.parse().ok()?, mac.to_lowercase())))?
        })
        .collect()
}

/// The canonical name of `getent hosts <ip>` output (`<ip> <name> [aliases]`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_getent_hosts(output: &str) -> Option<String> {
    output.lines().next()?.split_whitespace().nth(1).map(str::to_string)
}

/// Reverse DNS through the system resolver (NSS: DNS, mDNS, /etc/hosts) via `getent`.
#[cfg(target_os = "linux")]
async fn reverse_dns(ip: Ipv4Addr) -> Option<String> {
    let lookup = tokio::process::Command::new("getent")
        .arg("hosts")
        .arg(ip.to_string())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(2), lookup).await.ok()?.ok()?;
    if !output.status.success() {
        return None;
    }
    parse_getent_hosts(&String::from_utf8_lossy(&output.stdout)).filter(|name| *name != ip.to_string())
}

#[cfg(not(target_os = "linux"))]
async fn reverse_dns(_ip: Ipv4Addr) -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
async fn arp_mac(ip: Ipv4Addr) -> Option<String> {
    let table = tokio::fs::read_to_string("/proc/net/arp").await.ok()?;
    parse_arp_table(&table).remove(&ip)
}

#[cfg(not(target_os = "linux"))]
async fn arp_mac(_ip: Ipv4Addr) -> Option<String> {
    None
}

async fn probe(address: SocketAddr, timeout: Duration) -> PortState {
    match tokio::time::timeout(timeout, TcpStream::connect(address)).await {
        Ok(Ok(_)) => PortState::Open,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => PortState::Closed,
        _ => PortState::Filtered,
    }
}

/// TCP connect probes of every host x port, at most `concurrency` at a time, reported as
/// they complete. Returns whether the scan was cancelled (in-flight probes are aborted).
pub async fn probe_all<F: FnMut(Outcome)>(
    hosts: &[IpAddr],
    ports: &[u16],
    timeout: Duration,
    concurrency: usize,
    cancel: &CancelFlag,
    mut on_outcome: F,
) -> bool {
    let total = hosts.len() * ports.len();
    let mut next = 0;
    let mut running = JoinSet::new();
    loop {
        while running.len() < concurrency.max(1) && next < total && !cancel.is_cancelled() {
            let (Some(&host), Some(&port)) = (hosts.get(next / ports.len()), ports.get(next % ports.len())) else {
                break;
            };
            next += 1;
            running.spawn(async move {
                Outcome {
                    host,
                    port,
                    state: probe(SocketAddr::new(host, port), timeout).await,
                }
            });
        }
        if cancel.is_cancelled() {
            running.abort_all();
            return true;
        }
        tokio::select! {
            _ = cancel.cancelled() => {
                running.abort_all();
                return true;
            }
            next = running.join_next() => match next {
                Some(Ok(outcome)) => on_outcome(outcome),
                Some(Err(_)) => {}
                None => return false,
            }
        }
    }
}

/// Sends progress every this many probes (the final count goes in `Done`).
const PROGRESS_EVERY: usize = 32;

pub async fn run_port_scan(
    hosts: Vec<IpAddr>,
    ports: Vec<u16>,
    timeout: Duration,
    cancel: Arc<CancelFlag>,
    emit: ScanSink,
) {
    let total = hosts.len() * ports.len();
    emit(ScanEvent::Started { total });
    let mut done = 0;
    let cancelled = probe_all(&hosts, &ports, timeout, CONCURRENCY, &cancel, |outcome| {
        done += 1;
        if outcome.state == PortState::Open {
            emit(ScanEvent::Open {
                host: outcome.host.to_string(),
                port: outcome.port,
                service: service_name(outcome.port).map(str::to_string),
            });
        }
        if done % PROGRESS_EVERY == 0 {
            emit(ScanEvent::Progress { done, total });
        }
    })
    .await;
    emit(ScanEvent::Done { done, total, cancelled });
}

/// Finds live hosts: a host is live when any probed port answers (open or refused).
/// Each live host is reported once all its ports are probed, with its name and MAC.
pub async fn run_network_scan(
    hosts: Vec<Ipv4Addr>,
    ports: &[u16],
    timeout: Duration,
    cancel: Arc<CancelFlag>,
    emit: ScanSink,
) {
    let addresses: Vec<IpAddr> = hosts.iter().copied().map(IpAddr::V4).collect();
    let total = addresses.len() * ports.len();
    emit(ScanEvent::Started { total });
    // host -> (probes left, answered, open ports)
    let mut pending: HashMap<IpAddr, (usize, bool, Vec<u16>)> = HashMap::new();
    let mut describing = JoinSet::new();
    let mut done = 0;
    let cancelled = probe_all(&addresses, ports, timeout, CONCURRENCY, &cancel, |outcome| {
        done += 1;
        if done % PROGRESS_EVERY == 0 {
            emit(ScanEvent::Progress { done, total });
        }
        let entry = pending.entry(outcome.host).or_insert((ports.len(), false, Vec::new()));
        entry.0 -= 1;
        entry.1 |= outcome.state != PortState::Filtered;
        if outcome.state == PortState::Open {
            entry.2.push(outcome.port);
        }
        if let (0, true, IpAddr::V4(ip)) = (entry.0, entry.1, outcome.host) {
            let mut open = std::mem::take(&mut entry.2);
            open.sort_unstable();
            let emit = emit.clone();
            describing.spawn(async move {
                let (name, mac) = tokio::join!(reverse_dns(ip), arp_mac(ip));
                emit(ScanEvent::Host {
                    host: ip.to_string(),
                    name,
                    mac,
                    ports: open,
                });
            });
        }
    })
    .await;
    let cancelled = if cancelled {
        describing.abort_all();
        true
    } else {
        tokio::select! {
            _ = cancel.cancelled() => {
                describing.abort_all();
                true
            }
            _ = async { while describing.join_next().await.is_some() {} } => false,
        }
    };
    emit(ScanEvent::Done { done, total, cancelled });
}

/// Scans in progress, by id, so [`ScanManager::stop`] can cancel them.
#[derive(Default)]
pub struct ScanManager {
    next_id: AtomicU32,
    running: Arc<Mutex<HashMap<u32, Arc<CancelFlag>>>>,
}

impl ScanManager {
    /// Runs `scan` on the current Tokio runtime under a new id until it ends.
    fn start<F, Fut>(&self, scan: F) -> u32
    where
        F: FnOnce(Arc<CancelFlag>) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let cancel = Arc::new(CancelFlag::default());
        lock(&self.running).insert(id, cancel.clone());
        let running = self.running.clone();
        let task = scan(cancel);
        tokio::spawn(async move {
            task.await;
            lock(&running).remove(&id);
        });
        id
    }

    /// Starts a TCP connect scan of `targets` x `ports` on the current Tokio runtime; results
    /// stream through `emit`. Returns the scan id for [`ScanManager::stop`].
    pub async fn scan_ports(&self, targets: &str, ports: &str, emit: ScanSink) -> Result<u32, String> {
        let hosts = parse_targets(targets).await?;
        let ports = parse_ports(ports)?;
        check_probe_count(hosts.len(), ports.len())?;
        Ok(self.start(move |cancel| run_port_scan(hosts, ports, PROBE_TIMEOUT, cancel, emit)))
    }

    /// Starts a live-host scan of an IPv4 range or CIDR over [`NETWORK_PORTS`] on the current
    /// Tokio runtime; results stream through `emit`.
    pub fn scan_network(&self, targets: &str, emit: ScanSink) -> Result<u32, String> {
        let hosts = parse_ipv4_targets(targets)?;
        check_probe_count(hosts.len(), NETWORK_PORTS.len())?;
        Ok(self.start(move |cancel| async move {
            run_network_scan(hosts, &NETWORK_PORTS, PROBE_TIMEOUT, cancel, emit).await
        }))
    }

    /// Cancels scan `id`; it still reports `Done` (cancelled). Unknown or finished ids are
    /// ignored.
    pub fn stop(&self, id: u32) {
        if let Some(cancel) = lock(&self.running).remove(&id) {
            cancel.cancel();
        }
    }

    /// Scans still running.
    pub fn running(&self) -> usize {
        lock(&self.running).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    fn ip(text: &str) -> Ipv4Addr {
        text.parse().unwrap()
    }

    #[test]
    fn parses_a_single_ipv4_address() {
        assert_eq!(parse_ipv4_targets(" 192.168.1.10 "), Ok(vec![ip("192.168.1.10")]));
    }

    #[test]
    fn parses_a_last_octet_range() {
        let hosts = parse_ipv4_targets("192.168.1.10-20").unwrap();
        assert_eq!(hosts.len(), 11);
        assert_eq!(hosts[0], ip("192.168.1.10"));
        assert_eq!(hosts[10], ip("192.168.1.20"));
    }

    #[test]
    fn parses_cidr_without_network_and_broadcast_addresses() {
        let hosts = parse_ipv4_targets("10.0.0.77/24").unwrap();
        assert_eq!(hosts.len(), 254);
        assert_eq!(hosts[0], ip("10.0.0.1"));
        assert_eq!(hosts[253], ip("10.0.0.254"));
        assert_eq!(parse_ipv4_targets("10.0.0.5/32"), Ok(vec![ip("10.0.0.5")]));
        assert_eq!(
            parse_ipv4_targets("10.0.0.4/31"),
            Ok(vec![ip("10.0.0.4"), ip("10.0.0.5")])
        );
    }

    #[test]
    fn rejects_invalid_ipv4_targets_with_a_clear_error() {
        let cases = [
            (
                "",
                "Enter an IPv4 address, range (192.168.1.10-20) or CIDR (192.168.1.0/24)",
            ),
            ("192.168.1.300", "Invalid IPv4 address: 192.168.1.300"),
            ("router", "Invalid IPv4 address: router"),
            (
                "192.168.1.20-10",
                "Invalid range: 192.168.1.20-10 (use a.b.c.d-e with e between d and 255)",
            ),
            (
                "192.168.1.10-256",
                "Invalid range: 192.168.1.10-256 (use a.b.c.d-e with e between d and 255)",
            ),
            (
                "10.0.0.0/23",
                "CIDR prefix must be between /24 and /32 (at most 256 hosts)",
            ),
            (
                "10.0.0.0/x",
                "CIDR prefix must be between /24 and /32 (at most 256 hosts)",
            ),
            ("10.0.0/24", "Invalid IPv4 address: 10.0.0"),
        ];
        for (text, error) in cases {
            assert_eq!(parse_ipv4_targets(text), Err(error.to_string()), "{text:?}");
        }
    }

    #[tokio::test]
    async fn targets_accept_hostnames_and_ipv4_forms() {
        assert!(parse_targets("localhost").await.unwrap()[0].is_loopback());
        assert_eq!(parse_targets("127.0.0.1-2").await.unwrap().len(), 2);
        assert_eq!(
            parse_targets("  ").await,
            Err("Enter a host, IPv4 address or range".to_string())
        );
        assert_eq!(
            parse_targets("bad host").await,
            Err("Invalid host: bad host".to_string())
        );
        assert_eq!(
            parse_targets("no-such-host.invalid").await,
            Err("Could not resolve host: no-such-host.invalid".to_string())
        );
    }

    #[test]
    fn parses_port_lists_sorted_and_deduplicated() {
        assert_eq!(parse_ports("80, 22,8000-8002,22"), Ok(vec![22, 80, 8000, 8001, 8002]));
        assert_eq!(parse_ports("1-65535").unwrap().len(), 65535);
    }

    #[test]
    fn rejects_invalid_ports_with_a_clear_error() {
        let cases = [
            ("", "Enter at least one port"),
            (" , ", "Enter at least one port"),
            ("0", "Invalid port: 0 (use numbers from 1 to 65535)"),
            ("65536", "Invalid port: 65536 (use numbers from 1 to 65535)"),
            ("ssh", "Invalid port: ssh (use numbers from 1 to 65535)"),
            ("100-90", "Invalid port range: 100-90"),
            ("0-10", "Invalid port range: 0-10"),
        ];
        for (text, error) in cases {
            assert_eq!(parse_ports(text), Err(error.to_string()), "{text:?}");
        }
    }

    #[test]
    fn caps_the_total_number_of_probes() {
        assert_eq!(check_probe_count(1, 65535), Ok(65535));
        assert_eq!(check_probe_count(256, 256), Ok(65536));
        assert_eq!(
            check_probe_count(2, 65535),
            Err("Too many probes: 131070 (the limit is 65536; scan fewer hosts or ports)".to_string())
        );
    }

    #[test]
    fn names_well_known_services() {
        assert_eq!(service_name(22), Some("ssh"));
        assert_eq!(service_name(3389), Some("rdp"));
        assert_eq!(service_name(443), Some("https"));
        assert_eq!(service_name(40000), None);
    }

    #[test]
    fn parses_the_linux_arp_table() {
        let fixture = "IP address       HW type     Flags       HW address            Mask     Device\n\
                       192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:01     *        eth0\n\
                       192.168.1.50     0x1         0x0         00:00:00:00:00:00     *        eth0\n\
                       192.168.1.7      0x1         0x2         AA:BB:CC:DD:EE:07     *        wlan0\n";
        let table = parse_arp_table(fixture);
        assert_eq!(table.len(), 2);
        assert_eq!(
            table.get(&ip("192.168.1.1")).map(String::as_str),
            Some("aa:bb:cc:dd:ee:01")
        );
        assert_eq!(
            table.get(&ip("192.168.1.7")).map(String::as_str),
            Some("aa:bb:cc:dd:ee:07")
        );
    }

    #[test]
    fn parses_getent_hosts_output() {
        assert_eq!(
            parse_getent_hosts("192.168.1.1     router.lan router\n"),
            Some("router.lan".to_string())
        );
        assert_eq!(parse_getent_hosts(""), None);
        assert_eq!(parse_getent_hosts("192.168.1.1\n"), None);
    }

    /// One port with a listener (open) and one whose listener was dropped (closed).
    async fn open_and_closed_ports() -> (TcpListener, u16, u16) {
        let open = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let open_port = open.local_addr().unwrap().port();
        let closed_port = closed.local_addr().unwrap().port();
        drop(closed);
        (open, open_port, closed_port)
    }

    fn collector() -> (Arc<Mutex<Vec<ScanEvent>>>, ScanSink) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        (events, Arc::new(move |event| sink.lock().unwrap().push(event)))
    }

    #[tokio::test]
    async fn probes_report_open_and_closed_ports() {
        let (_listener, open, closed) = open_and_closed_ports().await;
        let mut outcomes = Vec::new();
        let localhost = [IpAddr::V4(Ipv4Addr::LOCALHOST)];
        let cancelled = probe_all(
            &localhost,
            &[open, closed],
            PROBE_TIMEOUT,
            4,
            &CancelFlag::default(),
            |o| outcomes.push(o),
        )
        .await;
        assert!(!cancelled);
        outcomes.sort_by_key(|o| o.port != open);
        assert_eq!(outcomes.len(), 2);
        assert_eq!((outcomes[0].port, outcomes[0].state), (open, PortState::Open));
        // Windows retries a refused connection for about two seconds, so within the one-second
        // probe a closed port can only be told apart from a filtered one elsewhere.
        let closed_states: &[PortState] = if cfg!(windows) {
            &[PortState::Closed, PortState::Filtered]
        } else {
            &[PortState::Closed]
        };
        assert_eq!(outcomes[1].port, closed);
        assert!(closed_states.contains(&outcomes[1].state), "{:?}", outcomes[1].state);
    }

    #[tokio::test]
    async fn cancelling_stops_the_scan_before_completion() {
        let cancel = CancelFlag::default();
        let ports: Vec<u16> = (20000..21000).collect();
        let mut seen = 0;
        let localhost = [IpAddr::V4(Ipv4Addr::LOCALHOST)];
        let cancelled = probe_all(&localhost, &ports, PROBE_TIMEOUT, 1, &cancel, |_| {
            seen += 1;
            cancel.cancel();
        })
        .await;
        assert!(cancelled);
        assert_eq!(seen, 1);
    }

    #[tokio::test]
    async fn port_scan_streams_open_ports_then_done() {
        let (_listener, open, closed) = open_and_closed_ports().await;
        let (events, emit) = collector();
        run_port_scan(
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            vec![open, closed],
            PROBE_TIMEOUT,
            Arc::default(),
            emit,
        )
        .await;
        let events = events.lock().unwrap();
        assert_eq!(events.first(), Some(&ScanEvent::Started { total: 2 }));
        let opens: Vec<_> = events.iter().filter(|e| matches!(e, ScanEvent::Open { .. })).collect();
        assert_eq!(
            opens,
            vec![&ScanEvent::Open {
                host: "127.0.0.1".into(),
                port: open,
                service: None
            }]
        );
        assert_eq!(
            events.last(),
            Some(&ScanEvent::Done {
                done: 2,
                total: 2,
                cancelled: false
            })
        );
    }

    #[tokio::test]
    async fn a_cancelled_port_scan_reports_done_as_cancelled() {
        let (events, emit) = collector();
        let cancel = Arc::new(CancelFlag::default());
        cancel.cancel();
        run_port_scan(
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            vec![1, 2, 3],
            PROBE_TIMEOUT,
            cancel,
            emit,
        )
        .await;
        let events = events.lock().unwrap();
        assert_eq!(
            events.last(),
            Some(&ScanEvent::Done {
                done: 0,
                total: 3,
                cancelled: true
            })
        );
    }

    #[tokio::test]
    async fn network_scan_reports_live_hosts_with_their_open_services() {
        let (_listener, open, closed) = open_and_closed_ports().await;
        let (events, emit) = collector();
        run_network_scan(
            vec![Ipv4Addr::LOCALHOST],
            &[open, closed],
            PROBE_TIMEOUT,
            Arc::default(),
            emit,
        )
        .await;
        let events = events.lock().unwrap();
        let hosts: Vec<_> = events.iter().filter(|e| matches!(e, ScanEvent::Host { .. })).collect();
        assert_eq!(hosts.len(), 1);
        let ScanEvent::Host { host, mac, ports, .. } = hosts[0] else {
            unreachable!()
        };
        assert_eq!((host.as_str(), mac, ports), ("127.0.0.1", &None, &vec![open]));
        assert_eq!(
            events.last(),
            Some(&ScanEvent::Done {
                done: 2,
                total: 2,
                cancelled: false
            })
        );
    }

    /// The events received so far, waiting up to 10 s for `Done`.
    async fn wait_done(events: &Arc<Mutex<Vec<ScanEvent>>>) -> Vec<ScanEvent> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let seen = events.lock().unwrap().clone();
            if matches!(seen.last(), Some(ScanEvent::Done { .. })) || tokio::time::Instant::now() > deadline {
                return seen;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn the_manager_runs_a_port_scan_and_forgets_it_when_done() {
        let (_listener, open, closed) = open_and_closed_ports().await;
        let scans = ScanManager::default();
        let (events, emit) = collector();
        let id = scans
            .scan_ports("127.0.0.1", &format!("{open},{closed}"), emit)
            .await
            .unwrap();
        assert!(id > 0);
        let events = wait_done(&events).await;
        assert_eq!(events.first(), Some(&ScanEvent::Started { total: 2 }));
        assert!(events.contains(&ScanEvent::Open {
            host: "127.0.0.1".into(),
            port: open,
            service: None
        }));
        assert_eq!(
            events.last(),
            Some(&ScanEvent::Done {
                done: 2,
                total: 2,
                cancelled: false
            })
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(scans.running(), 0);
        // Stopping a finished scan does nothing.
        scans.stop(id);
    }

    #[tokio::test]
    async fn the_manager_rejects_bad_targets_ports_and_probe_counts_before_starting() {
        let scans = ScanManager::default();
        let (events, emit) = collector();
        assert_eq!(
            scans.scan_ports("127.0.0.1", "0", emit.clone()).await,
            Err("Invalid port: 0 (use numbers from 1 to 65535)".to_string())
        );
        assert_eq!(
            scans.scan_ports("  ", "22", emit.clone()).await,
            Err("Enter a host, IPv4 address or range".to_string())
        );
        assert_eq!(
            scans.scan_ports("127.0.0.1-2", "1-65535", emit.clone()).await,
            Err("Too many probes: 131070 (the limit is 65536; scan fewer hosts or ports)".to_string())
        );
        assert_eq!(
            scans.scan_network("10.0.0.0/23", emit),
            Err("CIDR prefix must be between /24 and /32 (at most 256 hosts)".to_string())
        );
        assert_eq!(scans.running(), 0);
        assert!(events.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn stopping_a_running_scan_reports_done_as_cancelled() {
        let scans = ScanManager::default();
        let (events, emit) = collector();
        let id = scans.scan_ports("127.0.0.1", "1-65535", emit).await.unwrap();
        assert_eq!(scans.running(), 1);
        scans.stop(id);
        let events = wait_done(&events).await;
        let Some(ScanEvent::Done { done, total, cancelled }) = events.last().cloned() else {
            panic!("no Done event: {events:?}")
        };
        assert!(cancelled);
        assert_eq!(total, 65535);
        assert!(done < total);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(scans.running(), 0);
    }

    #[tokio::test]
    async fn the_manager_runs_a_network_scan_of_the_local_host() {
        let scans = ScanManager::default();
        let (events, emit) = collector();
        scans.scan_network("127.0.0.1", emit).unwrap();
        let events = wait_done(&events).await;
        assert_eq!(
            events.first(),
            Some(&ScanEvent::Started {
                total: NETWORK_PORTS.len()
            })
        );
        assert!(matches!(events.last(), Some(ScanEvent::Done { cancelled: false, .. })));
    }
}
