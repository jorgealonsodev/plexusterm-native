# M1 core — PlexusTerm Native

Objective: deliver PRD milestone M1 (`docs/PRD.md` §12.2): the core crates extracted from PlexusTerm v0.2.15 `src-tauri` behind ports, with their Rust tests green outside Tauri and DATA-4 round-trip tests. Runs in parallel with M0 (blocked on GPUI system packages, see `odd/tasks/m0-spike.md` L3).
Branch: `feat/m1-core` (from `feat/m0-spike`). Delivery strategy: `ask-on-risk`. Test runner: `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings`.

## Specs

- **S1** `core-storage`: config directory exactly as Tauri `app_config_dir` for `dev.plexusterm.app` (DATA-1), the v0.2.15 file names, atomic private writes with JSON validation and Windows rename retries (DATA-2), import/text/history caps.
- **S2** Keyring adapter with the v0.2.15 naming (DATA-3): service `dev.plexusterm.app`, accounts = session id / `ai:<provider>`, saved-password JSON `{"v":1,"endpoint","password"}`.
- **S3** Protocol crates ported with their tests: SSH/SFTP (russh), tunnels, telnet, serial, mosh, keygen, AI clients, git, local files, project settings.
- **S4** DATA-4 round-trip fixtures: every v0.2.15 file loads and re-saves equivalently.

## Tasks

| ID | Specs | Route | Status | Evidence |
| --- | --- | --- | --- | --- |
| T1 | S1 | inline | done | commit `feat(core-storage)…`; 14 tests (L2) |
| T2 | S2 | inline | pending | |
| T3 | S3 | inline | pending | split per protocol |
| T4 | S4 | inline | pending | |

## Log

- **L1** (2026-10-21, user, verbatim): "empieza con el desarrollo" — then "haz lo que consideres mas optimo" while the GPUI packages stay missing (`sudo` needs a password), so M1 starts in parallel as the PRD allows.
- **L2** T1: `core-storage` ported from `storage.rs` + `history.rs` (`load`, `save` with JSON validation, temp + fsync + rename, 0600, per-process write lock, Windows rename retries; `read_bounded`, `write_text`, `append_text`, `save_history`; constants for the 7 v0.2.15 files and the 5 MiB / 50 MiB / 32 MiB caps; `config_dir()` = `dirs::config_dir()/dev.plexusterm.app`, the same `dirs 7.0.0` Tauri resolves with). 13 tests ported + `replacing_an_existing_world_readable_file_makes_it_private`. Neuters observed failing: mode 0644 (2 tests), no JSON validation (1). Risk: item 1 (persisted user data) — behaviour identical to v0.2.15.
