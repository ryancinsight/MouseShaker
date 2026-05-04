# Implementation Checklist

## Core Features [100%]

- [x] Mouse movement automation
- [x] Keyboard input automation
- [x] Native OS threads (no async runtime overhead)
- [x] Core affinity management
- [x] Start/Stop functionality with correct per-thread stop signals
- [x] Configuration options for intervals
- [x] Mouse cursor position is neutral on stop (compensating −1,−1 move always applied before thread exit)

## UI Features [100%]

- [x] Basic window setup
- [x] Start/Stop button
- [x] Minimize-to-tray behavior for close, minimize, and explicit tray button
- [x] Left-click tray icon toggles open/minimize state
- [x] Input fields for customizing intervals
- [x] System tray integration with stateful open/minimize and start/stop toggle actions
- [x] Runtime option for tray left-click menu vs toggle behavior
- [x] Persistent local settings for intervals and tray left-click mode

## Performance Optimizations [100%]

- [x] Memory allocator optimization (rpmalloc)
- [x] Native thread management (no tokio overhead)
- [x] Mutex usage optimization (std::sync::Mutex scoped to stop-channel state only)
- [x] Atomic state management
- [x] Pinned dependency versions for reproducible builds
- [x] Viewport flags batched into single `ctx.input` closure (one RwLock acquisition per frame)
- [x] Redundant Arc clone eliminated in `create_tray_icon`
- [x] `save_settings_to` writes via `BufWriter` + `write!` (no intermediate String allocation)
- [x] `#[inline]` on `sanitize_interval_secs`, `tray_run_label`, `tray_window_label`, `is_running`
- [x] Worker threads named via `thread::Builder` (profiler/debugger visibility; spawn errors surfaced explicitly)

## Error Handling [100%]

- [x] Input device error handling
- [x] Thread communication error handling (separate channels per thread)
- [x] Enigo initialisation error surfaced via expect
- [x] Configuration validation

## Distribution Correctness [100%]

- [x] Tray icon embedded at compile time via `include_bytes!` (no runtime file dependency)
- [x] Settings path uses exe-adjacent location (stable across launch contexts)
- [x] `save_settings_to` / `load_settings_from` accept explicit `&Path` (fully testable)
- [x] Settings file I/O round-trip integration test (`settings_round_trip_via_file_io`)
- [x] `parse_settings` comment/blank-line skip paths covered by test
- [x] `serialize_settings → parse_settings` identity contract covered by test (no file I/O)
- [x] `sanitize_interval_secs` clamp identity at exact boundary values covered by test
- [x] `parse_settings` whitespace-padded `key = value` lines covered by test
- [x] `parse_settings` empty input returns defaults covered by test
- [x] `parse_settings` invalid value on known key uses default (fallback path covered)
- [x] `parse_settings` malformed line (no `=`) silently skipped (split_once path covered)
- [x] `#[must_use]` on all pure/read functions including `settings_path`, `load_settings_from`, `load_settings`
- [x] `use enigo::*` wildcard replaced with explicit imports
- [x] Windows ICO resource contains 16/32/48/256 px frames (no scaling artifacts at any DPI context)
- [x] Windows EXE `ProductName` and `FileDescription` set to "Mouse Shaker" (not derived from package name)
- [x] Package renamed to `mouse-shaker`; binary output matches application identity
- [x] Viewport minimum size enforced to prevent UI clipping

## Artifacts [100%]

- [x] README.md synchronized
- [x] CHECKLIST.md (this file) synchronized
- [x] NOTES.md synchronized
- [x] CHANGELOG.md created with full version history

Overall Progress: 100%
