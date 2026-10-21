//! Serial port listing from sysfs, so it works without libudev. Ported from v0.2.15
//! `src-tauri/src/serial/ports.rs`.
//!
//! Only reads directory entries and sysfs attributes; it never opens a device node.

use std::path::Path;

/// Device name prefixes offered as serial consoles.
const PREFIXES: [&str; 3] = ["ttyUSB", "ttyACM", "ttyS"];

/// Lists the serial devices present on this Linux machine (e.g. `/dev/ttyUSB0`).
#[cfg_attr(windows, allow(dead_code))]
pub fn sysfs_ports() -> Vec<String> {
    list_ports_in(Path::new("/sys/class/tty"), Path::new("/dev"))
}

/// Lists `<dev>/<name>` for every `<sys_tty>/<name>` that is a serial device backed by hardware:
/// it has a `device` link, its node exists in `dev`, and it is not an unpopulated UART
/// (`type` attribute `0`, which the kernel reports for the placeholder ttyS ports).
#[cfg_attr(windows, allow(dead_code))]
pub fn list_ports_in(sys_tty: &Path, dev: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(sys_tty) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| split_name(name).is_some())
        .filter(|name| {
            let dir = sys_tty.join(name);
            dir.join("device").exists() && !is_placeholder_uart(&dir) && dev.join(name).exists()
        })
        .collect();
    names.sort_by_key(|name| split_name(name));
    names
        .into_iter()
        .map(|name| dev.join(name).to_string_lossy().into_owned())
        .collect()
}

/// Splits `ttyUSB10` into `("ttyUSB", 10)` for a natural sort; `None` for non-serial names.
fn split_name(name: &str) -> Option<(&'static str, u32)> {
    PREFIXES.iter().find_map(|prefix| {
        let number = name.strip_prefix(prefix)?;
        Some((*prefix, number.parse().ok()?))
    })
}

fn is_placeholder_uart(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("type")).is_ok_and(|t| t.trim() == "0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn add_tty(sys: &Path, dev: &Path, name: &str, with_device: bool, uart_type: Option<&str>) {
        let dir = sys.join(name);
        fs::create_dir_all(&dir).unwrap();
        if with_device {
            fs::create_dir(dir.join("device")).unwrap();
        }
        if let Some(t) = uart_type {
            fs::write(dir.join("type"), format!("{t}\n")).unwrap();
        }
        fs::write(dev.join(name), "").unwrap();
    }

    #[test]
    fn lists_only_hardware_backed_serial_devices_sorted() {
        let root = tempfile::tempdir().unwrap();
        let sys = root.path().join("sys");
        let dev = root.path().join("dev");
        fs::create_dir_all(&sys).unwrap();
        fs::create_dir_all(&dev).unwrap();

        add_tty(&sys, &dev, "ttyUSB10", true, None);
        add_tty(&sys, &dev, "ttyUSB2", true, None);
        add_tty(&sys, &dev, "ttyACM0", true, None);
        add_tty(&sys, &dev, "ttyS0", true, Some("4"));
        // Placeholder UART without hardware behind it.
        add_tty(&sys, &dev, "ttyS1", true, Some("0"));
        // Virtual terminals and ptys have no `device` link.
        add_tty(&sys, &dev, "tty1", false, None);
        add_tty(&sys, &dev, "ttyUSB3", false, None);
        // Not a serial prefix even with a device link.
        add_tty(&sys, &dev, "console", true, None);
        // Device link but no node in /dev.
        fs::create_dir_all(sys.join("ttyUSB7/device")).unwrap();

        let d = |n: &str| dev.join(n).to_string_lossy().into_owned();
        assert_eq!(
            list_ports_in(&sys, &dev),
            vec![d("ttyACM0"), d("ttyS0"), d("ttyUSB2"), d("ttyUSB10")]
        );
    }

    #[test]
    fn missing_sysfs_directory_lists_nothing() {
        let root = tempfile::tempdir().unwrap();
        assert!(list_ports_in(&root.path().join("nope"), root.path()).is_empty());
    }
}
