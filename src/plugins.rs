use egui::{vec2, Align2, Color32, FontId, Response, Stroke, Ui};
use crate::windows::TraceNode;
use walkers::{Plugin, Projector};

#[derive(Default)]
pub struct TracePath {
    pub nodes: Vec<(usize, TraceNode)>,
    pub tracing: bool,
    copy_anim_time: Option<f64>,
}

impl TracePath {
    pub fn set_path(&mut self, nodes: Vec<TraceNode>) {
        self.nodes = nodes.into_iter().enumerate().collect();
    }
}

impl Plugin for &mut TracePath {
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &Projector) {
        if self.nodes.is_empty() {
            return;
        }

        let painter = ui.painter();
        let mut last_screen_pos = None;
        let screen_rect = ui.clip_rect();
        let mut arrow_segments = Vec::new();

        for (idx, node) in &self.nodes {
            let screen_pos = projector.project(node.position).to_pos2();
            
            // Determine node color based on position
            let (fill_color, stroke_color) = if *idx == 0 {
                (Color32::GREEN, Color32::DARK_GREEN)
            } else if *idx == self.nodes.len() - 1 {
                (Color32::RED, Color32::DARK_RED)
            } else {
                (Color32::YELLOW, Color32::from_rgb(180, 180, 0))
            };
            
            // Draw point with position-based colors
            painter.circle_filled(
                screen_pos,
                5.0,
                fill_color,
            );
            painter.circle_stroke(
                screen_pos,
                5.0,
                Stroke::new(1.0, stroke_color),
            );
            painter.text(
                screen_pos + vec2(7.0, -7.0),
                Align2::LEFT_TOP,
                idx.to_string(),
                FontId::monospace(12.0),
                Color32::RED,
            );

            // Draw line to previous point
            if let Some(last_pos) = last_screen_pos {
                // Draw full line segment always
                painter.line_segment(
                    [last_pos, screen_pos],
                    Stroke::new(2.0, Color32::RED),
                );
                
                // Early culling - check if line segment is completely outside view
                let line_rect = egui::Rect::from_two_pos(last_pos, screen_pos);
                if screen_rect.intersects(line_rect) {
                    let direction = screen_pos - last_pos;
                    if direction.length() > 0.0 {
                        // Find intersection points with screen rect
                        let start = line_rect_intersection(last_pos, screen_pos, screen_rect);
                        
                        if let Some((vis_start, vis_end)) = start {
                            let vis_direction = vis_end - vis_start;
                            let vis_length = vis_direction.length();
                            
                            if vis_length > 0.0 {
                                let dir_normalized = vis_direction.normalized();
                                let arrow_size = 5.0;
                                let arrow_spacing = 30.0;
                                let num_arrows = (vis_length / arrow_spacing).floor() as i32;

                                // Precalculate arrow properties
                                let arrow_dir = dir_normalized * arrow_size;
                                let perp = arrow_dir.rot90();

                                // Calculate all arrow positions along visible segment
                                for i in 0..num_arrows {
                                    let t = (i as f32 + 1.0) / (num_arrows + 1) as f32;
                                    let arrow_pos = vis_start + vis_direction * t;
                                    
                                    // Define arrow polygon points
                                    arrow_segments.push(vec![
                                        arrow_pos - arrow_dir + perp,  // Left wing
                                        arrow_pos,                     // Tip
                                        arrow_pos - arrow_dir - perp,  // Right wing
                                    ]);
                                }
                            }
                        }
                    }
                }
            }
            
            last_screen_pos = Some(screen_pos);
        }

        // Batch draw all arrow polygons at once
        if !arrow_segments.is_empty() {
            painter.add(egui::Shape::Vec(
                arrow_segments.into_iter()
                    .map(|points| egui::Shape::convex_polygon(
                        points,
                        Color32::RED,
                        Stroke::NONE,
                    ))
                    .collect()
            ));
        }

        // Handle hover tooltips
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        if let Some(mouse_pos) = hover_pos {
            for (idx, node) in &self.nodes {
                let screen_pos = projector.project(node.position).to_pos2();
                if mouse_pos.distance(screen_pos) < 10.0 {
                    let tooltip_id = egui::Id::new("trace_tooltip");
                    let layer_id = egui::LayerId::new(egui::Order::Tooltip, tooltip_id);
                    
                    // Get simple timer state
                    let show_copied = if let Some(start_time) = self.copy_anim_time {
                        let now = ui.input(|i| i.time);
                        let age = (now - start_time) as f32;
                        if age > 1.0 {
                            self.copy_anim_time = None;
                            false
                        } else {
                            true
                        }
                    } else {
                        false
                    };
                    
                    egui::show_tooltip(
                        ui.ctx(),
                        layer_id,
                        tooltip_id,
                        |ui| {
                            ui.set_min_width(0.0);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                let heading = egui::RichText::new(format!("#{}", idx))
                                    .heading()
                                    .size(16.0);
                                ui.label(heading);
                                ui.add_space(2.0);
                                
                                let text_style = egui::TextStyle::Body;
                                ui.style_mut().text_styles.get_mut(&text_style)
                                    .map(|font| font.size = 13.0);
                                
                                ui.label(format!("Host: {}", node.hostname));
                                ui.label(format!("IP: {}", node.ip));
                                ui.label(format!("ISP: {}", node.isp));
                                
                                // Show copy feedback with simple timer
                                let copy_text = if show_copied {
                                    egui::RichText::new("Copied!")
                                        .color(Color32::GREEN)
                                        .size(14.0)
                                } else {
                                    egui::RichText::new("Click to copy IP")
                                        .color(Color32::GRAY)
                                        .size(14.0)
                                };
                                ui.label(copy_text);
                            });
                        }
                    );
                    
                    if ui.input(|i| i.pointer.any_click()) {
                        ui.output_mut(|o| o.copied_text = node.ip.clone());
                        self.copy_anim_time = Some(ui.input(|i| i.time));
                        ui.ctx().request_repaint();
                    }
                    break;
                }
            }
        }
    }
}

fn line_rect_intersection(start: egui::Pos2, end: egui::Pos2, rect: egui::Rect) -> Option<(egui::Pos2, egui::Pos2)> {
    use egui::pos2;
    
    // Cohen-Sutherland region codes
    const INSIDE: u8 = 0;
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const BOTTOM: u8 = 4;
    const TOP: u8 = 8;

    let compute_code = |p: egui::Pos2| {
        let mut code = INSIDE;
        if p.x < rect.min.x {
            code |= LEFT;
        } else if p.x > rect.max.x {
            code |= RIGHT;
        }
        if p.y < rect.min.y {
            code |= TOP;
        } else if p.y > rect.max.y {
            code |= BOTTOM;
        }
        code
    };

    let mut x1 = start.x;
    let mut y1 = start.y;
    let mut x2 = end.x;
    let mut y2 = end.y;
    let mut code1 = compute_code(pos2(x1, y1));
    let mut code2 = compute_code(pos2(x2, y2));

    loop {
        if code1 == 0 && code2 == 0 {
            // Line completely inside
            return Some((pos2(x1, y1), pos2(x2, y2)));
        } else if code1 & code2 != 0 {
            // Line completely outside
            return None;
        } else {
            // Line partially inside
            let code = if code1 != 0 { code1 } else { code2 };
            let x;
            let y;

            if code & TOP != 0 {
                x = x1 + (x2 - x1) * (rect.min.y - y1) / (y2 - y1);
                y = rect.min.y;
            } else if code & BOTTOM != 0 {
                x = x1 + (x2 - x1) * (rect.max.y - y1) / (y2 - y1);
                y = rect.max.y;
            } else if code & RIGHT != 0 {
                y = y1 + (y2 - y1) * (rect.max.x - x1) / (x2 - x1);
                x = rect.max.x;
            } else {
                y = y1 + (y2 - y1) * (rect.min.x - x1) / (x2 - x1);
                x = rect.min.x;
            }

            if code == code1 {
                x1 = x;
                y1 = y;
                code1 = compute_code(pos2(x1, y1));
            } else {
                x2 = x;
                y2 = y;
                code2 = compute_code(pos2(x2, y2));
            }
        }
    }
}