use crate::app::Provider;
use crate::plugins::TracePath;
use log::{info, warn, error, debug};

use egui::{Align2, RichText, Ui, Window};
use serde::Deserialize;
use std::{
    net::IpAddr,
    sync::mpsc::{channel, Receiver, Sender}, thread,
};
use tokio::{runtime::Runtime, sync::mpsc};
use walkers::{sources::Attribution, MapMemory, Position};
use dns_lookup::lookup_host;

#[derive(Default)]
struct IpInput {
    value: String,
}

struct TraceChannel {
    sender: Sender<TraceEvent>,
    receiver: Receiver<TraceEvent>,
}

impl Default for TraceChannel {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self { sender, receiver }
    }
}

#[derive(Deserialize, Debug)]
struct IpApiResponse {
    lat: f64,
    lon: f64,
    status: String,
    isp: String,  // Add ISP field
}

// Add this new struct to store position with hostname
#[derive(Clone)]
pub struct TraceNode {
    pub position: Position,
    pub hostname: String,
    pub isp: String,  // Add ISP field
    pub ip: String,  // Add IP field
}

#[derive(Clone)]
pub enum TraceEvent {
    Node(TraceNode),
    Finish,
}

pub fn acknowledge(ui: &Ui, attribution: Attribution) {
    Window::new("Acknowledge")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_TOP, [10., 10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if let Some(logo) = attribution.logo_light {
                    ui.add(egui::Image::new(logo).max_height(30.0).max_width(80.0));
                }
                ui.hyperlink_to(attribution.text, attribution.url);
            });
        });
}

pub fn controls(
    ui: &Ui,
    selected_provider: &mut Provider,
    possible_providers: &mut dyn Iterator<Item = &Provider>,
) {
    Window::new("Satellite")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
        .fixed_size([150., 150.])
        .show(ui.ctx(), |ui| {
            ui.collapsing("Map", |ui| {
                egui::ComboBox::from_label("Tile Provider")
                    .selected_text(format!("{:?}", selected_provider))
                    .show_ui(ui, |ui| {
                        for p in possible_providers {
                            ui.selectable_value(selected_provider, *p, format!("{:?}", p));
                        }
                    });
            });
        });
}

/// Simple GUI to zoom in and out.
pub fn zoom(ui: &Ui, map_memory: &mut MapMemory) {
    Window::new("Map")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_BOTTOM, [10., -10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("➕").heading()).clicked() {
                    let _ = map_memory.zoom_in();
                }

                if ui.button(RichText::new("➖").heading()).clicked() {
                    let _ = map_memory.zoom_out();
                }
            });
        });
}

pub fn enter_ip(ui: &mut Ui, trace_path: &mut TracePath, runtime: &Runtime) {
    static IP_INPUT: std::sync::OnceLock<std::sync::Mutex<IpInput>> = std::sync::OnceLock::new();
    static TRACE_CHANNEL: std::sync::OnceLock<std::sync::Mutex<TraceChannel>> = std::sync::OnceLock::new();

    let ip_input = IP_INPUT.get_or_init(|| std::sync::Mutex::new(IpInput::default()));
    let trace_channel = TRACE_CHANNEL.get_or_init(|| std::sync::Mutex::new(TraceChannel::default()));

    Window::new("Enter IP or Domain")
        .resizable(false)
        .anchor(Align2::RIGHT_CENTER, [-10., 0.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                let mut ip_guard = ip_input.lock().unwrap();
                let text_edit = ui.add_enabled(
                    !trace_path.tracing,
                    egui::TextEdit::singleline(&mut ip_guard.value)
                );
                let trace_button = ui.add_enabled(
                    !trace_path.tracing,
                    egui::Button::new("Trace")
                );
                
                if !trace_path.tracing && 
                   ((text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) 
                   || trace_button.clicked()) {
                    let trace_guard = trace_channel.lock().unwrap();
                    let sender = trace_guard.sender.clone();
                    let ip = ip_guard.value.clone();
                    info!("Starting trace for IP: {}", ip);
                    trace_path.nodes.clear();
                    trace_path.tracing = true;
                    runtime.spawn(async move {
                        match trace(&ip).await {
                            Ok(mut events) => {
                                while let Some(event) = events.recv().await {
                                    sender.send(event).ok();
                                }
                            }
                            Err(e) => error!("Trace failed: {}", e),
                        }
                    });
                }
            });

            let trace_guard = trace_channel.lock().unwrap();
            if let Ok(event) = trace_guard.receiver.try_recv() {
                match event {
                    TraceEvent::Node(node) => {
                        trace_path.nodes.push((trace_path.nodes.len(), node));
                    }
                    TraceEvent::Finish => {
                        trace_path.tracing = false;
                    }
                }
            }
        });

    // Add loading spinner in bottom right
    if trace_path.tracing {
        Window::new("Loading")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::RIGHT_BOTTOM, [-10., -10.])
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.spinner(); // Built-in spinner widget
                    ui.label("Tracing route...");
                });
            });
    }
}

async fn get_my_ip(client: &reqwest::Client) -> Option<String> {
    match client.get("https://api.ipify.org").send().await {
        Ok(resp) => {
            if let Ok(my_ip) = resp.text().await {
                debug!("Retrieved IP: {}", my_ip);
                Some(my_ip)
            } else {
                None
            }
        }
        Err(e) => {
            warn!("Failed to get IP: {}", e);
            None
        }
    }
}

async fn get_location(client: &reqwest::Client, ip: &str) -> Option<(Position, String)> {
    if let Ok(resp) = client
        .get(format!("http://ip-api.com/json/{}", ip))
        .send()
        .await
    {
        if let Ok(location) = resp.json::<IpApiResponse>().await {
            if location.status == "success" {
                return Some((
                    Position::from_lat_lon(location.lat, location.lon),
                    location.isp,
                ));
            }
        }
    }
    None
}

// Modify trace function to use new helpers
async fn trace(
    target: &str,
) -> Result<mpsc::UnboundedReceiver<TraceEvent>, Box<dyn std::error::Error + Send + Sync>> {
    info!("Starting trace for target: {}", target);
    let (tx, rx) = mpsc::unbounded_channel();
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
    let client = reqwest::Client::new();

    // Get my ip first
    if let Some(ip) = get_my_ip(&client).await {
        if let Some((position, isp)) = get_location(&client, ip.as_str()).await {
            tx.send(TraceEvent::Node(TraceNode {
                position,
                hostname: "Local".to_string(),
                isp,
                ip,
            })).ok();
        }
    }

    // Resolve domain name or parse IP
    let ip = match target.parse::<IpAddr>() {
        Ok(ip) => {
            debug!("Parsed direct IP: {}", ip);
            Some(ip)
        }
        Err(_) => {
            debug!("Attempting DNS lookup for: {}", target);
            match lookup_host(target) {
                Ok(ips) => {
                    if let Some(ip) = ips.first() {
                        debug!("DNS lookup successful: {}", ip);
                        Some(*ip)
                    } else {
                        error!("DNS lookup returned no results");
                        None
                    }
                }
                Err(e) => {
                    error!("DNS lookup failed: {}", e);
                    None
                }
            }
        }
    };

    // Handle DNS resolution failure
    let ip = match ip {
        Some(ip) => ip,
        None => {
            tx.send(TraceEvent::Finish).ok();
            return Ok(rx);
        }
    };

    debug!("Starting tracer for IP: {}", ip);
    thread::spawn(move || {
        let tracer = tracert::trace::Tracer::new(ip).unwrap();
        let progress_receiver = tracer.get_progress_receiver();
        thread::spawn(move || tracer.trace());

        while let Ok(node) = progress_receiver.lock().unwrap().recv() {
            debug!("Got hop {}, sending", node.ip_addr);
            progress_tx.send(node).ok();
        }
    });

    debug!("Starting location lookup");
    tokio::spawn(async move {
        while let Some(node) = progress_rx.recv().await {
            let ip_str = node.ip_addr.to_string();
            debug!("Processing hop: {}", ip_str);
            
            if let Some((position, isp)) = get_location(&client, &ip_str).await {
                tx.send(TraceEvent::Node(TraceNode {
                    position,
                    hostname: node.host_name,
                    isp,
                    ip: ip_str,
                })).ok();
            }
        }
        tx.send(TraceEvent::Finish).ok();
    });

    Ok(rx)
}
