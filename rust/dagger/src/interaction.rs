//! Pin connect/disconnect, select, delete, box-select, node dragging.

use crate::canvas::Transform;
use crate::render;
use crate::state::EditorState;
use eframe::egui::{self, Id, Pos2, Rect, Sense, Vec2};

fn pin_rect(center: Pos2, view: &Transform) -> Rect {
    let half = view.scale(render::PIN_GRAB_RADIUS);
    Rect::from_center_size(center, Vec2::splat(2.0 * half))
}

/// Interact with every node's body and pins, and apply any node drag this frame.
///
/// A node's body senses drag so grabbing it anywhere (not just the title) moves the node,
/// matching Obsidian's graph view: the force layout keeps running and pulls neighbours
/// toward the moved node, but is free to settle it somewhere else entirely once the drag
/// is released.
///
/// Pins are interacted after the body, in the same order they're painted in, so a press on
/// the sliver of a pin that overlaps the node's edge is claimed by the pin rather than
/// starting a node drag — later interacts win ties in egui's hit test. Wiring a connection
/// from a pin drag is a later step, so the pin responses are unused for now.
///
/// Returns whether a node was dragged this frame.
pub fn update(ui: &mut egui::Ui, state: &mut EditorState) -> bool {
    let view = state.view;
    let (positions, sizes) = match (
        state.node_positions.try_borrow(),
        state.node_sizes.try_borrow(),
    ) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return false,
    };

    let mut dragged_node = None;
    for nh in state.workflow.node_iter() {
        let rect = render::node_rect(positions[nh], sizes[nh]);
        let body_response = ui.interact(
            view.rect_to_screen(rect),
            Id::new(("dagger-node-body", nh)),
            Sense::click_and_drag(),
        );

        for (i, ih) in state.workflow.node_inputs(nh).enumerate() {
            let center = view.to_screen(render::input_pin_pos(rect, i));
            ui.interact(
                pin_rect(center, &view),
                Id::new(("dagger-input-pin", ih)),
                Sense::click_and_drag(),
            );
        }
        for (i, oh) in state.workflow.node_outputs(nh).enumerate() {
            let center = view.to_screen(render::output_pin_pos(rect, i));
            ui.interact(
                pin_rect(center, &view),
                Id::new(("dagger-output-pin", oh)),
                Sense::click_and_drag(),
            );
        }

        if body_response.dragged_by(egui::PointerButton::Primary) {
            dragged_node = Some((nh, body_response.drag_delta()));
        }
    }
    drop(positions);
    drop(sizes);

    let Some((nh, delta)) = dragged_node else {
        return false;
    };
    if let Ok(mut positions) = state.node_positions.try_borrow_mut() {
        let canvas_delta = delta / view.zoom;
        positions[nh][0] += canvas_delta.x;
        positions[nh][1] += canvas_delta.y;
    }
    state.layout_converged = false;
    true
}
