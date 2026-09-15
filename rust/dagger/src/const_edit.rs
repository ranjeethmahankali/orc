//! Editable display for Constant nodes: a single multiline text box holding one value per line,
//! with a read-only ruler column to its left showing the same collapsed structure Inspect renders
//! (see `inspect::ruler_prefixes`), aligned line-for-line. Editability is gated on whether the
//! constant's handle lives in the host's own `DeckRegistry` (a plugin-owned type's storage can't
//! be reached this way at all) and, for the built-in scalar types this editor knows, on the deck
//! being flat (no nested groups) -- a nested constant is display-only, like a plugin-owned type,
//! since a single text box has no way to express nesting.
//!
//! On blur, the whole buffer is reparsed and the deck rebuilt from scratch (blank lines dropped);
//! a buffer that fails to parse leaves the deck untouched and paints the node red until fixed.

use crate::state::EditorState;
use orc_sdk::{NH, NodeInfo, ORC_TYPE_F64, ORC_TYPE_I64, OrcHandle, OrcMark, TypeOwner};
use std::mem::size_of;

/// The one mark a flat (unnested) multi-item deck carries -- see `normalize_flat_marks` and
/// `is_flat`.
const FLAT_MARK: OrcMark = OrcMark { depth: 0, pos: 0 };

/// A deck is "flat" -- expressible as one plain value per line, with no nesting -- if it has no
/// marks at all (a bare scalar) or exactly the one trivial mark a multi-item flat list carries.
/// Anything else (multiple marks, or a non-zero depth) has real nested structure a single text
/// box has no way to represent.
fn is_flat(marks: &[OrcMark]) -> bool {
    marks.is_empty() || marks == [FLAT_MARK]
}

/// Whether `handle`'s data lives in the host's own `DeckRegistry`, is a type this editor knows
/// how to parse/format (`f64` or `i64` today), and is flat (see `is_flat`). A plugin-owned type's
/// storage lives entirely on the other side of the FFI boundary -- possibly not even Rust -- so
/// there is no `with_mut` to borrow it through at all; those constants stay read-only, same as a
/// nested one.
///
/// `item_size` must match the scalar exactly, not just be a multiple of it: an aggregate (e.g. an
/// F64x3 handle, which shares F64's type_id) would otherwise be treated as if it were a plain
/// list of scalars, showing one line per item holding only that item's first component.
pub(crate) fn is_editable(handle: &OrcHandle) -> bool {
    if !matches!(
        crate::PLUGIN_SET.get_type_owner(handle.type_id),
        Some(TypeOwner::BuiltIn(_))
    ) {
        return false;
    }
    if !is_flat(handle.marks()) {
        return false;
    }
    match handle.type_id {
        ORC_TYPE_F64 => handle.item_size as usize == size_of::<f64>(),
        ORC_TYPE_I64 => handle.item_size as usize == size_of::<i64>(),
        _ => false,
    }
}

/// The same collapsed-ruler text `Inspect` shows for arbitrary connected data, used for a
/// Constant node whose type or structure isn't editable (a plugin-owned type, or a nested list),
/// and for any Constant's pop-out window, which is always read-only regardless of whether it's
/// editable inline.
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

/// Cached editable state for a Constant node: the whole multiline buffer (one value per line) and
/// its aligned read-only ruler column, plus enough of the deck's own structure (`marks`/`n_items`)
/// to know when the cache is stale relative to the handle and needs resyncing. A commit reparses
/// `buffer` in full and rebuilds the deck from scratch (see `commit`) -- there is no per-row
/// bookkeeping to keep in sync anymore.
#[derive(Default, Clone)]
pub(crate) struct ConstEditCache {
    pub(crate) editable: bool,
    pub(crate) buffer: String,
    pub(crate) ruler: String,
    marks: Vec<OrcMark>,
    n_items: u64,
    /// A commit failed to parse `buffer`. The node is painted red, the deck is left untouched,
    /// and the offending text is kept so the user can fix it; cleared again once a commit parses
    /// cleanly.
    pub(crate) invalid: bool,
}

/// Decodes `handle` into (one line per item for the editable buffer, one aligned ruler-prefix
/// line per item for the read-only column), straight from the ABI's own `to_str_deck` conversion
/// -- the same text the read-only `host_deck_to_str` path renders -- instead of hand-formatting
/// each item. `None` means the conversion failed (unreachable in practice for an editable
/// built-in scalar handle; kept non-panicking regardless).
fn decode(handle: &OrcHandle) -> Option<(String, String)> {
    let mut str_deck = orc_sdk::Deck::<u8>::default();
    let converted = match handle.type_id {
        ORC_TYPE_I64 => orc_sdk::to_str_deck::<i64>(handle, &mut str_deck),
        _ => orc_sdk::to_str_deck::<f64>(handle, &mut str_deck),
    };
    converted.ok()?;
    let items = str_deck.items();
    let marks = str_deck.marks();
    let mut lines = Vec::with_capacity(marks.len());
    for (i, mark) in marks.iter().enumerate() {
        let next = marks
            .get(i + 1)
            .map(|m| m.pos)
            .unwrap_or(items.len() as u64);
        let end = next.min(items.len() as u64) as usize;
        let start = (mark.pos as usize).min(end);
        lines.push(String::from_utf8(items[start..end].to_vec()).ok()?);
    }
    let buffer = lines.join("\n");
    let ruler = crate::inspect::ruler_prefixes(marks).join("\n");
    Some((buffer, ruler))
}

/// Resyncs every Constant node's edit cache. Called once per frame, same as
/// `inspect::refresh_all`; cheap when nothing has changed, since it only rebuilds a node's buffer
/// when its item count or mark structure no longer matches what's cached.
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
    let n_items = handle.n_items;
    if entry.editable && entry.marks == marks && entry.n_items == n_items {
        return;
    }
    let Some((buffer, ruler)) = decode(handle) else {
        // Unreachable in practice for an editable built-in scalar handle (see `decode`); degrade
        // to a red, empty cache rather than panic.
        *entry = ConstEditCache {
            editable,
            marks,
            n_items,
            invalid: true,
            ..Default::default()
        };
        return;
    };
    *entry = ConstEditCache {
        editable,
        buffer,
        ruler,
        marks,
        n_items,
        invalid: false,
    };
}

/// A deck with more than one item must carry at least one mark (`Deck::flatten()`'s own
/// invariant), or a consumer that reads structure off `marks` alone -- e.g. `DeckView::depth()`
/// in the Python code generator -- can't tell an unmarked multi-item deck apart from a bare
/// scalar, and silently renders just its first item. Parsing lines one at a time and pushing each
/// at depth 0 never introduces a mark on its own, so this normalizes the result afterward: the
/// single flat-list mark if there's more than one item, none at all otherwise.
fn normalize_flat_marks<T: Default + Clone>(deck: &mut orc_sdk::Deck<T>) {
    if deck.marks().is_empty() {
        if deck.len() > 1 {
            deck.flatten();
        }
    } else if deck.len() <= 1 && deck.marks() == [FLAT_MARK] {
        deck.assign_from_raw_data(deck.items().to_vec(), Vec::new());
    }
}

/// Parses `buffer` one line at a time, in order, dropping blank lines ("empty rows mean nothing")
/// -- not incremental, unlike the old per-row editor: there's one text box now, so a commit just
/// re-derives the whole deck from its current contents rather than splicing a single change into
/// the old one. A single unparseable line fails the whole rebuild.
fn rebuild_lines<T>(buffer: &str) -> Result<orc_sdk::Deck<T>, ()>
where
    T: Default + Clone + std::str::FromStr,
{
    let mut deck = orc_sdk::Deck::<T>::default();
    for line in buffer.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        deck.push(trimmed.parse().map_err(|_| ())?, 0);
    }
    normalize_flat_marks(&mut deck);
    Ok(deck)
}

/// Dispatch over the two editable scalar types (`f64`/`i64` -- the only reachable ones, since
/// `is_editable` gates on exactly those), parsing `buffer` and writing the result straight into
/// the registry-backed `handle`.
fn rebuild_and_alloc(type_id: u64, buffer: &str, handle: &mut OrcHandle) -> Result<(), ()> {
    if type_id == ORC_TYPE_I64 {
        let deck = rebuild_lines::<i64>(buffer)?;
        crate::REGISTRY
            .alloc_with_value(Some(deck), handle)
            .map_err(|_| ())
    } else {
        let deck = rebuild_lines::<f64>(buffer)?;
        crate::REGISTRY
            .alloc_with_value(Some(deck), handle)
            .map_err(|_| ())
    }
}

/// Parses the cached buffer in full and rebuilds the deck from scratch, called once when the text
/// box loses focus (see `render::draw_editable_const_content`) -- not on every keystroke, since
/// there's only one widget to lose focus now. A buffer that fails to parse leaves the deck
/// untouched, keeps the offending text on screen, and flags the node `invalid` (painted red)
/// until a later commit parses cleanly; nothing propagates downstream in that case either.
pub(crate) fn commit(state: &mut EditorState, nh: NH) {
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
    let Ok(mut cache) = state.const_edit_cache.try_borrow_mut() else {
        return;
    };
    let buffer = cache[nh].buffer.clone();

    if rebuild_and_alloc(type_id, &buffer, handle).is_err() {
        cache[nh].invalid = true;
        return;
    }
    let Some((canonical_buffer, ruler)) = decode(handle) else {
        cache[nh].invalid = true;
        return;
    };
    cache[nh].buffer = canonical_buffer;
    cache[nh].ruler = ruler;
    cache[nh].marks = handle.marks().to_vec();
    cache[nh].n_items = handle.n_items;
    cache[nh].invalid = false;
    drop(cache);
    drop(node_infos);
    after_edit(state, nh);
}

/// Nodes whose text box lost focus this frame -- collected while `render.rs` only has
/// `&EditorState` (mid-draw, inside a loop already holding other borrows a commit would conflict
/// with), applied once rendering is done and those borrows are gone.
pub(crate) type ConstEditEvents = Vec<NH>;

pub(crate) fn apply_events(state: &mut EditorState, events: ConstEditEvents) {
    for nh in events {
        commit(state, nh);
    }
}

/// Common tail for a successful commit: `computed_outputs` is the only place anything else
/// (Inspect, a downstream dispatch's constant-input clone) ever reads a constant's *current*
/// value from, so it must be refreshed, same as node creation already does. Downstream nodes are
/// marked dirty so they actually recompute -- both only happen here, once per real commit (i.e.
/// once the text box loses focus), never per keystroke.
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

    fn items_of(state: &EditorState, nh: NH) -> Vec<f64> {
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        handle.items::<f64>().unwrap().to_vec()
    }

    fn items_of_i64(state: &EditorState, nh: NH) -> Vec<i64> {
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        handle.items::<i64>().unwrap().to_vec()
    }

    fn marks_of(state: &EditorState, nh: NH) -> Vec<OrcMark> {
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        handle.marks().to_vec()
    }

    fn set_buffer(state: &mut EditorState, nh: NH, text: &str) {
        state.const_edit_cache.try_borrow_mut().unwrap()[nh].buffer = text.to_string();
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

    #[test]
    fn t_is_editable_true_for_a_flat_multi_item_deck() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        let (state, nh) = constant_node(deck);
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
    /// would silently show one line per item holding only its first component.
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

    /// A single text box has no way to express nesting, so a genuinely nested constant (more
    /// than the one trivial flat-list mark) must stay read-only rather than silently flattened.
    #[test]
    fn t_is_editable_false_for_a_nested_deck() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 2);
        deck.push(2.0, 0);
        deck.push(3.0, 1);
        let (state, nh) = constant_node(deck);
        let node_info_prop = state.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().unwrap();
        let NodeInfo::Constant(handle) = &node_infos[nh] else {
            panic!("expected a constant node")
        };
        assert!(!is_editable(handle));
    }

    #[test]
    fn t_refresh_decodes_one_line_per_item_and_an_aligned_ruler() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.5, 0);
        let (mut state, nh) = constant_node(deck);

        refresh(&mut state, nh);

        let cache = state.const_edit_cache.try_borrow().unwrap();
        assert_eq!(cache[nh].buffer, "1\n2.5");
        assert_eq!(
            cache[nh].ruler.lines().count(),
            2,
            "one ruler line per item, aligned with the buffer"
        );
    }

    #[test]
    fn t_commit_parses_and_writes_the_whole_buffer() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "1\n42.5");

        commit(&mut state, nh);

        assert_eq!(items_of(&state, nh), vec![1.0, 42.5]);
        assert_eq!(marks_of(&state, nh), vec![FLAT_MARK]);
    }

    /// The `ORC_TYPE_I64` branch is a distinct code path in `rebuild_and_alloc` from the `f64`
    /// one every other test here exercises -- this guards it directly rather than trusting it's a
    /// faithful mirror of the float branch.
    #[test]
    fn t_commit_i64() {
        let (mut state, nh) = constant_node_i64(Deck::from_value(1));
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "1\n42");

        commit(&mut state, nh);

        assert_eq!(items_of_i64(&state, nh), vec![1, 42]);
    }

    /// "Empty rows mean nothing": a blank (or whitespace-only) line anywhere in the buffer is
    /// simply skipped, not parsed as anything.
    #[test]
    fn t_commit_drops_blank_lines() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "1\n\n   \n3");

        commit(&mut state, nh);

        assert_eq!(items_of(&state, nh), vec![1.0, 3.0]);
    }

    /// The bug the flat-mark normalization exists to fix: a constant that starts as a bare scalar
    /// (no marks at all) must gain the single flat-list mark as soon as a second line makes it a
    /// real list -- otherwise a consumer that reads structure off `marks` alone (e.g. the Python
    /// code generator's `DeckView::depth()`) can't tell it apart from a scalar and silently
    /// exports only the first item.
    #[test]
    fn t_commit_growing_past_one_line_adds_the_flat_mark() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        assert_eq!(marks_of(&state, nh), Vec::new());
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "1\n2");

        commit(&mut state, nh);

        assert_eq!(items_of(&state, nh), vec![1.0, 2.0]);
        assert_eq!(marks_of(&state, nh), vec![FLAT_MARK]);
    }

    /// The mirror direction: shrinking back down to one line must drop the now-redundant
    /// flat-list mark, returning to the same markless "bare scalar" encoding a single value
    /// starts with.
    #[test]
    fn t_commit_shrinking_to_one_line_removes_the_flat_mark() {
        let mut deck = Deck::<f64>::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        let (mut state, nh) = constant_node(deck);
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "1");

        commit(&mut state, nh);

        assert_eq!(items_of(&state, nh), vec![1.0]);
        assert_eq!(marks_of(&state, nh), Vec::new());
    }

    #[test]
    fn t_commit_all_lines_blank_yields_an_empty_deck() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "\n\n");

        commit(&mut state, nh);

        assert_eq!(items_of(&state, nh), Vec::<f64>::new());
    }

    /// Unparseable text must neither corrupt the deck nor silently vanish from the screen: the
    /// offending buffer stays put (so the user can fix it), the deck is untouched, nothing
    /// propagates downstream, and the node is flagged invalid (painted red) until a clean commit.
    #[test]
    fn t_commit_keeps_the_buffer_and_flags_the_node_on_unparseable_text() {
        let (mut state, nh) = constant_node(Deck::from_value(7.0));
        let oh = state.workflow.node_outputs(nh).next().unwrap();
        refresh(&mut state, nh);
        let version_before = state.dirty_version.try_borrow().unwrap()[nh];
        let computed_before =
            std::sync::Arc::clone(&state.computed_outputs.try_borrow().unwrap()[oh]);
        set_buffer(&mut state, nh, "1\nnot a number");
        commit(&mut state, nh);
        assert_eq!(
            items_of(&state, nh),
            vec![7.0],
            "the deck must be unchanged"
        );
        assert_eq!(
            state.const_edit_cache.try_borrow().unwrap()[nh].buffer,
            "1\nnot a number",
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

    /// Fixing the offending text on a later commit clears the invalid flag and writes the value
    /// through -- the red node recovers without any structural change to the deck.
    #[test]
    fn t_commit_recovers_once_the_bad_text_is_fixed() {
        let (mut state, nh) = constant_node(Deck::from_value(7.0));
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "nope");
        commit(&mut state, nh);
        assert!(state.const_edit_cache.try_borrow().unwrap()[nh].invalid);

        set_buffer(&mut state, nh, "12.5");
        commit(&mut state, nh);

        assert_eq!(items_of(&state, nh), vec![12.5]);
        assert!(
            !state.const_edit_cache.try_borrow().unwrap()[nh].invalid,
            "a clean commit must clear the red invalid flag"
        );
    }

    /// End to end: committing must update `computed_outputs` (the only place anything else, e.g.
    /// an Inspect node downstream, reads a constant's *current* value from) and bump its own
    /// `dirty_version` immediately -- there's only one commit per real blur now, not one per
    /// keystroke, so there's no reason to defer this any further the way the old per-row editor
    /// had to.
    #[test]
    fn t_commit_refreshes_computed_outputs_and_bumps_dirty_version() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        let oh = state.workflow.node_outputs(nh).next().unwrap();
        let version_before = state.dirty_version.try_borrow().unwrap()[nh];
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "99");

        commit(&mut state, nh);

        let computed = state.computed_outputs.try_borrow().unwrap();
        assert_eq!(computed[oh].items::<f64>().unwrap(), &[99.0]);
        assert!(state.dirty_version.try_borrow().unwrap()[nh] > version_before);
        assert!(state.dirty);
    }

    #[test]
    fn t_apply_events_commits_every_blurred_node() {
        let (mut state, nh) = constant_node(Deck::from_value(1.0));
        refresh(&mut state, nh);
        set_buffer(&mut state, nh, "5");

        apply_events(&mut state, vec![nh]);

        assert_eq!(items_of(&state, nh), vec![5.0]);
    }
}
