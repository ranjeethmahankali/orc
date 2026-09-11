use crate::canvas;
use crate::const_edit;
use crate::context_menu;
use crate::exec;
use crate::file_menu;
use crate::inspect;
use crate::interaction::{self, FrameEvents};
use crate::nested;
use crate::render;
use crate::state::EditorState;
use eframe::egui;
use orc_sdk::{NH, Workflow};
use std::path::PathBuf;

/// One level of the nested-editing stack. Index 0 is always the root (the file originally opened
/// or created), with `workflow_name: None` since it isn't nested inside anything. Every later
/// entry owns the real `Workflow` moved out of the level below it by `nested::open`, tagged with
/// the name it needs to be moved back under when its window closes (`nested::close`).
struct StackEntry {
    state: EditorState,
    workflow_name: Option<String>,
}

/// A stack of ordinary editors, one OS window each. Only the last entry is ever active
/// (processes input, runs its own executor, can open a further nested editor); every entry below
/// it is frozen — still rendered so its window doesn't look broken, but inert, per PROJECT.org's
/// "Phase 5: Nested Workflow Editing". Opening/closing never clones a `Workflow`: `nested::open`
/// moves the real one out of the level below into a freshly pushed entry, and `nested::close`
/// moves it back, so editing it in a pop-out really is editing it in place.
pub struct DaggerApp {
    stack: Vec<StackEntry>,
}

/// Stable per-depth id for a pushed nested editor's OS window. Depth (not the workflow name)
/// keys it, since at most one window ever exists at a given depth at a time — reusing the id
/// after that window closes and a different nested workflow is opened at the same depth is fine,
/// egui only needs uniqueness among *currently* open viewports.
fn viewport_id_for(depth: usize) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(("dagger-nested-editor", depth))
}

/// Runs one frame's worth of normal editing for the active (top-of-stack) level: measuring,
/// canvas/pin/node interaction, background execution, rendering, popouts, the context menu. This
/// is exactly what the whole app used to do unconditionally before nested editing existed — now
/// it only runs for whichever single level is active, while every other level renders read-only
/// via `render_frozen` instead.
fn process_active_frame(ui: &mut egui::Ui, state: &mut EditorState) -> FrameEvents {
    if state.needs_measure {
        state.measure(ui.ctx());
    }
    let over_inspect_content = ui
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|pos| interaction::pointer_over_inspect_content(state, &state.view, pos));
    let (canvas_response, view_moved) =
        canvas::interact(ui, &mut state.view, !over_inspect_content);
    let deleted = interaction::delete_selected(ui, state);

    // Drains completed jobs and dispatches whatever just became ready. Cheap when nothing is
    // stale or in flight, so this runs unconditionally every active frame.
    exec::tick(state);
    inspect::refresh_all(state);
    const_edit::refresh_all(state);
    if exec::any_in_flight(state) {
        // Needed both to keep draining the results channel and to animate the in-progress sweep
        // on whichever node(s) are running.
        ui.ctx().request_repaint();
    }

    // Runs before rendering, using hit rects from the end of the previous frame, so a drag this
    // frame can tell rendering which node to draw wherever it's being dragged to.
    let mut events = interaction::update(ui, state, &canvas_response);
    if events.changed || deleted {
        ui.ctx().request_repaint();
    }
    if let Some(request) = events.context_menu.take() {
        context_menu::open(state, request);
    }
    if view_moved {
        // Scroll deltas are smoothed over several frames, so keep painting until the zoom has
        // caught up with the wheel.
        ui.ctx().request_repaint();
    }

    let const_edit_events = render::draw(ui, state);
    let const_edit_changed = !const_edit_events.committed_rows.is_empty()
        || !const_edit_events.inserted_after.is_empty()
        || !const_edit_events.deleted_rows.is_empty();
    const_edit::apply_events(state, const_edit_events);
    if const_edit_changed {
        ui.ctx().request_repaint();
    }
    inspect::update_popouts(ui.ctx(), state);

    context_menu::update(ui, state);
    if state.needs_measure {
        // A node created this frame (by the context menu) needs its real size measured before
        // it draws correctly, which only happens at the top of the next frame — nothing else
        // here guarantees one occurs.
        ui.ctx().request_repaint();
    }
    if state.session_info_open {
        let mut open = state.session_info_open;
        context_menu::session_info_window(ui.ctx(), &mut open);
        state.session_info_open = open;
    }

    events
}

/// Renders a frozen (non-active) level read-only, and redirects any attempt to interact with it
/// to the active window instead — clicking, dragging, or double-clicking anywhere in a frozen
/// window just brings the real, active editor to the front rather than doing anything here.
///
/// `ui.disable()` (rather than simply not processing `interaction::update` for this level, which
/// is also true) is what stops egui's own widgets — an editable Constant's text fields in
/// particular — from being independently interactive during `render::draw`; disabling is what
/// makes "frozen" actually mean frozen instead of "half-interactive by accident."
fn render_frozen(ui: &mut egui::Ui, state: &EditorState, focus_target: egui::ViewportId) {
    let full_rect = ui.max_rect();
    ui.scope(|ui| {
        ui.disable();
        render::draw(ui, state);
    });
    let catcher = ui.interact(
        full_rect,
        ui.id().with("dagger-frozen-catcher"),
        egui::Sense::click_and_drag(),
    );
    if catcher.clicked() || catcher.dragged() || catcher.double_clicked() {
        ui.ctx()
            .send_viewport_cmd_to(focus_target, egui::ViewportCommand::Focus);
    }
}

impl DaggerApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        workflow: Workflow,
        path: Option<PathBuf>,
    ) -> Self {
        // By default egui snaps text to the nearest physical pixel so static text stays crisp.
        // Node labels are not static here — they're carried along by node dragging and canvas
        // pan/zoom — so that snap becomes visible as the label hopping between pixels a frame
        // at a time instead of sliding smoothly. Shapes don't have this problem: they're
        // antialiased by feathering the edge, which blends smoothly across a sub-pixel offset
        // instead of rounding it away.
        cc.egui_ctx
            .tessellation_options_mut(|options| options.round_text_to_pixels = false);
        let mut state = EditorState::from_workflow(workflow);
        state.current_path = path;
        Self {
            stack: vec![StackEntry {
                state,
                workflow_name: None,
            }],
        }
    }

    /// Opens `nh` (a `NestedCall` node in the current top-of-stack level) for editing, pushing a
    /// new active level on top of it. A no-op if `nh` isn't a `NestedCall` node, or if that name
    /// is already checked out elsewhere (already open in another window) — see `nested::open`.
    fn push_nested(&mut self, nh: NH) {
        let Some(top) = self.stack.last_mut() else {
            return;
        };
        if let Some((workflow_name, pushed)) = nested::open(&mut top.state, nh) {
            self.stack.push(StackEntry {
                state: pushed,
                workflow_name: Some(workflow_name),
            });
        }
    }

    /// Closes the top-of-stack level, moving its (possibly edited) `Workflow` back into the level
    /// below under the name it was opened from. A no-op if the stack is already down to just the
    /// root — there's nothing to close.
    fn pop_nested(&mut self) {
        if self.stack.len() <= 1 {
            return;
        }
        let Some(popped) = self.stack.pop() else {
            return;
        };
        if let Some(workflow_name) = popped.workflow_name {
            let Some(parent) = self.stack.last_mut() else {
                return;
            };
            nested::close(&mut parent.state, workflow_name, popped.state);
        }
    }
}

impl eframe::App for DaggerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let depth = self.stack.len();
        let is_root_active = depth == 1;
        // Only meaningful while something is pushed on top of the root -- the id of whichever
        // window is currently active, for every frozen level's close/click redirect to target.
        let top_viewport_id = (depth > 1).then(|| viewport_id_for(depth - 1));

        // eframe's default close behavior exits immediately once the OS close event arrives, so
        // unlike File > New/Open (which already gate on confirm_discard), closing the window
        // directly would otherwise silently discard unsaved work with no prompt at all. While a
        // nested editor is open, the root is frozen like any other background level: closing it
        // just brings the active window forward instead.
        let close_requested = ctx.input(|i| {
            i.viewport()
                .events
                .iter()
                .any(|e| matches!(e, egui::ViewportEvent::Close))
        });
        if close_requested {
            if let Some(top_id) = top_viewport_id {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd_to(top_id, egui::ViewportCommand::Focus);
            } else if !file_menu::confirm_discard(&mut self.stack[0].state) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }

        if is_root_active {
            file_menu::handle_shortcuts(&ctx, &mut self.stack[0].state);
        }
        egui::Panel::top("dagger-menu-bar").show(ui, |ui| {
            ui.add_enabled_ui(is_root_active, |ui| {
                file_menu::menu_bar(ui, &mut self.stack[0].state);
            });
        });
        file_menu::error_window(&ctx, &mut self.stack[0].state);
        file_menu::update_window_title(&ctx, &mut self.stack[0].state);

        let mut pending_open: Option<NH> = None;
        let mut pending_close = false;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                if is_root_active {
                    pending_open = process_active_frame(ui, &mut self.stack[0].state).open_nested;
                } else if let Some(top_id) = top_viewport_id {
                    render_frozen(ui, &self.stack[0].state, top_id);
                }
            });

        for i in 1..depth {
            let is_active = i == depth - 1;
            let viewport_id = viewport_id_for(i);
            let title = format!(
                "Dagger - {}",
                self.stack[i].workflow_name.as_deref().unwrap_or("nested")
            );
            let entry = &mut self.stack[i];
            let mut close_requested_here = false;
            ctx.show_viewport_immediate(
                viewport_id,
                egui::ViewportBuilder::default()
                    .with_title(title)
                    .with_inner_size([1100.0, 700.0]),
                |ui, _class| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ui, |ui| {
                            if is_active {
                                pending_open =
                                    process_active_frame(ui, &mut entry.state).open_nested;
                            } else if let Some(top_id) = top_viewport_id {
                                render_frozen(ui, &entry.state, top_id);
                            }
                        });
                    close_requested_here = ui.ctx().input(|i| i.viewport().close_requested());
                },
            );
            if close_requested_here {
                if is_active {
                    pending_close = true;
                } else if let Some(top_id) = top_viewport_id {
                    ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::CancelClose);
                    ctx.send_viewport_cmd_to(top_id, egui::ViewportCommand::Focus);
                }
            }
        }

        if let Some(nh) = pending_open {
            self.push_nested(nh);
        }
        if pending_close {
            self.pop_nested();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use orc_sdk::{IH, NodeInfo, OH, PluginSet};

    /// Builds a two-level call chain: the root calls "outer", which itself calls "inner" -- so
    /// `push_nested` twice in a row (root -> outer -> inner) exercises genuinely nested-within-
    /// nested editing, not just a single push/pop.
    fn two_level_call_chain() -> (DaggerApp, NH) {
        let ps = PluginSet::default();
        let mut outer = Workflow::default();
        outer
            .push_nested_workflow("inner".to_string(), Workflow::default(), &ps)
            .unwrap();
        let mut outer_ihs: [IH; 0] = [];
        let mut outer_ohs: [OH; 0] = [];
        outer
            .add_nested_workflow_call("inner", &mut outer_ihs, &mut outer_ohs)
            .unwrap();

        let mut root = Workflow::default();
        root.push_nested_workflow("outer".to_string(), outer, &ps)
            .unwrap();
        let mut root_ihs: [IH; 0] = [];
        let mut root_ohs: [OH; 0] = [];
        let root_call_nh = root
            .add_nested_workflow_call("outer", &mut root_ihs, &mut root_ohs)
            .unwrap();

        let state = EditorState::from_workflow(root);
        (
            DaggerApp {
                stack: vec![StackEntry {
                    state,
                    workflow_name: None,
                }],
            },
            root_call_nh,
        )
    }

    #[test]
    fn t_push_nested_is_a_noop_for_a_non_nested_call_node() {
        let mut wf = Workflow::default();
        let mut outs = [OH::default()];
        let nh = wf
            .add_function(orc_sdk::FuncInfo::default(), &mut [], &mut outs)
            .unwrap();
        let state = EditorState::from_workflow(wf);
        let mut app = DaggerApp {
            stack: vec![StackEntry {
                state,
                workflow_name: None,
            }],
        };
        app.push_nested(nh);
        assert_eq!(app.stack.len(), 1, "nothing to open, nothing should push");
    }

    #[test]
    fn t_pop_nested_is_a_noop_at_the_root() {
        let state = EditorState::from_workflow(Workflow::default());
        let mut app = DaggerApp {
            stack: vec![StackEntry {
                state,
                workflow_name: None,
            }],
        };
        app.pop_nested();
        assert_eq!(app.stack.len(), 1, "the root alone must never pop");
    }

    #[test]
    fn t_push_then_push_again_reaches_two_levels_of_nesting() {
        let (mut app, root_call_nh) = two_level_call_chain();
        app.push_nested(root_call_nh);
        assert_eq!(app.stack.len(), 2, "opened \"outer\" on top of the root");

        let outer_call_nh = app.stack[1]
            .state
            .workflow
            .node_iter()
            .find(|&nh| {
                let node_info_prop = app.stack[1].state.workflow.node_info_prop();
                let node_infos = node_info_prop.try_borrow().unwrap();
                matches!(&node_infos[nh], NodeInfo::NestedCall { workflow_name } if workflow_name == "inner")
            })
            .expect("outer calls inner");
        app.push_nested(outer_call_nh);
        assert_eq!(
            app.stack.len(),
            3,
            "opened \"inner\" on top of \"outer\" on top of the root"
        );

        app.pop_nested();
        assert_eq!(
            app.stack.len(),
            2,
            "closed \"inner\", back down to \"outer\""
        );
        app.pop_nested();
        assert_eq!(
            app.stack.len(),
            1,
            "closed \"outer\", back down to the root"
        );
    }
}
