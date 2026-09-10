use crate::canvas::Transform;
use crate::context_menu::ContextMenuState;
use crate::exec;
use crate::inspect;
use crate::interaction::SelectBoxKind;
use crate::layout;
use crate::render;
use eframe::egui::{self, Rect};
use orc_sdk::{NodeProperty, OH, OrcHandle, OutputProperty, Workflow};
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
    /// Message from the last failed load/save, shown in a popup until dismissed.
    pub file_error: Option<String>,
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
}

impl EditorState {
    pub fn from_workflow(mut workflow: Workflow) -> Self {
        let node_positions = workflow.create_node_property();
        let node_sizes = workflow.create_node_property();
        let node_in_cycle = workflow.create_node_property();
        let selected = workflow.create_node_property();
        let computed_outputs = workflow.create_output_property();
        let dirty_version = workflow.create_node_property();
        let execution_error = workflow.create_node_property();
        let inspect_cache = workflow.create_node_property();
        let exec_state = exec::ExecState::new(&mut workflow);
        let mut state = Self {
            workflow,
            node_positions,
            node_sizes,
            node_in_cycle,
            selected,
            needs_measure: true,
            layout_computed: false,
            view: Transform::default(),
            select_box: None,
            pending_wire: None,
            context_menu: None,
            session_info_open: false,
            current_path: None,
            file_error: None,
            dirty: false,
            last_window_title: String::new(),
            computed_outputs,
            dirty_version,
            execution_error,
            exec: exec_state,
            inspect_cache,
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
