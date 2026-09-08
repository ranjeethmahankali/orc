//! Pin connect/disconnect, select, delete, box-select, node dragging.

use crate::canvas::Transform;
use crate::render;
use crate::state::EditorState;
use eframe::egui::{self, Id, Key, PointerButton, Pos2, Rect, Sense, Vec2};
use orc_sdk::{IH, NH, NodeInfo, OH, Workflow};
use std::collections::{HashMap, HashSet};

/// Where a right-click asked for a context menu to open, and what (if anything) it should
/// connect the new node's first input to.
pub enum ContextMenuRequest {
    Empty(Pos2),
    FromOutput(OH, Pos2),
}

/// What happened this frame, for the caller to act on.
#[derive(Default)]
pub struct FrameEvents {
    /// Something changed that's worth an extra repaint (a drag, a selection, a connection).
    pub changed: bool,
    pub context_menu: Option<ContextMenuRequest>,
    /// The node being actively dragged this frame, if any, so the layout step can exclude it
    /// from the physics displacement while still reacting to its (cursor-driven) position.
    pub dragged_node: Option<NH>,
}

fn pin_rect(center: Pos2, view: &Transform) -> Rect {
    let half = view.scale(render::PIN_GRAB_RADIUS);
    Rect::from_center_size(center, Vec2::splat(2.0 * half))
}

fn select_node(state: &mut EditorState, target: NH, additive: bool) {
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    let Ok(mut selected) = state.selected.try_borrow_mut() else {
        return;
    };
    if additive {
        selected[target] = !selected[target];
    } else {
        for nh in nodes {
            selected[nh] = nh == target;
        }
    }
}

fn deselect_all(state: &mut EditorState) {
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    if let Ok(mut selected) = state.selected.try_borrow_mut() {
        for nh in nodes {
            selected[nh] = false;
        }
    }
}

fn update_box_select(ui: &mut egui::Ui, state: &mut EditorState, view: &Transform) -> Option<Rect> {
    let (start, current) = match ui.input(|i| (i.pointer.press_origin(), i.pointer.interact_pos()))
    {
        (Some(s), Some(c)) => (s, c),
        _ => return None,
    };
    let screen_rect = Rect::from_two_pos(start, current);
    let canvas_rect = Rect::from_two_pos(view.to_canvas(start), view.to_canvas(current));
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    if let (Ok(positions), Ok(sizes), Ok(mut selected)) = (
        state.node_positions.try_borrow(),
        state.node_sizes.try_borrow(),
        state.selected.try_borrow_mut(),
    ) {
        for nh in nodes {
            selected[nh] = render::node_rect(positions[nh], sizes[nh]).intersects(canvas_rect);
        }
    }
    Some(screen_rect)
}

/// Whether adding an edge from `src` to `dst` would create a cycle, i.e. whether `dst` can
/// already reach `src` through the graph's existing links. Rebuilt from scratch on every call
/// since it only runs once per pin-drop gesture, not per frame.
fn creates_cycle(workflow: &Workflow, src: NH, dst: NH) -> bool {
    if src == dst {
        return true;
    }
    let mut forward: HashMap<NH, Vec<NH>> = HashMap::new();
    for node in workflow.node_iter() {
        for ih in workflow.node_inputs(node) {
            if let Some(oh) = workflow.input_source(ih) {
                forward
                    .entry(workflow.node_from_output(oh))
                    .or_default()
                    .push(node);
            }
        }
    }
    let mut stack = vec![dst];
    let mut visited = HashSet::new();
    while let Some(node) = stack.pop() {
        if node == src {
            return true;
        }
        if !visited.insert(node) {
            continue;
        }
        if let Some(next) = forward.get(&node) {
            stack.extend(next.iter().copied());
        }
    }
    false
}

pub(crate) fn find_input_pin_at(state: &EditorState, screen_pos: Pos2, view: &Transform) -> Option<IH> {
    let positions = state.node_positions.try_borrow().ok()?;
    let sizes = state.node_sizes.try_borrow().ok()?;
    let grab = view.scale(render::PIN_GRAB_RADIUS);
    for nh in state.workflow.node_iter() {
        let rect = render::node_rect(positions[nh], sizes[nh]);
        for (i, ih) in state.workflow.node_inputs(nh).enumerate() {
            let center = view.to_screen(render::input_pin_pos(rect, i));
            if center.distance(screen_pos) <= grab {
                return Some(ih);
            }
        }
    }
    None
}

/// Finish a wire drag on release: connect to whatever input pin is under the cursor, or drop
/// the wire if it lands on empty canvas, a node body, or a connection that would close a cycle.
fn update_pending_wire(ui: &mut egui::Ui, state: &mut EditorState, source: OH) -> bool {
    if !ui.input(|i| i.pointer.button_released(PointerButton::Primary)) {
        return false;
    }
    let view = state.view;
    let target = ui
        .input(|i| i.pointer.interact_pos())
        .and_then(|pos| find_input_pin_at(state, pos, &view));
    if let Some(target) = target {
        let dst = state.workflow.node_from_input(target);
        let src = state.workflow.node_from_output(source);
        if !creates_cycle(&state.workflow, src, dst) {
            let _ = state.workflow.connect(source, target);
            state.layout_converged = false;
        }
    }
    state.pending_wire = None;
    true
}

/// Interact with every node's body and pins, and apply any node drag, selection or wire drag
/// this frame.
///
/// A node's body senses drag so grabbing it anywhere (not just the title) moves the node,
/// matching Obsidian's graph view: the force layout keeps running and pulls neighbours toward
/// the moved node, but is free to settle it somewhere else entirely once the drag is released.
///
/// Pins are interacted after the body, in the same order they're painted in, so a press on the
/// sliver of a pin that overlaps the node's edge is claimed by the pin rather than starting a
/// node drag — later interacts win ties in egui's hit test.
pub fn update(ui: &mut egui::Ui, state: &mut EditorState, canvas_response: &egui::Response) -> FrameEvents {
    let view = state.view;
    let shift = ui.input(|i| i.modifiers.shift);
    let mut events = FrameEvents::default();

    let (positions, sizes) = match (
        state.node_positions.try_borrow(),
        state.node_sizes.try_borrow(),
    ) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return events,
    };

    let mut dragged_node: Option<(NH, Vec2)> = None;
    let mut wire_start: Option<OH> = None;
    let mut clicked_node: Option<NH> = None;

    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    for nh in nodes {
        let rect = render::node_rect(positions[nh], sizes[nh]);
        let body_response = ui.interact(
            view.rect_to_screen(rect),
            Id::new(("dagger-node-body", nh)),
            Sense::click_and_drag(),
        );

        let inputs: Vec<IH> = state.workflow.node_inputs(nh).collect();
        for (i, ih) in inputs.into_iter().enumerate() {
            let center = view.to_screen(render::input_pin_pos(rect, i));
            let response = ui.interact(
                pin_rect(center, &view),
                Id::new(("dagger-input-pin", ih)),
                Sense::click_and_drag(),
            );
            if response.drag_started()
                && let Some(upstream) = state.workflow.input_source(ih)
            {
                state.workflow.disconnect(upstream, ih);
                wire_start = Some(upstream);
                events.changed = true;
            }
        }
        let outputs: Vec<OH> = state.workflow.node_outputs(nh).collect();
        for (i, oh) in outputs.into_iter().enumerate() {
            let center = view.to_screen(render::output_pin_pos(rect, i));
            let response = ui.interact(
                pin_rect(center, &view),
                Id::new(("dagger-output-pin", oh)),
                Sense::click_and_drag(),
            );
            if response.drag_started() {
                wire_start = Some(oh);
            } else if response.secondary_clicked()
                && let Some(pos) = response.interact_pointer_pos()
            {
                events.context_menu = Some(ContextMenuRequest::FromOutput(oh, pos));
            }
        }

        if body_response.dragged_by(PointerButton::Primary) {
            dragged_node = Some((nh, body_response.drag_delta()));
        } else if body_response.clicked() {
            clicked_node = Some(nh);
        }
    }
    drop(positions);
    drop(sizes);

    if let Some(nh) = clicked_node {
        select_node(state, nh, shift);
        events.changed = true;
    }

    if let Some(oh) = wire_start {
        state.pending_wire = Some(oh);
        events.changed = true;
    }

    if let Some((nh, delta)) = dragged_node {
        if let Ok(mut positions) = state.node_positions.try_borrow_mut() {
            let canvas_delta = delta / view.zoom;
            positions[nh][0] += canvas_delta.x;
            positions[nh][1] += canvas_delta.y;
        }
        state.layout_converged = false;
        events.changed = true;
        events.dragged_node = Some(nh);
    }

    if let Some(source) = state.pending_wire {
        events.changed |= update_pending_wire(ui, state, source);
    } else if canvas_response.secondary_clicked()
        && let Some(pos) = canvas_response.interact_pointer_pos()
    {
        events.context_menu = Some(ContextMenuRequest::Empty(pos));
    } else if canvas_response.clicked() {
        deselect_all(state);
        events.changed = true;
    } else if canvas_response.dragged_by(PointerButton::Primary) {
        state.select_box = update_box_select(ui, state, &view);
        events.changed = true;
    } else {
        state.select_box = None;
    }

    events
}

/// Delete key removes every selected node, freeing any constant deck it owned. Guarded by
/// keyboard focus so pressing Delete while typing in the context menu's search field doesn't
/// also delete the current selection.
pub fn delete_selected(ui: &mut egui::Ui, state: &mut EditorState) -> bool {
    if ui.memory(|m| m.focused()).is_some() {
        return false;
    }
    if !ui.input(|i| i.key_pressed(Key::Delete)) {
        return false;
    }
    let selected_nodes: Vec<NH> = {
        let Ok(selected) = state.selected.try_borrow() else {
            return false;
        };
        state
            .workflow
            .node_iter()
            .filter(|&nh| selected[nh])
            .collect()
    };
    if selected_nodes.is_empty() {
        return false;
    }
    {
        let node_info_prop = state.workflow.node_info_prop();
        if let Ok(node_infos) = node_info_prop.try_borrow() {
            for &nh in &selected_nodes {
                if let NodeInfo::Constant(handle) = &node_infos[nh] {
                    let _ = crate::REGISTRY.free(handle.handle);
                }
            }
        }
    }
    for nh in selected_nodes {
        state.workflow.delete_node(nh);
    }
    state.layout_converged = false;
    true
}

#[cfg(test)]
mod test {
    use super::*;
    use orc_sdk::FuncInfo;

    fn node(wf: &mut Workflow, n_in: usize, n_out: usize) -> (NH, Vec<IH>, Vec<OH>) {
        let mut ins = vec![IH::default(); n_in];
        let mut outs = vec![OH::default(); n_out];
        let nh = wf
            .add_function(FuncInfo::default(), &mut ins, &mut outs)
            .unwrap();
        (nh, ins, outs)
    }

    #[test]
    fn t_no_cycle_between_disjoint_nodes() {
        let mut wf = Workflow::default();
        let (a, _, _) = node(&mut wf, 0, 1);
        let (b, _, _) = node(&mut wf, 1, 0);
        assert!(!creates_cycle(&wf, a, b));
    }

    #[test]
    fn t_no_cycle_extending_a_forward_chain() {
        let mut wf = Workflow::default();
        let (_a, _, a_out) = node(&mut wf, 0, 1);
        let (b, b_in, _b_out) = node(&mut wf, 1, 1);
        let (c, _c_in, _) = node(&mut wf, 1, 0);
        wf.connect(a_out[0], b_in[0]).unwrap();
        // b -> c would just extend the chain, not close a loop.
        assert!(!creates_cycle(&wf, b, c));
    }

    #[test]
    fn t_self_loop_is_a_cycle() {
        let mut wf = Workflow::default();
        let (a, _, _) = node(&mut wf, 1, 1);
        assert!(creates_cycle(&wf, a, a));
    }

    #[test]
    fn t_closing_edge_back_to_an_ancestor_is_a_cycle() {
        let mut wf = Workflow::default();
        let (a, _a_in, a_out) = node(&mut wf, 1, 1);
        let (b, b_in, _b_out) = node(&mut wf, 1, 1);
        wf.connect(a_out[0], b_in[0]).unwrap();
        // b -> a would close a loop, since a already reaches b.
        assert!(creates_cycle(&wf, b, a));
    }
}
