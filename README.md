# Mouse Shaker

A Rust application that prevents system idle by simulating mouse movements and keyboard input.

## Architecture

- Two native OS threads are spawned on Start: one for mouse movement, one for F15 key press.
- Each thread owns its own `crossbeam_channel::Receiver<()>` stop signal.
- Each worker thread owns its own `Enigo` instance, removing cross-thread input-device lock contention.
- Stop signals both workers and joins both thread handles before returning, so thread resources are reclaimed deterministically.
- Worker loops block on `recv_timeout` between actions, which lowers idle CPU use and lets Stop interrupt sleeps quickly.
- Thread state and lifecycle are coordinated through a shared `AutomationController`.
- `std::sync::Mutex` guards only stop-channel sender state.
- `rpmalloc` is used as the global allocator.
- Core affinity pins each thread to a distinct logical CPU core when available.
- A tray icon is created from `src/assets/icon.png` and remains active while the app is hidden.
- The tray menu exposes two stateful toggle actions: `Start`/`Stop` for automation and `Open`/`Minimize` for window visibility, plus `Exit`.

## File Structure

```text
Cargo.toml
CHECKLIST.md
NOTES.md
README.md
src/
    main.rs
    assets/
        icon.png
```

## Building

```sh
cargo build --release
```

## Usage

Run the binary and configure **Mouse interval (sec)** and **Key interval (sec)** as needed (range: 1 to 3600 seconds).

Click **Start**. The application moves the mouse ±1 pixel at the configured mouse interval and presses F15 at the configured key interval. Click **Stop** to halt both threads.

Closing the window, using the native minimize button, or clicking **Minimize to Tray** hides the app to the Windows system tray instead of exiting it. Left-clicking the tray icon now toggles the window state: it opens the app when hidden and minimizes it when visible.

Starting from the tray menu uses the same configured intervals.

You can switch tray left-click behavior at runtime with the checkbox in the main window:

- disabled (default): left-click toggles open/minimize
- enabled: left-click opens the tray menu

Settings persist across restarts in a local configuration file named `mouse_shaker_settings.conf`.

The tray menu also stays synchronized with the app state. The visibility action shows `Open` when the window is hidden and `Minimize` when it is visible. The automation action shows `Start` when idle and `Stop` while automation is running. `Exit` closes the application.
