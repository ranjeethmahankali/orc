//! Right-click menu with search field and case-insensitive substring filtering.

use crate::interaction::ContextMenuRequest;
use crate::state::EditorState;
use eframe::egui::{self, Key, Modifiers, Pos2};
use orc_sdk::{Deck, FuncInfo, IH, OH, OrcHandle, PluginSet, Workflow};
use std::sync::atomic::Ordering;

const ADD_CONSTANT: &str = "Add Constant Node";
const ADD_INSPECT: &str = "Add Inspect Node";
const SESSION_INFO: &str = "Session Info...";

/// Persists across frames while the popup is open: where it's anchored, what (if anything) the
/// created node's first input should connect to, the search text and the highlighted entry.
pub struct ContextMenuState {
    screen_pos: Pos2,
    connect_from: Option<OH>,
    query: String,
    selected: usize,
    focus_requested: bool,
}

/// Whether `name` matches the user's typed query -- case-insensitive substring match. `query`
/// is passed already lowercased, since a single `menu_entries` call checks it against every
/// plugin function and nested workflow name; lowercasing it once there instead of once per
/// candidate here is the whole reason it's a parameter instead of computed inline.
fn matches_query(query_lower: &str, name: &str) -> bool {
    query_lower.is_empty() || name.to_lowercase().contains(query_lower)
}

enum MenuAction {
    SessionInfo,
    Constant(Vec<f64>),
    Inspect,
    Function(FuncInfo),
    /// Call an existing nested workflow, by name. There is no UI to author a nested workflow's
    /// own interface in this pass (see PROJECT.org's Phase 5) -- this only wires up a `NestedCall`
    /// node to whatever nested workflows the loaded file already registers.
    NestedCall(String),
}

/// A bare number or a bracketed, comma-separated list of numbers, e.g. `3.14` or `[1, 2, 3]`.
/// Just enough of a literal grammar to make the search field double as a way to author a
/// constant deck, without a full nested-list parser.
fn parse_literal(text: &str) -> Option<Vec<f64>> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let mut values = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            values.push(part.parse::<f64>().ok()?);
        }
        (!values.is_empty()).then_some(values)
    } else {
        text.parse::<f64>().ok().map(|v| vec![v])
    }
}

/// Before anything is typed, the menu doesn't assume the user wants to add a node — it just
/// offers the other context menu actions. Typing anything is equivalent to picking "add a
/// node": the menu switches to showing filtered node candidates instead.
fn menu_entries(query: &str, workflow: &Workflow) -> Vec<(String, MenuAction)> {
    if query.is_empty() {
        return vec![(SESSION_INFO.to_string(), MenuAction::SessionInfo)];
    }

    let query_lower = query.to_lowercase();
    let mut entries = Vec::new();
    match parse_literal(query) {
        Some(values) => entries.push((
            format!("{ADD_CONSTANT}: {values:?}"),
            MenuAction::Constant(values),
        )),
        None if matches_query(&query_lower, ADD_CONSTANT) => {
            entries.push((ADD_CONSTANT.to_string(), MenuAction::Constant(vec![0.0])));
        }
        None => {}
    }
    if matches_query(&query_lower, ADD_INSPECT) {
        entries.push((ADD_INSPECT.to_string(), MenuAction::Inspect));
    }
    let plugin_set: &PluginSet = &crate::PLUGIN_SET;
    for plugin in plugin_set.plugins() {
        for func in plugin.functions() {
            if matches_query(&query_lower, &func.name) {
                entries.push((func.name.clone(), MenuAction::Function(func.clone())));
            }
        }
    }
    for name in workflow.nested_workflow_names() {
        if matches_query(&query_lower, name) {
            entries.push((
                format!("{name} (nested)"),
                MenuAction::NestedCall(name.to_string()),
            ));
        }
    }
    entries
}

/// Open (or re-anchor) the context menu in response to a right-click detected this frame.
pub fn open(state: &mut EditorState, request: ContextMenuRequest) {
    let (screen_pos, connect_from) = match request {
        ContextMenuRequest::Empty(pos) => (pos, None),
        ContextMenuRequest::FromOutput(oh, pos) => (pos, Some(oh)),
    };
    state.context_menu = Some(ContextMenuState {
        screen_pos,
        connect_from,
        query: String::new(),
        selected: 0,
        focus_requested: false,
    });
}

fn finish_node_creation(state: &mut EditorState, nh: orc_sdk::NH, screen_pos: Pos2) {
    let canvas_pos = state.view.screen_to_canvas(screen_pos);
    if let Ok(mut positions) = state.node_positions.try_borrow_mut() {
        positions[nh] = [canvas_pos.x, canvas_pos.y];
    }
    // Only needs its own size measured — the new node stays exactly where it was placed, and
    // `measure` only recomputes the whole layout once, the very first time it ever runs.
    state.needs_measure = true;
    state.dirty = true;
}

fn create_function_node(
    state: &mut EditorState,
    info: FuncInfo,
    screen_pos: Pos2,
    connect_from: Option<OH>,
) {
    let n_in = info.n_inputs.unwrap_or(2);
    let n_out = info.n_outputs.unwrap_or(2);
    let mut inputs = vec![IH::default(); n_in];
    let mut outputs = vec![OH::default(); n_out];
    let nh = match state.workflow.add_function(info, &mut inputs, &mut outputs) {
        Ok(nh) => nh,
        Err(e) => {
            state.last_error = Some(format!("Failed to create node: {e}"));
            return;
        }
    };
    if let (Some(from), Some(&first)) = (connect_from, inputs.first()) {
        crate::interaction::connect_pins(state, from, first);
    }
    crate::exec::mark_dirty(state, nh);
    finish_node_creation(state, nh, screen_pos);
}

fn create_inspect_node(
    state: &mut EditorState,
    label: String,
    screen_pos: Pos2,
    connect_from: Option<OH>,
) {
    let mut inputs = vec![IH::default(); 1];
    let nh = match state.workflow.add_inspect_node(label, &mut inputs) {
        Ok(nh) => nh,
        Err(e) => {
            state.last_error = Some(format!("Failed to create node: {e}"));
            return;
        }
    };
    if let Some(from) = connect_from {
        crate::interaction::connect_pins(state, from, inputs[0]);
    }
    // Nothing to compute for an Inspect node itself, but it still needs to enter `pending` so
    // `exec::settle_trivially` actually runs for it -- otherwise it never explicitly settles,
    // relying only on `dirty_version`/`computed_version` coincidentally sharing a default of 0
    // (see review notes on this being harmless today only by chance).
    crate::exec::mark_dirty(state, nh);
    finish_node_creation(state, nh, screen_pos);
}

fn create_constant_node(state: &mut EditorState, values: &[f64], screen_pos: Pos2) {
    let mut handle = OrcHandle {
        handle: crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed),
        ..Default::default()
    };
    let mut deck = Deck::<f64>::default();
    for (i, &v) in values.iter().enumerate() {
        deck.push(v, if values.len() > 1 && i == 0 { 1 } else { 0 });
    }
    if let Err(e) = crate::REGISTRY.alloc_with_value(Some(deck), &mut handle) {
        state.last_error = Some(format!("Failed to allocate constant: {e}"));
        return;
    }
    // A constant's own value never goes through `exec`'s dispatch/settle machinery (nothing
    // ever computes it), so nothing else would ever populate `computed_outputs` for it --
    // without this, anything reading a constant directly (e.g. an Inspect node) would see the
    // "not yet computed" sentinel forever. A cloned copy is stored here immediately, matching
    // the clone `exec::dispatch` already makes when a constant feeds a function's input.
    let cloned = crate::host_clone_orc_handle(handle.borrowed());
    let (nh, oh) = match state.workflow.add_constant(handle) {
        Ok(pair) => pair,
        Err(e) => {
            state.last_error = Some(format!("Failed to create node: {e}"));
            return;
        }
    };
    if let (Ok(cloned), Ok(mut computed_outputs)) =
        (cloned, state.computed_outputs.try_borrow_mut())
    {
        computed_outputs[oh] = std::sync::Arc::new(cloned);
    }
    finish_node_creation(state, nh, screen_pos);
}

/// Creates a `NestedCall` node referencing an existing nested workflow, sized and labeled to
/// match its declared interface -- `add_nested_workflow_call` itself just takes a pin count, so
/// the arity and pin labels have to come from whatever `name` currently declares via
/// `set_inputs`/`set_outputs`. Reading that requires a `take`/`put` round trip through
/// `Workflow::take_nested_workflow` (there's no borrowing accessor -- see PROJECT.org), which is
/// fine here: it's two `BTreeMap` operations, not a clone of the nested workflow's contents, and
/// this only runs once per node creation, not on any hot path.
/// Copies `name`'s declared input/output names onto `nh`'s own pins in `workflow`, so a
/// `NestedCall` node shows real labels instead of bare circles. `nh` must already have exactly as
/// many pins as `name` currently declares -- true right after `add_nested_workflow_call`, and
/// true for a node loaded from disk too, since the pin count is part of the saved graph. A no-op
/// if `name` is currently checked out for editing elsewhere (nothing to read labels from until
/// it's closed) -- there is no UI to rename a nested workflow's own declared inputs once set, so
/// this can never actually drift once it's synced.
pub(crate) fn label_nested_call_pins(workflow: &mut Workflow, nh: orc_sdk::NH, name: &str) {
    let Some(nested) = workflow.take_nested_workflow(name) else {
        return;
    };
    let in_names = nested.input_names().to_vec();
    let out_names: Vec<String> = nested
        .workflow_outputs()
        .iter()
        .map(|(_, n)| n.clone())
        .collect();
    workflow.put_nested_workflow(name.to_string(), nested);

    let inputs: Vec<IH> = workflow.node_inputs(nh).collect();
    let outputs: Vec<OH> = workflow.node_outputs(nh).collect();
    for (ih, label) in inputs.into_iter().zip(in_names) {
        let _ = workflow.set_input_label(ih, label);
    }
    for (oh, label) in outputs.into_iter().zip(out_names) {
        let _ = workflow.set_output_label(oh, label);
    }
}

fn create_nested_call_node(
    state: &mut EditorState,
    name: &str,
    screen_pos: Pos2,
    connect_from: Option<OH>,
) {
    let Some(nested) = state.workflow.take_nested_workflow(name) else {
        // Only reachable if the name was deleted or checked out for editing between the menu
        // listing it and the user selecting it -- nothing sensible to create in that case.
        return;
    };
    let n_inputs = nested.input_names().len();
    let n_outputs = nested.workflow_outputs().len();
    state.workflow.put_nested_workflow(name.to_string(), nested);

    let mut inputs = vec![IH::default(); n_inputs];
    let mut outputs = vec![OH::default(); n_outputs];
    let nh = match state
        .workflow
        .add_nested_workflow_call(name, &mut inputs, &mut outputs)
    {
        Ok(nh) => nh,
        Err(e) => {
            state.last_error = Some(format!("Failed to create node: {e}"));
            return;
        }
    };
    label_nested_call_pins(&mut state.workflow, nh, name);
    if let (Some(from), Some(&first)) = (connect_from, inputs.first()) {
        crate::interaction::connect_pins(state, from, first);
    }
    crate::exec::mark_dirty(state, nh);
    finish_node_creation(state, nh, screen_pos);
}

/// Draw the popup and act on whatever the user selects. Escape and clicking outside close it
/// via egui's own popup close behavior; selecting an entry creates the node and closes it here.
pub fn update(ui: &mut egui::Ui, state: &mut EditorState) {
    let Some(mut menu) = state.context_menu.take() else {
        return;
    };

    // Consumed before the text field is drawn below, otherwise egui's own focus navigation and
    // the field itself would swallow these instead of us seeing them. Wraps against last
    // frame's entry count; the field's own edit (if any) happens after, so the count used here
    // is one frame stale, same as `menu.selected` already is.
    let prev_count = menu_entries(&menu.query, &state.workflow).len();
    let (up, down, enter) = ui.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if prev_count > 0 {
        if down {
            menu.selected = (menu.selected + 1) % prev_count;
        }
        if up {
            menu.selected = (menu.selected + prev_count - 1) % prev_count;
        }
    }

    let mut open = true;
    let mut activate = enter;
    let screen_pos = menu.screen_pos;

    // Computed once inside the popup closure below (after this frame's edit to menu.query, if
    // any) and reused for both rendering and activation — `menu_entries` does a full plugin scan
    // with per-candidate allocations, so recomputing it a second time for the exact same query
    // on the same frame is pure waste.
    let mut entries: Vec<(String, MenuAction)> = Vec::new();

    egui::Popup::new(
        egui::Id::new("dagger-context-menu"),
        ui.ctx().clone(),
        screen_pos,
        ui.layer_id(),
    )
    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
    .open_bool(&mut open)
    .width(220.0)
    // Justified so each entry's selectable label stretches to the full row width instead of
    // just the bounds of its text — a much bigger, easier target for the mouse.
    .layout(egui::Layout::top_down_justified(egui::Align::Min))
    .show(|ui| {
        let response =
            ui.add(egui::TextEdit::singleline(&mut menu.query).hint_text("type to add node..."));
        if !menu.focus_requested {
            response.request_focus();
            menu.focus_requested = true;
        }
        if response.changed() {
            menu.selected = 0;
        }

        ui.separator();
        entries = menu_entries(&menu.query, &state.workflow);
        if !entries.is_empty() {
            menu.selected = menu.selected.min(entries.len() - 1);
        }
        // Before typing, these are just plain actions — none of them is "the" default choice,
        // so none is drawn as pre-selected. Once in add-node mode, the highlight tracks arrow
        // key navigation as usual.
        let show_highlight = !menu.query.is_empty();
        for (i, (label, _)) in entries.iter().enumerate() {
            let selected = show_highlight && menu.selected == i;
            if ui.selectable_label(selected, label).clicked() {
                menu.selected = i;
                activate = true;
            }
        }
    });

    if activate {
        if let Some((_, action)) = entries.into_iter().nth(menu.selected) {
            let connect_from = menu.connect_from;
            match action {
                MenuAction::SessionInfo => state.session_info_open = true,
                MenuAction::Constant(values) => create_constant_node(state, &values, screen_pos),
                MenuAction::Inspect => {
                    // The search field is a node-name filter, not a label prompt — typing "a"
                    // to match "Add Inspect Node" must not make that the node's title. There is
                    // no rename UI in any phase, so this is the node's title for good.
                    create_inspect_node(state, "inspect".to_string(), screen_pos, connect_from);
                }
                MenuAction::Function(info) => {
                    create_function_node(state, info, screen_pos, connect_from);
                }
                MenuAction::NestedCall(name) => {
                    create_nested_call_node(state, &name, screen_pos, connect_from);
                }
            }
        }
        return;
    }

    if open {
        state.context_menu = Some(menu);
    }
}

/// Plugins, their functions and their registered types, opened from the context menu's
/// "Session Info..." entry.
pub fn session_info_window(ctx: &egui::Context, open: &mut bool) {
    egui::Window::new("Session Info")
        .open(open)
        .show(ctx, |ui| {
            let plugin_set: &PluginSet = &crate::PLUGIN_SET;
            for plugin in plugin_set.plugins() {
                ui.collapsing(plugin.name(), |ui| {
                    ui.label("Functions:");
                    for f in plugin.functions() {
                        let arity =
                            |n: Option<usize>| n.map_or("variadic".to_string(), |n| n.to_string());
                        ui.label(format!(
                            "{} ({} in, {} out) — {}",
                            f.name,
                            arity(f.n_inputs),
                            arity(f.n_outputs),
                            f.desc
                        ));
                    }
                    ui.label("Types:");
                    for t in plugin.types() {
                        ui.label(format!("{} — {}", t.name, t.desc));
                    }
                });
            }
        });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn t_parses_a_bare_number() {
        assert_eq!(parse_literal("3.123"), Some(vec![3.123]));
        assert_eq!(parse_literal("  -2  "), Some(vec![-2.0]));
    }

    #[test]
    fn t_parses_a_bracketed_list() {
        assert_eq!(parse_literal("[1, 2, 3]"), Some(vec![1.0, 2.0, 3.0]));
    }

    #[test]
    fn t_rejects_non_numeric_text() {
        assert_eq!(parse_literal("add"), None);
        assert_eq!(parse_literal(""), None);
    }

    #[test]
    fn t_rejects_an_empty_list() {
        assert_eq!(parse_literal("[]"), None);
        assert_eq!(parse_literal("[ , ]"), None);
    }

    #[test]
    fn t_rejects_a_list_with_a_non_numeric_element() {
        assert_eq!(parse_literal("[1, x, 3]"), None);
    }

    #[test]
    fn t_creating_a_node_marks_the_workflow_dirty() {
        let mut state = EditorState::from_workflow(orc_sdk::Workflow::default());
        assert!(!state.dirty);
        create_constant_node(&mut state, &[1.0], Pos2::ZERO);
        assert!(state.dirty, "adding a node changes the saved file");
    }

    #[test]
    fn t_creating_an_inspect_node_marks_it_dirty_for_execution_too() {
        // Regression test: `create_inspect_node` used to skip `exec::mark_dirty` entirely,
        // relying on an Inspect node's `dirty_version`/`computed_version` coincidentally both
        // starting at 0 to read as "settled" without ever actually going through the scheduler.
        // Calling `mark_dirty` bumps `dirty_version` past that shared default, so the node is
        // genuinely unsettled until a tick's `dispatch_ready` -> `settle_trivially` catches it up
        // -- pinning the real path instead of the coincidence.
        let mut state = EditorState::from_workflow(orc_sdk::Workflow::default());
        create_inspect_node(&mut state, "inspect".to_string(), Pos2::ZERO, None);
        let nh = state.workflow.node_iter().next().unwrap();
        assert!(
            !crate::exec::is_settled(&state, nh),
            "unsettled until a tick catches it up"
        );
        crate::exec::tick(&mut state);
        assert!(crate::exec::is_settled(&state, nh));
    }

    #[test]
    fn t_menu_entries_before_typing_only_offers_session_info() {
        let wf = orc_sdk::Workflow::default();
        let entries = menu_entries("", &wf);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, SESSION_INFO);
    }

    #[test]
    fn t_menu_entries_filters_case_insensitively() {
        let wf = orc_sdk::Workflow::default();
        assert!(
            menu_entries("inspect", &wf)
                .iter()
                .any(|(label, _)| label == ADD_INSPECT)
        );
        assert!(
            menu_entries("INSPECT", &wf)
                .iter()
                .any(|(label, _)| label == ADD_INSPECT)
        );
    }

    #[test]
    fn t_menu_entries_lists_a_nested_workflow_with_a_suffix() {
        let mut wf = orc_sdk::Workflow::default();
        wf.push_nested_workflow(
            "inner".to_string(),
            orc_sdk::Workflow::default(),
            &PluginSet::default(),
        )
        .unwrap();
        let entries = menu_entries("inner", &wf);
        assert!(
            entries.iter().any(|(label, _)| label == "inner (nested)"),
            "got: {:?}",
            entries.iter().map(|(l, _)| l).collect::<Vec<_>>()
        );
    }

    #[test]
    fn t_menu_entries_a_bare_number_only_offers_add_constant() {
        let wf = orc_sdk::Workflow::default();
        let entries = menu_entries("3.14", &wf);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].0.starts_with(ADD_CONSTANT));
    }

    /// Regression guard for the fan-in/fan-out-index-sensitive part of node creation: a
    /// mismatched zip between the nested workflow's declared pin names and the new node's actual
    /// pins would silently mislabel every pin, with nothing else in the crate to catch it.
    #[test]
    fn t_create_nested_call_node_copies_the_nested_workflows_pin_labels() {
        let mut state = EditorState::from_workflow(orc_sdk::Workflow::default());
        let mut inner = orc_sdk::Workflow::default();
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 1];
        inner
            .add_function(FuncInfo::default(), &mut ins, &mut outs)
            .unwrap();
        inner
            .set_inputs(&[(ins[0], 0, "a"), (ins[1], 1, "b")])
            .unwrap();
        inner.set_outputs(&[(outs[0], "sum".to_string())]).unwrap();
        state
            .workflow
            .push_nested_workflow("inner".to_string(), inner, &PluginSet::default())
            .unwrap();

        create_nested_call_node(&mut state, "inner", Pos2::ZERO, None);

        let call_nh = state.workflow.node_iter().next().unwrap();
        let call_ins: Vec<IH> = state.workflow.node_inputs(call_nh).collect();
        let call_outs: Vec<OH> = state.workflow.node_outputs(call_nh).collect();
        let input_labels = state.workflow.input_labels_prop();
        let output_labels = state.workflow.output_labels_prop();
        assert_eq!(input_labels.try_borrow().unwrap()[call_ins[0]], "a");
        assert_eq!(input_labels.try_borrow().unwrap()[call_ins[1]], "b");
        assert_eq!(output_labels.try_borrow().unwrap()[call_outs[0]], "sum");
    }
}
