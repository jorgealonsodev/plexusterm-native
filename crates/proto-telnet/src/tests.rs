//! Telnet client tests against an in-process TCP server that negotiates like a telnetd.
//! Ported from v0.2.15 `src-tauri/src/telnet/tests.rs`; output is read from the session's
//! [`ByteRing`] instead of a Tauri channel.

use super::protocol::{DO, DONT, ECHO, IAC, NAWS, SB, SE, SGA, TTYPE, WILL};
use super::*;
use std::time::Instant;
use tokio::net::TcpListener;

/// Collects ring output until `needle` appears; the flag tells whether the session ended
/// (ring closed and drained).
async fn wait_for(ring: &ByteRing, needle: &[u8]) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(bytes) = ring.try_pop(usize::MAX) {
            out.extend(bytes);
            if !needle.is_empty() && contains(&out, needle) {
                return (out, false);
            }
            continue;
        }
        if ring.is_closed() && ring.is_empty() {
            return (out, true);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    (out, false)
}

fn request(port: u16) -> TelnetRequest {
    TelnetRequest {
        host: "127.0.0.1".into(),
        port,
        cols: 80,
        rows: 24,
    }
}

fn ring() -> ByteRing {
    ByteRing::new(ByteRing::SESSION_CAPACITY)
}

/// Server side of one accepted connection.
struct Peer {
    stream: TcpStream,
    received: Vec<u8>,
}

impl Peer {
    /// Reads until the bytes received so far contain `needle` (5 s at most).
    #[allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        reason = "test helper"
    )]
    async fn expect(&mut self, needle: &[u8]) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut buf = [0u8; 1024];
        while !contains(&self.received, needle) {
            match tokio::time::timeout_at(deadline, self.stream.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => self.received.extend_from_slice(&buf[..n]),
                _ => panic!("server never received {needle:?}; got {:?}", self.received),
            }
        }
    }

    #[allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        reason = "test helper"
    )]
    async fn send(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.unwrap();
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helper"
)]
async fn listen() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helper"
)]
async fn accept(listener: &TcpListener) -> Peer {
    let (stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("client never connected")
        .unwrap();
    Peer {
        stream,
        received: Vec::new(),
    }
}

/// Connects a client with `output` and walks the server through the usual telnetd negotiation.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helper"
)]
async fn negotiated_with(output: ByteRing) -> (TelnetManager, TelnetSession, Peer) {
    let (listener, port) = listen().await;
    let mgr = TelnetManager::default();
    let session = mgr.connect(request(port), output).await.unwrap();
    let mut peer = accept(&listener).await;
    peer.expect(&[IAC, WILL, TTYPE, IAC, WILL, NAWS]).await;
    peer.send(&[IAC, DO, TTYPE, IAC, DO, NAWS]).await;
    peer.expect(&[IAC, SB, NAWS, 0, 80, 0, 24, IAC, SE]).await;
    (mgr, session, peer)
}

async fn negotiated() -> (TelnetManager, TelnetSession, Peer, ByteRing) {
    let output = ring();
    let (mgr, session, peer) = negotiated_with(output.clone()).await;
    (mgr, session, peer, output)
}

#[tokio::test]
async fn naws_is_sent_with_the_initial_size_and_again_on_resize() {
    let (mgr, session, mut peer, _out) = negotiated().await;
    mgr.resize(session.id, 132, 50).unwrap();
    peer.expect(&[IAC, SB, NAWS, 0, 132, 0, 50, IAC, SE]).await;
}

#[tokio::test]
async fn terminal_type_is_answered() {
    let (_mgr, _session, mut peer, _out) = negotiated().await;
    peer.send(&[IAC, SB, TTYPE, 1, IAC, SE]).await;
    let mut expected = vec![IAC, SB, TTYPE, 0];
    expected.extend_from_slice(b"xterm-256color");
    expected.extend_from_slice(&[IAC, SE]);
    peer.expect(&expected).await;
}

#[tokio::test]
async fn server_echo_and_sga_are_accepted_and_other_options_refused() {
    let (_mgr, _session, mut peer, _out) = negotiated().await;
    peer.send(&[IAC, WILL, ECHO, IAC, WILL, SGA, IAC, WILL, 42]).await;
    peer.expect(&[IAC, DO, ECHO, IAC, DO, SGA, IAC, DONT, 42]).await;
}

#[tokio::test]
async fn input_iac_is_escaped_and_enter_is_sent_as_cr_nul() {
    let (mgr, session, mut peer, _out) = negotiated().await;
    mgr.write(session.id, &[b'a', IAC, b'b', b'\r']).await.unwrap();
    peer.expect(&[b'a', IAC, IAC, b'b', b'\r', 0]).await;
}

#[tokio::test]
async fn output_is_decoded_with_iac_unescaped_and_commands_stripped() {
    let (_mgr, _session, mut peer, out) = negotiated().await;
    // "é" is C3 A9; an escaped IAC is a lone 0xFF byte, which is not valid UTF-8. v0.2.15
    // decoded it to U+FFFD here; the term-engine parser now does that, so the ring holds bytes.
    peer.send(&[b'h', IAC, 241, b'i', 0xC3, 0xA9, IAC, IAC, b'!', b'\r', 0, b'\n'])
        .await;
    let (bytes, _) = wait_for(&out, b"!\r\n").await;
    assert_eq!(bytes, vec![b'h', b'i', 0xC3, 0xA9, 0xFF, b'!', b'\r', b'\n']);
}

#[tokio::test]
async fn server_closing_ends_the_session() {
    let (_mgr, _session, mut peer, out) = negotiated().await;
    peer.send(b"bye\r\n").await;
    drop(peer);
    let (bytes, exited) = wait_for(&out, b"").await;
    assert!(contains(&bytes, b"bye"), "output was {bytes:?}");
    assert!(exited, "the session did not end");
}

#[tokio::test]
async fn close_disconnects_and_forgets_the_session() {
    let (mgr, session, mut peer, out) = negotiated().await;
    mgr.close(session.id).unwrap();
    let mut buf = [0u8; 64];
    let read = tokio::time::timeout(Duration::from_secs(5), peer.stream.read(&mut buf))
        .await
        .expect("the client did not disconnect");
    assert!(matches!(read, Ok(0)), "unexpected read {read:?}");
    assert!(wait_for(&out, b"").await.1);
    assert!(matches!(
        mgr.write(session.id, b"x").await,
        Err(TelnetError::UnknownSession(_))
    ));
}

#[tokio::test]
async fn refused_connection_is_a_clear_error() {
    let (listener, port) = listen().await;
    drop(listener);
    let err = TelnetManager::default()
        .connect(request(port), ring())
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .starts_with(&format!("cannot connect to 127.0.0.1:{port}:")),
        "{err}"
    );
}

#[tokio::test]
async fn connect_times_out() {
    // A non-routable address (TEST-NET-1) never answers.
    let mgr = TelnetManager::default().with_connect_timeout(Duration::from_millis(200));
    let mut req = request(23);
    req.host = "192.0.2.1".into();
    let err = mgr.connect(req, ring()).await.unwrap_err();
    let message = err.to_string();
    // Some networks reject the address at once instead of letting it time out.
    assert!(message.starts_with("cannot connect to 192.0.2.1:23:"), "{message}");
}

#[test]
fn request_port_defaults_to_23() {
    let req = TelnetRequest::new("h", 80, 24);
    assert_eq!(req.port, 23);
}

#[tokio::test]
async fn connects_to_an_ipv6_literal() {
    // Skipped where the machine has no IPv6 loopback.
    let Ok(listener) = TcpListener::bind("[::1]:0").await else {
        return;
    };
    let port = listener.local_addr().unwrap().port();
    let mgr = TelnetManager::default();
    let mut req = request(port);
    req.host = "::1".into();
    mgr.connect(req, ring()).await.unwrap();
    let mut peer = accept(&listener).await;
    peer.expect(&[IAC, WILL, TTYPE, IAC, WILL, NAWS]).await;
}

// --- Native additions (term-engine integration, PRD SSH-4a, TEL-1, ST-3) ---

#[tokio::test]
async fn connects_to_a_bracketed_ipv6_literal() {
    // Skipped where the machine has no IPv6 loopback.
    let Ok(listener) = TcpListener::bind("[::1]:0").await else {
        return;
    };
    let port = listener.local_addr().unwrap().port();
    let mut req = request(port);
    req.host = "[::1]".into();
    TelnetManager::default().connect(req, ring()).await.unwrap();
    let mut peer = accept(&listener).await;
    peer.expect(&[IAC, WILL, TTYPE, IAC, WILL, NAWS]).await;
}

#[test]
fn the_unencrypted_line_is_the_yellow_v0215_notice() {
    assert_eq!(UNENCRYPTED_LINE, "\x1b[33mTelnet is not encrypted.\x1b[0m\r\n");
    assert!(UNENCRYPTED_LINE.contains(UNENCRYPTED_NOTICE));
}

#[tokio::test]
async fn input_handle_and_resize_hook_reach_the_server() {
    let (_mgr, session, mut peer, _out) = negotiated().await;
    let TelnetSession { input, resize, .. } = session;
    let hook = resize.into_hook();
    hook(100, 30);
    peer.expect(&[IAC, SB, NAWS, 0, 100, 0, 30, IAC, SE]).await;
    // The input handle runs on term-engine's writer thread, a plain thread.
    tokio::task::spawn_blocking(move || {
        let mut input = input;
        input.write_all(&[b'x', IAC, b'\r']).unwrap();
    })
    .await
    .unwrap();
    peer.expect(&[b'x', IAC, IAC, b'\r', 0]).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_ring_holds_the_reader_back_without_losing_bytes() {
    const CAPACITY: usize = 4096;
    const TOTAL: usize = 32 * 1024 * 1024;
    let output = ByteRing::new(CAPACITY);
    let (_mgr, _session, mut peer) = negotiated_with(output.clone()).await;
    // Bytes 0..=254 only, so nothing is a Telnet command.
    let payload: Vec<u8> = (0..TOTAL).map(|i| (i % 255) as u8).collect();
    let expected = payload.clone();
    let sender = tokio::spawn(async move {
        peer.stream.write_all(&payload).await.unwrap();
        peer
    });

    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(output.len(), CAPACITY, "the ring should be full");
    assert!(
        !sender.is_finished(),
        "the server could send everything into a full ring"
    );

    // Drain like the parser thread does: blocking pops until everything arrived or the ring ends.
    let drain = output.clone();
    let got = tokio::task::spawn_blocking(move || {
        let mut got = Vec::with_capacity(TOTAL);
        while got.len() < TOTAL {
            match drain.pop(usize::MAX) {
                Some(bytes) => got.extend(bytes),
                None => break,
            }
        }
        got
    });
    let got = tokio::time::timeout(Duration::from_secs(30), got)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.len(), TOTAL, "bytes were lost or never arrived");
    assert!(got == expected, "bytes arrived out of order or changed");
    sender.await.unwrap();
}

#[tokio::test]
async fn a_closed_ring_ends_the_connection() {
    let (_mgr, _session, mut peer, out) = negotiated().await;
    out.close();
    // The client notices the closed ring on the next output it has to deliver.
    peer.send(b"anyone there?\r\n").await;
    let mut buf = [0u8; 256];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match tokio::time::timeout_at(deadline, peer.stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(_)) => {}
            other => panic!("the client kept the connection open: {other:?}"),
        }
    }
}
