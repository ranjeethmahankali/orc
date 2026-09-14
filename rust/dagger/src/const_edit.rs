//! Editable ruler display for Constant nodes: per-value text boxes laid out with the same ruler
//! prefixes as `Deck`'s own `Display` impl, editability gated on whether the constant's handle
//! lives in the host's own `DeckRegistry` (a plugin-owned type's storage can't be reached this
//! way at all). The cache holds one string per item plus the deck's mark structure, coupled to
//! the UI and converted back into a fresh deck on commit -- via the ABI (`to_str_deck` for
//! reading, `FromStr` for writing) instead of hand-formatting items or nudging the deck in
//! place. A commit that fails to parse leaves the deck untouched and paints the node red until
//! the text is fixed.

use crate::state::EditorState;
use orc_sdk::{NH, NodeInfo, ORC_TYPE_F64, ORC_TYPE_I64, OrcHandle, OrcMark, TypeOwner};
use std::mem::size_of;

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
///
/// `item_size` must match the scalar exactly, not just be a multiple of it: an aggregate (e.g. an
/// F64x3 handle, which shares F64's type_id) would otherwise be treated as if it were a plain list
/// of scalars, and this editor would show one text box per aggregate item holding only that
/// item's first component, silently discarding the rest.
pub(crate) fn is_editable(handle: &OrcHandle) -> bool {
    if !matches!(
        crate::PLUGIN_SET.get_type_owner(handle.type_id),
        Some(TypeOwner::BuiltIn(_))
    ) {
        return false;
    }
    match handle.type_id {
        ORC_TYPE_F64 => handle.item_size as usize == size_of::<f64>(),
        ORC_TYPE_I64 => handle.item_size as usize == size_of::<i64>(),
        _ => false,
    }
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

/// Cached per-row edit buffers for a Constant node's editable ruler rows, plus the deck's mark
/// structure as of the last time the cache was synced. Everything the editor can change lives in
/// here -- each row's text and the mark structure it sits in -- and a commit parses all of it
/// back into a fresh deck (via the ABI's `to_str_deck`/`FromStr`) rather than formatting items
/// by hand or poking at the deck in place.
#[derive(Default, Clone)]
pub(crate) struct ConstEditCache {
    pub(crate) editable: bool,
    pub(crate) buffers: Vec<String>,
    pub(crate) marks: Vec<OrcMark>,
    /// An edit attempt failed to parse one of `buffers`. The node is painted red, the deck is
    /// left untouched, and the offending text is kept so the user can fix it; cleared again once
    /// a commit parses cleanly.
    pub(crate) invalid: bool,
}

/// Decodes one string per item straight from the ABI's own `to_str_deck` conversion -- the same
/// text the read-only `host_deck_to_str` path renders -- instead of this module hand-formatting
/// each item. A string deck (`Deck<u8>`) emits exactly one mark per original item, each mark's
/// byte range `[pos, next_pos)` being that item's `Display` string, so the result indexes 1:1 by
/// item. `None` means the conversion failed (unreachable in practice for an editable built-in
/// scalar handle; kept non-panicking regardless).
fn decode_buffers(handle: &OrcHandle) -> Option<Vec<String>> {
    let mut str_deck = orc_sdk::Deck::<u8>::default();
    let converted = match handle.type_id {
        ORC_TYPE_I64 => orc_sdk::to_str_deck::<i64>(handle, &mut str_deck),
        _ => orc_sdk::to_str_deck::<f64>(handle, &mut str_deck),
    };
    converted.ok()?;
    let items = str_deck.items();
    let marks = str_deck.marks();
    let mut buffers = Vec::with_capacity(marks.len());
    for (i, mark) in marks.iter().enumerate() {
        let next = marks
            .get(i + 1)
            .map(|m| m.pos)
            .unwrap_or(items.len() as u64);
        let end = next.min(items.len() as u64) as usize;
        let start = (mark.pos as usize).min(end);
        buffers.push(String::from_utf8(items[start..end].to_vec()).ok()?);
    }
    Some(buffers)
}

/// Resyncs every Constant node's edit-buffer cache. Called once per frame, same as
/// `inspect::refresh_all`; cheap when nothing has changed, since it only rebuilds a node's
/// buffers when its item count or mark structure no longer matches what's cached.
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
    let entry = &mut cache[nh];
    if !editable {
        if entry.editable {
            *entry = ConstEditCache::default();
        }
        return;
    }
    let marks = handle.marks().to_vec();
    if entry.editable && entry.buffers.len() == n_items && entry.marks == marks {
        return;
    }
    let Some(buffers) = decode_buffers(handle) else {
        // Unreachable in practice for an editable built-in scalar handle (see `decode_buffers`);
        // degrade to a red, empty cache rather than panic.
        *entry = ConstEditCache {
            editable,
            buffers: Vec::new(),
            marks,
            invalid: true,
        };
        return;
    };
    debug_assert_eq!(buffers.len(), n_items);
    *entry = ConstEditCache {
        editable,
        buffers,
        marks,
        invalid: false,
    };
}

/// The structural change to apply while rebuilding a constant's deck from its edit buffers.
#[derive(Clone, Copy)]
enum ConstEdit {
    /// A plain value commit: parse every buffer, dropping any that are empty.
    Commit,
    /// Insert a fresh default-valued row right after `after_index`.
    InsertAfter { after_index: usize },
    /// Drop the row at `delete_index` outright.
    DeleteAt { delete_index: usize },
}

/// The deck that a rebuild produced, plus bookkeeping for callers that must track positions in
/// the newly built deck (keyboard focus).
struct BuiltDeck {
    deck: BuiltDeckEnum,
    /// New index in the rebuilt deck of each retained original item; `None` for one that was
    /// eliminated (an empty buffer, or the explicit delete target).
    new_indices: Vec<Option<usize>>,
    /// Where an inserted default row landed, when this edit was an insertion.
    inserted_at: Option<usize>,
}

/// Generic intermediate: the rebuilt deck plus the same bookkeeping fields as `BuiltDeck`
/// (which stores it through `BuiltDeckEnum` once its type is known).
struct Rebuilt<T: Default> {
    deck: orc_sdk::Deck<T>,
    new_indices: Vec<Option<usize>>,
    inserted_at: Option<usize>,
}

enum BuiltDeckEnum {
    F64(orc_sdk::Deck<f64>),
    I64(orc_sdk::Deck<i64>),
}

impl BuiltDeck {
    /// The `Display` text of every item in the rebuilt deck -- what the rows snap to after a
    /// successful commit (round-trips exactly through `FromStr`).
    fn canonical_strings(&self) -> Vec<String> {
        match &self.deck {
            BuiltDeckEnum::F64(d) => d.items().iter().map(|v| v.to_string()).collect(),
            BuiltDeckEnum::I64(d) => d.items().iter().map(|v| v.to_string()).collect(),
        }
    }

    fn marks(&self) -> Vec<OrcMark> {
        match &self.deck {
            BuiltDeckEnum::F64(d) => d.marks().to_vec(),
            BuiltDeckEnum::I64(d) => d.marks().to_vec(),
        }
    }
}

/// Replays one item (by its original index) into the deck being rebuilt. The empty-buffer /
/// explicit-deletion elimination happens here, as does the "insert a default row right after
/// this item" splice, so `walk_runs`' event order carries the original structure over verbatim
/// (including an empty group's bare mark, since `start_new_arr` runs before this for its run).
fn replay_item<T>(
    new_deck: &mut orc_sdk::Deck<T>,
    index: usize,
    delete_at: Option<usize>,
    insert_after: Option<usize>,
    values: &[Option<T>],
    new_indices: &mut [Option<usize>],
    inserted_at: &mut Option<usize>,
) where
    T: Copy + Default,
{
    if delete_at == Some(index) {
        return;
    }
    if let Some(v) = values[index] {
        new_indices[index] = Some(new_deck.items().len());
        new_deck.push(v, 0);
    }
    if insert_after == Some(index) {
        *inserted_at = Some(new_deck.items().len());
        new_deck.push(T::default(), 0);
    }
}

/// Generic core of `rebuild_typed`: parses every buffer (dropping empty ones -- "empty rows
/// mean nothing"), applies `edit`'s structural change, and replays items + marks into a fresh
/// deck just like the old `rebuild_with_insertion`/`rebuild_with_deletion` did -- the replay,
/// rather than hand-shifting mark positions, is what correctly preserves an empty group's bare
/// mark and keeps later groups at the right offset. A single unparseable buffer fails the whole
/// rebuild.
fn rebuild_buffers<T>(
    buffers: &[String],
    marks: &[OrcMark],
    edit: ConstEdit,
) -> Result<Rebuilt<T>, ()>
where
    T: Copy + Default + std::str::FromStr,
{
    let insert_after = match edit {
        ConstEdit::InsertAfter { after_index } if after_index < buffers.len() => Some(after_index),
        _ => None,
    };
    let delete_at = match edit {
        ConstEdit::DeleteAt { delete_index } if delete_index < buffers.len() => Some(delete_index),
        _ => None,
    };

    // Parse everything up front: any failure aborts the commit before the deck is touched.
    let mut values: Vec<Option<T>> = Vec::with_capacity(buffers.len());
    for text in buffers {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            values.push(None);
        } else {
            values.push(Some(trimmed.parse::<T>().map_err(|_| ())?));
        }
    }

    let mut new_deck = orc_sdk::Deck::<T>::default();
    let mut new_indices = vec![None; buffers.len()];
    let mut inserted_at = None;
    let n_items = buffers.len() as u64;

    walk_runs(n_items, marks, |event| match event {
        RunEvent::Run {
            depth,
            pos,
            next_pos,
        } => {
            new_deck.start_new_arr(depth.saturating_add(1));
            for idx in pos..next_pos.min(n_items) {
                replay_item(
                    &mut new_deck,
                    idx as usize,
                    delete_at,
                    insert_after,
                    &values,
                    &mut new_indices,
                    &mut inserted_at,
                );
            }
        }
        RunEvent::TailItem(i) => replay_item(
            &mut new_deck,
            i as usize,
            delete_at,
            insert_after,
            &values,
            &mut new_indices,
            &mut inserted_at,
        ),
    });

    Ok(Rebuilt {
        deck: new_deck,
        new_indices,
        inserted_at,
    })
}

/// Dispatch over the two editable scalar types (`f64`/`i64` -- the only reachable ones, since
/// `is_editable` gates on exactly those).
fn rebuild_typed(
    type_id: u64,
    buffers: &[String],
    marks: &[OrcMark],
    edit: ConstEdit,
) -> Result<BuiltDeck, ()> {
    if type_id == ORC_TYPE_I64 {
        rebuild_buffers::<i64>(buffers, marks, edit).map(|rebuilt| BuiltDeck {
            deck: BuiltDeckEnum::I64(rebuilt.deck),
            new_indices: rebuilt.new_indices,
            inserted_at: rebuilt.inserted_at,
        })
    } else {
        rebuild_buffers::<f64>(buffers, marks, edit).map(|rebuilt| BuiltDeck {
            deck: BuiltDeckEnum::F64(rebuilt.deck),
            new_indices: rebuilt.new_indices,
            inserted_at: rebuilt.inserted_at,
        })
    }
}

/// The shared implementation behind `commit_row`/`insert_after`/`delete_row`: parses *every*
/// edit buffer back into a fresh deck (`FromStr`), applies the requested structural edit, drops
/// empty rows, swaps the new deck into the registry, resyncs the cache, and propagates
/// downstream -- the registry's own lock is what makes this safe, nothing about the handle
/// itself: a dispatched job never shares this memory in the first place (see "Handle lifetime
/// across threads" in PROJECT.org), so nothing is ever reading through the backing storage this
/// mutates.
///
/// Returns `(inserted_at, focus_after_delete)` so `insert_after`/`delete_row` can steal
/// keyboard focus onto the right row; `None` when nothing was applied. The meaningful `None`
/// case is a buffer that failed to parse: the deck is left untouched, nothing propagates
/// downstream (`after_edit` is not called), the offending text stays on screen for the user to
/// fix, and the node is flagged `invalid` (painted red) until a later commit parses cleanly.
fn rebuild_and_apply(
    state: &mut EditorState,
    nh: NH,
    edit: ConstEdit,
) -> Option<(Option<usize>, Option<usize>)> {
    let mut node_info_prop = state.workflow.node_info_prop();
    let Ok(mut node_infos) = node_info_prop.try_borrow_mut() else {
        return None;
    };
    let NodeInfo::Constant(handle) = &mut node_infos[nh] else {
        return None;
    };
    if !is_editable(handle) {
        return None;
    }
    let type_id = handle.type_id;
    let Ok(mut cache) = state.const_edit_cache.try_borrow_mut() else {
        return None;
    };

    // The cache is the thing being committed. If it never got a chance to sync against the
    // handle (e.g. an event lands on the same frame a node was created, before `refresh_all`
    // runs), mint it from the deck first so there is always a 1:1 row set to parse.
    if cache[nh].buffers.len() != handle.n_items as usize || cache[nh].marks != handle.marks() {
        let Some(minted) = decode_buffers(handle) else {
            cache[nh].invalid = true;
            return None;
        };
        cache[nh].buffers = minted;
        cache[nh].marks = handle.marks().to_vec();
    }
    let buffers = cache[nh].buffers.clone();
    let marks = cache[nh].marks.clone();

    let Ok(built) = rebuild_typed(type_id, &buffers, &marks, edit) else {
        cache[nh].invalid = true;
        return None;
    };
    let canonical = built.canonical_strings();
    let new_marks = built.marks();
    let alloc_result = match built.deck {
        BuiltDeckEnum::F64(deck) => crate::REGISTRY.alloc_with_value(Some(deck), handle),
        BuiltDeckEnum::I64(deck) => crate::REGISTRY.alloc_with_value(Some(deck), handle),
    };
    if alloc_result.is_err() {
        // Registry-level failure (concurrency etc., theoretically unreachable here): leave the
        // cache alone and back out -- no dirty flag, no propagation.
        return None;
    }
    cache[nh].buffers = canonical;
    cache[nh].marks = new_marks;
    cache[nh].invalid = false;
    let focus_after_delete = match edit {
        // Deleting a row moves focus to the closest remaining row above it (spreadsheet-style
        // merge-back), matching how deleting an empty line merges you back into the one above.
        ConstEdit::DeleteAt { delete_index } => (0..delete_index)
            .rev()
            .find_map(|j| built.new_indices[j])
            .filter(|idx| *idx > 0),
        _ => None,
    };
    drop(cache);
    drop(node_infos);
    after_edit(state, nh);
    Some((built.inserted_at, focus_after_delete))
}

/// Parses the cached edit buffer for `item_index` (in practice, every row: the whole buffer set
/// is what a commit re-parses) and writes it back into the real deck. See `rebuild_and_apply`
/// for the parse-failure behavior (text kept, node flagged red, nothing propagates).
pub fn commit_row(state: &mut EditorState, nh: NH, _item_index: usize) {
    let _ = rebuild_and_apply(state, nh, ConstEdit::Commit);
}

/// Inserts one new zero-valued depth-0 row immediately after `after_index` -- the "hit Enter to
/// add a row" gesture, wherever in the list Enter was pressed, not just at the end.
pub fn insert_after(state: &mut EditorState, nh: NH, after_index: usize) {
    if let Some((Some(new_row), _)) =
        rebuild_and_apply(state, nh, ConstEdit::InsertAfter { after_index })
    {
        state.pending_focus_row.set(Some((nh, new_row)));
    }
}

/// Removes the value at `delete_index` -- backspacing an already-empty row's gesture for
/// deleting it outright. Focus moves to the previous row, if there is one, matching how deleting
/// an empty line merges you back into the one above it in most text/list editors.
pub fn delete_row(state: &mut EditorState, nh: NH, delete_index: usize) {
    if let Some((_, Some(focus))) =
        rebuild_and_apply(state, nh, ConstEdit::DeleteAt { delete_index })
    {
        state.pending_focus_row.set(Some((nh, focus)));
    }
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
    // A row that had Enter pressed in it (i.e. it also appears in `inserted_after`) is fully
    // committed by `insert_after`'s own rebuild of every buffer -- deferring its commit keeps
    // the insertion's `after_index` meaningful, since committing it first could eliminate an
    // empty row (or the later insertion itself) and shift indices out from under it. Every other
    // committed row still commits before any insertion; those don't relocate this frame's rows.
    for (nh, item_index) in events.committed_rows {
        let inserted = events
            .inserted_after
            .iter()
            .any(|(n, i)| *n == nh && *i == item_index);
        if !inserted {
            commit_row(state, nh, item_index);
        }
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

    /// An aggregate handle (item_size a multiple of, but not equal to, the scalar size) shares
    /// F64's type_id but must not be treated as an editable list of plain f64 scalars -- that
    /// would silently show one text box per item holding only its first component. This pins
    /// down the exact bug `is_editable`'s `item_size == size_of::<T>()` guard exists to prevent
    /// (see its doc comment); a future simplification back to a bare `type_id`-only check would
    /// silently reintroduce it without this test catching it.
    #[test]
    fn t_is_editable_false_for_an_aggregate_f64_handle() {
        let mut deck = Deck::<[f64; 3]>::default();
        deck.push([1.0, 2.0, 3.0], 1);
        let mut handle = OrcHandle {
            handle: crate::HANDLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        };
        crate::REGISTRY
            .alloc_with_value(Some(deck), &mut handle)
            .unwrap();
        assert_eq!(handle.item_size as usize, size_of::<[f64; 3]>());
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
        assert_eq!(handle.items::<f64>().unwrap(), &[1.0, 42.5]);
        assert!(
            state.dirty,
            "committing a value must mark the workflow dirty"
        );
    }

    /// Unparseable text must neither corrupt the deck nor silently vanish from the screen: the
    /// offending buffer stays put (so the user can fix it), the deck is untouched, nothing
    /// propagates downstream, and the node is flagged invalid (painted red) until a clean commit.
    #[test]
    fn t_commit_row_keeps_the_buffer_and_flags_the_node_on_unparseable_text() {
        let (mut state, nh) = constant_node(Deck::from_value(7.0));
        let oh = state.workflow.node_outputs(nh).next().unwrap();
        refresh(&mut state, nh);
        let version_before = state.dirty_version.try_borrow().unwrap()[nh];
        let computed_before =
            std::sync::Arc::clone(&state.computed_outputs.try_borrow().unwrap()[oh]);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] =
            "not a number".to_string();

        commit_row(&mut state, nh, 0);

        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert_eq!(
            handle.items::<f64>().unwrap(),
            &[7.0],
            "the deck must be unchanged"
        );
        drop(node_infos);
        assert_eq!(
            state.const_edit_cache.try_borrow().unwrap()[nh].buffers[0],
            "not a number",
            "the offending text must stay on screen so the user can fix it"
        );
        assert!(
            state.const_edit_cache.try_borrow().unwrap()[nh].invalid,
            "a failed parse must flag the node (red) rather than being silently dropped"
        );
        assert_eq!(
            state.dirty_version.try_borrow().unwrap()[nh],
            version_before,
            "a failed commit must not propagate downstream (no dirty bump, no computed refresh)"
        );
        assert!(
            std::sync::Arc::ptr_eq(
                &computed_before,
                &state.computed_outputs.try_borrow().unwrap()[oh]
            ),
            "a failed commit must not replace the computed output handle"
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
        handle.items::<i64>().unwrap().to_vec()
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
    fn t_commit_row_keeps_an_i64_buffer_and_flags_the_node_on_unparseable_text() {
        let (mut state, nh) = constant_node_i64(Deck::from_value(7i64));
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] =
            "not a number".to_string();

        commit_row(&mut state, nh, 0);

        assert_eq!(items_of_i64(&state, nh), vec![7]);
        assert_eq!(
            state.const_edit_cache.try_borrow().unwrap()[nh].buffers[0],
            "not a number"
        );
        assert!(state.const_edit_cache.try_borrow().unwrap()[nh].invalid);
    }

    /// Fixing the offending text on a later commit clears the invalid flag and writes the value
    /// through -- the red node recovers without any structural change to the deck.
    #[test]
    fn t_commit_row_recovers_once_the_bad_text_is_fixed() {
        let (mut state, nh) = constant_node(Deck::from_value(7.0));
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] = "nope".to_string();
        commit_row(&mut state, nh, 0);
        assert!(state.const_edit_cache.try_borrow().unwrap()[nh].invalid);

        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[0] = "12.5".to_string();
        commit_row(&mut state, nh, 0);

        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert_eq!(handle.items::<f64>().unwrap(), &[12.5]);
        assert!(
            !state.const_edit_cache.try_borrow().unwrap()[nh].invalid,
            "a clean commit must clear the red invalid flag"
        );
    }

    /// "Empty rows mean nothing": committing one eliminates it from both the data and the
    /// editing rows (whitespace-only counts as empty too).
    #[test]
    fn t_commit_row_eliminates_an_empty_row() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        deck.push(3.0, 0);
        let (mut state, nh) = constant_node(deck);
        refresh(&mut state, nh);
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffers[1] = "   ".to_string();

        commit_row(&mut state, nh, 1);

        assert_eq!(items_of(&state, nh), vec![1.0, 3.0]);
        let cache = state.const_edit_cache.try_borrow().unwrap();
        assert_eq!(cache[nh].buffers, vec!["1".to_string(), "3".to_string()]);
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
        handle.items::<f64>().unwrap().to_vec()
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
        assert_eq!(computed[oh].items::<f64>().unwrap(), &[99.0]);
        assert!(state.dirty_version.try_borrow().unwrap()[nh] > version_before);
    }
}
