//! NFR-3/NFR-4: load only the Vulkan driver of the GPU that drives the display (T6c).
//!
//! The Vulkan loader loads every installed ICD (Intel, radv with `libLLVM`, nouveau, lavapipe…)
//! when it creates an instance, which costs ≈ 80 MB of RSS and ≈ 70 ms here
//! (`docs/benchmarks/m0.md`, T6b/T6c). `VK_DRIVER_FILES` restricts it, but it must be set before
//! the loader runs and `std::env::set_var` is `unsafe` in edition 2024, so `main` re-executes
//! the binary once with the variable set. Any doubt (a user choice, no single display GPU, an
//! unknown driver, no matching ICD) means no restriction.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Set on the re-executed process, so it never re-executes again.
pub const MARKER: &str = "PLEXUS_VK_DRIVER_PICKED";

/// Variables through which the user already chose drivers or a GPU; any of them, even empty,
/// means the app does not restrict anything.
const USER_CHOICES: [&str; 7] = [
    "VK_DRIVER_FILES",
    "VK_ICD_FILENAMES",
    "VK_ADD_DRIVER_FILES",
    "VK_LOADER_DRIVERS_SELECT",
    "VK_LOADER_DRIVERS_DISABLE",
    "DRI_PRIME",
    "__NV_PRIME_RENDER_OFFLOAD",
];

/// A DRM card (`/sys/class/drm/cardN/device`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// `boot_vga` reads `1`: the firmware's primary display GPU.
    pub boot_vga: bool,
    /// Kernel driver name (`device/driver` link): `nvidia`, `i915`, `amdgpu`…
    pub driver: String,
}

/// An ICD manifest found in a Vulkan search directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icd {
    pub path: PathBuf,
    /// `ICD.library_path` from the manifest.
    pub library: String,
}

/// Library name prefixes of the Vulkan drivers for a kernel driver.
fn libraries_for(driver: &str) -> Option<&'static [&'static str]> {
    Some(match driver {
        "nvidia" => &["libGLX_nvidia"],
        // anv and hasvk (older Intel generations); the loader drops the one that does not apply.
        "i915" | "xe" => &["libvulkan_intel"],
        "amdgpu" => &["libvulkan_radeon", "amdvlk"],
        "radeon" => &["libvulkan_radeon"],
        "nouveau" => &["libvulkan_nouveau"],
        "virtio_gpu" => &["libvulkan_virtio"],
        _ => return None,
    })
}

/// The `VK_DRIVER_FILES` value to restart with, or `None` to leave the loader alone.
pub fn choose(env: &dyn Fn(&str) -> Option<OsString>, cards: &[Card], icds: &[Icd]) -> Option<OsString> {
    if env(MARKER).is_some() || USER_CHOICES.iter().any(|name| env(name).is_some()) {
        return None;
    }
    let mut display = cards.iter().filter(|card| card.boot_vga);
    let card = display.next()?;
    if display.next().is_some() {
        return None;
    }
    let wanted = libraries_for(&card.driver)?;
    let mut chosen: Vec<&Path> = Vec::new();
    for icd in icds {
        let name = icd.library.rsplit('/').next().unwrap_or(&icd.library);
        if wanted.iter().any(|prefix| name.starts_with(prefix)) && !chosen.contains(&icd.path.as_path()) {
            chosen.push(&icd.path);
        }
    }
    if chosen.is_empty() {
        return None;
    }
    std::env::join_paths(chosen).ok()
}

/// `ICD.library_path` of a manifest, without a JSON dependency (the field is a plain string).
pub fn library_path(manifest: &str) -> Option<String> {
    let after_key = manifest.split_once("\"library_path\"")?.1;
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    let value = after_colon.strip_prefix('"')?;
    Some(value.split_once('"')?.0.replace("\\/", "/"))
}

/// Whether a manifest file name is for another architecture (`intel_icd.i686.json` on x86_64).
fn other_arch(file: &OsStr) -> bool {
    const ARCHES: [&str; 8] = [
        "i386", "i686", "x86_64", "aarch64", "armv7l", "armv7hl", "ppc64le", "riscv64",
    ];
    let Some(stem) = file.to_str().and_then(|name| name.strip_suffix(".json")) else {
        return false;
    };
    match stem.rsplit_once('.') {
        Some((_, arch)) if ARCHES.contains(&arch) => arch != std::env::consts::ARCH,
        _ => false,
    }
}

/// The ICD search directories, in the loader's order, without duplicates.
pub fn icd_dirs(env: &dyn Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let data_dirs = env("XDG_DATA_DIRS")
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| OsString::from("/usr/local/share:/usr/share"));
    let mut dirs = vec![PathBuf::from("/etc/vulkan/icd.d")];
    dirs.extend(std::env::split_paths(&data_dirs).map(|dir| dir.join("vulkan/icd.d")));
    dirs.push(PathBuf::from("/usr/share/vulkan/icd.d"));
    let mut unique: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        if !unique.contains(&dir) {
            unique.push(dir);
        }
    }
    unique
}

fn read_icds(dirs: &[PathBuf]) -> Vec<Icd> {
    let mut icds = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut paths: Vec<PathBuf> = entries.filter_map(|entry| entry.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths {
            let Some(file) = path.file_name() else { continue };
            if other_arch(file) || path.extension() != Some(OsStr::new("json")) {
                continue;
            }
            if let Some(library) = std::fs::read_to_string(&path).ok().as_deref().and_then(library_path) {
                icds.push(Icd { path, library });
            }
        }
    }
    icds
}

fn read_cards() -> Vec<Card> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        // `card1`, not its connectors (`card1-DP-1`), whose `device` is the same card.
        let is_card = name
            .to_str()
            .and_then(|n| n.strip_prefix("card"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        if !is_card {
            continue;
        }
        let device = entry.path().join("device");
        let boot_vga = std::fs::read_to_string(device.join("boot_vga")).is_ok_and(|v| v.trim() == "1");
        let driver = std::fs::read_link(device.join("driver"))
            .ok()
            .and_then(|link| link.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        cards.push(Card { boot_vga, driver });
    }
    cards
}

/// The variables the restart added, which the terminal's shell must not inherit: programs run
/// from it pick their own Vulkan drivers. Empty unless this process was restarted by
/// [`reexec_with_display_driver`] (a user's own `VK_DRIVER_FILES` is never touched).
pub fn variables_to_hide(env: &dyn Fn(&str) -> Option<OsString>) -> &'static [&'static str] {
    if env(MARKER).is_some() {
        &["VK_DRIVER_FILES", MARKER]
    } else {
        &[]
    }
}

/// Re-executes this binary with `VK_DRIVER_FILES` set to the display GPU's ICD when [`choose`]
/// picks one; returns only when it does not, or when the exec failed.
pub fn reexec_with_display_driver() {
    use std::os::unix::process::CommandExt;

    let env = |name: &str| std::env::var_os(name);
    if env(MARKER).is_some() || USER_CHOICES.iter().any(|name| env(name).is_some()) {
        return;
    }
    let Some(files) = choose(&env, &read_cards(), &read_icds(&icd_dirs(&env))) else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else { return };
    let mut args = std::env::args_os();
    let mut command = std::process::Command::new(exe);
    if let Some(arg0) = args.next() {
        command.arg0(arg0);
    }
    let error = command.args(args).env("VK_DRIVER_FILES", files).env(MARKER, "1").exec();
    eprintln!("plexusterm: could not restart with the display GPU's Vulkan driver: {error}");
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_shell_never_inherits_the_driver_restriction_the_restart_added() {
        let restarted = |name: &str| (name == MARKER || name == "VK_DRIVER_FILES").then(|| OsString::from("1"));
        assert_eq!(variables_to_hide(&restarted), ["VK_DRIVER_FILES", MARKER]);
        let users_own = |name: &str| (name == "VK_DRIVER_FILES").then(|| OsString::from("/x.json"));
        assert!(variables_to_hide(&users_own).is_empty());
    }

    use super::*;

    fn env_of(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let vars: Vec<(String, OsString)> = vars.iter().map(|(k, v)| ((*k).to_owned(), OsString::from(v))).collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    fn card(boot_vga: bool, driver: &str) -> Card {
        Card {
            boot_vga,
            driver: driver.to_owned(),
        }
    }

    fn icd(path: &str, library: &str) -> Icd {
        Icd {
            path: PathBuf::from(path),
            library: library.to_owned(),
        }
    }

    /// The manifests installed on the T6 machine (Ubuntu, NVIDIA proprietary + Mesa).
    fn installed() -> Vec<Icd> {
        vec![
            icd("/usr/share/vulkan/icd.d/asahi_icd.json", "libvulkan_asahi.so"),
            icd(
                "/usr/share/vulkan/icd.d/intel_hasvk_icd.json",
                "libvulkan_intel_hasvk.so",
            ),
            icd("/usr/share/vulkan/icd.d/intel_icd.json", "libvulkan_intel.so"),
            icd("/usr/share/vulkan/icd.d/lvp_icd.json", "libvulkan_lvp.so"),
            icd("/usr/share/vulkan/icd.d/nouveau_icd.json", "libvulkan_nouveau.so"),
            icd("/usr/share/vulkan/icd.d/nvidia_icd.json", "libGLX_nvidia.so.0"),
            icd(
                "/usr/share/vulkan/icd.d/radeon_icd.json",
                "/usr/lib/x86_64-linux-gnu/libvulkan_radeon.so",
            ),
            icd("/opt/amdvlk/amd_icd64.json", "/opt/amdvlk/amdvlk64.so"),
        ]
    }

    fn pick(env: &[(&str, &str)], cards: &[Card]) -> Option<String> {
        choose(&env_of(env), cards, &installed()).map(|v| v.to_string_lossy().into_owned())
    }

    #[test]
    fn nvidia_display_gpu_picks_the_nvidia_icd() {
        assert_eq!(
            pick(&[], &[card(true, "nvidia")]).as_deref(),
            Some("/usr/share/vulkan/icd.d/nvidia_icd.json")
        );
    }

    #[test]
    fn intel_display_gpu_picks_anv_and_hasvk() {
        let both = "/usr/share/vulkan/icd.d/intel_hasvk_icd.json:/usr/share/vulkan/icd.d/intel_icd.json";
        assert_eq!(pick(&[], &[card(true, "i915")]).as_deref(), Some(both));
        assert_eq!(pick(&[], &[card(true, "xe")]).as_deref(), Some(both));
    }

    #[test]
    fn amd_display_gpu_picks_radv_and_amdvlk() {
        assert_eq!(
            pick(&[], &[card(true, "amdgpu")]).as_deref(),
            Some("/usr/share/vulkan/icd.d/radeon_icd.json:/opt/amdvlk/amd_icd64.json")
        );
    }

    #[test]
    fn hybrid_machine_picks_the_gpu_that_drives_the_display() {
        // The T6 machine: Intel iGPU (card1, boot_vga 0) and the GTX 1080 driving the monitors.
        let desktop = [card(false, "i915"), card(true, "nvidia")];
        assert_eq!(
            pick(&[], &desktop).as_deref(),
            Some("/usr/share/vulkan/icd.d/nvidia_icd.json")
        );
        // A laptop whose panel is on the Intel GPU, with an NVIDIA GPU for offload.
        let laptop = [card(true, "i915"), card(false, "nvidia")];
        assert_eq!(
            pick(&[], &laptop).as_deref(),
            Some("/usr/share/vulkan/icd.d/intel_hasvk_icd.json:/usr/share/vulkan/icd.d/intel_icd.json")
        );
    }

    #[test]
    fn a_user_choice_is_respected() {
        let cards = [card(true, "nvidia")];
        for name in USER_CHOICES {
            assert_eq!(pick(&[(name, "/my/icd.json")], &cards), None, "{name}");
            assert_eq!(pick(&[(name, "")], &cards), None, "{name} set but empty");
        }
    }

    #[test]
    fn the_marker_prevents_a_second_restart() {
        assert_eq!(pick(&[(MARKER, "1")], &[card(true, "nvidia")]), None);
    }

    #[test]
    fn missing_or_ambiguous_information_restricts_nothing() {
        // No ICD for the display GPU's driver.
        assert_eq!(choose(&env_of(&[]), &[card(true, "nvidia")], &[]), None);
        assert_eq!(
            choose(
                &env_of(&[]),
                &[card(true, "nouveau")],
                &[icd("/a/nvidia_icd.json", "libGLX_nvidia.so.0")]
            ),
            None
        );
        // An unknown driver, no display GPU, two display GPUs, no cards at all.
        assert_eq!(pick(&[], &[card(true, "vmwgfx")]), None);
        assert_eq!(pick(&[], &[card(false, "nvidia"), card(false, "i915")]), None);
        assert_eq!(pick(&[], &[card(true, "nvidia"), card(true, "i915")]), None);
        assert_eq!(pick(&[], &[]), None);
    }

    #[test]
    fn the_same_manifest_found_twice_is_listed_once() {
        let icds = [
            icd("/usr/share/vulkan/icd.d/nvidia_icd.json", "libGLX_nvidia.so.0"),
            icd("/usr/share/vulkan/icd.d/nvidia_icd.json", "libGLX_nvidia.so.0"),
        ];
        assert_eq!(
            choose(&env_of(&[]), &[card(true, "nvidia")], &icds),
            Some(OsString::from("/usr/share/vulkan/icd.d/nvidia_icd.json"))
        );
    }

    #[test]
    fn library_path_is_read_from_a_manifest() {
        let manifest = r#"{
            "file_format_version" : "1.0.1",
            "ICD": {
                "library_path": "libGLX_nvidia.so.0",
                "api_version" : "1.4.312"
            }
        }"#;
        assert_eq!(library_path(manifest).as_deref(), Some("libGLX_nvidia.so.0"));
        assert_eq!(
            library_path(r#"{"ICD":{"library_path":"\/usr\/lib\/libvulkan_intel.so"}}"#).as_deref(),
            Some("/usr/lib/libvulkan_intel.so")
        );
        assert_eq!(library_path(r#"{"ICD":{}}"#), None);
    }

    #[test]
    fn manifests_for_another_architecture_are_skipped() {
        let other = if std::env::consts::ARCH == "x86_64" {
            "i686"
        } else {
            "x86_64"
        };
        assert!(other_arch(OsStr::new(&format!("intel_icd.{other}.json"))));
        assert!(!other_arch(OsStr::new(&format!(
            "intel_icd.{}.json",
            std::env::consts::ARCH
        ))));
        assert!(!other_arch(OsStr::new("nvidia_icd.json")));
    }

    #[test]
    fn search_directories_follow_xdg_data_dirs() {
        assert_eq!(
            icd_dirs(&env_of(&[("XDG_DATA_DIRS", "/opt/share:/usr/share")])),
            [
                PathBuf::from("/etc/vulkan/icd.d"),
                PathBuf::from("/opt/share/vulkan/icd.d"),
                PathBuf::from("/usr/share/vulkan/icd.d"),
            ]
        );
        assert_eq!(
            icd_dirs(&env_of(&[])),
            [
                PathBuf::from("/etc/vulkan/icd.d"),
                PathBuf::from("/usr/local/share/vulkan/icd.d"),
                PathBuf::from("/usr/share/vulkan/icd.d"),
            ]
        );
    }
}
