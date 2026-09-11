use crate::canvas::Transform;
use crate::const_edit;
use crate::context_menu::ContextMenuState;
use crate::exec;
use crate::inspect;
use crate::interaction::SelectBoxKind;
use crate::layout;
use crate::render;
use eframe::egui::{self, Rect};
use orc_sdk::{NH, NodeInfo, NodeProperty, OH, OrcHandle, OutputProperty, Workflow};
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;

pub struct EditorState {
    pub workflow: Workflow,
    pub node_positions: NodeProperty<[f32; 2]>,
    pub node_sizes: NodeProperty<[f32; 2]>,
    /// Set on every node that takes part in a cycle. The graph is allowed to hold cycles, so
    /// these are drawn as an error rather than rejected.
    pub node_in_cycle: NodeProperty<bool>,
    pub selected: NodeProperty<bool>,
    /// Node sizes are derived from measured label text, which needs a live frame, so they are
    /// computed on the first frame rather than at construction.
    pub needs_measure: bool,
    /// Whether `layout::compute_layout` has run with real sizes yet. Sizes aren't known until
    /// the first live frame (see `needs_measure`), so the real layout happens exactly once,
    /// the first time `measure` runs. Later calls to `measure` (e.g. to size a newly created
    /// node) must not repeat it — a full re-layout would move every existing node, undoing
    /// anything the user dragged.
    layout_computed: bool,
    /// One entry per distinct declared workflow input, positioned by `layout::compute_layout`
    /// alongside everything else: the input's name, and `[right_edge_x, center_y]` -- the chip's
    /// right edge, not its min corner, since that's the one point that needs to stay stable for
    /// `render.rs` to anchor a link from regardless of how wide the label measures at draw time
    /// (the chip itself is never resized/dragged, so nothing else needs the rect).
    pub(crate) input_chip_positions: Vec<(String, [f32; 2])>,
    /// Canvas to screen transform, driven by pan/zoom input.
    pub view: Transform,
    /// Screen-space rect and direction of an in-progress box-select drag, for rendering the
    /// marquee.
    pub select_box: Option<(Rect, SelectBoxKind)>,
    /// Output pin a wire is currently being dragged from, for rendering the in-progress bezier.
    pub pending_wire: Option<OH>,
    /// The open right-click context menu, if any.
    pub context_menu: Option<ContextMenuState>,
    /// Whether the "Session Info" window is open.
    pub session_info_open: bool,
    /// Path last opened or saved to, if any. `Save` writes here directly; with no path yet it
    /// falls back to `Save As`.
    pub current_path: Option<PathBuf>,
    /// Message from the last failed operation -- file load/save, a rejected connection, a failed
    /// node creation -- shown in a popup until dismissed. Named generically (not `file_error`)
    /// since it long ago grew into the crate's one generic user-facing error channel, not just a
    /// file-I/O one.
    pub last_error: Option<String>,
    /// Whether the workflow has changed since the last save (or since it was opened). Node
    /// positions, selection, pan/zoom etc. don't count — none of that is persisted to disk, so
    /// none of it should mark the file dirty.
    pub dirty: bool,
    /// The OS window title as of the last time it was actually set, so `update_window_title` can
    /// skip `ViewportCommand::Title` on frames where nothing changed — sending it unconditionally
    /// every frame (as plain dragging does, at whatever frame rate the drag repaints at) makes an
    /// OS call for no reason on the overwhelming majority of frames.
    pub(crate) last_window_title: String,
    /// Cached execution result per output pin, `Arc`-wrapped so an in-flight job on another
    /// thread can share a clone without copying the underlying deck — see "Handle lifetime
    /// across threads" in PROJECT.org. The "not yet computed" sentinel is a handle whose
    /// `free_fn` is `None` (the zeroed default), not a specific `handle` id.
    pub computed_outputs: OutputProperty<Arc<OrcHandle>>,
    /// Bumped (not just flagged) whenever a node is directly edited or something upstream of it
    /// is. A node is settled once its cached result reflects its current `dirty_version`.
    pub dirty_version: NodeProperty<u64>,
    /// Set when a node's last execution attempt returned a real (non-cancellation) error.
    /// Cleared the next time it computes successfully.
    pub execution_error: NodeProperty<Option<String>>,
    /// Scheduling bookkeeping private to `exec` (in-flight jobs, the dispatch worklist).
    pub(crate) exec: exec::ExecState,
    /// Cached `deck_to_str` text per Inspect node, refreshed lazily (only when the upstream
    /// value actually changes) rather than reconverted every frame.
    pub(crate) inspect_cache: NodeProperty<inspect::InspectCache>,
    /// Whether an Inspect or Constant node's content is currently popped out into its own OS
    /// window. Set by clicking its pop-out button; cleared once that window's close is observed
    /// (see `inspect::update_popouts`) — there is no other way back to `false`, so a node whose
    /// window the user just closed still shows `true` for one more frame before the check runs.
    pub(crate) content_popout: NodeProperty<bool>,
    /// Per-row edit buffers for a Constant node's editable ruler display, and whether its handle
    /// is actually editable at all (see `const_edit::is_editable`).
    pub(crate) const_edit_cache: NodeProperty<const_edit::ConstEditCache>,
    /// Set by `const_edit::insert_after` for the row the new value landed at; read (and cleared)
    /// by `render::draw_editable_const_content` the next time it draws that row, to steal
    /// keyboard focus onto it -- matches the "press Enter, keep typing on the new row" feel of a
    /// spreadsheet. A `Cell`, not a `NodeProperty`, since it's read from `render.rs` through only
    /// `&EditorState` and needs no per-node storage or garbage collection, just one slot.
    pub(crate) pending_focus_row: Cell<Option<(NH, usize)>>,
    /// Only ever non-empty for an `EditorState` pushed by `nested::open`: the current values that
    /// were feeding the calling `NestedCall` node's own input pins one level down, positionally
    /// matching `Workflow::workflow_input_position`. `exec`'s dispatch reads this to resolve this
    /// workflow's own dangling workflow-input pins, so editing a nested workflow previews live
    /// data instead of nothing -- but nothing here is ever written into the graph itself, so
    /// there's nothing to bake in or revert; reopening a different caller of the same nested
    /// workflow just gathers a different set of values (see `nested.rs`).
    pub(crate) simulated_inputs: Vec<Arc<OrcHandle>>,
}

/// Copies every `NestedCall` node's declared input/output names from its referenced nested
/// workflow onto its own pins, wherever they're missing -- needed for a node loaded from a file
/// authored outside dagger's own context menu (e.g. directly via pyorc), which has no reason to
/// have called `set_input_label`/`set_output_label` itself even though the names the labels
/// would show are already right there in the nested workflow's own declared interface. Run once,
/// right after construction, same cadence as `mark_all_dirty` below.
fn backfill_nested_call_labels(workflow: &mut Workflow) {
    let node_info_prop = workflow.node_info_prop();
    let calls: Vec<(NH, String)> = {
        let Ok(node_infos) = node_info_prop.try_borrow() else {
            return;
        };
        workflow
            .node_iter()
            .filter_map(|nh| match &node_infos[nh] {
                NodeInfo::NestedCall { workflow_name } => Some((nh, workflow_name.clone())),
                _ => None,
            })
            .collect()
    };
    for (nh, name) in calls {
        crate::context_menu::label_nested_call_pins(workflow, nh, &name);
    }
}

impl EditorState {
    pub fn from_workflow(mut workflow: Workflow) -> Self {
        backfill_nested_call_labels(&mut workflow);
        let node_positions = workflow.create_node_property();
        let node_sizes = workflow.create_node_property();
        let node_in_cycle = workflow.create_node_property();
        let selected = workflow.create_node_property();
        let computed_outputs = workflow.create_output_property();
        let dirty_version = workflow.create_node_property();
        let execution_error = workflow.create_node_property();
        let inspect_cache = workflow.create_node_property();
        let content_popout = workflow.create_node_property();
        let const_edit_cache = workflow.create_node_property();
        let exec_state = exec::ExecState::new(&mut workflow);
        let mut state = Self {
            workflow,
            node_positions,
            node_sizes,
            node_in_cycle,
            selected,
            needs_measure: true,
            layout_computed: false,
            input_chip_positions: Vec::new(),
            view: Transform::default(),
            select_box: None,
            pending_wire: None,
            context_menu: None,
            session_info_open: false,
            current_path: None,
            last_error: None,
            dirty: false,
            last_window_title: String::new(),
            computed_outputs,
            dirty_version,
            execution_error,
            exec: exec_state,
            inspect_cache,
            content_popout,
            const_edit_cache,
            pending_focus_row: Cell::new(None),
            simulated_inputs: Vec::new(),
        };
        exec::mark_all_dirty(&mut state);
        state
    }

    /// Re-measure node sizes from the current labels.
    ///
    /// The very first call also computes the whole layout: sizes aren't known until this runs
    /// (measuring text needs a live frame, so it can't happen at construction), and the layout
    /// needs real sizes to space nodes correctly. Later calls — e.g. to size a node just
    /// created by the context menu — only remeasure; they must not repeat the layout, since
    /// that would move every existing node back to a computed position, discarding any drag.
    pub fn measure(&mut self, ctx: &egui::Context) {
        if let Ok(mut sizes) = self.node_sizes.try_borrow_mut() {
            render::measure_nodes(ctx, &self.workflow, &mut sizes);
        }
        self.needs_measure = false;
        if !self.layout_computed {
            layout::compute_layout(self);
            self.layout_computed = true;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use orc_sdk::FuncInfo;

    fn node(wf: &mut Workflow, n_in: usize, n_out: usize) -> NH {
        let mut ins = vec![orc_sdk::IH::default(); n_in];
        let mut outs = vec![OH::default(); n_out];
        wf.add_function(FuncInfo::default(), &mut ins, &mut outs)
            .unwrap()
    }

    /// `measure`'s own doc comment calls out exactly this regression: a later call must only
    /// remeasure sizes, never repeat the layout -- otherwise a node the user just dragged would
    /// snap back to its computed position the next time anything (e.g. a new node created
    /// elsewhere, which sets `needs_measure` again) triggers a remeasure.
    #[test]
    fn t_measure_only_computes_the_layout_once() {
        let mut wf = Workflow::default();
        let a = node(&mut wf, 0, 1);
        let mut state = EditorState::from_workflow(wf);

        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            state.measure(ui.ctx());
        });
        output.drop_without_applying_deltas();
        assert!(state.layout_computed);

        // Simulate a drag, then trigger a second `measure` as if an unrelated edit (e.g. a new
        // node created elsewhere) had just set `needs_measure` again without touching
        // `layout_computed`.
        let dragged_to = [999.0, 888.0];
        state.node_positions.try_borrow_mut().unwrap()[a] = dragged_to;
        state.needs_measure = true;

        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            state.measure(ui.ctx());
        });
        output.drop_without_applying_deltas();

        assert_eq!(
            state.node_positions.try_borrow().unwrap()[a],
            dragged_to,
            "a second measure must not re-run the layout and discard the drag"
        );
    }

    /// Regression test: a `NestedCall` node loaded from a file authored outside dagger's own
    /// context menu (e.g. directly via pyorc) never has `set_input_label`/`set_output_label`
    /// called on its behalf, even though the names are already declared on the nested workflow
    /// it references -- `from_workflow` must backfill them so the pins show real labels instead
    /// of bare circles.
    #[test]
    fn t_from_workflow_backfills_nested_call_pin_labels_for_a_node_loaded_without_them() {
        let mut inner = Workflow::default();
        let mut inner_ins = vec![orc_sdk::IH::default(); 2];
        let mut inner_outs = vec![OH::default(); 1];
        inner
            .add_function(FuncInfo::default(), &mut inner_ins, &mut inner_outs)
            .unwrap();
        inner
            .set_inputs(&[(inner_ins[0], 0, "a"), (inner_ins[1], 1, "b")])
            .unwrap();
        inner
            .set_outputs(&[(inner_outs[0], "sum".to_string())])
            .unwrap();

        let mut outer = Workflow::default();
        outer
            .push_nested_workflow("inner".to_string(), inner, &orc_sdk::PluginSet::default())
            .unwrap();
        let mut call_ins = vec![orc_sdk::IH::default(); 2];
        let mut call_outs = vec![OH::default(); 1];
        let call_nh = outer
            .add_nested_workflow_call("inner", &mut call_ins, &mut call_outs)
            .unwrap();
        // Deliberately never calls set_input_label/set_output_label here, simulating a node
        // that arrived from outside dagger's own node-creation path.

        let state = EditorState::from_workflow(outer);
        let input_labels = state.workflow.input_labels_prop();
        let output_labels = state.workflow.output_labels_prop();
        let synced_ins: Vec<_> = state.workflow.node_inputs(call_nh).collect();
        let synced_outs: Vec<_> = state.workflow.node_outputs(call_nh).collect();
        assert_eq!(input_labels.try_borrow().unwrap()[synced_ins[0]], "a");
        assert_eq!(input_labels.try_borrow().unwrap()[synced_ins[1]], "b");
        assert_eq!(output_labels.try_borrow().unwrap()[synced_outs[0]], "sum");
    }
}
