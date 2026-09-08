use crate::node_render;
use crate::state::EditorState;
use eframe::egui::{self, Color32, Pos2, Shape};
use eframe::epaint::{CubicBezierShape, PathStroke};

const LINK_COLOR: Color32 = Color32::from_rgb(180, 180, 180);
const LINK_WIDTH: f32 = 2.0;
const CONTROL_POINT_OFFSET: f32 = 80.0;

pub fn draw_links(ui: &mut egui::Ui, state: &EditorState) {
    let painter = ui.painter();
    let positions = match state.node_positions.try_borrow() {
        Ok(p) => p,
        Err(_) => return,
    };

    for nh in state.workflow.node_iter() {
        let dst_pos_arr = positions[nh];
        let dst_node_pos = Pos2::new(dst_pos_arr[0], dst_pos_arr[1]);

        for (input_idx, ih) in state.workflow.node_inputs(nh).enumerate() {
            let src_oh = match state.workflow.input_source(ih) {
                Some(oh) => oh,
                None => continue,
            };

            // Find the source node and the output pin index within that node.
            let src_nh = state.workflow.node_from_output(src_oh);
            let src_pos_arr = positions[src_nh];
            let src_node_pos = Pos2::new(src_pos_arr[0], src_pos_arr[1]);

            let output_idx = state
                .workflow
                .node_outputs(src_nh)
                .position(|o| o == src_oh)
                .unwrap_or(0);

            let start = node_render::output_pin_pos(src_node_pos, output_idx);
            let end = node_render::input_pin_pos(dst_node_pos, input_idx);

            let dx = (end.x - start.x).abs().max(CONTROL_POINT_OFFSET) * 0.5;
            let cp1 = Pos2::new(start.x + dx, start.y);
            let cp2 = Pos2::new(end.x - dx, end.y);

            painter.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
                [start, cp1, cp2, end],
                false,
                Color32::TRANSPARENT,
                PathStroke::new(LINK_WIDTH, LINK_COLOR),
            )));
        }
    }
}
