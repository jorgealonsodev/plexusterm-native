//! SOCKS5 server handshake (RFC 1928) for dynamic port forwarding, ported from PlexusTerm
//! v0.2.15 `src-tauri/src/tunnels/socks.rs`.
//!
//! Only the "no authentication" method and the CONNECT command are supported.
//! Parsing is pure: each parser takes the bytes received so far and reports
//! whether a complete message is available, so it is tested without sockets.

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const VERSION: u8 = 5;
const NO_AUTH: u8 = 0x00;
const NO_ACCEPTABLE_METHOD: u8 = 0xFF;
const CMD_CONNECT: u8 = 0x01;
const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;

/// Reply codes sent after the request.
pub const REPLY_SUCCEEDED: u8 = 0x00;
pub const REPLY_GENERAL_FAILURE: u8 = 0x01;
pub const REPLY_COMMAND_NOT_SUPPORTED: u8 = 0x07;
pub const REPLY_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;

#[derive(Debug, PartialEq, Eq)]
pub enum Parsed<T> {
    /// More bytes are needed.
    Incomplete,
    /// The message and the number of bytes it used.
    Complete(T, usize),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SocksError {
    #[error("unsupported SOCKS version {0}")]
    Version(u8),
    #[error("the client does not offer the no-authentication method")]
    NoAcceptableMethod,
    #[error("unsupported SOCKS command {0}")]
    CommandNotSupported(u8),
    #[error("unsupported SOCKS address type {0}")]
    AddressTypeNotSupported(u8),
    #[error("invalid domain name in SOCKS request")]
    InvalidDomain,
    #[error("the SOCKS client did not finish the handshake in time")]
    Timeout,
    #[error("{0}")]
    Io(String),
}

impl SocksError {
    /// Reply code to send for a rejected request, if the error happened at that stage.
    fn reply_code(&self) -> Option<u8> {
        match self {
            Self::CommandNotSupported(_) => Some(REPLY_COMMAND_NOT_SUPPORTED),
            Self::AddressTypeNotSupported(_) => Some(REPLY_ADDRESS_TYPE_NOT_SUPPORTED),
            Self::InvalidDomain => Some(REPLY_GENERAL_FAILURE),
            _ => None,
        }
    }
}

/// Target of a CONNECT request; IPv6 hosts are written without brackets.
#[derive(Debug, PartialEq, Eq)]
pub struct ConnectTarget {
    pub host: String,
    pub port: u16,
}

/// Parses the greeting (`VER NMETHODS METHODS...`); succeeds only when "no authentication" is offered.
pub fn parse_greeting(buf: &[u8]) -> Result<Parsed<()>, SocksError> {
    let [version, count, rest @ ..] = buf else {
        return Ok(Parsed::Incomplete);
    };
    if *version != VERSION {
        return Err(SocksError::Version(*version));
    }
    let count = usize::from(*count);
    let Some(methods) = rest.get(..count) else {
        return Ok(Parsed::Incomplete);
    };
    if methods.contains(&NO_AUTH) {
        Ok(Parsed::Complete((), 2 + count))
    } else {
        Err(SocksError::NoAcceptableMethod)
    }
}

/// Parses the request (`VER CMD RSV ATYP DST.ADDR DST.PORT`).
pub fn parse_request(buf: &[u8]) -> Result<Parsed<ConnectTarget>, SocksError> {
    let [version, command, _reserved, address_type, rest @ ..] = buf else {
        return Ok(Parsed::Incomplete);
    };
    if *version != VERSION {
        return Err(SocksError::Version(*version));
    }
    if *command != CMD_CONNECT {
        return Err(SocksError::CommandNotSupported(*command));
    }
    // Length of the address field, and where the address bytes start in `rest`.
    let (len, skip) = match *address_type {
        ATYP_IPV4 => (4, 0),
        ATYP_IPV6 => (16, 0),
        ATYP_DOMAIN => match rest.first() {
            Some(len) => (usize::from(*len), 1),
            None => return Ok(Parsed::Incomplete),
        },
        other => return Err(SocksError::AddressTypeNotSupported(other)),
    };
    let used = skip + len + 2;
    let (Some(address), Some(&[port_hi, port_lo])) = (rest.get(skip..skip + len), rest.get(skip + len..used)) else {
        return Ok(Parsed::Incomplete);
    };
    let host = match *address_type {
        ATYP_IPV4 => match <[u8; 4]>::try_from(address) {
            Ok(octets) => std::net::Ipv4Addr::from(octets).to_string(),
            Err(_) => return Ok(Parsed::Incomplete),
        },
        ATYP_IPV6 => match <[u8; 16]>::try_from(address) {
            Ok(octets) => std::net::Ipv6Addr::from(octets).to_string(),
            Err(_) => return Ok(Parsed::Incomplete),
        },
        _ => match std::str::from_utf8(address) {
            Ok(name) if !name.is_empty() => name.to_string(),
            _ => return Err(SocksError::InvalidDomain),
        },
    };
    let port = u16::from_be_bytes([port_hi, port_lo]);
    Ok(Parsed::Complete(ConnectTarget { host, port }, 4 + used))
}

/// A reply with an unspecified bound address (`0.0.0.0:0`).
pub fn reply(code: u8) -> [u8; 10] {
    [VERSION, code, 0, ATYP_IPV4, 0, 0, 0, 0, 0, 0]
}

/// Reads until `parse` reports a complete message.
async fn read_message<S, T>(
    stream: &mut S,
    buf: &mut Vec<u8>,
    parse: fn(&[u8]) -> Result<Parsed<T>, SocksError>,
) -> Result<T, SocksError>
where
    S: AsyncRead + Unpin,
{
    loop {
        if let Parsed::Complete(value, used) = parse(buf)? {
            buf.drain(..used);
            return Ok(value);
        }
        let mut chunk = [0u8; 512];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|err| SocksError::Io(err.to_string()))?;
        if read == 0 {
            return Err(SocksError::Io("connection closed during SOCKS handshake".into()));
        }
        buf.extend_from_slice(chunk.get(..read).unwrap_or_default());
    }
}

/// Runs the server side of the handshake up to the CONNECT request.
///
/// Rejections are answered on the stream before returning the error; on
/// success the caller must send [`reply`] once the target is reached or not.
/// A client that does not send its greeting and request within `limit` gets
/// [`SocksError::Timeout`] and no reply, so a stalled client cannot hold a
/// connection forever.
pub async fn handshake<S>(stream: &mut S, limit: Duration) -> Result<ConnectTarget, SocksError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    tokio::time::timeout(limit, negotiate(stream))
        .await
        .unwrap_or(Err(SocksError::Timeout))
}

async fn negotiate<S>(stream: &mut S) -> Result<ConnectTarget, SocksError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf = Vec::new();
    let write = |err: std::io::Error| SocksError::Io(err.to_string());
    match read_message(stream, &mut buf, parse_greeting).await {
        Ok(()) => stream.write_all(&[VERSION, NO_AUTH]).await.map_err(write)?,
        Err(err) => {
            if err == SocksError::NoAcceptableMethod {
                let _ = stream.write_all(&[VERSION, NO_ACCEPTABLE_METHOD]).await;
            }
            return Err(err);
        }
    }
    match read_message(stream, &mut buf, parse_request).await {
        Ok(target) => Ok(target),
        Err(err) => {
            if let Some(code) = err.reply_code() {
                let _ = stream.write_all(&reply(code)).await;
            }
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_needs_the_whole_method_list() {
        assert_eq!(parse_greeting(&[]), Ok(Parsed::Incomplete));
        assert_eq!(parse_greeting(&[5]), Ok(Parsed::Incomplete));
        assert_eq!(parse_greeting(&[5, 2, 2]), Ok(Parsed::Incomplete));
    }

    #[test]
    fn greeting_accepts_no_auth_among_other_methods() {
        assert_eq!(parse_greeting(&[5, 2, 2, 0]), Ok(Parsed::Complete((), 4)));
        assert_eq!(parse_greeting(&[5, 1, 0, 9]), Ok(Parsed::Complete((), 3)));
    }

    #[test]
    fn greeting_without_no_auth_or_with_another_version_is_rejected() {
        assert_eq!(parse_greeting(&[5, 1, 2]), Err(SocksError::NoAcceptableMethod));
        assert_eq!(parse_greeting(&[4, 1, 0]), Err(SocksError::Version(4)));
    }

    #[test]
    fn request_parses_ipv4_targets() {
        assert_eq!(
            parse_request(&[5, 1, 0, 1, 127, 0, 0, 1, 0x1F, 0x90]),
            Ok(Parsed::Complete(
                ConnectTarget {
                    host: "127.0.0.1".into(),
                    port: 8080
                },
                10
            ))
        );
    }

    #[test]
    fn request_parses_domain_targets() {
        let mut buf = vec![5, 1, 0, 3, 9];
        buf.extend_from_slice(b"localhost");
        buf.extend_from_slice(&[0, 80]);
        assert_eq!(
            parse_request(&buf),
            Ok(Parsed::Complete(
                ConnectTarget {
                    host: "localhost".into(),
                    port: 80
                },
                buf.len()
            ))
        );
        assert_eq!(parse_request(&buf[..8]), Ok(Parsed::Incomplete));
    }

    #[test]
    fn request_parses_ipv6_targets() {
        let mut buf = vec![5, 1, 0, 4];
        buf.extend_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
        buf.extend_from_slice(&[0, 22]);
        assert_eq!(
            parse_request(&buf),
            Ok(Parsed::Complete(
                ConnectTarget {
                    host: "::1".into(),
                    port: 22
                },
                22
            ))
        );
    }

    #[test]
    fn request_rejects_other_commands_and_address_types() {
        assert_eq!(
            parse_request(&[5, 2, 0, 1, 127, 0, 0, 1, 0, 80]),
            Err(SocksError::CommandNotSupported(2))
        );
        assert_eq!(
            parse_request(&[5, 1, 0, 9, 0, 0]),
            Err(SocksError::AddressTypeNotSupported(9))
        );
        assert_eq!(
            parse_request(&[5, 1, 0, 3, 2, 0xFF, 0xFE, 0, 80]),
            Err(SocksError::InvalidDomain)
        );
    }

    /// Runs the handshake with a 200 ms limit against a client that sent `sent`;
    /// returns the result and every byte the client received before the server closed.
    async fn stalled_handshake(sent: &[u8]) -> (Result<ConnectTarget, SocksError>, Vec<u8>) {
        let (mut server, mut client) = tokio::io::duplex(64);
        client.write_all(sent).await.unwrap();
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            handshake(&mut server, Duration::from_millis(200)),
        )
        .await
        .expect("handshake still waiting after 2 s");
        assert!(started.elapsed() >= Duration::from_millis(200));
        drop(server);
        let mut received = Vec::new();
        client.read_to_end(&mut received).await.unwrap();
        (result, received)
    }

    #[tokio::test]
    async fn handshake_closes_a_silent_client_without_a_reply() {
        let (result, received) = stalled_handshake(&[]).await;
        assert_eq!(result, Err(SocksError::Timeout));
        assert!(received.is_empty(), "unexpected reply {received:?}");
    }

    #[tokio::test]
    async fn handshake_closes_a_client_that_stops_mid_greeting() {
        let (result, received) = stalled_handshake(&[5, 2, 0]).await;
        assert_eq!(result, Err(SocksError::Timeout));
        assert!(received.is_empty(), "unexpected reply {received:?}");
    }

    #[tokio::test]
    async fn handshake_limit_covers_greeting_and_request_together() {
        // The greeting is answered, then the request never comes.
        let (result, received) = stalled_handshake(&[5, 1, 0, 5, 1]).await;
        assert_eq!(result, Err(SocksError::Timeout));
        assert_eq!(received, [5, 0]);
    }

    #[test]
    fn reply_reports_the_code_with_an_unspecified_address() {
        assert_eq!(reply(7), [5, 7, 0, 1, 0, 0, 0, 0, 0, 0]);
    }
}
