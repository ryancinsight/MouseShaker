#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;
use std::{sync::{Arc}};
use enigo::*;
use eframe::egui;
use tokio::time::Duration;
use std::{cell::RefCell, rc::Rc};
use tray_icon::{TrayIconBuilder, menu::{MenuEvent,Menu,AboutMetadata, MenuItem, PredefinedMenuItem},TrayIconEvent};

fn load_icon(path: &std::path::Path) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}

struct App {
    runner: Option<tokio::task::JoinHandle<()>>,
    running: Arc<tokio::sync::RwLock<bool>>,
    enigo: Arc<tokio::sync::Mutex<Enigo>>,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            runner: None,
            running: Arc::new(tokio::sync::RwLock::new(false)),
            enigo: Arc::new(tokio::sync::Mutex::new(Enigo::new())),
        }
    }
    fn start(&mut self) {
        let running = Arc::clone(&self.running);
        let enigo = Arc::clone(&self.enigo);
        self.runner = Some(tokio::spawn(async move {
            {
                let mut running = running.write().await;
                *running = true;
            }
            let mouse_future = tokio::spawn(Self::move_mouse(Arc::clone(&enigo), Arc::clone(&running)));
            let key_future = tokio::spawn(Self::press_key(Arc::clone(&enigo), Arc::clone(&running)));
            let _ = tokio::try_join!(mouse_future, key_future);
        }));
    }

    fn stop(&mut self) {
        let running = Arc::clone(&self.running);
        if let Some(runner) = self.runner.take() {
            tokio::spawn(async move {
                {
                    let mut running = running.write().await;
                    *running = false;
                }
                let _ = runner.await;
            });
        }
    }
    async fn move_mouse(enigo: Arc<tokio::sync::Mutex<Enigo>>, running: Arc<tokio::sync::RwLock<bool>>) {
        while *running.read().await {
            println!("move");
            {
                let mut enigo = enigo.lock().await;
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    enigo.mouse_move_relative(1,1);
                }));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            {
                let mut enigo = enigo.lock().await;
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    enigo.mouse_move_relative(-1,-1);
                }));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    async fn press_key(enigo: Arc<tokio::sync::Mutex<Enigo>>, running: Arc<tokio::sync::RwLock<bool>>) {
        while *running.read().await {
            println!("click");
            {
                let mut enigo = enigo.lock().await;
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    enigo.key_click(Key::F15);
                }));
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
    
    
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            println!("tray event: {event:?}");
        };
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            println!("menu event: {:?}", event);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center),|ui| {
                if ui.button(egui::RichText::new("Start").size(20.0)).clicked() {
                    self.start();
                }
                if ui.button(egui::RichText::new("Stop").size(20.0)).clicked() {
                    self.stop();
                }
            });
        });
    }
}


#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shake.png");
    println!("{}",path);
    let icon = load_icon(std::path::Path::new(path));
    let mut _tray_icon = Rc::new(RefCell::new(None));
    let tray_c = _tray_icon.clone();
    let native_options = eframe::NativeOptions {
        active: true,
        centered: true,
        icon_data: Some(eframe::IconData::try_from_png_bytes(include_bytes!("shake.png")).unwrap()),
        always_on_top: false,
        maximized: false,
        decorated: true,
        drag_and_drop_support: false,
        initial_window_pos: None,
        initial_window_size: Some(egui::vec2(256.0, 64.0)),
        min_window_size: Some(egui::vec2(256.0, 64.0)),
        max_window_size: Some(egui::vec2(256.0, 64.0)),
        resizable: false,
        transparent: true,
        vsync: true,
        ..Default::default()
    };
    let tray_menu = Menu::new();
    let quit_i = MenuItem::new("Quit", true, None);
    tray_menu.append_items(&[
        &PredefinedMenuItem::about(
            None,
            Some(AboutMetadata {
                name: Some("tao".to_string()),
                copyright: Some("Copyright SonALAsense".to_string()),
                ..Default::default()
            }),
        ),
        &PredefinedMenuItem::separator(),
        &quit_i,
    ]);
    eframe::run_native(
        "Mouse Shacker",
        native_options,
        Box::new(move |cc| {
            #[cfg(not(target_os = "linux"))]
            {
                tray_c
                    .borrow_mut()
                    .replace(TrayIconBuilder::new()
                        .with_tooltip("Mouse Shacker!")
                        .with_menu(Box::new(tray_menu))
                        .with_icon(icon).build().unwrap());
            }
            Box::new(App::new(cc))
        }),
    );
}