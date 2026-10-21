//! Remote monitor: CPU, memory, root disk, network and users of an SSH server,
//! read with one POSIX `sh` command over the session's existing connection.
//!
//! Only cumulative counters are returned; the frontend turns two consecutive
//! samples into CPU % and network rates. A server without Linux `/proc`
//! (or any unparsable answer) yields [`MonitorSample::Unavailable`].
//!
//! Ported from PlexusTerm v0.2.15 `src-tauri/src/tools/monitor.rs`; the caller polls
//! [`sample`] (every 3 s, the next one after the previous answer, PRD TOOL-4).

use proto_ssh::{SshError, SshManager};

/// Prints `uname -s`, then one `@@<name>` marker before each section. Wrapped in
/// `sh -c` so it works whatever the user's login shell is; only stock tools.
pub const MONITOR_COMMAND: &str = "LC_ALL=C sh -c 'uname -s; \
echo @@stat; head -n 1 /proc/stat 2>/dev/null; \
echo @@mem; grep -E \"^(MemTotal|MemAvailable):\" /proc/meminfo 2>/dev/null; \
echo @@df; df -Pk / 2>/dev/null | tail -n 1; \
echo @@net; cat /proc/net/dev 2>/dev/null; \
echo @@users; who 2>/dev/null | cut -d\" \" -f1 | sort -u | wc -l'";

/// Cumulative counters of one sample; sizes in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Sum of the CPU time fields of `/proc/stat` (jiffies, all CPUs).
    pub cpu_total: u64,
    /// Idle plus iowait jiffies.
    pub cpu_idle: u64,
    pub mem_total: u64,
    pub mem_available: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    /// Bytes received and sent by every interface except `lo`.
    pub net_rx: u64,
    pub net_tx: u64,
    /// Distinct logged-in user names.
    pub users: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorSample {
    Ok(Snapshot),
    Unavailable { reason: String },
}

/// Parses the output of [`MONITOR_COMMAND`].
pub fn parse(output: &str) -> MonitorSample {
    match parse_snapshot(output) {
        Ok(snapshot) => MonitorSample::Ok(snapshot),
        Err(reason) => MonitorSample::Unavailable { reason },
    }
}

fn parse_snapshot(output: &str) -> Result<Snapshot, String> {
    let (system, sections) = split_sections(output);
    let system = system.trim();
    if system != "Linux" {
        let name = if system.is_empty() { "unknown" } else { system };
        return Err(format!("not a Linux server ({name})"));
    }
    let section = |name: &str| {
        sections
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, body)| body.as_str())
            .ok_or_else(|| format!("missing {name} section"))
    };
    let (cpu_total, cpu_idle) = parse_cpu(section("stat")?)?;
    let (mem_total, mem_available) = parse_mem(section("mem")?)?;
    let (disk_total, disk_used) = parse_df(section("df")?)?;
    let (net_rx, net_tx) = parse_net(section("net")?)?;
    let users = section("users")?
        .trim()
        .parse()
        .map_err(|_| "unreadable user count".to_string())?;
    Ok(Snapshot {
        cpu_total,
        cpu_idle,
        mem_total,
        mem_available,
        disk_total,
        disk_used,
        net_rx,
        net_tx,
        users,
    })
}

/// Splits at `@@<name>` lines: the text before the first marker, then each named body.
fn split_sections(output: &str) -> (String, Vec<(&str, String)>) {
    let mut head = String::new();
    let mut sections: Vec<(&str, String)> = Vec::new();
    for line in output.lines() {
        if let Some(name) = line.trim().strip_prefix("@@") {
            sections.push((name, String::new()));
        } else if let Some((_, body)) = sections.last_mut() {
            body.push_str(line);
            body.push('\n');
        } else {
            head.push_str(line);
            head.push('\n');
        }
    }
    (head, sections)
}

fn number(field: &str, what: &str) -> Result<u64, String> {
    field.parse().map_err(|_| format!("unreadable {what}"))
}

/// The aggregate `cpu` line: total of user..steal, and idle + iowait.
fn parse_cpu(body: &str) -> Result<(u64, u64), String> {
    let line = body
        .lines()
        .find(|l| l.split_whitespace().next() == Some("cpu"))
        .ok_or("no cpu line in /proc/stat")?;
    let fields = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|f| number(f, "/proc/stat"))
        .collect::<Result<Vec<_>, _>>()?;
    let Some(idle) = fields.get(3) else {
        return Err("short cpu line in /proc/stat".into());
    };
    let idle = idle + fields.get(4).copied().unwrap_or(0);
    Ok((fields.iter().sum(), idle))
}

fn parse_mem(body: &str) -> Result<(u64, u64), String> {
    let kib = |key: &str| {
        body.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|rest| rest.split_whitespace().next())
            .ok_or_else(|| format!("no {key} in /proc/meminfo"))
            .and_then(|v| number(v, "/proc/meminfo"))
            .map(|v| v * 1024)
    };
    Ok((kib("MemTotal:")?, kib("MemAvailable:")?))
}

/// The last line of `df -Pk /`: filesystem, 1024-blocks, used, available, capacity, mount.
fn parse_df(body: &str) -> Result<(u64, u64), String> {
    let line = body
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or("no df output")?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    // A filesystem name with spaces shifts the numbers right: count from the end.
    let (Some(total), Some(used), true) = (
        fields.iter().rev().nth(4),
        fields.iter().rev().nth(3),
        fields.len() >= 6,
    ) else {
        return Err("unreadable df output".into());
    };
    Ok((number(total, "df")? * 1024, number(used, "df")? * 1024))
}

/// Sums received and sent bytes of `/proc/net/dev`, skipping the loopback interface.
fn parse_net(body: &str) -> Result<(u64, u64), String> {
    let (mut rx, mut tx, mut interfaces) = (0, 0, 0);
    for line in body.lines() {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        if name.contains('|') {
            continue;
        }
        if name.trim() == "lo" {
            interfaces += 1;
            continue;
        }
        let fields: Vec<&str> = counters.split_whitespace().collect();
        let (Some(received), Some(sent)) = (fields.first(), fields.get(8)) else {
            return Err("unreadable /proc/net/dev".into());
        };
        rx += number(received, "/proc/net/dev")?;
        tx += number(sent, "/proc/net/dev")?;
        interfaces += 1;
    }
    if interfaces == 0 {
        return Err("no interfaces in /proc/net/dev".into());
    }
    Ok((rx, tx))
}

/// Samples session `id`: an SSH failure is an error, an unusable answer `Unavailable`.
pub async fn sample(ssh: &SshManager, id: u32) -> Result<MonitorSample, SshError> {
    let output = ssh.exec(id, MONITOR_COMMAND).await?;
    Ok(parse(&output))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINUX: &str = "Linux
@@stat
cpu  100 20 30 800 50 0 0 0 0 0
@@mem
MemTotal:        2048000 kB
MemAvailable:    1024000 kB
@@df
/dev/sda1          10000000   4000000   5500000      43% /
@@net
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo:  999999     100    0    0    0     0          0         0   999999     100    0    0    0     0       0          0
  eth0: 1000       10    0    0    0     0          0         0     2000      20    0    0    0     0       0          0
  wlan0:500 5    0    0    0     0          0         0     700      7    0    0    0     0       0          0
@@users
2
";

    fn reason(sample: MonitorSample) -> String {
        match sample {
            MonitorSample::Unavailable { reason } => reason,
            ok => panic!("expected unavailable, got {ok:?}"),
        }
    }

    #[test]
    fn parses_a_linux_sample() {
        assert_eq!(
            parse(LINUX),
            MonitorSample::Ok(Snapshot {
                cpu_total: 1000,
                cpu_idle: 850,
                mem_total: 2048000 * 1024,
                mem_available: 1024000 * 1024,
                disk_total: 10000000 * 1024,
                disk_used: 4000000 * 1024,
                net_rx: 1500,
                net_tx: 2700,
                users: 2,
            })
        );
    }

    #[test]
    fn a_non_linux_server_is_unavailable() {
        let output = LINUX.replacen("Linux", "Darwin", 1);
        assert!(reason(parse(&output)).contains("Darwin"));
    }

    #[test]
    fn missing_proc_sections_are_unavailable() {
        let output = "Linux\n@@stat\n@@mem\n@@df\n/dev/sda1 10 4 5 43% /\n@@net\n@@users\n0\n";
        assert!(!reason(parse(output)).is_empty());
    }

    #[test]
    fn garbled_output_is_unavailable() {
        assert!(!reason(parse("")).is_empty());
        assert!(!reason(parse(&LINUX.replace("2048000", "lots"))).is_empty());
    }

    #[tokio::test]
    async fn sample_runs_the_monitor_command_over_the_session() {
        use proto_ssh::test_server::{self, PASSWORD};
        use proto_ssh::{ConnectRequest, SshAuth};
        use term_engine::ByteRing;

        let server = test_server::start(1, None).await;
        let dir = tempfile::tempdir().unwrap();
        let known_hosts = dir.path().join("known_hosts");
        russh::keys::known_hosts::learn_known_hosts_path(
            "127.0.0.1",
            server.addr.port(),
            &server.host_key,
            &known_hosts,
        )
        .unwrap();
        // A missing agent socket, so the environment's SSH agent never changes the login.
        let ssh = SshManager::new(known_hosts).with_agent_socket(dir.path().join("no-agent"));
        let id = ssh
            .connect_with(
                ConnectRequest {
                    host: "127.0.0.1".into(),
                    port: server.addr.port(),
                    username: "tester".into(),
                    auth: SshAuth::Password {
                        password: PASSWORD.to_string().into(),
                    },
                    cols: 80,
                    rows: 24,
                    trusted_fingerprint: None,
                    jumps: vec![],
                },
                None,
                ByteRing::new(ByteRing::SESSION_CAPACITY),
            )
            .await
            .unwrap()
            .id;

        // The fake server has no `sh`, so the command fails: an error, not numbers.
        assert!(sample(&ssh, id).await.is_err());
        assert_eq!(server.exec_commands(), vec![MONITOR_COMMAND.to_string()]);

        // A server that answers: the sample is parsed.
        server.reply_to_exec(MONITOR_COMMAND, LINUX);
        let MonitorSample::Ok(snapshot) = sample(&ssh, id).await.unwrap() else {
            panic!("expected a sample")
        };
        assert_eq!(snapshot.users, 2);
    }
}
