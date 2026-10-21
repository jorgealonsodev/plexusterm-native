//! Serial fields (v0.2.15 `serial/serialSettings.ts`).

use super::FormError;
use crate::model::{SerialFlow, SerialParity, SerialSettings};
use crate::text::{all_digits, js_trim};

pub const COMMON_BAUD_RATES: [u64; 8] = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

/// Baud select value meaning "use `custom_baud`".
pub const CUSTOM_BAUD: &str = "custom";

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// The serial fields as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialForm {
    pub path: String,
    /// One of [`COMMON_BAUD_RATES`] as text, or [`CUSTOM_BAUD`].
    pub baud: String,
    pub custom_baud: String,
    pub data_bits: String,
    pub parity: SerialParity,
    pub stop_bits: String,
    pub flow: SerialFlow,
}

/// 115200 8N1 without flow control.
impl Default for SerialForm {
    fn default() -> Self {
        Self {
            path: String::new(),
            baud: "115200".into(),
            custom_baud: String::new(),
            data_bits: "8".into(),
            parity: SerialParity::None,
            stop_bits: "1".into(),
            flow: SerialFlow::None,
        }
    }
}

pub fn parse_serial_form(form: &SerialForm) -> Result<SerialSettings, FormError> {
    let path = js_trim(&form.path);
    if path.is_empty() {
        return Err(FormError::SerialPortRequired);
    }
    let baud_text = js_trim(if form.baud == CUSTOM_BAUD {
        &form.custom_baud
    } else {
        &form.baud
    });
    let baud = Some(baud_text)
        .filter(|t| all_digits(t))
        .and_then(|t| t.parse::<u64>().ok())
        .filter(|b| (1..=MAX_SAFE_INTEGER).contains(b))
        .ok_or(FormError::BaudInvalid)?;
    let data_bits = [5u8, 6, 7, 8]
        .into_iter()
        .find(|n| n.to_string() == form.data_bits)
        .ok_or(FormError::DataBitsInvalid)?;
    let stop_bits = [1u8, 2]
        .into_iter()
        .find(|n| n.to_string() == form.stop_bits)
        .ok_or(FormError::StopBitsInvalid)?;
    Ok(SerialSettings {
        path: path.to_string(),
        baud,
        data_bits,
        parity: form.parity,
        stop_bits,
        flow: form.flow,
    })
}

/// The form of saved settings; a baud rate outside the presets reopens as custom.
pub fn form_from_settings(settings: &SerialSettings) -> SerialForm {
    let common = COMMON_BAUD_RATES.contains(&settings.baud);
    SerialForm {
        path: settings.path.clone(),
        baud: if common {
            settings.baud.to_string()
        } else {
            CUSTOM_BAUD.to_string()
        },
        custom_baud: if common {
            String::new()
        } else {
            settings.baud.to_string()
        },
        data_bits: settings.data_bits.to_string(),
        parity: settings.parity,
        stop_bits: settings.stop_bits.to_string(),
        flow: settings.flow,
    }
}
