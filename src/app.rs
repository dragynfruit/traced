use std::collections::HashMap;
use tokio::runtime::Runtime;

use egui::Context;
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, Position, Tiles};

use crate::{plugins, windows};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    OpenStreetMap
}

fn http_options() -> HttpOptions {
    HttpOptions {
        // Not sure where to put cache on Android, so it will be disabled for now.
        cache: if cfg!(target_os = "android") || std::env::var("NO_HTTP_CACHE").is_ok() {
            None
        } else {
            Some(".cache".into())
        },
        ..Default::default()
    }
}

fn providers(egui_ctx: Context) -> HashMap<Provider, Box<dyn Tiles + Send>> {
    let mut providers: HashMap<Provider, Box<dyn Tiles + Send>> = HashMap::default();

    providers.insert(
        Provider::OpenStreetMap,
        Box::new(HttpTiles::with_options(
            walkers::sources::OpenStreetMap,
            http_options(),
            egui_ctx.to_owned(),
        )),
    );

    providers
}

pub struct App {
    providers: HashMap<Provider, Box<dyn Tiles + Send>>,
    selected_provider: Provider,
    map_memory: MapMemory,
    trace_path: plugins::TracePath,
    runtime: Runtime,
    show_debug: bool,
}

impl App {
    pub fn new(egui_ctx: Context) -> Self {
        egui_extras::install_image_loaders(&egui_ctx);

        let mut map_memory = MapMemory::default();
        map_memory.set_zoom(1.0).ok();

        Self {
            providers: providers(egui_ctx.to_owned()),
            selected_provider: Provider::OpenStreetMap,
            map_memory,
            trace_path: Default::default(),
            runtime: Runtime::new().unwrap(),
            show_debug: false,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for F12 key press to toggle debug window
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.show_debug = !self.show_debug;
        }

        // Show debug window if enabled
        if self.show_debug {
            egui::Window::new("Debug Info")
                .resizable(true)
                .show(ctx, |ui| {
                    ctx.inspection_ui(ui);
                });
        }

        let rimless = egui::Frame {
            fill: ctx.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(rimless)
            .show(ctx, |ui| {
                let tiles = self
                    .providers
                    .get_mut(&self.selected_provider)
                    .unwrap()
                    .as_mut();
                let attribution = tiles.attribution();

                // In egui, widgets are constructed and consumed in each frame.
                let map = Map::new(Some(tiles), &mut self.map_memory, Position::from_lat_lon(0.0, 0.0));

                // Attach the trace path plugin instead of click watcher
                let map = map.with_plugin(&mut self.trace_path);

                // Draw the map widget.
                ui.add(map);

                // Draw utility windows.
                {
                    use windows::*;

                    zoom(ui, &mut self.map_memory);
                    enter_ip(ui, &mut self.trace_path, &self.runtime);
                    controls(
                        ui,
                        &mut self.selected_provider,
                        &mut self.providers.keys(),
                    );
                    acknowledge(ui, attribution);
                }
            });
    }
}
