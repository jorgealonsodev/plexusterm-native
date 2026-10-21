//! Serial console sessions (USB adapters, on-board UARTs), ported from PlexusTerm v0.2.15
//! `src-tauri/src/serial/mod.rs` (PRD SER-1).
//!
//! The device's output is pushed into a [`term_engine::ByteRing`] by a reader thread; a full
//! ring blocks that thread, so the device is not read (its driver buffers, then flow control or
//! the line itself holds the sender back) and no byte is dropped (PRD ST-3). A closed ring
//! (nobody reads the pane any more) ends the session and releases the device. Input comes from
//! [`SerialInput`], the `output` writer of a [`term_engine::Session`]; Enter is sent as the bare
//! CR the terminal produces. A serial line has no window size, so resizes are ignored
//! ([`resize_hook`]).
//!
//! When the device disappears (an adapter pulled out), the error is written into the ring in
//! red and the ring is closed; the pane then stays open with [`SESSION_ENDED_LINE`] (v0.2.15
//! `TerminalView.tsx`).

mod ports;
#[cfg(all(test, unix))]
mod tests;

use std::collections::HashMap;
use std::io::{ErrorKind as IoKind, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serialport::{DataBits, FlowControl, Parity, StopBits};
use term_engine::ByteRing;

/// What a serial pane shows once its session ended (v0.2.15 `EN/terminal.ts` `serialEnded`).
pub const SESSION_ENDED_NOTE: &str = "Session ended. Close this tab or open the port again.";
/// [`SESSION_ENDED_NOTE`] exactly as v0.2.15 writes it into the pane.
pub const SESSION_ENDED_LINE: &str = "\r\n[Session ended. Close this tab or open the port again.]\r\n";

/// An open serial device: the native type on Unix (exclusive access), the cross-platform
/// trait object elsewhere.
#[cfg(unix)]
type SerialPort = serialport::TTYPort;
#[cfg(windows)]
type SerialPort = Box<dyn serialport::SerialPort>;

fn open_serial(builder: serialport::SerialPortBuilder) -> serialport::Result<SerialPort> {
    #[cfg(unix)]
    return builder.open_native();
    #[cfg(windows)]
    return builder.open();
}

/// A second handle to `port`, for the reader thread.
fn clone_serial(port: &SerialPort) -> serialport::Result<SerialPort> {
    #[cfg(unix)]
    return port.try_clone_native();
    #[cfg(windows)]
    return port.try_clone();
}

/// Lists the serial devices present on this machine (`/dev/ttyUSB0`, `COM3`, ...): sysfs on
/// Linux (no libudev needed), the COM ports reported by the system on Windows. Listing never
/// opens a device.
pub fn list_ports() -> Vec<String> {
    #[cfg(unix)]
    let ports = ports::sysfs_ports();
    #[cfg(windows)]
    let ports = serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .unwrap_or_default();
    ports
}

/// The `on_resize` hook of a serial [`term_engine::Session`]: a serial line has no window size.
pub fn resize_hook() -> Arc<dyn Fn(u16, u16) + Send + Sync> {
    Arc::new(|_, _| {})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParitySetting {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowSetting {
    None,
    Software,
    Hardware,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialRequest {
    pub path: String,
    pub baud: u32,
    pub data_bits: u8,
    pub parity: ParitySetting,
    pub stop_bits: u8,
    pub flow: FlowSetting,
}

#[derive(Debug, thiserror::Error)]
pub enum SerialError {
    #[error(
        "permission denied opening {0}: add your user to the \"dialout\" group (sudo usermod -aG dialout $USER), then log out and back in"
    )]
    PermissionDenied(String),
    #[error("serial device {0} not found")]
    NotFound(String),
    #[error("serial device {0} is busy (it is already open in another program)")]
    Busy(String),
    #[error("invalid serial settings: {0}")]
    InvalidSettings(String),
    #[error("unknown serial session {0}")]
    UnknownSession(u32),
    #[error("{0}")]
    Io(String),
}

fn open_error(path: &str, err: serialport::Error) -> SerialError {
    match err.kind {
        serialport::ErrorKind::Io(IoKind::PermissionDenied) => SerialError::PermissionDenied(path.into()),
        serialport::ErrorKind::Io(IoKind::NotFound) => SerialError::NotFound(path.into()),
        serialport::ErrorKind::NoDevice => SerialError::Busy(path.into()),
        serialport::ErrorKind::InvalidInput => SerialError::InvalidSettings(err.description),
        _ => SerialError::Io(format!("{path}: {}", err.description)),
    }
}

fn io_err(err: impl std::fmt::Display) -> SerialError {
    SerialError::Io(err.to_string())
}

/// Checks the request and translates it into serialport settings.
fn settings(req: &SerialRequest) -> Result<(DataBits, Parity, StopBits, FlowControl), SerialError> {
    if req.path.trim().is_empty() {
        return Err(SerialError::InvalidSettings("device path is empty".into()));
    }
    if req.baud == 0 {
        return Err(SerialError::InvalidSettings("baud rate must be greater than 0".into()));
    }
    let data_bits = match req.data_bits {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        8 => DataBits::Eight,
        n => {
            return Err(SerialError::InvalidSettings(format!(
                "data bits must be 5, 6, 7 or 8 (got {n})"
            )));
        }
    };
    let stop_bits = match req.stop_bits {
        1 => StopBits::One,
        2 => StopBits::Two,
        n => {
            return Err(SerialError::InvalidSettings(format!(
                "stop bits must be 1 or 2 (got {n})"
            )));
        }
    };
    let parity = match req.parity {
        ParitySetting::None => Parity::None,
        ParitySetting::Odd => Parity::Odd,
        ParitySetting::Even => Parity::Even,
    };
    let flow = match req.flow {
        FlowSetting::None => FlowControl::None,
        FlowSetting::Software => FlowControl::Software,
        FlowSetting::Hardware => FlowControl::Hardware,
    };
    Ok((data_bits, parity, stop_bits, flow))
}

/// How long one read waits before checking whether the session was closed.
const READ_TIMEOUT: Duration = Duration::from_millis(100);
const READ_CHUNK: usize = 4096;

/// The write handle of a session; `None` once the session ended, which releases the device.
type PortSlot = Arc<Mutex<Option<SerialPort>>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn write_to(slot: &PortSlot, data: &[u8]) -> std::io::Result<()> {
    let mut port = lock(slot);
    let port = port
        .as_mut()
        .ok_or_else(|| std::io::Error::new(IoKind::BrokenPipe, "the serial session has ended"))?;
    port.write_all(data)?;
    port.flush()
}

/// Input of a serial session: the `output` writer of a [`term_engine::Session`]. Bytes go to
/// the device unchanged (Enter is the terminal's CR).
pub struct SerialInput(PortSlot);

impl Write for SerialInput {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        write_to(&self.0, buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A device opened by [`SerialManager::open`].
pub struct SerialSession {
    pub id: u32,
    pub input: SerialInput,
}

impl std::fmt::Debug for SerialSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialSession")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

struct Entry {
    port: PortSlot,
    stop: Arc<AtomicBool>,
    output: ByteRing,
}

#[derive(Default)]
pub struct SerialManager {
    next_id: AtomicU32,
    sessions: Mutex<HashMap<u32, Entry>>,
}

impl SerialManager {
    /// Opens the device; its output goes to `output`, which is closed when the session ends.
    pub fn open(&self, req: SerialRequest, output: ByteRing) -> Result<SerialSession, SerialError> {
        let (data_bits, parity, stop_bits, flow) = settings(&req)?;
        let builder = serialport::new(&req.path, req.baud)
            .data_bits(data_bits)
            .parity(parity)
            .stop_bits(stop_bits)
            .flow_control(flow)
            .timeout(READ_TIMEOUT);
        let port = open_serial(builder).map_err(|err| open_error(&req.path, err))?;
        let reader = clone_serial(&port).map_err(io_err)?;

        let port: PortSlot = Arc::new(Mutex::new(Some(port)));
        let stop = Arc::new(AtomicBool::new(false));
        let reading = {
            let (port, stop, output) = (port.clone(), stop.clone(), output.clone());
            let path = req.path;
            move || read_into(reader, &path, &output, &stop, &port)
        };
        std::thread::Builder::new()
            .name("serial-reader".into())
            .spawn(reading)
            .map_err(io_err)?;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.sessions).insert(
            id,
            Entry {
                port: port.clone(),
                stop,
                output,
            },
        );
        Ok(SerialSession {
            id,
            input: SerialInput(port),
        })
    }

    pub fn write(&self, id: u32, data: &[u8]) -> Result<(), SerialError> {
        let port = lock(&self.sessions)
            .get(&id)
            .map(|entry| entry.port.clone())
            .ok_or(SerialError::UnknownSession(id))?;
        write_to(&port, data).map_err(io_err)
    }

    /// Stops the reader thread, closes the device and the output ring, without the
    /// disconnection message.
    pub fn close(&self, id: u32) -> Result<(), SerialError> {
        let entry = lock(&self.sessions)
            .remove(&id)
            .ok_or(SerialError::UnknownSession(id))?;
        entry.stop.store(true, Ordering::Relaxed);
        lock(&entry.port).take();
        entry.output.close();
        Ok(())
    }
}

/// The reader thread: device output into `output` until the session is closed, the ring is
/// closed or the device fails.
fn read_into(mut reader: SerialPort, path: &str, output: &ByteRing, stop: &AtomicBool, port: &PortSlot) {
    let mut buf = vec![0u8; READ_CHUNK];
    while !stop.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            Ok(n) => {
                let Some(bytes) = buf.get(..n) else { break };
                // Nobody reads this terminal any more: release the device.
                if !bytes.is_empty() && output.push(bytes).is_err() {
                    break;
                }
            }
            Err(err) if matches!(err.kind(), IoKind::TimedOut | IoKind::WouldBlock | IoKind::Interrupted) => {}
            Err(err) => {
                // Closing the session from our side is not an error worth reporting.
                if !stop.load(Ordering::Relaxed) {
                    let message = format!("\r\n\x1b[31mSerial device {path} disconnected: {err}\x1b[0m\r\n");
                    let _ = output.push(message.as_bytes());
                }
                break;
            }
        }
    }
    lock(port).take();
    output.close();
}
