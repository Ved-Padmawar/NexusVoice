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
1. **Visual QA vs the old app**: pages were compared by eye, not pixel-diffed.
   Known deviations: no letter-spacing (GPUI lacks it), no `title=` tooltips in
   the pill, some exit animations skipped, kit DatePicker is 32px not 28px.
2. **MSI upgrade code** differs from the Tauri one (cargo-packager has no
   option for it; it would need a custom WiX template), so an old Tauri `.msi`
   install won't upgrade in place. NSIS and the self-updater are unaffected.
3. Linux/macOS `cfg` code was not compiled in the last pass (done on Windows).
4. Delete `GPUI_PORT_STATUS.md` once the above is done.
