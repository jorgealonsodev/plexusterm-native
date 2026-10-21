# Changes to the vendored `gpui_wgpu`

Source: `crates/gpui_wgpu` of <https://github.com/zed-industries/zed> at
`a484d108da9164cd6a705e7422a0ddd33499fb50` (the rev the app pins, ST-13), Apache-2.0 (PRD §13
D5). It replaces the git crate through `[patch."https://github.com/zed-industries/zed"]` in the
workspace `Cargo.toml`, and is excluded from the workspace, so it keeps its own lints.

1. **`src/wgpu_context.rs`, `WgpuContext::instance`** (the only source change). Upstream creates
   the instance with `Backends::VULKAN | Backends::GL`, so every window start also loads the
   EGL/GL stacks (NVIDIA's and Mesa's, with `libgallium`) next to Vulkan. The patched function
   creates a Vulkan-only instance and keeps it when it enumerates at least one Vulkan adapter;
   otherwise it logs a warning and creates a GL-only instance, so a machine without a Vulkan
   driver still starts (NFR-10). The display handle is shared through an `Arc` so both instances
   can receive it. Residual difference: upstream could fall back to a GL adapter when every
   Vulkan adapter exists but fails the surface test; the patched code cannot. Measured effect:
   `docs/benchmarks/m0.md`, T6c.
2. **`Cargo.toml`** rewritten for a stand-alone crate: Zed's `workspace = true` entries replaced by
   the specs of Zed's workspace manifest at the same rev (`gpui`, `collections`, `gpui_util` as
   git dependencies on that rev, `wgpu` on its pinned git rev); `[lints] workspace = true`
   replaced by Zed's one rustc lint (`unexpected_cfgs = allow`); dev-dependencies and the
   `layout_line` bench dropped (not built here; the `src` tests need Zed's font assets and are
   not run).
3. **Added** `NOTICE`, `LICENSE-APACHE` (Zed's repository licence file, which the crate's
   `LICENSE-APACHE` symlink pointed to) and this file.

To update: copy `crates/gpui_wgpu/src` of the new pinned rev, re-apply change 1, and refresh the
dependency specs of change 2 from that rev's workspace `Cargo.toml`.
