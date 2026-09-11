//! Editable ruler display for Constant nodes: per-value text boxes laid out with the same ruler
//! prefixes as `Deck`'s own `Display` impl, editability gated on whether the constant's handle
//! lives in the host's own `DeckRegistry` (a plugin-owned type's storage can't be reached this
//! way at all), and append-at-depth-0 to grow the deck's last run without touching its existing
//! structure.

use crate::state::EditorState;
use orc_sdk::{
    NH, NodeInfo, ORC_TYPE_F64, ORC_TYPE_I64, OrcHandle, OrcMark, TypeOwner,
    update_handle_from_deck,
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
    let d_current = depth.saturating_add(1);
    format!(
        "{:>indent$}{:>3} {:─>bw$}",
        "",
        d_current,
        "┤",
        indent = dmax.saturating_sub(depth) as usize * TAB_WIDTH,
        bw = d_current as usize * TAB_WIDTH,
    )
}

fn continuation_ruler(dmax: u8) -> String {
    format!(
        "{:>indent$}   ┤",
        "",
        indent = (dmax as usize + 1) * TAB_WIDTH
    )
}

/// One "event" produced while walking a deck's structure: either a run (a mark, paired with
/// wherever the next one starts, or `n_items` for the last), or an item that comes after every
/// mark has been walked (there's no run covering it -- `Deck` only ever gets a mark when
/// `start_new_arr`/an explicit push depth introduces one).
enum RunEvent {
    Run { depth: u8, pos: u64, next_pos: u64 },
    TailItem(u64),
}

/// Walks `marks` (plus whatever plain items trail the last one) as a sequence of `RunEvent`s, in
/// the same order `Deck`'s own `Display` prints them. Shared by every function here that needs to
/// walk a deck's structure this way: `deck_rows` (emitting display rows), and
/// `rebuild_with_insertion`/`rebuild_with_deletion` (replaying into a fresh deck) -- all three
/// used to independently re-derive this exact traversal.
fn walk_runs(n_items: u64, marks: &[OrcMark], mut on_event: impl FnMut(RunEvent)) {
    let mut tail_start = 0u64;
    for w in marks.windows(2) {
        on_event(RunEvent::Run {
            depth: w[0].depth,
            pos: w[0].pos,
            next_pos: w[1].pos,
        });
        tail_start = w[1].pos;
    }
    if let Some(last) = marks.last() {
        on_event(RunEvent::Run {
            depth: last.depth,
            pos: last.pos,
            next_pos: n_items,
        });
        tail_start = n_items;
    }
    for i in tail_start..n_items {
        on_event(RunEvent::TailItem(i));
    }
}

/// One row per item (plus one for a run with no items at all), in the same order `Deck`'s own
/// `Display` prints them. A direct port of `fmt_raw_deck`'s walk over `marks`/`items`, just
/// emitting rows for a caller to turn into widgets instead of writing formatted text.
pub(crate) fn deck_rows(n_items: usize, marks: &[OrcMark]) -> Vec<Row> {
    if n_items == 0 && marks.is_empty() {
        return Vec::new();
    }
    let n_items = n_items as u64;
    let dmax = marks
        .first()
        .map(|m| m.depth.saturating_add(1))
        .unwrap_or(0);
    let mut rows = Vec::new();

    walk_runs(n_items, marks, |event| match event {
        RunEvent::Run {
            depth,
            pos,
            next_pos,
        } => {
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
        }
        RunEvent::TailItem(i) => rows.push(Row {
            ruler: continuation_ruler(dmax),
            value: RowValue::Item(i as usize),
        }),
    });
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
    let write_result = if type_id == ORC_TYPE_I64 {
        let Ok(value) = text.trim().parse::<i64>() else {
            cache[nh].buffers[item_index] = format_item(handle, item_index);
            return;
        };
        crate::REGISTRY.with_mut(&[handle.handle], |decks| -> Result<(), orc_sdk::Error> {
            let deck = decks[0]
                .downcast_mut::<orc_sdk::Deck<i64>>()
                .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
            deck.items_mut()[item_index] = value;
            unsafe { update_handle_from_deck(deck, &mut *handle) };
            Ok(())
        })
    } else {
        let Ok(value) = text.trim().parse::<f64>() else {
            cache[nh].buffers[item_index] = format_item(handle, item_index);
            return;
        };
        crate::REGISTRY.with_mut(&[handle.handle], |decks| -> Result<(), orc_sdk::Error> {
            let deck = decks[0]
                .downcast_mut::<orc_sdk::Deck<f64>>()
                .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
            deck.items_mut()[item_index] = value;
            unsafe { update_handle_from_deck(deck, &mut *handle) };
            Ok(())
        })
    };
    // Whether the write succeeded or the registry rejected it (unreachable in practice --
    // `is_editable` already gated the type against what's actually stored), the buffer always
    // ends up showing whatever the deck actually holds now, never leftover unsynced text.
    cache[nh].buffers[item_index] = format_item(handle, item_index);
    if write_result.is_err() {
        return;
    }
    drop(cache);
    drop(node_infos);
    after_edit(state, nh);
}

/// Appends one more value at depth 0 -- extends the deck's last (currently open) run without
/// creating a new mark for it (it's a plain continuation, depth 0), so nothing about the
/// existing structure changes except the one new slot. Works by replaying every original item's
/// exact external push depth into a fresh deck (recovering that depth is exactly what
/// `deck_rows`' ruler math already does, just used here to drive `push`/`start_new_arr` instead
/// of a ruler string), splicing the new value in right after `after_index` -- which also
/// correctly preserves an empty group's bare mark, unlike naively shifting mark positions by
/// hand would if not done carefully.
fn rebuild_with_insertion<T: Copy + Default>(
    items: &[T],
    marks: &[OrcMark],
    after_index: usize,
    new_value: T,
) -> orc_sdk::Deck<T> {
    let mut new_deck = orc_sdk::Deck::<T>::default();
    let n_items = items.len() as u64;

    let push_item = |new_deck: &mut orc_sdk::Deck<T>, i: u64| {
        new_deck.push(items[i as usize], 0);
        if i as usize == after_index {
            new_deck.push(new_value, 0);
        }
    };
    walk_runs(n_items, marks, |event| match event {
        RunEvent::Run {
            depth,
            pos,
            next_pos,
        } => {
            new_deck.start_new_arr(depth.saturating_add(1));
            for i in pos..next_pos.min(n_items) {
                push_item(&mut new_deck, i);
            }
        }
        RunEvent::TailItem(i) => push_item(&mut new_deck, i),
    });
    new_deck
}

/// Inserts one new zero-valued depth-0 row immediately after `after_index` -- the "hit Enter to
/// add a row" gesture, wherever in the list Enter was pressed, not just at the end.
pub fn insert_after(state: &mut EditorState, nh: NH, after_index: usize) {
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
    // Copied out (constants are small) rather than borrowed, since `alloc_with_value` below
    // needs `&mut handle` while these would otherwise still be borrowing from it.
    let marks = handle.marks().to_vec();
    let alloc_result = if type_id == ORC_TYPE_I64 {
        let items = handle.items::<i64>().to_vec();
        let new_deck = rebuild_with_insertion(&items, &marks, after_index, 0i64);
        crate::REGISTRY.alloc_with_value(Some(new_deck), handle)
    } else {
        let items = handle.items::<f64>().to_vec();
        let new_deck = rebuild_with_insertion(&items, &marks, after_index, 0.0f64);
        crate::REGISTRY.alloc_with_value(Some(new_deck), handle)
    };
    if alloc_result.is_err() {
        return;
    }
    drop(node_infos);
    state.pending_focus_row.set(Some((nh, after_index + 1)));
    after_edit(state, nh);
    refresh(state, nh);
}

/// Same replay strategy as `rebuild_with_insertion`, just omitting `delete_index` instead of
/// splicing a value in. `start_new_arr` for a run still runs unconditionally before that run's
/// items are replayed, so deleting the only item in a run leaves behind a legitimate empty-group
/// mark (the same bare-ruler-line state `deck_rows`/`Display` already know how to show) rather
/// than silently discarding the group along with its one item.
fn rebuild_with_deletion<T: Copy + Default>(
    items: &[T],
    marks: &[OrcMark],
    delete_index: usize,
) -> orc_sdk::Deck<T> {
    let mut new_deck = orc_sdk::Deck::<T>::default();
    let n_items = items.len() as u64;

    let push_item = |new_deck: &mut orc_sdk::Deck<T>, i: u64| {
        if i as usize != delete_index {
            new_deck.push(items[i as usize], 0);
        }
    };
    walk_runs(n_items, marks, |event| match event {
        RunEvent::Run {
            depth,
            pos,
            next_pos,
        } => {
            new_deck.start_new_arr(depth.saturating_add(1));
            for i in pos..next_pos.min(n_items) {
                push_item(&mut new_deck, i);
            }
        }
        RunEvent::TailItem(i) => push_item(&mut new_deck, i),
    });
    new_deck
}

/// Removes the value at `delete_index` -- backspacing an already-empty row's gesture for
/// deleting it outright. Focus moves to the previous row, if there is one, matching how deleting
/// an empty line merges you back into the one above it in most text/list editors.
pub fn delete_row(state: &mut EditorState, nh: NH, delete_index: usize) {
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
    let marks = handle.marks().to_vec();
    let alloc_result = if type_id == ORC_TYPE_I64 {
        let items = handle.items::<i64>().to_vec();
        let new_deck = rebuild_with_deletion(&items, &marks, delete_index);
        crate::REGISTRY.alloc_with_value(Some(new_deck), handle)
    } else {
        let items = handle.items::<f64>().to_vec();
        let new_deck = rebuild_with_deletion(&items, &marks, delete_index);
        crate::REGISTRY.alloc_with_value(Some(new_deck), handle)
    };
    if alloc_result.is_err() {
        return;
    }
    drop(node_infos);
    if delete_index > 0 {
        state.pending_focus_row.set(Some((nh, delete_index - 1)));
    }
    after_edit(state, nh);
    refresh(state, nh);
}

/// What happened this frame in a Constant node's editable rows -- collected while `render.rs`
/// only has `&EditorState` (mid-draw, inside a loop already holding other borrows a commit would
/// conflict with), applied once rendering is done and those borrows are gone.
#[derive(Default)]
pub(crate) struct ConstEditEvents {
    pub(crate) committed_rows: Vec<(NH, usize)>,
    /// `(node, index)`: insert a new row right after this item, requested by pressing Enter
    /// while editing it.
    pub(crate) inserted_after: Vec<(NH, usize)>,
    /// `(node, index)`: delete this row outright, requested by pressing Backspace while it was
    /// already empty.
    pub(crate) deleted_rows: Vec<(NH, usize)>,
}

pub(crate) fn apply_events(state: &mut EditorState, events: ConstEditEvents) {
    // A row that both changed text and had Enter pressed in it must commit that text before the
    // insertion shifts indices out from under it. A row that triggers a deletion is always empty
    // (nothing meaningful to commit), so ordering against `committed_rows` doesn't matter there.
    for (nh, item_index) in events.committed_rows {
        commit_row(state, nh, item_index);
    }
    for (nh, delete_index) in events.deleted_rows {
        delete_row(state, nh, delete_index);
    }
    for (nh, after_index) in events.inserted_after {
        insert_after(state, nh, after_index);
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
    if let (Ok(cloned), Ok(mut computed_outputs)) =
        (cloned, state.computed_outputs.try_borrow_mut())
    {
        computed_outputs[oh] = std::sync::Arc::new(cloned);
    }
    crate::exec::mark_edited(state, nh);
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
        crate::REGISTRY
            .alloc_with_value(Some(deck), &mut handle)
            .unwrap();
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
        assert!(
            state.dirty,
            "committing a value must mark the workflow dirty"
        );
    }

    /// Unparseable text must not corrupt the deck -- the buffer reverts to whatever the deck
    /// actually holds instead.
    #[test]
    fn t_commit_row_reverts_the_buffer_on_unparseable_text() {
        let (mut state, nh) = constant_node(Deck::from_value(7.0));
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] =
            "not a number".to_string();

        commit_row(&mut state, nh, 0);

        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert_eq!(handle.items::<f64>(), &[7.0], "the deck must be unchanged");
        assert_eq!(
            state.const_edit_cache.try_borrow().unwrap()[nh].buffers[0],
            "7"
        );
    }

    fn constant_node_i64(deck: Deck<i64>) -> (EditorState, NH) {
        let mut handle = OrcHandle {
            handle: crate::HANDLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        };
        crate::REGISTRY
            .alloc_with_value(Some(deck), &mut handle)
            .unwrap();
        let mut wf = orc_sdk::Workflow::default();
        let (nh, _oh) = wf.add_constant(handle).unwrap();
        (EditorState::from_workflow(wf), nh)
    }

    fn items_of_i64(state: &EditorState, nh: NH) -> Vec<i64> {
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        handle.items::<i64>().to_vec()
    }

    /// The `ORC_TYPE_I64` branch is a distinct code path in `commit_row`/`insert_after`/
    /// `delete_row` from the `f64` one every other test here exercises -- these three guard it
    /// directly rather than trusting it's a faithful mirror of the float branch.
    #[test]
    fn t_commit_row_parses_and_writes_an_i64_value() {
        let mut deck = Deck::<i64>::default();
        deck.push(1, 1);
        deck.push(2, 0);
        let (mut state, nh) = constant_node_i64(deck);
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[1] = "42".to_string();

        commit_row(&mut state, nh, 1);

        assert_eq!(items_of_i64(&state, nh), vec![1, 42]);
        assert!(state.dirty);
    }

    #[test]
    fn t_commit_row_reverts_an_i64_buffer_on_unparseable_text() {
        let (mut state, nh) = constant_node_i64(Deck::from_value(7i64));
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] =
            "not a number".to_string();

        commit_row(&mut state, nh, 0);

        assert_eq!(items_of_i64(&state, nh), vec![7]);
        assert_eq!(
            state.const_edit_cache.try_borrow().unwrap()[nh].buffers[0],
            "7"
        );
    }

    #[test]
    fn t_insert_after_grows_an_i64_deck_without_a_new_mark() {
        let mut deck = Deck::<i64>::default();
        deck.push(1, 1);
        deck.push(2, 0);
        let (mut state, nh) = constant_node_i64(deck);
        let n_marks_before = n_marks_of(&state, nh);

        insert_after(&mut state, nh, 1);

        assert_eq!(items_of_i64(&state, nh), vec![1, 2, 0]);
        assert_eq!(n_marks_of(&state, nh), n_marks_before);
    }

    #[test]
    fn t_delete_row_removes_an_i64_value() {
        let mut deck = Deck::<i64>::default();
        deck.push(1, 1);
        deck.push(2, 0);
        deck.push(3, 0);
        let (mut state, nh) = constant_node_i64(deck);

        delete_row(&mut state, nh, 1);

        assert_eq!(items_of_i64(&state, nh), vec![1, 3]);
    }

    fn items_of(state: &EditorState, nh: NH) -> Vec<f64> {
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        handle.items::<f64>().to_vec()
    }

    fn n_marks_of(state: &EditorState, nh: NH) -> u64 {
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        handle.n_marks
    }

    #[test]
    fn t_insert_after_the_last_row_grows_the_deck_without_a_new_mark() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        let (mut state, nh) = constant_node(deck);
        let n_marks_before = n_marks_of(&state, nh);

        insert_after(&mut state, nh, 1);

        assert_eq!(items_of(&state, nh), vec![1.0, 2.0, 0.0]);
        assert_eq!(
            n_marks_of(&state, nh),
            n_marks_before,
            "a depth-0 insertion adds no new mark"
        );
        assert_eq!(state.pending_focus_row.get(), Some((nh, 2)));
    }

    /// The whole point of replacing the "+ Add" button: Enter can be pressed on *any* row, not
    /// just the last one, and the new value must land immediately after that specific row,
    /// shifting everything after it down by one -- not tacked onto the end.
    #[test]
    fn t_insert_after_a_middle_row_splices_in_rather_than_appending() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        deck.push(3.0, 0);
        let (mut state, nh) = constant_node(deck);

        insert_after(&mut state, nh, 0); // right after the first value, "1.0"

        assert_eq!(items_of(&state, nh), vec![1.0, 0.0, 2.0, 3.0]);
        assert_eq!(state.pending_focus_row.get(), Some((nh, 1)));
    }

    /// `rebuild_with_insertion` reconstructs the whole deck via replay, so this guards against a
    /// regression where that replay merges a nearby group into the one being inserted into, or
    /// fails to shift a later group's mark position by the one newly inserted item.
    #[test]
    fn t_insert_after_preserves_nesting_around_the_insertion_point() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 2);
        deck.push(2.0, 0);
        deck.push(3.0, 1);
        let (mut state, nh) = constant_node(deck);

        insert_after(&mut state, nh, 0); // inside the first (2-item) group

        assert_eq!(items_of(&state, nh), vec![1.0, 0.0, 2.0, 3.0]);
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        // The first group grew from 2 items to 3 (same mark, same position); the second group
        // ("3.0") is still its own mark, shifted from position 2 to 3 by the insertion ahead of
        // it, not merged into the first.
        assert_eq!(
            handle.marks(),
            &[OrcMark { depth: 1, pos: 0 }, OrcMark { depth: 0, pos: 3 },]
        );
    }

    #[test]
    fn t_delete_row_removes_the_value_and_shifts_later_ones_down() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        deck.push(3.0, 0);
        let (mut state, nh) = constant_node(deck);

        delete_row(&mut state, nh, 1); // remove "2.0"

        assert_eq!(items_of(&state, nh), vec![1.0, 3.0]);
    }

    #[test]
    fn t_delete_row_moves_focus_to_the_previous_row() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        insert_after(&mut state, nh, 0);
        insert_after(&mut state, nh, 1);
        // Deck is now [1.0, 0.0, 0.0].

        delete_row(&mut state, nh, 2);
        assert_eq!(state.pending_focus_row.get(), Some((nh, 1)));

        // `pending_focus_row` is only ever cleared by `render.rs` consuming it (once per real
        // frame) -- reset it here to test "deleting the first row" in isolation from the
        // previous call's still-unread request.
        state.pending_focus_row.set(None);
        delete_row(&mut state, nh, 0);
        assert_eq!(
            state.pending_focus_row.get(),
            None,
            "deleting the first row has no previous row to focus"
        );
    }

    /// Deleting the *only* item in a nested group must leave a legitimate empty-group marker
    /// behind (the same bare-ruler-line state `Display`/`deck_rows` already render), rather than
    /// silently deleting the group's own mark along with its one item.
    #[test]
    fn t_delete_row_leaves_an_empty_group_marker_when_it_was_the_only_item_in_its_group() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 1);
        let (mut state, nh) = constant_node(deck);

        delete_row(&mut state, nh, 1); // "2.0" was the sole item of the second group

        assert_eq!(items_of(&state, nh), vec![1.0]);
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert_eq!(
            handle.marks(),
            &[OrcMark { depth: 0, pos: 0 }, OrcMark { depth: 0, pos: 1 }],
            "the second group's mark must survive as an empty group, not vanish"
        );
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
