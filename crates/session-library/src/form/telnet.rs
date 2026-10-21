//! Telnet fields (v0.2.15 `telnet/telnetForm.ts`).

use super::FormError;
use crate::text::{js_trim, port_of};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TelnetForm {
    pub host: String,
    /// Empty means 23.
    pub port: String,
}

/// Validates host and port (default 23).
pub fn parse_telnet_form(form: &TelnetForm) -> Result<(String, u16), FormError> {
    let host = js_trim(&form.host);
    if host.is_empty() {
        return Err(FormError::HostRequired);
    }
    let port_text = js_trim(&form.port);
    let port = if port_text.is_empty() {
        23
    } else {
        port_of(port_text).ok_or(FormError::PortRange)?
    };
    Ok((host.to_string(), port))
}
