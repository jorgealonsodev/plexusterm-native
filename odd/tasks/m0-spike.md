# M0 spike — PlexusTerm Native

Objective: deliver PRD milestone M0 (`docs/PRD.md` §12.2): a GPUI terminal on `alacritty_terminal` with a local PTY, Tokio bridge, theme derivation and the §6.3 runtime, measured against NFR-1/NFR-2.
Branch: `feat/m0-spike`. Delivery strategy: `ask-on-risk`. Test runner: `cargo test --workspace` (+ `cargo clippy --workspace --all-targets -- -D warnings`).

## Specs

- **S1** Workspace per PRD §6.1 (domain-named crates; only `ui-*` and `app` depend on GPUI), Rust edition 2024, `rust-toolchain.toml`, committed `Cargo.lock`, ST-12 lints (`unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `await_holding_lock` denied outside tests), `panic = "unwind"` release profile (ST-8).
- **S2** `ui-theme`: §8.1 THEME derivation (sRGB per-channel `mix`, Rec.709 luminance on non-linearised channels, 0.4 threshold) and the 57 built-in schemes of Appendix C as generated Rust data, default `catppuccin-mocha`, unknown id falls back to it.
- **S3** `term-engine`: session runtime per §6.3 ST-1..ST-6 and ST-9 — parser thread owning `alacritty_terminal::Term` under `FairMutex`, bounded 4 MiB byte ring with backpressure (never drop, never unbounded), parse slices ≤ 1 MiB / 4 ms, dirty flag + damage snapshot, bounded write queue with 64 KiB paste chunks, coalesced resize, input caps.
- **S4** `proto-pty`: local PTY source via `portable-pty` (`TERM=xterm-256color`, `COLORTERM=truecolor`), shell detection from `$SHELL`, exit reporting; failures behave as v0.2.15 abnormal end (ST-8).
- **S5** `app`: GPUI window 1200×760 "PlexusTerm", one terminal pane painting only damaged lines, keyboard input incl. IME handler, Tokio runtime bridge (`gpui_tokio` pattern), GPUI pinned to an exact Zed git rev.
- **S6** `PLEXUS_LATENCY_LOG` (TERM-17) and frame-time log; NFR-1/NFR-2 tier-1 measurements recorded in `docs/benchmarks/m0.md`.
- **S7** AccessKit smoke (a button and the tab strip readable by Orca) and D4 evaluation (`gpui-kit` vs own widgets) recorded as decisions.

## Tasks

| ID | Specs | Route | Status | Evidence |
| --- | --- | --- | --- | --- |
| T1 | S1 | inline | done | commit `feat: scaffold workspace and ui-theme` (see L4) |
| T2 | S2 | inline | done | RED 3/13 failing on `mix`/`readable_on` stubs → GREEN 13/13 (L4) |
| T3 | S3 (ST-1..6, ST-8) | inline | done | commit `feat(term-engine)…`; 15 tests, 4 neuter checks (L5) |
| T3b | S3 (ST-9 input caps) + review advisories of L7 | inline | pending | |
| T4 | S4 | inline | done | commit `feat(proto-pty)…`; 16 tests incl. end-to-end PTY→Session→Frame (L8) |
| T5 | S5 | inline | pending | needs system packages (see Log) |
| T6 | S6 | inline | pending | |
| T7 | S7 | inline | pending | |

## Log

- **L1** (2026-10-21, user, verbatim): "empieza con el desarrollo"
- **L2** Context: PRD v4.0 (`docs/PRD.md`), parity matrix (`docs/parity-matrix.md`), reference screenshots (`docs/reference/`). Owner decisions so far: GPUI (D1), stability architecture §6.3 (D7), GPU on by default on Linux (§4.1), no invented crash UI (ST-8), relay crates are the owner's, MIT.
- **L3** Host: rustc 1.98.1, X11 session, NVIDIA GTX 1080 + Intel UHD 630 (Vulkan 1.4). Missing for GPUI builds: `libxkbcommon-x11-dev`, `libvulkan-dev`, `libx11-xcb-dev`, `libssl-dev`, `clang` (T5 blocker until installed).
- **L4** T1+T2: workspace (edition 2024, toolchain 1.98.1, ST-12 lints with clippy.toml test allowances, `panic = "unwind"`), `ui-theme` with 57 generated schemes (`scripts/gen_schemes.py`) and THEME-1 derivation. Tests ported from `uiTheme.test.ts`/`themes.test.ts` + golden `surface` and 0.4-threshold edge cases. `cargo test --workspace`: 13 passed; `cargo clippy --workspace --all-targets -- -D warnings`: clean. Risk: none.
- **L5** T3: `term-engine` = `ByteRing` (bounded, blocking push = backpressure, try_push for keys), `Session` (parser thread with `FairMutex<Term>`, ≤ 1 MiB / 4 ms slices, dirty flag + `Wakeup` once per batch, damage-only `Frame`s, coalesced resize via `on_resize`, write queue 4 MiB drained in ≤ 64 KiB writes, paste helper thread, `catch_unwind` supervision → `EndReason::Failed`, OSC 52 disabled as in v0.2.15). Tests: ring 5, session 10, all green ×5 runs. Neuters observed failing: always-full snapshot (4 fail), no panic isolation (1 fail), unbounded ring (2 fail), single-write paste (1 fail). Known gap: keystrokes typed during a running paste may interleave with it (v0.2.15 sends a paste at once) — revisit with the app. ST-9 caps split into T3b. Risk: item 4 (concurrency: locks and thread handoff) — covered by the tests above; independent verifier deferred to the M0 end per the feature-level verification timing.
- **L6** Native review of `28a29cf..4666fdc` (lenses resilience + reliability, consent granted): **approved**, acknowledged (authority burned, lineage `review-45eda13515ac3313`). 9 advisory findings fixed in a follow-up: one paste worker per session so pastes and keys typed behind them stay in order; input after the end refused and the write queue closed on end; owner events emitted after the terminal lock is released; `on_resize` panics end only the session; first end reason wins (spawn-failure race); per-row flags in `read_frame`; deadline-polled ring test. New tests: 4 (ordering under a slow source with 2×6 MiB pastes, input after end, hook reads screen without deadlock, failing resize hook). Neuters observed failing: paste-after-end, unprotected resize hook. Suite 19 term-engine + 13 ui-theme green ×5; clippy clean. The keystroke/paste interleave gap of L5 is closed. Review boundary advances to the follow-up commit once assessed.
- **L7** Native review of `4666fdc..a77f467` (resilience + reliability, consent granted): **approved**, acknowledged (authority burned). Advisories carried into T3b: keys typed during a paste count per message against the 32-slot paste queue (merge consecutive keys or budget by bytes, add an overflow test); a panicking `on_resize` emits `Ended` on the snapshot caller's thread, contradicting the hook doc (defer the event to the parser thread or document it); buffered owner events are unbounded per batch (coalesce `Bell`) and events raised during a snapshot resize wait for the next batch (drain after snapshot).
- **L8** T4: `proto-pty` ported from v0.2.15 `pty/mod.rs` (default shell `$SHELL`/`/bin/bash`, Windows pwsh→powershell→COMSPEC; `~` and blank cwd → home; `TERM=xterm-256color`, `COLORTERM=truecolor`; `ChildEnv` hides values in `Debug`; `/proc` cwd and foreground program cwd). Native change: the reader pushes into the session `ByteRing` (blocking = backpressure) and kills the program when the session stops reading, so no orphaned process. 14 ported tests + `a_session_that_stops_reading_ends_its_program` (neuter observed: "the process never exited") + end-to-end shell → `Session` → frame. `ByteRing::is_closed` added with a test. Workspace: 10 test binaries green ×3, clippy clean. v0.2.15 `a_large_paste_never_blocks…` is covered by term-engine's paste worker; `unknown_session_is_an_error` has no native equivalent (no session ids; the type owns the PTY). Risk: item 4 (thread handoff, process lifetime).
