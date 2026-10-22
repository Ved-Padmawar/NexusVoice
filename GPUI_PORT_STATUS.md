# GPUI port: status and remaining work

Branch `feat/gpui-port`. Tauri, React, Specta and the IPC bridge are gone; the
app is one native Rust process using GPUI Kit 0.8 with Phosphor icons.

## Done
- Backend decoupled from Tauri: `src/events.rs` (typed event channel),
  `src/transcription/controller.rs` (recording/pill state machine, moved out of
  the old PillApp.tsx), `src/api/` (plain functions replacing commands).
- Platform services in `src/platform/`: tray, global hotkeys, single instance
  (forwards `--toggle-dictation` etc.), clipboard, JSON-line logging,
  minisign-verified self-updater compatible with the existing `latest.json`.
- UI in `src/ui/`: theme (8 themes from the original OKLCH tokens), shared
  components, shell (title bar, sidebar, update prompt), all pages, modals,
  pill window (`src/ui/pill/`, zero frames when idle).
- Build: `just dev` (debug, CPU, dev data dir + Dev badge), `just run`
  (release, CPU). CI builds `--features gpu` (Vulkan), CUDA pack stays a
  separate job, installers via cargo-packager (+ cargo-generate-rpm for rpm).
- `cargo clippy --all-targets -- -W clippy::pedantic -D warnings` clean and
  `cargo test` 470 tests passing (even) as of commit 568ad0c.

## Remaining
1. **Widget consolidation** (was in progress when work stopped): merge the
   duplicated button implementations (`pages/dashboard/button.rs` vs
   `pages/models/widgets.rs` vs `edge_button` in `settings/voice/card.rs`),
   move `pill`/`icon_button`/`progress_bar`/`Tone` and the General tab's
   `switch`/`section_heading` into `src/ui/components/`, share one popover panel
   style between `dashboard/toolbar.rs` and the Dropdown, unify `over_window`
   (General) with `components::modal`. Keep output pixel-identical and keep the
   test count even. If this commit contains partial edits from that pass,
   run `cargo clippy` / `cargo test` first and fix or revert those files.
2. **Pill re-raise on Windows**: add `pill::raise(cx)` and call it from the
   shell when the main window is activated (old app re-promoted the pill
   immediately; now only a 2 s topmost loop runs).
3. **Windows compile check**: `cfg(windows)` code (pill `native.rs`, tray,
   updater, `winresource` icon in `build.rs`) has not been compiled for
   Windows. Run CI or `cargo check --target x86_64-pc-windows-msvc` on Windows.
4. **Visual QA vs the old app**: each page was checked by screenshot against
   the legacy CSS by eye, not pixel-diffed. Known small deviations are listed
   in the agents' notes: no letter-spacing (GPUI lacks it), no `title=`
   tooltips in the pill, some framer-motion exit animations skipped, kit
   DatePicker is 32px instead of 28px.
5. **Verify the Voice tab catalog** renders model cards in a normal run (one
   agent saw an empty catalog while testing with a private data dir).
6. **MSI upgrade code** differs from the Tauri one (cargo-packager derives it),
   so an old Tauri `.msi` install won't upgrade in place; the NSIS installer
   and the self-updater are unaffected.
7. Delete `GPUI_PORT_STATUS.md` once the above is done.
