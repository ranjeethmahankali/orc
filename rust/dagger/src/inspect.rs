//! Inspect node display: scrollable text, resizable viewport, pop-out.
//!
//! Rendering a `deck_to_str()` result (a `Deck<u8>` handle) as raw bytes via the existing
//! `fmt_raw_deck`/`Display for Deck<u8>` would splay every individual character out as its own
//! ruler entry -- unreadable. `render_str_deck` instead collapses the deepest level (the run of
//! bytes making up one string) into a single line. See PROJECT.org's "Inspect node display" for
//! the worked example and the derivation of the depth arithmetic below.

use crate::state::EditorState;
use eframe::egui;
use orc_sdk::{NH, NodeInfo, OrcHandle, OrcMark};
use std::fmt::Write as _;

const TAB_WIDTH: usize = 3;

/// What `InspectCache::text` currently reflects. Three genuinely distinct states -- collapsing
/// "not connected" and "not yet computed" into a single `Option<u64>` (both are "no handle")
/// made them indistinguishable to the "skip if unchanged" check below, so moving from one to the
/// other silently failed to update the displayed text. Kept as its own enum specifically so each
/// state has its own identity to compare against.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum CacheSource {
    #[default]
    NotConnected,
    /// This node (or its upstream chain) hasn't caught up with the latest edit yet -- nothing
    /// to show until it settles, rather than displaying now-questionable old data.
    Stale,
    NotYetComputed,
    Error,
    Computed(u64),
}

/// Cached text for one Inspect node, keyed by what it was rendered from so it only gets
/// recomputed when that actually changes (not every frame).
#[derive(Default)]
pub(crate) struct InspectCache {
    source: CacheSource,
    pub(crate) text: String,
}

/// Refreshes every Inspect node's cached text. Called once per frame; cheap when nothing has
/// changed, since `refresh` bails out immediately once it sees the cached source still matches.
pub fn refresh_all(state: &mut EditorState) {
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    for nh in nodes {
        refresh(state, nh);
    }
}

/// Keeps every popped-out Inspect window alive for one more pass, or notices it was closed.
///
/// `ctx.show_viewport_deferred` must be called every pass a deferred viewport should keep
/// existing — not calling it is how it goes away — so this runs unconditionally every frame,
/// same as `refresh_all`. Each node's viewport id is deterministic (hashed from its `NH`), so
/// re-registering it here with a fresh snapshot of the cached text is exactly the documented way
/// to push updated content into an already-open window.
pub fn update_popouts(ctx: &egui::Context, state: &mut EditorState) {
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    for nh in nodes {
        let is_open = state
            .inspect_popout
            .try_borrow()
            .map(|p| p[nh])
            .unwrap_or(false);
        if !is_open {
            continue;
        }

        let viewport_id = egui::ViewportId::from_hash_of(("dagger-inspect-popout", nh));
        // The close request lands in the *child* viewport's own input, observable from the root
        // pass via `input_for` -- no shared flag or channel needed to hear about it.
        if ctx.input_for(viewport_id, |i| i.viewport().close_requested()) {
            if let Ok(mut popout) = state.inspect_popout.try_borrow_mut() {
                popout[nh] = false;
            }
            continue;
        }

        let title = {
            let node_info_prop = state.workflow.node_info_prop();
            node_info_prop
                .try_borrow()
                .map(|infos| infos[nh].name().to_string())
                .unwrap_or_else(|_| "Inspect".to_string())
        };
        let text = state
            .inspect_cache
            .try_borrow()
            .map(|c| c[nh].text.clone())
            .unwrap_or_default();

        ctx.show_viewport_deferred(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title(title)
                .with_inner_size([420.0, 320.0]),
            move |ui, _class| {
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&text).monospace())
                            .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
            },
        );
    }
}

/// Recomputes `nh`'s cached text if its upstream input's handle has changed since the last
/// render. `nh` must be an Inspect node; anything else is a no-op.
fn refresh(state: &mut EditorState, nh: NH) {
    let node_info_prop = state.workflow.node_info_prop();
    let Ok(node_infos) = node_info_prop.try_borrow() else {
        return;
    };
    if !matches!(node_infos[nh], NodeInfo::Inspect { .. }) {
        return;
    }
    drop(node_infos);

    // Downstream nodes -- Inspect included -- are marked stale on every edit and only settle
    // again once whatever they depend on catches up (see `exec::mark_dirty` and the trivial
    // settle `dispatch` gives Inspect nodes). While stale there's nothing worth showing: the
    // last cached text reflects data from before whatever just changed.
    if !crate::exec::is_settled(state, nh) {
        let Ok(mut cache) = state.inspect_cache.try_borrow_mut() else {
            return;
        };
        if cache[nh].source != CacheSource::Stale {
            cache[nh] = InspectCache {
                source: CacheSource::Stale,
                text: String::new(),
            };
        }
        return;
    }

    let Some(ih) = state.workflow.node_inputs(nh).next() else {
        return;
    };
    let Ok(mut cache) = state.inspect_cache.try_borrow_mut() else {
        return;
    };
    let Some(oh) = state.workflow.input_source(ih) else {
        // `CacheSource::NotConnected` is also this cache's `Default`, so a freshly created node
        // that's never been touched at all would otherwise look like "no update needed" here --
        // `text.is_empty()` catches that first-ever call specifically.
        if cache[nh].source != CacheSource::NotConnected || cache[nh].text.is_empty() {
            cache[nh] = InspectCache {
                source: CacheSource::NotConnected,
                text: "<not connected>".to_string(),
            };
        }
        return;
    };
    // A genuine execution fault on the upstream node "settles" without touching
    // `computed_outputs` (see `exec`'s commit policy) -- so on its own, `computed_outputs[oh]`
    // would keep showing whatever it last held, with nothing indicating it's now stale relative
    // to an upstream that's actively failing. Surface that error here instead of silently
    // trusting old data.
    let upstream_error = {
        let upstream_nh = state.workflow.node_from_output(oh);
        state
            .execution_error
            .try_borrow()
            .ok()
            .and_then(|errors| errors[upstream_nh].clone())
    };
    if let Some(err) = upstream_error {
        let text = format!("<upstream error: {err}>");
        if cache[nh].text != text {
            cache[nh] = InspectCache {
                source: CacheSource::Error,
                text,
            };
        }
        return;
    }

    let Ok(computed) = state.computed_outputs.try_borrow() else {
        return;
    };
    let handle = &computed[oh];
    if handle.free_fn.is_none() {
        if cache[nh].source != CacheSource::NotYetComputed {
            cache[nh] = InspectCache {
                source: CacheSource::NotYetComputed,
                text: "<not yet computed>".to_string(),
            };
        }
        return;
    }
    if cache[nh].source == CacheSource::Computed(handle.handle) {
        return; // already rendered this exact value.
    }
    let text = match crate::host_deck_to_str(handle) {
        // `str_handle` is a throwaway -- it's dropped (and freed, via `OrcHandle`'s own `Drop`)
        // at the end of this arm, right after its contents are rendered into an owned `String`.
        Ok(str_handle) => {
            let mut text = String::new();
            render_str_deck(&str_handle, &mut text);
            text
        }
        Err(e) => format!("<error: {e}>"),
    };
    cache[nh] = InspectCache {
        source: CacheSource::Computed(handle.handle),
        text,
    };
}

/// Decodes one run's bytes as a string. `to_str_deck` only ever writes valid UTF-8 (it formats
/// numbers and other `Display` types), so the lossy fallback only matters for a pathological
/// custom `Display` impl -- kept defensive rather than panicking on it.
fn decode(items: &[u8], range: std::ops::Range<usize>) -> &str {
    std::str::from_utf8(&items[range]).unwrap_or("<invalid utf8>")
}

/// Appends the collapsed text rendering of a raw `Deck<u8>` (`items` + `marks`, straight off the
/// wire) to `out`. No allocation beyond `out` itself growing -- every decoded string is a
/// zero-copy view into `items`.
///
/// `to_str_deck` emits one mark *per original item*, not one mark per input structural boundary
/// -- a former "continuation" item (no mark at all in the input, sharing its neighbor's ruler
/// line) still gets its own mark here, just at the minimum depth (raw `0`), since every item now
/// needs its own string's byte range recorded. So raw depth `0` marks are exactly the ones that
/// must render as a continuation of the previous ruler line (matching how `fmt_raw_deck` prints
/// multiple items under one mark), not as a fresh ruler entry -- only depth `>= 1` marks start a
/// new one. This is also why the loop can't simply mirror `fmt_raw_deck`'s "advance through a
/// run of several items" shape: here every mark *is* its own single-item run by construction.
fn render_str_deck_raw(items: &[u8], marks: &[OrcMark], out: &mut String) {
    if items.is_empty() && marks.is_empty() {
        out.push_str("<empty_deck>\n");
        return;
    }
    if marks.is_empty() {
        // No structure at all: the whole buffer is one string, same as `fmt_raw_deck`'s
        // no-marks tail loop but collapsed to a single line instead of one line per byte.
        let _ = writeln!(out, "   ┤ {}", decode(items, 0..items.len()));
        return;
    }
    let n_items = items.len() as u64;
    // One level shallower than `Deck::max_depth()` (which is `marks.first().depth + 1`) --
    // that `+ 1` is exactly the level `to_str_deck` added, so dropping it here recovers the
    // original (pre-string) deck's own depth numbering.
    let dmax = marks[0].depth;
    let continuation_indent = (dmax as usize + 1) * TAB_WIDTH;
    for (i, m) in marks.iter().enumerate() {
        let next_pos = marks.get(i + 1).map(|n| n.pos).unwrap_or(n_items);
        let s = decode(items, m.pos as usize..next_pos.min(n_items) as usize);
        if m.depth == 0 {
            let _ = writeln!(out, "{:>indent$}   ┤ {}", "", s, indent = continuation_indent);
        } else {
            let indent = (dmax - m.depth) as usize * TAB_WIDTH + TAB_WIDTH;
            let bracket_width = m.depth as usize * TAB_WIDTH;
            let _ = writeln!(
                out,
                "{:>indent$}{:>3} {:─>bw$} {}",
                "",
                m.depth,
                "┤",
                s,
                indent = indent,
                bw = bracket_width,
            );
        }
    }
}

/// Renders a `deck_to_str()` result handle -- reads `items`/`marks` straight off the raw
/// pointers, same as `HandleDisplayWrapper` does for the plain `Display` case.
pub fn render_str_deck(handle: &OrcHandle, out: &mut String) {
    let items: &[u8] = handle.items::<u8>();
    let marks: &[OrcMark] = unsafe { orc_sdk::slice_from_ptr(handle.marks, handle.n_marks as usize) };
    render_str_deck_raw(items, marks, out);
}

#[cfg(test)]
mod test {
    use super::*;
    use orc_sdk::{Deck, OrcHandle};
    use std::sync::atomic::Ordering;

    fn handle_for<T: orc_sdk::TOrcData + std::any::Any + Send + Sync>(deck: Deck<T>) -> OrcHandle {
        let mut handle = OrcHandle {
            handle: crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed),
            ..Default::default()
        };
        crate::REGISTRY
            .alloc_with_value(Some(deck), &mut handle)
            .unwrap();
        handle
    }

    #[test]
    fn t_collapsed_display_matches_the_original_deck_for_integers() {
        // [[10, 20], [30]] -- deliberately the exact shape from PROJECT.org's worked example.
        let mut original = Deck::<i32>::default();
        original.push(10, 2);
        original.push(20, 0);
        original.push(30, 1);
        let original_text = original.to_string();

        let handle = handle_for(original);
        let mut str_deck = Deck::<u8>::default();
        orc_sdk::to_str_deck::<i32>(&handle, &mut str_deck).unwrap();

        let mut collapsed = String::new();
        render_str_deck_raw(str_deck.items(), str_deck.marks(), &mut collapsed);

        assert_eq!(
            collapsed, original_text,
            "collapsed string deck must read identically to the original integer deck"
        );
    }

    #[test]
    fn t_collapsed_display_matches_the_original_deck_at_three_levels_of_nesting() {
        // [[[1, 2], [3]], [[4]]] -- exercises depth values beyond just 0/1/2, and more than one
        // top-level group, so the indent/bracket-width formulas get checked at every depth.
        let mut original = Deck::<i32>::default();
        original.push(1, 3);
        original.push(2, 0);
        original.push(3, 1);
        original.push(4, 2);
        let original_text = original.to_string();

        let handle = handle_for(original);
        let mut str_deck = Deck::<u8>::default();
        orc_sdk::to_str_deck::<i32>(&handle, &mut str_deck).unwrap();

        let mut collapsed = String::new();
        render_str_deck_raw(str_deck.items(), str_deck.marks(), &mut collapsed);

        assert_eq!(collapsed, original_text);
    }

    #[test]
    fn t_collapsed_display_decodes_multi_char_strings() {
        let mut original = Deck::<f64>::default();
        original.push(1.5, 1);
        original.push(-20.0, 1);

        let handle = handle_for(original);
        let mut str_deck = Deck::<u8>::default();
        orc_sdk::to_str_deck::<f64>(&handle, &mut str_deck).unwrap();

        let mut collapsed = String::new();
        render_str_deck_raw(str_deck.items(), str_deck.marks(), &mut collapsed);
        assert!(collapsed.contains("1.5"), "got: {collapsed}");
        assert!(collapsed.contains("-20"), "got: {collapsed}");
        // Exactly one ruler line per original scalar -- no byte ever gets its own line.
        assert_eq!(collapsed.lines().count(), 2);
    }

    #[test]
    fn t_collapsed_display_flat_deck_no_marks() {
        // A single unnested scalar (`Deck::from_value`) carries no marks at all.
        let original = Deck::<i32>::from_value(7);
        let handle = handle_for(original);
        let mut str_deck = Deck::<u8>::default();
        orc_sdk::to_str_deck::<i32>(&handle, &mut str_deck).unwrap();

        let mut collapsed = String::new();
        render_str_deck_raw(str_deck.items(), str_deck.marks(), &mut collapsed);
        assert!(collapsed.contains('7'), "got: {collapsed}");
        assert_eq!(collapsed.lines().count(), 1);
    }

    #[test]
    fn t_collapsed_display_empty_deck() {
        let original = Deck::<f64>::default();
        let handle = handle_for(original);
        let mut str_deck = Deck::<u8>::default();
        orc_sdk::to_str_deck::<f64>(&handle, &mut str_deck).unwrap();

        let mut collapsed = String::new();
        render_str_deck_raw(str_deck.items(), str_deck.marks(), &mut collapsed);
        assert_eq!(collapsed, "<empty_deck>\n");
    }

    /// End to end: constant(3) + constant(4) -> add -> inspect, run through the real thread
    /// pool via `exec::tick`, then confirm the Inspect node's cached text shows "7" -- this is
    /// the exact path (execution engine + inspect display together) a user would exercise by
    /// building this graph in the running app.
    #[test]
    fn t_inspect_shows_the_real_computed_value() {
        let _guard = crate::exec::POOL_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Some(add) = crate::PLUGIN_SET.get_function("add").cloned() else {
            println!("skipping: no plugin providing `add` was loaded");
            return;
        };
        let mut wf = orc_sdk::Workflow::default();
        let lhs = handle_for(Deck::<f64>::from_value(3.0));
        let rhs = handle_for(Deck::<f64>::from_value(4.0));
        let (_, lhs_out) = wf.add_constant(lhs).unwrap();
        let (_, rhs_out) = wf.add_constant(rhs).unwrap();
        let mut ins = vec![orc_sdk::IH::default(); 2];
        let mut outs = vec![orc_sdk::OH::default()];
        wf.add_function(add, &mut ins, &mut outs).unwrap();
        wf.connect(lhs_out, ins[0]).unwrap();
        wf.connect(rhs_out, ins[1]).unwrap();
        let mut inspect_ins = vec![orc_sdk::IH::default()];
        let inspect_nh = wf
            .add_inspect_node("inspect".to_string(), &mut inspect_ins)
            .unwrap();
        wf.connect(outs[0], inspect_ins[0]).unwrap();

        let mut state = EditorState::from_workflow(wf);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            crate::exec::tick(&mut state);
            refresh_all(&mut state);
            let done = state
                .inspect_cache
                .try_borrow()
                .map(|c| c[inspect_nh].text.contains('7'))
                .unwrap_or(false);
            if done {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "inspect text never showed 7");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Regression test: an Inspect node created unconnected, then wired up afterward (via
    /// `Workflow::connect` + `exec::mark_dirty`, the same calls `context_menu`/`interaction` make)
    /// must actually update away from "<not connected>" -- an earlier bug collapsed "not
    /// connected" and "not yet computed" into the same `Option<u64>` state internally, so the
    /// "skip if unchanged" check silently left the stale text in place once it had ever shown
    /// "<not connected>".
    #[test]
    fn t_refresh_updates_after_connecting_a_previously_unconnected_input() {
        let mut wf = orc_sdk::Workflow::default();
        let mut fn_ins = vec![];
        let mut fn_outs = vec![orc_sdk::OH::default()];
        wf.add_function(orc_sdk::FuncInfo::default(), &mut fn_ins, &mut fn_outs)
            .unwrap();
        let mut inspect_ins = vec![orc_sdk::IH::default()];
        let inspect_nh = wf
            .add_inspect_node("inspect".to_string(), &mut inspect_ins)
            .unwrap();

        let mut state = EditorState::from_workflow(wf);
        // `refresh_all` alone doesn't settle a node -- that's `exec::dispatch_ready`'s job (via
        // `exec::tick`), same as every real frame in `app.rs` calls `tick` before
        // `inspect::refresh_all`. A couple of ticks lets the Inspect node's trivial settle land.
        for _ in 0..3 {
            crate::exec::tick(&mut state);
            refresh_all(&mut state);
        }
        assert_eq!(
            state.inspect_cache.try_borrow().unwrap()[inspect_nh].text,
            "<not connected>"
        );

        state.workflow.connect(fn_outs[0], inspect_ins[0]).unwrap();
        crate::exec::mark_dirty(&mut state, inspect_nh);
        for _ in 0..3 {
            crate::exec::tick(&mut state);
            refresh_all(&mut state);
        }

        assert_ne!(
            state.inspect_cache.try_borrow().unwrap()[inspect_nh].text,
            "<not connected>",
            "must not still show 'not connected' after a real connection"
        );
    }

    /// Regression test: disconnecting a link *upstream* of the node an Inspect is watching (not
    /// the Inspect's own input) can leave that upstream node in a genuine error state -- e.g.
    /// `add` rejecting a now-empty input -- which per `exec`'s commit policy settles without
    /// touching `computed_outputs`. Without surfacing that error, the Inspect display would
    /// silently keep showing the old, now-meaningless value forever with no indication anything
    /// changed. It must show the upstream error instead.
    #[test]
    fn t_refresh_surfaces_an_upstream_error_instead_of_stale_data() {
        let _guard = crate::exec::POOL_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Some(add) = crate::PLUGIN_SET.get_function("add").cloned() else {
            println!("skipping: no plugin providing `add` was loaded");
            return;
        };
        let mut wf = orc_sdk::Workflow::default();
        let (_, lhs_out) = wf.add_constant(handle_for(Deck::from_value(3.0_f64))).unwrap();
        let (_, rhs_out) = wf.add_constant(handle_for(Deck::from_value(4.0_f64))).unwrap();
        let mut ins = vec![orc_sdk::IH::default(); 2];
        let mut outs = vec![orc_sdk::OH::default()];
        let sum_nh = wf.add_function(add, &mut ins, &mut outs).unwrap();
        wf.connect(lhs_out, ins[0]).unwrap();
        wf.connect(rhs_out, ins[1]).unwrap();
        let mut inspect_ins = vec![orc_sdk::IH::default()];
        let inspect_nh = wf
            .add_inspect_node("inspect".to_string(), &mut inspect_ins)
            .unwrap();
        wf.connect(outs[0], inspect_ins[0]).unwrap();

        let mut state = EditorState::from_workflow(wf);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            crate::exec::tick(&mut state);
            refresh_all(&mut state);
            if state.inspect_cache.try_borrow().unwrap()[inspect_nh]
                .text
                .contains('7')
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "never showed 7");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // Disconnect lhs -> add.ins[0], the same call `interaction.rs` makes on a rewire-away
        // gesture, which leaves `add` with a genuinely empty input.
        state.workflow.disconnect(lhs_out, ins[0]);
        crate::exec::mark_dirty(&mut state, sum_nh);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            crate::exec::tick(&mut state);
            refresh_all(&mut state);
            let text = state.inspect_cache.try_borrow().unwrap()[inspect_nh].text.clone();
            if text.starts_with("<upstream error") {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "never surfaced the upstream error, still showing: {text:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// A stale node (this edit's whole point) must show nothing the instant it's marked stale --
    /// not the old value lingering until something eventually recomputes. Checked *before* any
    /// further `tick`, so this only passes if `mark_dirty` alone (no dispatch yet) is enough to
    /// make the Inspect node's own settled-ness go false.
    #[test]
    fn t_disconnecting_upstream_blanks_the_display_immediately() {
        let _guard = crate::exec::POOL_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Some(add) = crate::PLUGIN_SET.get_function("add").cloned() else {
            println!("skipping: no plugin providing `add` was loaded");
            return;
        };
        let mut wf = orc_sdk::Workflow::default();
        let (_, lhs_out) = wf.add_constant(handle_for(Deck::from_value(3.0_f64))).unwrap();
        let (_, rhs_out) = wf.add_constant(handle_for(Deck::from_value(4.0_f64))).unwrap();
        let mut ins = vec![orc_sdk::IH::default(); 2];
        let mut outs = vec![orc_sdk::OH::default()];
        let sum_nh = wf.add_function(add, &mut ins, &mut outs).unwrap();
        wf.connect(lhs_out, ins[0]).unwrap();
        wf.connect(rhs_out, ins[1]).unwrap();
        let mut inspect_ins = vec![orc_sdk::IH::default()];
        let inspect_nh = wf
            .add_inspect_node("inspect".to_string(), &mut inspect_ins)
            .unwrap();
        wf.connect(outs[0], inspect_ins[0]).unwrap();

        let mut state = EditorState::from_workflow(wf);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            crate::exec::tick(&mut state);
            refresh_all(&mut state);
            if state.inspect_cache.try_borrow().unwrap()[inspect_nh]
                .text
                .contains('7')
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "never showed 7");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        state.workflow.disconnect(lhs_out, ins[0]);
        crate::exec::mark_dirty(&mut state, sum_nh);
        // No `tick` here -- `mark_dirty` alone must be enough to make the Inspect node stale.
        refresh_all(&mut state);

        assert_eq!(
            state.inspect_cache.try_borrow().unwrap()[inspect_nh].text,
            "",
            "a freshly-stale Inspect node must show nothing, not the old value"
        );
    }
}
