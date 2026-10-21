# Handoff — 2026-10-21

Work continues from the `integration` branch: every feature branch plus T6c and the priority
fixes, merged; `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace`
(73 suites) and `cargo fmt --check` all pass. `main` is untouched: merging is a user decision.

## Open first
1. **SFTP upload fix (d9e338b) — native review escalated, regressions corroborated.** The temp
   file + rename swap (a) creates the temp with the server's default mode, so a 0600 file's new
   contents are readable during the upload; (b) needs write access to the folder, where the old
   in-place write only needed it on the file; (c) replaces a symlink or hard link with a plain
   file and leaves the real target stale. Planned direction: create the temp with the original's
   mode up front, upload in place (as before, but only after the bytes are staged) when the target
   is a symlink, has several links or the folder is not writable, and test each case
   (OpenSSH interop: `odd/followups/openssh-interop.sh`, env INTEROP_DIR and INTEROP_CRATE).
2. Native reviews still to run (each asks for consent): session-library fixes 23e910c and c1e7f99,
   settings-model fix b8e6d72, T6c vendor patch range 24f82bd..76a4c23 (feat/m0-spike),
   workspace-model p3/p4 (b70e232, 0ed10bc), settings-model p3 (d29c45a).
3. Advisory follow-ups from approved reviews: `odd/followups/queue.txt` plus the per-crate notes
   in the same folder.

## Then
- Wire ui-kit and the logic crates into the app (sidebar, tabs, panes, settings): the first
  visible M2 slice.
- M0 T5b (IME, DECCKM, selection/copy, bundled font, cursor blink), T7 (AccessKit smoke).
- M1: Mosh, share client, DATA-4 round trips; dedupe proto-sftp `text.rs` with local-files.
