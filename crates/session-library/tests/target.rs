//! Port of v0.2.15 `src/features/sessions/types.test.ts` (tab titles, SES-1e).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail loudly like the #[test] functions they serve"
)]

use proto_ssh::SshAuth;
use session_library::target::*;
use session_library::{SerialFlow, SerialParity, SerialSettings};

fn ssh(host: &str, port: u16, username: &str) -> SshTarget {
    SshTarget {
        host: host.into(),
        port,
        username: username.into(),
        auth: SshAuth::Agent,
        storage: None,
        jumps: vec![],
    }
}

#[test]
fn names_serial_tabs_after_the_device_and_baud_rate() {
    let target = SessionTarget::Serial(SerialSettings {
        path: "/dev/ttyUSB0".into(),
        baud: 115200,
        data_bits: 8,
        parity: SerialParity::None,
        stop_bits: 1,
        flow: SerialFlow::None,
    });
    assert_eq!(target_title(&target), "/dev/ttyUSB0 @ 115200");
}

#[test]
fn names_telnet_tabs_after_the_host_port_only_when_not_23_and_mosh_tabs_after_the_ssh_login() {
    // it("names Telnet tabs after the host (port only when not 23) and Mosh tabs after the SSH login")
    let telnet = |port| SessionTarget::Telnet {
        host: "switch.lan".into(),
        port,
    };
    assert_eq!(target_title(&telnet(23)), "telnet switch.lan");
    assert_eq!(target_title(&telnet(2323)), "telnet switch.lan:2323");
    let mosh = SessionTarget::Mosh(MoshTarget {
        ssh: ssh("h", 22, "u"),
        server_command: None,
    });
    assert_eq!(target_title(&mosh), "mosh u@h");
}

#[test]
fn finds_the_ssh_login_of_ssh_and_mosh_targets_only() {
    let login = |t: &SessionTarget| ssh_login_of(t).map(|s| format!("{}@{}", s.username, s.host));
    assert_eq!(login(&SessionTarget::Ssh(ssh("h", 22, "u"))).as_deref(), Some("u@h"));
    let mosh = SessionTarget::Mosh(MoshTarget {
        ssh: ssh("h", 22, "u"),
        server_command: None,
    });
    assert_eq!(login(&mosh).as_deref(), Some("u@h"));
    assert!(
        login(&SessionTarget::Telnet {
            host: "h".into(),
            port: 23
        })
        .is_none()
    );
    assert!(login(&SessionTarget::Local(LocalTarget::default())).is_none());
}

#[test]
fn keeps_the_local_and_ssh_titles() {
    assert_eq!(
        target_title(&SessionTarget::Local(LocalTarget::default())),
        "Local shell"
    );
    assert_eq!(target_title(&SessionTarget::Ssh(ssh("h", 2222, "u"))), "u@h:2222");
    assert_eq!(target_title(&SessionTarget::Ssh(ssh("h", 22, "u"))), "u@h");
}

#[test]
fn names_local_tabs_after_the_shell_and_shared_tabs_after_the_relay() {
    // SES-1e: local tabs titled with the shell name; shared tabs `Shared · {host}` (SHARE-13).
    let local = |program: &str| {
        SessionTarget::Local(LocalTarget {
            program: Some(program.into()),
            ..Default::default()
        })
    };
    assert_eq!(
        target_title(&local("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.EXE")),
        "powershell"
    );
    assert_eq!(target_title(&local("/usr/bin/fish")), "fish");
    assert_eq!(target_title(&local("   ")), "Local shell");
    let shared = SessionTarget::Shared(SharedTarget {
        url: "https://relay.example/s/abc#key".into(),
        relay_url: "wss://relay.example".into(),
        relay_host: "relay.example".into(),
    });
    assert_eq!(target_title(&shared), "Shared · relay.example");
}
