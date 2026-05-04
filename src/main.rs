#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rpmalloc::RpMalloc;
#[global_allocator]
static GLOBAL: RpMalloc = RpMalloc;

use crossbeam_channel::{bounded, Sender};
use eframe::egui;
use enigo::{Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, ShowWindow, SW_HIDE, SW_RESTORE, SW_SHOW,
};
#[cfg(debug_assertions)]
use tracing::{info, debug, error};

const DEFAULT_MOUSE_INTERVAL_SECS: u64 = 5;
const DEFAULT_KEY_INTERVAL_SECS: u64 = 10;
const MAX_INTERVAL_SECS: u64 = 3600;
const SETTINGS_FILE_NAME: &str = "mouse_shaker_settings.conf";
type WindowHandle = isize;

fn setup_logging() {
    #[cfg(debug_assertions)]
    {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_target(false)
            .with_thread_ids(true)
            .with_line_number(true)
            .with_file(true)
            .init();
    }
}

struct AutomationController {
    core_ids: Vec<core_affinity::CoreId>,
    stop_senders: Mutex<Option<(Sender<()>, Sender<()>)>>,
    worker_handles: Mutex<Option<(thread::JoinHandle<()>, thread::JoinHandle<()>)>>,
    running: AtomicBool,
}

impl AutomationController {
    fn new(core_ids: Vec<core_affinity::CoreId>) -> Self {
        Self {
            core_ids,
            stop_senders: Mutex::new(None),
            worker_handles: Mutex::new(None),
            running: AtomicBool::new(false),
        }
    }

    #[inline]
    fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    fn start(self: &Arc<Self>, mouse_interval: Duration, key_interval: Duration) {
        if self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        #[cfg(debug_assertions)]
        info!("Starting automation threads");

        let (stop_tx1, stop_rx1) = bounded::<()>(1);
        let (stop_tx2, stop_rx2) = bounded::<()>(1);
        *self.stop_senders.lock().expect("stop_senders mutex poisoned") = Some((stop_tx1, stop_tx2));

        let core_id0 = self.core_ids.first().copied();
        let core_id1 = self.core_ids.get(1).copied();
        let controller1 = Arc::clone(self);
        let controller2 = Arc::clone(self);

        let mouse_handle = thread::Builder::new()
            .name("mouse-shaker-mouse".to_owned())
            .spawn(move || {
                if let Some(id) = core_id0 {
                    core_affinity::set_for_current(id);
                }
                let enigo = Enigo::new(&Settings::default()).expect("failed to initialize Enigo for mouse thread");
                controller1.move_mouse(enigo, stop_rx1, mouse_interval);
            })
            .expect("failed to spawn mouse worker thread");

        let key_handle = thread::Builder::new()
            .name("mouse-shaker-key".to_owned())
            .spawn(move || {
                if let Some(id) = core_id1 {
                    core_affinity::set_for_current(id);
                }
                let enigo = Enigo::new(&Settings::default()).expect("failed to initialize Enigo for key thread");
                controller2.press_key(enigo, stop_rx2, key_interval);
            })
            .expect("failed to spawn key worker thread");

        *self
            .worker_handles
            .lock()
            .expect("worker_handles mutex poisoned") = Some((mouse_handle, key_handle));
    }

    fn stop(&self) {
        if self
            .running
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        #[cfg(debug_assertions)]
        info!("Stopping automation");

        if let Some((tx1, tx2)) = self
            .stop_senders
            .lock()
            .expect("stop_senders mutex poisoned")
            .take()
        {
            let _ = tx1.send(());
            let _ = tx2.send(());
        }

        if let Some((mouse_handle, key_handle)) = self
            .worker_handles
            .lock()
            .expect("worker_handles mutex poisoned")
            .take()
        {
            if let Err(join_err) = mouse_handle.join() {
                #[cfg(debug_assertions)]
                error!("Mouse thread join failed: {:?}", join_err);
                #[cfg(not(debug_assertions))]
                let _ = join_err;
            }
            if let Err(join_err) = key_handle.join() {
                #[cfg(debug_assertions)]
                error!("Key thread join failed: {:?}", join_err);
                #[cfg(not(debug_assertions))]
                let _ = join_err;
            }
        }
    }

    fn move_mouse(
        &self,
        mut enigo: Enigo,
        stop_rx: crossbeam_channel::Receiver<()>,
        interval: Duration,
    ) {
        #[cfg(debug_assertions)]
        info!("Starting mouse movement thread");
        while self.running.load(Ordering::Acquire) {
            #[cfg(debug_assertions)]
            debug!("Moving mouse +1,+1");
            let _ = enigo.move_mouse(1, 1, Coordinate::Rel);
            // Always apply the compensating -1,-1 move regardless of whether the thread
            // is stopping or continuing.  Without this, every start/stop cycle leaves the
            // cursor permanently displaced by +1 px, accumulating unbounded drift.
            let stop_signaled = matches!(
                stop_rx.recv_timeout(interval),
                Ok(()) | Err(crossbeam_channel::RecvTimeoutError::Disconnected)
            );
            #[cfg(debug_assertions)]
            debug!("Moving mouse -1,-1");
            let _ = enigo.move_mouse(-1, -1, Coordinate::Rel);
            if stop_signaled {
                #[cfg(debug_assertions)]
                info!("Stopping mouse movement thread");
                break;
            }
        }
    }

    fn press_key(
        &self,
        mut enigo: Enigo,
        stop_rx: crossbeam_channel::Receiver<()>,
        interval: Duration,
    ) {
        #[cfg(debug_assertions)]
        info!("Starting key press thread");
        while self.running.load(Ordering::Acquire) {
            #[cfg(debug_assertions)]
            debug!("Attempting to press F15 key");
            match enigo.key(Key::F15, Direction::Click) {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    debug!("Successfully pressed F15 key");
                }
                Err(_err) => {
                    #[cfg(debug_assertions)]
                    error!("Failed to press F15 key: {:?}", _err);
                }
            }
            #[cfg(debug_assertions)]
            debug!("Sleeping for {} seconds", interval.as_secs());
            match stop_rx.recv_timeout(interval) {
                Ok(_) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    #[cfg(debug_assertions)]
                    info!("Stopping key press thread");
                    break;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

struct AppState {
    controller: Arc<AutomationController>,
    allow_exit: Arc<AtomicBool>,
    is_window_hidden: Arc<AtomicBool>,
    tray_menu_on_left_click: Arc<AtomicBool>,
    window_hwnd: WindowHandle,
    tray_window_toggle_item: MenuItem,
    tray_run_toggle_item: MenuItem,
    tray_icon: TrayIcon,
    was_minimized: bool,
    mouse_interval_secs: Arc<AtomicU64>,
    key_interval_secs: Arc<AtomicU64>,
    persisted_settings: PersistedSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PersistedSettings {
    mouse_interval_secs: u64,
    key_interval_secs: u64,
    tray_menu_on_left_click: bool,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            mouse_interval_secs: DEFAULT_MOUSE_INTERVAL_SECS,
            key_interval_secs: DEFAULT_KEY_INTERVAL_SECS,
            tray_menu_on_left_click: false,
        }
    }
}

struct TraySharedState {
    allow_exit: Arc<AtomicBool>,
    is_window_hidden: Arc<AtomicBool>,
    mouse_interval_secs: Arc<AtomicU64>,
    key_interval_secs: Arc<AtomicU64>,
    tray_menu_on_left_click: Arc<AtomicBool>,
}

struct App {
    state: AppState,
}

impl App {
    fn load_app_icon_data() -> egui::IconData {
        const ICON_BYTES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/icon.png"));
        eframe::icon_data::from_png_bytes(ICON_BYTES).expect("failed to decode embedded application icon")
    }

    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(debug_assertions)]
        info!("Initializing App");
        let settings = Self::load_settings();
        let core_ids = core_affinity::get_core_ids().unwrap_or_default();
        let allow_exit = Arc::new(AtomicBool::new(false));
        let is_window_hidden = Arc::new(AtomicBool::new(false));
        let tray_menu_on_left_click = Arc::new(AtomicBool::new(settings.tray_menu_on_left_click));
        let mouse_interval_secs = Arc::new(AtomicU64::new(settings.mouse_interval_secs));
        let key_interval_secs = Arc::new(AtomicU64::new(settings.key_interval_secs));
        let controller = Arc::new(AutomationController::new(core_ids));
        let tray_state = TraySharedState {
            allow_exit: allow_exit.clone(),
            is_window_hidden: is_window_hidden.clone(),
            mouse_interval_secs: mouse_interval_secs.clone(),
            key_interval_secs: key_interval_secs.clone(),
            tray_menu_on_left_click: tray_menu_on_left_click.clone(),
        };
        let window_hwnd = Self::extract_window_hwnd(cc);
        let (tray_icon, tray_window_toggle_item, tray_run_toggle_item) = Self::create_tray_icon(
            &cc.egui_ctx,
            window_hwnd,
            controller.clone(),
            tray_state,
        );
        tray_icon.set_show_menu_on_left_click(settings.tray_menu_on_left_click);
        Self {
            state: AppState {
                controller,
                allow_exit,
                is_window_hidden,
                tray_menu_on_left_click,
                window_hwnd,
                tray_window_toggle_item,
                tray_run_toggle_item,
                tray_icon,
                was_minimized: false,
                mouse_interval_secs,
                key_interval_secs,
                persisted_settings: settings,
            },
        }
    }

    #[inline]
    #[must_use]
    fn sanitize_interval_secs(value: u64) -> u64 {
        value.clamp(1, MAX_INTERVAL_SECS)
    }

    #[must_use]
    fn settings_path() -> PathBuf {
        // Store settings adjacent to the executable so the path is stable
        // regardless of the working directory at launch time.
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join(SETTINGS_FILE_NAME)))
            .unwrap_or_else(|| PathBuf::from(SETTINGS_FILE_NAME))
    }

    #[must_use]
    fn parse_settings(contents: &str) -> PersistedSettings {
        let mut settings = PersistedSettings::default();

        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((raw_key, raw_value)) = line.split_once('=') else {
                continue;
            };
            let key = raw_key.trim();
            let value = raw_value.trim();

            match key {
                "mouse_interval_secs" => {
                    if let Ok(parsed) = value.parse::<u64>() {
                        settings.mouse_interval_secs = Self::sanitize_interval_secs(parsed);
                    }
                }
                "key_interval_secs" => {
                    if let Ok(parsed) = value.parse::<u64>() {
                        settings.key_interval_secs = Self::sanitize_interval_secs(parsed);
                    }
                }
                "tray_menu_on_left_click" => {
                    if let Ok(parsed) = value.parse::<bool>() {
                        settings.tray_menu_on_left_click = parsed;
                    }
                }
                _ => {}
            }
        }

        settings
    }

    // Production path: write directly to a BufWriter-backed file, avoiding the intermediate
    // String allocation that a format! + fs::write pattern would require.
    fn save_settings_to(settings: PersistedSettings, path: &std::path::Path) -> Result<(), std::io::Error> {
        use std::io::Write as _;
        let file = std::fs::File::create(path)?;
        let mut w = std::io::BufWriter::new(file);
        write!(
            w,
            "mouse_interval_secs={}\nkey_interval_secs={}\ntray_menu_on_left_click={}\n",
            settings.mouse_interval_secs,
            settings.key_interval_secs,
            settings.tray_menu_on_left_click
        )?;
        w.flush()
    }

    #[must_use]
    fn load_settings_from(path: &std::path::Path) -> PersistedSettings {
        match fs::read_to_string(path) {
            Ok(contents) => Self::parse_settings(&contents),
            Err(_) => PersistedSettings::default(),
        }
    }

    #[must_use]
    fn load_settings() -> PersistedSettings {
        Self::load_settings_from(&Self::settings_path())
    }

    // Retained for serialization format contract tests only.
    #[cfg(test)]
    #[must_use]
    fn serialize_settings(settings: PersistedSettings) -> String {
        format!(
            "mouse_interval_secs={}\nkey_interval_secs={}\ntray_menu_on_left_click={}\n",
            settings.mouse_interval_secs,
            settings.key_interval_secs,
            settings.tray_menu_on_left_click
        )
    }

    fn save_settings(settings: PersistedSettings) -> Result<(), std::io::Error> {
        Self::save_settings_to(settings, &Self::settings_path())
    }

    #[must_use]
    fn current_settings(&self) -> PersistedSettings {
        PersistedSettings {
            mouse_interval_secs: Self::sanitize_interval_secs(
                self.state.mouse_interval_secs.load(Ordering::Acquire),
            ),
            key_interval_secs: Self::sanitize_interval_secs(
                self.state.key_interval_secs.load(Ordering::Acquire),
            ),
            tray_menu_on_left_click: self
                .state
                .tray_menu_on_left_click
                .load(Ordering::Acquire),
        }
    }

    fn persist_settings_if_changed(&mut self) {
        let current = self.current_settings();
        if current == self.state.persisted_settings {
            return;
        }

        match Self::save_settings(current) {
            Ok(()) => {
                self.state.persisted_settings = current;
            }
            Err(err) => {
                #[cfg(debug_assertions)]
                error!("Failed to persist settings: {:?}", err);
                #[cfg(not(debug_assertions))]
                let _ = err;
            }
        }
    }

    #[inline]
    #[must_use]
    fn tray_run_label(is_running: bool) -> &'static str {
        if is_running { "Stop" } else { "Start" }
    }

    #[inline]
    #[must_use]
    fn tray_window_label(is_window_hidden: bool) -> &'static str {
        if is_window_hidden { "Open" } else { "Minimize" }
    }

    fn extract_window_hwnd(cc: &eframe::CreationContext<'_>) -> WindowHandle {
        match cc
            .window_handle()
            .expect("failed to get root window handle")
            .as_raw()
        {
            RawWindowHandle::Win32(handle) => handle.hwnd.get(),
            other => panic!("unsupported window handle for tray restore: {other:?}"),
        }
    }

    fn create_tray_icon(
        ctx: &egui::Context,
        window_hwnd: WindowHandle,
        controller: Arc<AutomationController>,
        tray_state: TraySharedState,
    ) -> (TrayIcon, MenuItem, MenuItem) {
        let menu = Menu::new();
        let window_toggle_item = MenuItem::new("Minimize", true, None);
        let run_toggle_item = MenuItem::new("Start", true, None);
        let quit_item = MenuItem::new("Exit", true, None);

        menu.append(&window_toggle_item)
            .expect("failed to add tray window toggle item");
        menu.append(&run_toggle_item)
            .expect("failed to add tray run toggle item");
        menu.append(&PredefinedMenuItem::separator())
            .expect("failed to add tray separator");
        menu.append(&quit_item).expect("failed to add tray exit item");

        let window_toggle_id = window_toggle_item.id().clone();
        let run_toggle_id = run_toggle_item.id().clone();
        let quit_id = quit_item.id().clone();

        let menu_ctx = ctx.clone();
        let menu_allow_exit = tray_state.allow_exit.clone();
        let menu_hidden = tray_state.is_window_hidden.clone();
        // `controller` is already an Arc clone (caller passes controller.clone()); move it
        // directly into the closure rather than cloning again to avoid a redundant refcount
        // increment/decrement.
        let menu_mouse_interval = tray_state.mouse_interval_secs.clone();
        let menu_key_interval = tray_state.key_interval_secs.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == window_toggle_id {
                if menu_hidden.load(Ordering::Acquire) {
                    App::restore_from_tray(window_hwnd, &menu_ctx, &menu_hidden);
                } else {
                    App::hide_to_tray(window_hwnd, &menu_ctx, &menu_hidden);
                }
                menu_ctx.request_repaint();
            } else if event.id == run_toggle_id {
                if controller.is_running() {
                    controller.stop();
                } else {
                    controller.start(
                        Duration::from_secs(App::sanitize_interval_secs(
                            menu_mouse_interval.load(Ordering::Acquire),
                        )),
                        Duration::from_secs(App::sanitize_interval_secs(
                            menu_key_interval.load(Ordering::Acquire),
                        )),
                    );
                }
                menu_ctx.request_repaint();
            } else if event.id == quit_id {
                controller.stop();
                menu_allow_exit.store(true, Ordering::Release);
                App::restore_from_tray(window_hwnd, &menu_ctx, &menu_hidden);
                menu_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                menu_ctx.request_repaint();
            }
        }));

        let tray_ctx = ctx.clone();
        let tray_hidden = tray_state.is_window_hidden.clone();
        let tray_menu_on_left_click_state = tray_state.tray_menu_on_left_click.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                if tray_menu_on_left_click_state.load(Ordering::Acquire) {
                    return;
                }
                if tray_hidden.load(Ordering::Acquire) {
                    App::restore_from_tray(window_hwnd, &tray_ctx, &tray_hidden);
                } else {
                    App::hide_to_tray(window_hwnd, &tray_ctx, &tray_hidden);
                }
                tray_ctx.request_repaint();
            }
        }));

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Mouse Shaker")
            .with_icon(Self::load_tray_icon())
            .build()
            .expect("failed to create tray icon");

        (tray_icon, window_toggle_item, run_toggle_item)
    }

    fn load_tray_icon() -> Icon {
        // Icon bytes are embedded at compile time; no runtime file access required.
        const ICON_BYTES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/icon.png"));
        let image = image::load_from_memory(ICON_BYTES)
            .expect("failed to decode embedded tray icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        Icon::from_rgba(image.into_raw(), width, height).expect("failed to build Icon from embedded tray icon")
    }

    fn restore_from_tray(
        window_hwnd: WindowHandle,
        ctx: &egui::Context,
        is_window_hidden: &AtomicBool,
    ) {
        let hwnd = window_hwnd as HWND;
        is_window_hidden.store(false, Ordering::Release);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn hide_to_tray(
        window_hwnd: WindowHandle,
        ctx: &egui::Context,
        is_window_hidden: &AtomicBool,
    ) {
        let hwnd = window_hwnd as HWND;
        is_window_hidden.store(true, Ordering::Release);
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    fn sync_tray_toggle_labels(
        tray_window_toggle_item: &MenuItem,
        tray_run_toggle_item: &MenuItem,
        is_window_hidden: bool,
        is_running: bool,
    ) {
        tray_run_toggle_item.set_text(Self::tray_run_label(is_running));
        tray_window_toggle_item.set_text(Self::tray_window_label(is_window_hidden));
    }

    fn sync_tray_labels(&self) {
        Self::sync_tray_toggle_labels(
            &self.state.tray_window_toggle_item,
            &self.state.tray_run_toggle_item,
            self.state.is_window_hidden.load(Ordering::Acquire),
            self.state.controller.is_running(),
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_tray_labels();

        // Read both viewport flags in a single ctx.input call to avoid two RwLock acquisitions.
        let (close_requested, is_minimized) = ctx.input(|input| {
            let vp = input.viewport();
            (vp.close_requested(), vp.minimized.unwrap_or(false))
        });
        if close_requested && !self.state.allow_exit.load(Ordering::Acquire) {
            #[cfg(debug_assertions)]
            info!("Close requested; hiding window to tray");
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            Self::hide_to_tray(self.state.window_hwnd, ctx, &self.state.is_window_hidden);
            self.sync_tray_labels();
            return;
        }

        if is_minimized && !self.state.was_minimized {
            #[cfg(debug_assertions)]
            info!("Minimized; hiding window to tray");
            Self::hide_to_tray(self.state.window_hwnd, ctx, &self.state.is_window_hidden);
            self.sync_tray_labels();
            self.state.was_minimized = true;
            return;
        }
        self.state.was_minimized = is_minimized;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                let is_running = self.state.controller.is_running();
                let button_text = if is_running { "Stop" } else { "Start" };

                ui.label("Closing or minimizing the window keeps Mouse Shaker running in the system tray.");

                if ui.button(egui::RichText::new(button_text).size(20.0)).clicked() {
                    if is_running {
                        self.state.controller.stop();
                    } else {
                        self.state.controller.start(
                            Duration::from_secs(Self::sanitize_interval_secs(
                                self.state.mouse_interval_secs.load(Ordering::Acquire),
                            )),
                            Duration::from_secs(Self::sanitize_interval_secs(
                                self.state.key_interval_secs.load(Ordering::Acquire),
                            )),
                        );
                    }
                }

                let mut mouse_interval_secs = self.state.mouse_interval_secs.load(Ordering::Acquire);
                ui.horizontal(|ui| {
                    ui.label("Mouse interval (sec)");
                    ui.add(
                        egui::DragValue::new(&mut mouse_interval_secs)
                            .range(1..=MAX_INTERVAL_SECS)
                            .speed(1),
                    );
                });
                self.state.mouse_interval_secs.store(
                    Self::sanitize_interval_secs(mouse_interval_secs),
                    Ordering::Release,
                );

                let mut key_interval_secs = self.state.key_interval_secs.load(Ordering::Acquire);
                ui.horizontal(|ui| {
                    ui.label("Key interval (sec)");
                    ui.add(
                        egui::DragValue::new(&mut key_interval_secs)
                            .range(1..=MAX_INTERVAL_SECS)
                            .speed(1),
                    );
                });
                self.state.key_interval_secs.store(
                    Self::sanitize_interval_secs(key_interval_secs),
                    Ordering::Release,
                );

                if ui.button(egui::RichText::new("Minimize to Tray").size(16.0)).clicked() {
                    Self::hide_to_tray(self.state.window_hwnd, ctx, &self.state.is_window_hidden);
                    self.sync_tray_labels();
                }

                let mut left_click_menu = self
                    .state
                    .tray_menu_on_left_click
                    .load(Ordering::Acquire);
                if ui
                    .checkbox(
                        &mut left_click_menu,
                        "Tray left-click opens menu (instead of open/minimize toggle)",
                    )
                    .changed()
                {
                    self.state
                        .tray_menu_on_left_click
                        .store(left_click_menu, Ordering::Release);
                    self.state.tray_icon.set_show_menu_on_left_click(left_click_menu);
                }

                self.persist_settings_if_changed();
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.state.controller.stop();
    }
}

fn main() {
    setup_logging();
    #[cfg(debug_assertions)]
    info!("Application starting");

    let options = eframe::NativeOptions {
        centered: true,
        multisampling: 0,
        vsync: true,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        viewport: egui::ViewportBuilder::default()
            .with_icon(App::load_app_icon_data())
            // Prevent the window from being resized so small that UI elements are clipped.
            .with_min_inner_size([320.0, 220.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Mouse Shaker",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .expect("eframe failed");
}

#[cfg(test)]
mod tests {
    use super::{App, PersistedSettings};

    #[test]
    fn tray_window_label_reflects_inverse_action() {
        assert_eq!(App::tray_window_label(true), "Open");
        assert_eq!(App::tray_window_label(false), "Minimize");
    }

    #[test]
    fn tray_run_label_reflects_inverse_action() {
        assert_eq!(App::tray_run_label(true), "Stop");
        assert_eq!(App::tray_run_label(false), "Start");
    }

    #[test]
    fn interval_is_sanitized_to_one_second_minimum() {
        assert_eq!(App::sanitize_interval_secs(0), 1);
        assert_eq!(App::sanitize_interval_secs(5), 5);
    }

    #[test]
    fn interval_is_capped_to_configured_maximum() {
        assert_eq!(App::sanitize_interval_secs(u64::MAX), super::MAX_INTERVAL_SECS);
    }

    #[test]
    fn parse_settings_applies_bounds_and_known_keys() {
        let parsed = App::parse_settings(
            "mouse_interval_secs=0\nkey_interval_secs=999999\ntray_menu_on_left_click=true\nignored=x\n",
        );
        assert_eq!(parsed.mouse_interval_secs, 1);
        assert_eq!(parsed.key_interval_secs, super::MAX_INTERVAL_SECS);
        assert!(parsed.tray_menu_on_left_click);
    }

    #[test]
    fn serialize_settings_includes_all_fields() {
        let text = App::serialize_settings(PersistedSettings {
            mouse_interval_secs: 7,
            key_interval_secs: 13,
            tray_menu_on_left_click: false,
        });
        assert!(text.contains("mouse_interval_secs=7"));
        assert!(text.contains("key_interval_secs=13"));
        assert!(text.contains("tray_menu_on_left_click=false"));
    }

    /// Invariant: save_settings_to ∘ load_settings_from = identity for any valid PersistedSettings.
    /// Verified by round-tripping through a real temp file using only stdlib I/O.
    #[test]
    fn settings_round_trip_via_file_io() {
        use std::fs;

        let original = super::PersistedSettings {
            mouse_interval_secs: 42,
            key_interval_secs: 99,
            tray_menu_on_left_click: true,
        };

        let dir = std::env::temp_dir();
        let path = dir.join("mouse_shaker_test_round_trip.conf");

        App::save_settings_to(original, &path).expect("save_settings_to failed");
        let loaded = App::load_settings_from(&path);
        // Clean up before asserting so failure doesn't leave stale files.
        let _ = fs::remove_file(&path);

        assert_eq!(loaded.mouse_interval_secs, original.mouse_interval_secs);
        assert_eq!(loaded.key_interval_secs, original.key_interval_secs);
        assert_eq!(loaded.tray_menu_on_left_click, original.tray_menu_on_left_click);
    }

    /// `parse_settings` must silently skip blank lines and `#`-prefixed comment lines,
    /// leaving unset fields at their defaults.
    #[test]
    fn parse_settings_ignores_comments_and_blank_lines() {
        let parsed = App::parse_settings(
            "\n# comment at top\nmouse_interval_secs=15\n\n# another comment\n",
        );
        assert_eq!(parsed.mouse_interval_secs, 15);
        assert_eq!(parsed.key_interval_secs, super::DEFAULT_KEY_INTERVAL_SECS);
        assert!(!parsed.tray_menu_on_left_click);
    }

    /// Invariant: serialize_settings ∘ parse_settings = identity (pure serialization contract,
    /// no file I/O).  Stronger than the individual serialize/parse unit tests because it
    /// validates that the two functions are inverse over the full PersistedSettings domain.
    #[test]
    fn serialize_then_parse_is_identity() {
        let original = PersistedSettings {
            mouse_interval_secs: 30,
            key_interval_secs: 60,
            tray_menu_on_left_click: true,
        };
        let serialized = App::serialize_settings(original);
        let parsed = App::parse_settings(&serialized);
        assert_eq!(parsed, original);
    }

    /// `sanitize_interval_secs` must be identity at both exact clamp boundaries:
    /// 1 (minimum) and MAX_INTERVAL_SECS (maximum).  Values already in range must pass through.
    #[test]
    fn sanitize_interval_secs_preserves_exact_bounds() {
        assert_eq!(App::sanitize_interval_secs(1), 1);
        assert_eq!(
            App::sanitize_interval_secs(super::MAX_INTERVAL_SECS),
            super::MAX_INTERVAL_SECS
        );
    }

    /// `parse_settings` trims whitespace around both the key and the value.
    /// A settings file with `  key = value  ` lines must parse identically to `key=value`.
    #[test]
    fn parse_settings_whitespace_padded_keys_and_values() {
        let parsed = App::parse_settings(
            " mouse_interval_secs = 20 \n key_interval_secs = 40 \n tray_menu_on_left_click = true \n",
        );
        assert_eq!(parsed.mouse_interval_secs, 20);
        assert_eq!(parsed.key_interval_secs, 40);
        assert!(parsed.tray_menu_on_left_click);
    }

    /// `parse_settings` must return `PersistedSettings::default()` for empty input;
    /// no panic, no undefined state.
    #[test]
    fn parse_settings_empty_input_returns_defaults() {
        let parsed = App::parse_settings("");
        assert_eq!(parsed.mouse_interval_secs, super::DEFAULT_MOUSE_INTERVAL_SECS);
        assert_eq!(parsed.key_interval_secs, super::DEFAULT_KEY_INTERVAL_SECS);
        assert!(!parsed.tray_menu_on_left_click);
    }

    /// `parse_settings` must silently skip known keys whose values cannot be parsed,
    /// leaving those fields at their defaults.  Covers the `value.parse::<T>().is_err()` path.
    #[test]
    fn parse_settings_invalid_value_uses_default() {
        let parsed = App::parse_settings(
            // "abc" is not a valid u64; "true" is not a valid u64; "7" is not a valid bool.
            "mouse_interval_secs=abc
key_interval_secs=true
tray_menu_on_left_click=7
",
        );
        assert_eq!(parsed.mouse_interval_secs, super::DEFAULT_MOUSE_INTERVAL_SECS);
        assert_eq!(parsed.key_interval_secs, super::DEFAULT_KEY_INTERVAL_SECS);
        assert!(!parsed.tray_menu_on_left_click);
    }

    /// `parse_settings` must silently skip lines that contain no `=` separator.
    /// Covers the `split_once('=').is_none()` path.
    #[test]
    fn parse_settings_malformed_line_is_skipped() {
        let parsed = App::parse_settings("notakeyvalue
mouse_interval_secs=25
");
        assert_eq!(parsed.mouse_interval_secs, 25);
        assert_eq!(parsed.key_interval_secs, super::DEFAULT_KEY_INTERVAL_SECS);
        assert!(!parsed.tray_menu_on_left_click);
    }
}

