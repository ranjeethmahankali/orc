//! Editable ruler display for Constant nodes: per-value text boxes laid out with the same ruler
//! prefixes as `Deck`'s own `Display` impl, editability gated on whether the constant's handle
//! lives in the host's own `DeckRegistry` (a plugin-owned type's storage can't be reached this
//! way at all), and append-at-depth-0 to grow the deck's last run without touching its existing
//! structure.

use crate::state::EditorState;
use orc_sdk::{
    NH, NodeInfo, ORC_TYPE_F64, ORC_TYPE_I64, OrcHandle, OrcMark, TypeOwner, update_handle_from_deck,
};

const TAB_WIDTH: usize = 3;

/// What a row in the editable display corresponds to.
pub(crate) enum RowValue {
    /// The index (into the deck's flat `items`) of the value this row edits.
    Item(usize),
    /// A mark whose run has no items at all -- not producible via `Deck::push`/`start_new_arr`
    /// from this app, but a hand-built or imported deck could still contain one, and `Deck`'s own
    /// `Display` renders a bare ruler line for it rather than skipping it.
    EmptyGroup,
}

pub(crate) struct Row {
    /// Static ruler-prefix text, formatted identically to `Deck`'s own `Display` impl (see
    /// `orc_sdk::deck::fmt_raw_deck`) -- a ruler number and brackets for the first item of a new
    /// run, or matching blank indent for a continuation.
    pub(crate) ruler: String,
    pub(crate) value: RowValue,
}

fn ruler_for(dmax: u8, depth: u8) -> String {
    let d_current = depth + 1;
    format!(
        "{:>indent$}{:>3} {:─>bw$}",
        "",
        d_current,
        "┤",
        indent = (dmax - depth) as usize * TAB_WIDTH,
        bw = d_current as usize * TAB_WIDTH,
    )
}

fn continuation_ruler(dmax: u8) -> String {
    format!("{:>indent$}   ┤", "", indent = (dmax as usize + 1) * TAB_WIDTH)
}

/// One row per item (plus one for a run with no items at all), in the same order `Deck`'s own
/// `Display` prints them. A direct port of `fmt_raw_deck`'s walk over `marks`/`items`, just
/// emitting rows for a caller to turn into widgets instead of writing formatted text.
pub(crate) fn deck_rows(n_items: usize, marks: &[OrcMark]) -> Vec<Row> {
    if n_items == 0 && marks.is_empty() {
        return Vec::new();
    }
    let n_items = n_items as u64;
    let dmax = marks.first().map(|m| m.depth + 1).unwrap_or(0);
    let mut rows = Vec::new();

    let push_run = |rows: &mut Vec<Row>, depth: u8, pos: u64, next_pos: u64| {
        if pos < next_pos {
            let end = next_pos.min(n_items);
            let mut iter = pos..end;
            if let Some(i) = iter.next() {
                rows.push(Row {
                    ruler: ruler_for(dmax, depth),
                    value: RowValue::Item(i as usize),
                });
            }
            for i in iter {
                rows.push(Row {
                    ruler: continuation_ruler(dmax),
                    value: RowValue::Item(i as usize),
                });
            }
        } else {
            rows.push(Row {
                ruler: ruler_for(dmax, depth),
                value: RowValue::EmptyGroup,
            });
        }
    };

    let mut tail_start = 0u64;
    for w in marks.windows(2) {
        let (m, next_pos) = (&w[0], w[1].pos);
        push_run(&mut rows, m.depth, m.pos, next_pos);
        tail_start = next_pos;
    }
    if let Some(last) = marks.last() {
        push_run(&mut rows, last.depth, last.pos, n_items);
        tail_start = n_items;
    }
    for i in tail_start..n_items {
        rows.push(Row {
            ruler: continuation_ruler(dmax),
            value: RowValue::Item(i as usize),
        });
    }
    rows
}

/// Whether `handle`'s data lives in the host's own `DeckRegistry` and is a type this editor knows
/// how to parse/format (`f64` or `i64` today). A plugin-owned type's storage lives entirely on
/// the other side of the FFI boundary -- possibly not even Rust -- so there is no `with_mut` to
/// borrow it through at all; those constants stay read-only, same as before this feature existed.
pub(crate) fn is_editable(handle: &OrcHandle) -> bool {
    matches!(
        crate::PLUGIN_SET.get_type_owner(handle.type_id),
        Some(TypeOwner::BuiltIn(_))
    ) && matches!(handle.type_id, ORC_TYPE_F64 | ORC_TYPE_I64)
}

/// The same collapsed-ruler text `Inspect` shows for arbitrary connected data, used for a
/// Constant node whose type isn't editable (e.g. a plugin-owned complex number), and for any
/// Constant's pop-out window, which is always read-only regardless of whether it's editable
/// inline.
pub(crate) fn render_readonly(handle: &OrcHandle) -> String {
    match crate::host_deck_to_str(handle) {
        Ok(str_handle) => {
            let mut text = String::new();
            crate::inspect::render_str_deck(&str_handle, &mut text);
            text
        }
        Err(e) => format!("<error: {e}>"),
    }
}

/// Cached per-row edit buffers for a Constant node, resynced from the deck whenever the item
/// count changes (a fresh node, or this module's own append) -- never on a plain value edit,
/// since that's this module committing exactly what the buffer already holds.
#[derive(Default, Clone)]
pub(crate) struct ConstEditCache {
    pub(crate) editable: bool,
    pub(crate) buffers: Vec<String>,
}

fn format_item(handle: &OrcHandle, index: usize) -> String {
    match handle.type_id {
        ORC_TYPE_I64 => handle.items::<i64>()[index].to_string(),
        _ => handle.items::<f64>()[index].to_string(),
    }
}

/// Resyncs every Constant node's edit-buffer cache. Called once per frame, same as
/// `inspect::refresh_all`; cheap when nothing has changed, since it only rebuilds a node's
/// buffers when its item count no longer matches what's cached.
pub fn refresh_all(state: &mut EditorState) {
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    for nh in nodes {
        refresh(state, nh);
    }
}

fn refresh(state: &mut EditorState, nh: NH) {
    let node_info_prop = state.workflow.node_info_prop();
    let Ok(node_infos) = node_info_prop.try_borrow() else {
        return;
    };
    let NodeInfo::Constant(handle) = &node_infos[nh] else {
        return;
    };
    let editable = is_editable(handle);
    let n_items = handle.n_items as usize;
    let Ok(mut cache) = state.const_edit_cache.try_borrow_mut() else {
        return;
    };
    if cache[nh].buffers.len() == n_items && cache[nh].editable == editable {
        return;
    }
    cache[nh] = ConstEditCache {
        editable,
        buffers: (0..n_items).map(|i| format_item(handle, i)).collect(),
    };
}

/// Parses `state.const_edit_cache[nh].buffers[item_index]` and writes it back into the real
/// deck via `DeckRegistry::with_mut` -- the registry's own lock is what makes this safe, not
/// anything about the handle itself: a dispatched job never shares this memory in the first
/// place, since `exec::dispatch` clones a fresh, independent copy of a constant's handle for
/// every job (see "Handle lifetime across threads" in PROJECT.org), so nothing is ever reading
/// through the same backing storage this mutates. On a parse failure, the buffer is reset back
/// to the value actually stored, rather than silently keeping unparseable text on screen.
pub fn commit_row(state: &mut EditorState, nh: NH, item_index: usize) {
    let mut node_info_prop = state.workflow.node_info_prop();
    let Ok(mut node_infos) = node_info_prop.try_borrow_mut() else {
        return;
    };
    let NodeInfo::Constant(handle) = &mut node_infos[nh] else {
        return;
    };
    if !is_editable(handle) {
        return;
    }
    let Ok(mut cache) = state.const_edit_cache.try_borrow_mut() else {
        return;
    };
    let Some(text) = cache[nh].buffers.get(item_index).cloned() else {
        return;
    };

    let type_id = handle.type_id;
    let parsed = matches!(type_id, ORC_TYPE_I64)
        .then(|| text.trim().parse::<i64>().is_ok())
        .unwrap_or_else(|| text.trim().parse::<f64>().is_ok());
    if !parsed {
        // Revert: put back whatever the deck actually holds, discarding the unparseable text.
        cache[nh].buffers[item_index] = format_item(handle, item_index);
        return;
    }

    let write_result = crate::REGISTRY.with_mut(&[handle.handle], |decks| -> Result<(), orc_sdk::Error> {
        match type_id {
            ORC_TYPE_I64 => {
                let deck = decks[0]
                    .downcast_mut::<orc_sdk::Deck<i64>>()
                    .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
                deck.items_mut()[item_index] = text.trim().parse::<i64>().unwrap();
                unsafe { update_handle_from_deck(deck, &mut *handle) };
            }
            _ => {
                let deck = decks[0]
                    .downcast_mut::<orc_sdk::Deck<f64>>()
                    .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
                deck.items_mut()[item_index] = text.trim().parse::<f64>().unwrap();
                unsafe { update_handle_from_deck(deck, &mut *handle) };
            }
        }
        Ok(())
    });
    if write_result.is_err() {
        return;
    }
    cache[nh].buffers[item_index] = format_item(handle, item_index);
    drop(cache);
    drop(node_infos);
    after_edit(state, nh);
}

/// Appends one more value at depth 0 -- extends the deck's last (currently open) run without
/// creating a new mark, so nothing about the existing structure changes. The new value starts at
/// zero, same as a freshly created constant's default.
pub fn append_value(state: &mut EditorState, nh: NH) {
    let mut node_info_prop = state.workflow.node_info_prop();
    let Ok(mut node_infos) = node_info_prop.try_borrow_mut() else {
        return;
    };
    let NodeInfo::Constant(handle) = &mut node_infos[nh] else {
        return;
    };
    if !is_editable(handle) {
        return;
    }
    let type_id = handle.type_id;
    let write_result = crate::REGISTRY.with_mut(&[handle.handle], |decks| -> Result<(), orc_sdk::Error> {
        match type_id {
            ORC_TYPE_I64 => {
                let deck = decks[0]
                    .downcast_mut::<orc_sdk::Deck<i64>>()
                    .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
                deck.push(0, 0);
                unsafe { update_handle_from_deck(deck, &mut *handle) };
            }
            _ => {
                let deck = decks[0]
                    .downcast_mut::<orc_sdk::Deck<f64>>()
                    .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
                deck.push(0.0, 0);
                unsafe { update_handle_from_deck(deck, &mut *handle) };
            }
        }
        Ok(())
    });
    if write_result.is_err() {
        return;
    }
    drop(node_infos);
    after_edit(state, nh);
    refresh(state, nh);
}

/// What happened this frame in a Constant node's editable rows -- collected while `render.rs`
/// only has `&EditorState` (mid-draw, inside a loop already holding other borrows a commit would
/// conflict with), applied once rendering is done and those borrows are gone.
#[derive(Default)]
pub(crate) struct ConstEditEvents {
    pub(crate) committed_rows: Vec<(NH, usize)>,
    pub(crate) appended: Vec<NH>,
}

pub(crate) fn apply_events(state: &mut EditorState, events: ConstEditEvents) {
    for (nh, item_index) in events.committed_rows {
        commit_row(state, nh, item_index);
    }
    for nh in events.appended {
        append_value(state, nh);
    }
}

/// Common tail for any edit that changed a constant's value: `computed_outputs` is the only
/// place anything else (Inspect, a downstream dispatch's constant-input clone) ever reads a
/// constant's *current* value from, so it must be refreshed immediately, same as node creation
/// already does. Downstream nodes are marked dirty so they actually recompute.
fn after_edit(state: &mut EditorState, nh: NH) {
    let node_info_prop = state.workflow.node_info_prop();
    let (cloned, oh) = {
        let Ok(node_infos) = node_info_prop.try_borrow() else {
            return;
        };
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            return;
        };
        let Some(oh) = state.workflow.node_outputs(nh).next() else {
            return;
        };
        (crate::host_clone_orc_handle(handle.borrowed()), oh)
    };
    if let (Ok(cloned), Ok(mut computed_outputs)) = (cloned, state.computed_outputs.try_borrow_mut())
    {
        computed_outputs[oh] = std::sync::Arc::new(cloned);
    }
    state.dirty = true;
    crate::exec::mark_dirty(state, nh);
}

#[cfg(test)]
mod test {
    use super::*;
    use orc_sdk::Deck;

    /// Reconstructs `Deck`'s own multi-line `Display` output from `deck_rows` alone (ruler +
    /// value per row, joined with newlines), to check the port didn't drift from the original
    /// algorithm it mirrors.
    fn reconstruct<T: std::fmt::Display>(items: &[T], marks: &[OrcMark]) -> String {
        let mut out = String::new();
        for row in deck_rows(items.len(), marks) {
            match row.value {
                RowValue::Item(i) => out.push_str(&format!("{} {}\n", row.ruler, items[i])),
                RowValue::EmptyGroup => out.push_str(&format!("{}\n", row.ruler)),
            }
        }
        out
    }

    #[test]
    fn t_deck_rows_matches_display_for_the_project_org_worked_example() {
        let mut deck = Deck::<i32>::default();
        deck.push(10, 2);
        deck.push(20, 0);
        deck.push(30, 1);
        assert_eq!(reconstruct(deck.items(), deck.marks()), deck.to_string());
    }

    #[test]
    fn t_deck_rows_matches_display_at_three_levels_of_nesting() {
        let mut deck = Deck::<i32>::default();
        deck.push(1, 3);
        deck.push(2, 0);
        deck.push(3, 1);
        deck.push(4, 2);
        assert_eq!(reconstruct(deck.items(), deck.marks()), deck.to_string());
    }

    #[test]
    fn t_deck_rows_matches_display_for_a_flat_list() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        deck.push(3.0, 0);
        assert_eq!(reconstruct(deck.items(), deck.marks()), deck.to_string());
    }

    #[test]
    fn t_deck_rows_matches_display_for_a_single_bare_value() {
        let deck = Deck::<f64>::from_value(7.0);
        assert_eq!(reconstruct(deck.items(), deck.marks()), deck.to_string());
    }

    #[test]
    fn t_deck_rows_empty_deck_is_empty() {
        let deck = Deck::<f64>::default();
        assert!(deck_rows(deck.items().len(), deck.marks()).is_empty());
    }

    #[test]
    fn t_appending_a_depth_zero_value_adds_one_more_row_with_no_new_ruler() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        let before = deck_rows(deck.items().len(), deck.marks()).len();
        deck.push(3.0, 0);
        let rows = deck_rows(deck.items().len(), deck.marks());
        assert_eq!(rows.len(), before + 1);
        assert!(matches!(rows.last().unwrap().value, RowValue::Item(2)));
        // A depth-0 push extends the last run rather than starting a new one, so the new row's
        // ruler must read as a continuation, identical to the row before it.
        assert_eq!(rows[rows.len() - 1].ruler, rows[rows.len() - 2].ruler);
    }

    fn constant_node(deck: Deck<f64>) -> (EditorState, NH) {
        let mut handle = OrcHandle {
            handle: crate::HANDLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        };
        crate::REGISTRY.alloc_with_value(Some(deck), &mut handle).unwrap();
        let mut wf = orc_sdk::Workflow::default();
        let (nh, _oh) = wf.add_constant(handle).unwrap();
        (EditorState::from_workflow(wf), nh)
    }

    #[test]
    fn t_is_editable_true_for_a_registry_backed_f64_handle() {
        let (state, nh) = constant_node(Deck::from_value(3.0));
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert!(is_editable(handle));
    }

    /// A handle whose `type_id` the plugin set doesn't recognize at all (never registered as
    /// either `BuiltIn` or `Plugin`) must not be treated as editable -- this is the same
    /// `type_id` an unrecognized/plugin-owned deck (e.g. a complex number) would carry, and
    /// `is_editable` must refuse it rather than assume the registry can be used just because a
    /// `with_mut` call on it happens not to panic.
    #[test]
    fn t_is_editable_false_for_an_unrecognized_type_id() {
        let handle = OrcHandle {
            type_id: 0xdead_beef,
            ..Default::default()
        };
        assert!(!is_editable(&handle));
    }

    #[test]
    fn t_commit_row_parses_and_writes_a_new_value() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        let (mut state, nh) = constant_node(deck);
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[1] = "42.5".to_string();

        commit_row(&mut state, nh, 1);

        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert_eq!(handle.items::<f64>(), &[1.0, 42.5]);
        assert!(state.dirty, "committing a value must mark the workflow dirty");
    }

    /// Unparseable text must not corrupt the deck -- the buffer reverts to whatever the deck
    /// actually holds instead.
    #[test]
    fn t_commit_row_reverts_the_buffer_on_unparseable_text() {
        let (mut state, nh) = constant_node(Deck::from_value(7.0));
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] = "not a number".to_string();

        commit_row(&mut state, nh, 0);

        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert_eq!(handle.items::<f64>(), &[7.0], "the deck must be unchanged");
        assert_eq!(state.const_edit_cache.try_borrow().unwrap()[nh].buffers[0], "7");
    }

    #[test]
    fn t_append_value_grows_the_deck_without_a_new_mark() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        let (mut state, nh) = constant_node(deck);
        let n_marks_before = {
            let node_info_prop = state.workflow.node_info_prop();
            let node_infos = node_info_prop.try_borrow().unwrap();
            let NodeInfo::Constant(handle) = &node_infos[nh] else {
                panic!("expected a constant node")
            };
            handle.n_marks
        };

        append_value(&mut state, nh);

        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert_eq!(handle.items::<f64>(), &[1.0, 2.0, 0.0]);
        assert_eq!(handle.n_marks, n_marks_before, "appending at depth 0 adds no new mark");
    }

    /// End to end: editing a constant must update `computed_outputs` immediately (the only place
    /// anything else, e.g. an Inspect node downstream, reads a constant's *current* value from)
    /// and bump its own `dirty_version` so anything downstream re-checks readiness.
    #[test]
    fn t_commit_row_refreshes_computed_outputs_and_bumps_dirty_version() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        let oh = state.workflow.node_outputs(nh).next().unwrap();
        let version_before = state.dirty_version.try_borrow().unwrap()[nh];

        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] = "99".to_string();
        commit_row(&mut state, nh, 0);

        let computed = state.computed_outputs.try_borrow().unwrap();
        assert_eq!(computed[oh].items::<f64>(), &[99.0]);
        assert!(state.dirty_version.try_borrow().unwrap()[nh] > version_before);
    }
}
