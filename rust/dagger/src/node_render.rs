use crate::state::EditorState;
use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};
use orc_sdk::{IH, NodeInfo, OH};

const NODE_WIDTH: f32 = 160.0;
const TITLE_HEIGHT: f32 = 24.0;
const PIN_RADIUS: f32 = 5.0;
const PIN_SPACING: f32 = 20.0;
const PIN_TOP_OFFSET: f32 = TITLE_HEIGHT + 12.0;
const NODE_ROUNDING: f32 = 6.0;
const FONT_SIZE: f32 = 13.0;

fn node_color(info: &NodeInfo) -> Color32 {
    match info {
        NodeInfo::Constant(_) => Color32::from_rgb(80, 120, 80),
        NodeInfo::Function(_) => Color32::from_rgb(60, 90, 140),
        NodeInfo::NestedCall { .. } => Color32::from_rgb(120, 80, 120),
        NodeInfo::Inspect { .. } => Color32::from_rgb(140, 100, 50),
    }
}

fn title_color(info: &NodeInfo) -> Color32 {
    match info {
        NodeInfo::Constant(_) => Color32::from_rgb(100, 150, 100),
        NodeInfo::Function(_) => Color32::from_rgb(80, 115, 170),
        NodeInfo::NestedCall { .. } => Color32::from_rgb(150, 100, 150),
        NodeInfo::Inspect { .. } => Color32::from_rgb(170, 125, 65),
    }
}

pub fn node_height(n_inputs: usize, n_outputs: usize) -> f32 {
    let n_pins = n_inputs.max(n_outputs).max(1);
    PIN_TOP_OFFSET + n_pins as f32 * PIN_SPACING + 8.0
}

/// Returns the canvas-space position of an input pin given the node's top-left position.
pub fn input_pin_pos(node_pos: Pos2, pin_index: usize) -> Pos2 {
    Pos2::new(
        node_pos.x,
        node_pos.y + PIN_TOP_OFFSET + pin_index as f32 * PIN_SPACING,
    )
}

/// Returns the canvas-space position of an output pin given the node's top-left position.
pub fn output_pin_pos(node_pos: Pos2, pin_index: usize) -> Pos2 {
    Pos2::new(
        node_pos.x + NODE_WIDTH,
        node_pos.y + PIN_TOP_OFFSET + pin_index as f32 * PIN_SPACING,
    )
}

pub fn draw_nodes(ui: &mut egui::Ui, state: &EditorState) {
    let painter = ui.painter();
    let font = FontId::new(FONT_SIZE, FontFamily::Monospace);
    let label_font = FontId::new(11.0, FontFamily::Monospace);

    let node_info_prop = state.workflow.node_info_prop();
    let input_labels_prop = state.workflow.input_labels_prop();
    let output_labels_prop = state.workflow.output_labels_prop();

    let node_infos = match node_info_prop.try_borrow() {
        Ok(infos) => infos,
        Err(_) => return,
    };
    let input_labels = match input_labels_prop.try_borrow() {
        Ok(l) => l,
        Err(_) => return,
    };
    let output_labels = match output_labels_prop.try_borrow() {
        Ok(l) => l,
        Err(_) => return,
    };
    let positions = match state.node_positions.try_borrow() {
        Ok(p) => p,
        Err(_) => return,
    };

    for nh in state.workflow.node_iter() {
        let info = &node_infos[nh];
        let pos = positions[nh];
        let node_pos = Pos2::new(pos[0], pos[1]);

        let inputs: Vec<IH> = state.workflow.node_inputs(nh).collect();
        let outputs: Vec<OH> = state.workflow.node_outputs(nh).collect();
        let height = node_height(inputs.len(), outputs.len());

        let body_rect = Rect::from_min_size(node_pos, Vec2::new(NODE_WIDTH, height));
        let title_rect = Rect::from_min_size(node_pos, Vec2::new(NODE_WIDTH, TITLE_HEIGHT));

        // Node body.
        painter.rect_filled(body_rect, NODE_ROUNDING, node_color(info));
        painter.rect_stroke(
            body_rect,
            NODE_ROUNDING,
            Stroke::new(1.0, Color32::from_gray(40)),
            StrokeKind::Outside,
        );

        // Title bar.
        painter.rect_filled(title_rect, NODE_ROUNDING, title_color(info));
        // Clip the bottom corners of the title bar by overdrawing a small rect.
        if height > TITLE_HEIGHT {
            let patch = Rect::from_min_size(
                Pos2::new(node_pos.x, node_pos.y + TITLE_HEIGHT - NODE_ROUNDING),
                Vec2::new(NODE_WIDTH, NODE_ROUNDING),
            );
            painter.rect_filled(patch, 0.0, title_color(info));
        }

        // Title text.
        painter.text(
            Pos2::new(node_pos.x + 8.0, node_pos.y + 4.0),
            egui::Align2::LEFT_TOP,
            info.name(),
            font.clone(),
            Color32::WHITE,
        );

        // Input pins.
        for (i, ih) in inputs.iter().enumerate() {
            let pin_center = input_pin_pos(node_pos, i);
            let connected = state.workflow.input_source(*ih).is_some();
            if connected {
                painter.circle_filled(pin_center, PIN_RADIUS, Color32::from_rgb(200, 200, 200));
            } else {
                painter.circle_stroke(
                    pin_center,
                    PIN_RADIUS,
                    Stroke::new(1.5, Color32::from_rgb(160, 160, 160)),
                );
            }
            let label = &input_labels[*ih];
            if !label.is_empty() {
                painter.text(
                    Pos2::new(pin_center.x + PIN_RADIUS + 4.0, pin_center.y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    label_font.clone(),
                    Color32::from_gray(200),
                );
            }
        }

        // Output pins.
        for (i, oh) in outputs.iter().enumerate() {
            let pin_center = output_pin_pos(node_pos, i);
            painter.circle_filled(pin_center, PIN_RADIUS, Color32::from_rgb(200, 200, 200));
            let label = &output_labels[*oh];
            if !label.is_empty() {
                painter.text(
                    Pos2::new(pin_center.x - PIN_RADIUS - 4.0, pin_center.y),
                    egui::Align2::RIGHT_CENTER,
                    label,
                    label_font.clone(),
                    Color32::from_gray(200),
                );
            }
        }
    }
}
