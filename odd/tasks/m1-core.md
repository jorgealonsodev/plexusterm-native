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
- **L3** Native review of `8c6ef85..bd4f865` (risk + resilience): **approved**, acknowledged (lineage `review-47249067a063b3c1`). Applied: the parent directory is fsynced after the rename (a reported save survives a crash); `write_text` writes a temp file and renames it (never truncates first), through a symlink to its file and keeping that file's mode; `save` removes this document's temp files left by a dead writer, only when older than 10 minutes and not ours, so a concurrent v0.2.15 save is never broken (DATA-2a); `config_dir()` assertion only when `dirs` finds a base. RED observed for the stale-temp and atomic `write_text` tests; neuters observed failing (age check, non-atomic write). Not applied: text files keep the umask default mode (v0.2.15 parity; they are files the user chose); a per-path write lock (performance only).
- **L4** Native review of `bd4f865..8e4fd67` (risk + resilience): **approved**, acknowledged (lineage `review-d378396f445aea16`). Two findings changed the design: (1) `write_text` back to writing in place, as v0.2.15 — a temp + rename replaced the inode of a file the user chose (owner, group, ACLs, xattrs, hard links lost) and failed for a writable file in a read-only directory; config documents keep the atomic write; (2) a temp name left by a crashed process with our pid (pid namespaces) made `create_new` fail on every save — it is now removed and the create retried (unit test, neuter observed failing). Text-file temp leftovers no longer exist (no temp for text files).
- **L5** Native review of `8e4fd67..d11e73c` (risk + resilience): **approved**, acknowledged (lineage `review-65ea8aff390678e4`). Applied: unlinking a taken temp name could destroy a live writer's file (two containers sharing the config dir, both pid 1) — temp names now add the clock's nanoseconds and a taken name is skipped for another, never removed (unit test with an injected name sequence; neuter observed failing); the stale sweep applies the 10-minute age rule to every `<name>.tmp-*`, ours included (neuter observed failing); `write_text` stays in place (v0.2.15) but fsyncs before reporting success. Accepted: in-place `write_text` can still lose the old contents on ENOSPC, as v0.2.15 does, to keep owner/ACL/links and read-only-directory writes working.
