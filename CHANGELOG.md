# Changelog

All notable changes follow [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) conventions.
Versioning follows [SemVer 2.0.0](https://semver.org/).

---

## [Unreleased]

### Fixed
- **Mouse cursor drift on stop** (`AutomationController::move_mouse`): the compensating `-1,-1` move was unreachable when a stop signal arrived, leaving the cursor permanently displaced by +1 px per start/stop cycle. The loop is restructured so the `-1,-1` move always executes before breaking [patch].

### Added
- `generate_multi_size_ico` in `build.rs`: 4-size ICO (16/32/48/256 px PNG frames) via direct binary format; no extra crate dependency [patch].
- `with_min_inner_size([320.0, 220.0])` on `ViewportBuilder`: prevents UI clipping [patch].
- Worker threads named `mouse-shaker-mouse` and `mouse-shaker-key` via `thread::Builder`; thread spawn failure is now surfaced via `expect` instead of a panic inside `std::thread::spawn` [patch].
- Tests (12–14): `parse_settings_invalid_value_uses_default`, `parse_settings_malformed_line_is_skipped`, `parse_settings_whitespace_padded_keys_and_values`, `parse_settings_empty_input_returns_defaults`, `serialize_then_parse_is_identity`, `sanitize_interval_secs_preserves_exact_bounds` [patch].

### Changed
- Package renamed `windowon` → `mouse-shaker`; binary is now `mouse-shaker.exe` [patch].
- `winresource`: `ProductName` and `FileDescription` set to "Mouse Shaker" [patch].
- `Cargo.toml`: added `description` field [patch].
- `use enigo::*` replaced with explicit imports [patch].
- `save_settings_to`: writes via `BufWriter` + `write!`; no intermediate `String` allocation; `serialize_settings` moved to `#[cfg(test)]` [patch].
- `update()`: viewport flags batched into single `ctx.input` closure [patch].
- Redundant Arc clone removed in `create_tray_icon` [patch].
- `#[must_use]` added to `sanitize_interval_secs`, `tray_run_label`, `tray_window_label`, `current_settings`, `parse_settings`, `serialize_settings`, `settings_path`, `load_settings_from`, `load_settings` [patch].
- `#[inline]` added to `sanitize_interval_secs`, `tray_run_label`, `tray_window_label`, `is_running` [patch].

---

## [0.5.0] — 2026-04-30

### Added
- `PersistedSettings` struct with `parse_settings`, `serialize_settings`, `load_settings`, `save_settings`,
  `current_settings`, and `persist_settings_if_changed` — file-based persistence using stdlib only.
  Save path defaults to exe-adjacent (this release used CWD; corrected in Unreleased) [minor].
- Two new unit tests: `parse_settings_applies_bounds_and_known_keys`,
  `serialize_settings_includes_all_fields` [patch].
- `App::new` reads `PersistedSettings` on startup and applies saved values to interval atomics and
  `tray_menu_on_left_click` [minor].
- UI-side `persist_settings_if_changed` call in `update`: saves on change without redundant writes [patch].

---

## [0.4.0] — 2026-04-29

### Added
- Tray left-click mode toggle: checkbox in UI and `tray_menu_on_left_click` atomic [minor].
- `TraySharedState` struct to satisfy `clippy::too_many_arguments` on `create_tray_icon` [patch].

---

## [0.3.0] — 2026-04-28

### Added
- `worker_handles: Mutex<Option<(JoinHandle, JoinHandle)>>` stored in `AutomationController`;
  `stop()` now joins both handles deterministically, eliminating thread leaks on repeated start/stop [minor].
- Per-thread `Enigo` instances (removed shared `Arc<Mutex<Enigo>>`); each worker constructs its own
  handle, eliminating lock contention on the input simulation path [minor].
- `crossbeam_channel::bounded(1)` stop channels replace the previous unbounded design [patch].
- Interval clamp bounds: `sanitize_interval_secs` enforces `[1, MAX_INTERVAL_SECS]` where
  `MAX_INTERVAL_SECS = 3600` [patch].
- Two new unit tests: `interval_is_sanitized_to_one_second_minimum`,
  `interval_is_capped_to_configured_maximum` [patch].

### Removed
- `spin_sleep` dependency: worker timing replaced by `recv_timeout` (blocking, interruptible) [patch].
- `parking_lot` dependency: replaced by `std::sync::Mutex` scoped to stop-channel state only [patch].

---

## [0.2.0] — 2026-04-27

### Added
- System tray icon via `tray-icon 0.23` with context menu [minor].
- Stateful tray menu items: "Open"/"Minimize" window toggle and "Start"/"Stop" automation toggle [minor].
- `sync_tray_labels` / `sync_tray_toggle_labels`: labels updated on UI thread before every hide path
  and at top of each `update` frame to prevent stale labels [patch].
- Left-click tray icon toggles window open/minimize state (respects `tray_menu_on_left_click`) [minor].
- `hide_to_tray` / `restore_from_tray` using Win32 `ShowWindow` / `SetForegroundWindow` for reliable
  visibility management [minor].
- Close and minimize window events intercepted and redirected to tray hide [minor].
- Two new unit tests: `tray_window_label_reflects_inverse_action`,
  `tray_run_label_reflects_inverse_action` [patch].

---

## [0.1.0] — 2026-04-26

### Added
- Mouse automation worker: moves cursor +1/+1 then −1/−1 on each tick [minor].
- Keyboard automation worker: presses F15 on each tick [minor].
- `AutomationController` with `start(mouse_interval, key_interval)` and `stop()` [minor].
- Per-worker `crossbeam_channel::bounded(1)` stop signal channels [minor].
- `core_affinity` pins workers to distinct CPU cores [minor].
- `rpmalloc` global allocator [minor].
- `eframe 0.29` / `egui 0.29` native window with Start/Stop button and interval `DragValue` inputs [minor].
- `raw-window-handle 0.6` + `windows-sys 0.61` for HWND extraction [minor].
- `tracing` / `tracing-subscriber` structured logging (debug builds only) [patch].
- Release profile: `lto=fat`, `strip=true`, `codegen-units=1`, `panic=abort`, `opt-level=3` [patch].

---

### Settings file location

The settings file (`mouse_shaker_settings.conf`) is written adjacent to the application executable.
Format: line-delimited `key=value` text; unknown keys and malformed lines are silently ignored;
out-of-range numeric values are clamped to `[1, 3600]`.
