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
| T3 | S3 | inline | in progress | |
| T4 | S4 | inline | pending | |
| T5 | S5 | inline | pending | needs system packages (see Log) |
| T6 | S6 | inline | pending | |
| T7 | S7 | inline | pending | |

## Log

- **L1** (2026-10-21, user, verbatim): "empieza con el desarrollo"
- **L2** Context: PRD v4.0 (`docs/PRD.md`), parity matrix (`docs/parity-matrix.md`), reference screenshots (`docs/reference/`). Owner decisions so far: GPUI (D1), stability architecture §6.3 (D7), GPU on by default on Linux (§4.1), no invented crash UI (ST-8), relay crates are the owner's, MIT.
- **L3** Host: rustc 1.98.1, X11 session, NVIDIA GTX 1080 + Intel UHD 630 (Vulkan 1.4). Missing for GPUI builds: `libxkbcommon-x11-dev`, `libvulkan-dev`, `libx11-xcb-dev`, `libssl-dev`, `clang` (T5 blocker until installed).
- **L4** T1+T2: workspace (edition 2024, toolchain 1.98.1, ST-12 lints with clippy.toml test allowances, `panic = "unwind"`), `ui-theme` with 57 generated schemes (`scripts/gen_schemes.py`) and THEME-1 derivation. Tests ported from `uiTheme.test.ts`/`themes.test.ts` + golden `surface` and 0.4-threshold edge cases. `cargo test --workspace`: 13 passed; `cargo clippy --workspace --all-targets -- -D warnings`: clean. Risk: none.
