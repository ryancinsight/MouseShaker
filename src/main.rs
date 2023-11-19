#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;
use enigo::*;
use eframe::{Frame,egui::{self,RichText}};
use tokio::time::Duration;
use once_cell::sync::{Lazy,OnceCell}; // 1.5.2
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
static RUNNER: tokio::sync::OnceCell<tokio::task::JoinHandle<()>> = tokio::sync::OnceCell::const_new();
static ENIGO: Lazy<tokio::sync::Mutex<Enigo>> = Lazy::new(|| tokio::sync::Mutex::new(Enigo::new()));
static RUNNING: AtomicBool = AtomicBool::new(false);
static RERUN: AtomicBool = AtomicBool::new(false);
static SHOW_CONFIRMATION_DIALOG: AtomicBool = AtomicBool::new(false);
static ALLOWED_TO_CLOSE: AtomicBool =  AtomicBool::new(false);
static minimized: AtomicBool =  AtomicBool::new(false);
static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap());
static RUNTIMERUNNING: AtomicBool = AtomicBool::new(false);

struct App;

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {}
    }
    
}
async fn stop() {
    if let Some(handle) = RUNNER.get() {
        handle.abort();
    }
    println!("Stopped");
}



async fn start() {
    RUNNER.set(RUNTIME.spawn(async move {
        tokio::select! {
            _ = move_mouse() => {},
            _ = press_key() => {},
        }
    }));
}

async fn move_mouse() {
    while RUNNING.load(Ordering::Relaxed) {
        println!("move");
        {
            let mut enigo = ENIGO.lock().await;
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                enigo.mouse_move_relative(1,1);
            }));
        }
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        {
            let mut enigo = ENIGO.lock().await;
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                enigo.mouse_move_relative(-1,-1);
            }));
        }
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn press_key() {
    while RUNNING.load(Ordering::Relaxed) {
        println!("click");
        {
            let mut enigo = ENIGO.lock().await;
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                enigo.key_click(Key::F15);
            }));
        }
        tokio::time::sleep(Duration::from_secs(10)).await;

    }
}
impl eframe::App for App {
    fn on_close_event(&mut self) -> bool {
        SHOW_CONFIRMATION_DIALOG.compare_exchange(false,true ,Ordering::Acquire, Ordering::Relaxed);
        ALLOWED_TO_CLOSE.load(Ordering::Relaxed)
    }
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Each frame:
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center),|ui| {
                if ui.button(egui::RichText::new("Start").size(20.0)).clicked() {
                    if RUNNING.compare_exchange(false,true,Ordering::Acquire, Ordering::Relaxed).is_ok() {
                        RUNTIME.spawn(async {
                            start().await;
                        });
                    }
                }
                if ui.button(egui::RichText::new("Stop").size(20.0)).clicked() {
                    if RUNNING.compare_exchange(true,false,Ordering::Acquire, Ordering::Relaxed).is_ok() {
                        RUNTIME.spawn(async {
                            stop().await;
                        });
                    }
                }
            });
        });
        if SHOW_CONFIRMATION_DIALOG.load(Ordering::Relaxed) {
            // Show confirmation dialog:
            egui::Window::new(RichText::new("Do you want to quit?").size(12.0))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Cancel").size(12.0)).clicked() {
                            SHOW_CONFIRMATION_DIALOG.compare_exchange(true,false ,Ordering::Acquire, Ordering::Relaxed);
                        }

                        if ui.button(RichText::new("Yes!").size(12.0)).clicked() {
                            ALLOWED_TO_CLOSE.compare_exchange(false,true ,Ordering::Acquire, Ordering::Relaxed);
                            frame.close();
                        }
                    });
                });
        };
    }
}

fn main() {
    // Initialize the static variables
    RUNTIME.enter();
    let native_options = eframe::NativeOptions {
        active: true,
        centered: true,
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

    
    eframe::run_native(
        "Mouse Shacker",
        native_options,
        Box::new(move |cc| {
            Box::new(App::new(cc))
        }),
    );
}
