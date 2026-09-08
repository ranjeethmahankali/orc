//! Right-click menu with search field, pluggable FunctionFilter trait.

use crate::interaction::ContextMenuRequest;
use crate::state::EditorState;
use eframe::egui::{self, Key, Modifiers, Pos2};
use orc_sdk::{Deck, FuncInfo, IH, OH, OrcHandle, PluginSet};
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

/// Filters the menu's entries as the user types. Case-insensitive substring match today;
/// swappable later for fuzzy/Levenshtein matching without touching the menu itself.
pub trait FunctionFilter {
    fn matches(&self, query: &str, name: &str) -> bool;
}

pub struct SubstringFilter;

impl FunctionFilter for SubstringFilter {
    fn matches(&self, query: &str, name: &str) -> bool {
        query.is_empty() || name.to_lowercase().contains(&query.to_lowercase())
    }
}

enum MenuAction {
    SessionInfo,
    Constant(Vec<f64>),
    Inspect,
    Function(FuncInfo),
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
fn menu_entries(query: &str) -> Vec<(String, MenuAction)> {
    if query.is_empty() {
        return vec![(SESSION_INFO.to_string(), MenuAction::SessionInfo)];
    }

    let filter = SubstringFilter;
    let mut entries = Vec::new();
    match parse_literal(query) {
        Some(values) => entries.push((
            format!("{ADD_CONSTANT}: {values:?}"),
            MenuAction::Constant(values),
        )),
        None if filter.matches(query, ADD_CONSTANT) => {
            entries.push((ADD_CONSTANT.to_string(), MenuAction::Constant(vec![0.0])));
        }
        None => {}
    }
    if filter.matches(query, ADD_INSPECT) {
        entries.push((ADD_INSPECT.to_string(), MenuAction::Inspect));
    }
    let plugin_set: &PluginSet = &crate::PLUGIN_SET;
    for plugin in plugin_set.plugins() {
        for func in plugin.functions() {
            if filter.matches(query, &func.name) {
                entries.push((func.name.clone(), MenuAction::Function(func.clone())));
            }
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
    state.needs_measure = true;
    state.layout_converged = false;
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
    let Ok(nh) = state.workflow.add_function(info, &mut inputs, &mut outputs) else {
        return;
    };
    if let (Some(from), Some(&first)) = (connect_from, inputs.first()) {
        let _ = state.workflow.connect(from, first);
    }
    finish_node_creation(state, nh, screen_pos);
}

fn create_inspect_node(
    state: &mut EditorState,
    label: String,
    screen_pos: Pos2,
    connect_from: Option<OH>,
) {
    let mut inputs = vec![IH::default(); 1];
    let Ok(nh) = state.workflow.add_inspect_node(label, &mut inputs) else {
        return;
    };
    if let Some(from) = connect_from {
        let _ = state.workflow.connect(from, inputs[0]);
    }
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
    if crate::REGISTRY
        .alloc_with_value(Some(deck), &mut handle)
        .is_err()
    {
        return;
    }
    let Ok((nh, _oh)) = state.workflow.add_constant(handle) else {
        return;
    };
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
    let prev_count = menu_entries(&menu.query).len();
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
        let entries = menu_entries(&menu.query);
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
        let entries = menu_entries(&menu.query);
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
        assert_eq!(parse_literal("3.14"), Some(vec![3.14]));
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
}
