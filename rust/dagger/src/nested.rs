//! Opening/closing a `NestedCall` node's own nested workflow for interactive editing, in place --
//! no cloning. See PROJECT.org's "Phase 5: Nested Workflow Editing" for the full design and the
//! rationale for moving the real `Workflow` rather than cloning it.

use crate::exec;
use crate::state::EditorState;
use orc_sdk::{NH, NodeInfo, OrcHandle};
use std::sync::Arc;

/// Moves the real nested `Workflow` referenced by `nh` (a `NestedCall` node in `caller`) out of
/// `caller.workflow`'s nested workflows and wraps it in a fresh, fully ordinary `EditorState` --
/// genuine ownership transfer, not a clone, so nothing is left in `caller` aliasing it while it's
/// being edited (and `caller`'s own background execution of any node calling this name simply
/// finds nothing to run until it's put back, per `exec::dispatch_nested_call`).
///
/// Returns `None` if `nh` isn't (or no longer is) a `NestedCall` node, or if that name is already
/// checked out elsewhere -- opening a nested workflow that's already open in another window is
/// disallowed, and this is exactly why: there's nothing left to take.
pub fn open(caller: &mut EditorState, nh: NH) -> Option<(String, EditorState)> {
    let workflow_name = {
        let node_info_prop = caller.workflow.node_info_prop();
        let node_infos = node_info_prop.try_borrow().ok()?;
        match &node_infos[nh] {
            NodeInfo::NestedCall { workflow_name } => workflow_name.clone(),
            _ => return None,
        }
    };
    let nested_workflow = caller.workflow.take_nested_workflow(&workflow_name)?;

    let mut pushed = EditorState::from_workflow(nested_workflow);
    pushed.simulated_inputs = gather_current_inputs(caller, nh);
    Some((workflow_name, pushed))
}

/// Current argument values feeding `nh`'s own input pins in `caller`, gathered the same way
/// `exec::gather_inputs` gathers a Function node's inputs -- owned (a real clone for a `Constant`
/// upstream, an `Arc` share for a computed one), since the pushed editor outlives this call. Used
/// to resolve the nested workflow's own dangling workflow-input pins so it previews live data
/// instead of nothing; never written into the graph itself (see `EditorState::simulated_inputs`),
/// so nothing is ever baked in, and reopening a different caller of the same nested workflow just
/// gathers a different set of values.
fn gather_current_inputs(caller: &EditorState, nh: NH) -> Vec<Arc<OrcHandle>> {
    let node_info_prop = caller.workflow.node_info_prop();
    let (Ok(node_infos), Ok(computed_outputs)) = (
        node_info_prop.try_borrow(),
        caller.computed_outputs.try_borrow(),
    ) else {
        return Vec::new();
    };
    let empty = Arc::<OrcHandle>::default();
    caller
        .workflow
        .node_inputs(nh)
        .map(|ih| match caller.workflow.input_source(ih) {
            Some(oh) => exec::resolve_connected_value(
                &*node_infos,
                &*computed_outputs,
                &caller.workflow,
                oh,
            )
            .unwrap_or_else(|| Arc::clone(&empty)),
            None => Arc::clone(&empty),
        })
        .collect()
}

/// Moves `popped`'s (possibly edited) `Workflow` back into `parent`'s nested workflows under
/// `workflow_name` -- since nothing was ever cloned, this edit already happened in place, so
/// there's nothing to "save" here and no discard-vs-save decision to make; closing the window is
/// simply the commit. Bubbles `popped.dirty` up to `parent` (so the title-bar asterisk and the
/// existing save-prompt flow cover the whole stack unchanged), and marks every `NestedCall` node
/// in `parent` that references this name dirty -- not just the one node that was originally
/// double-clicked, since the definition is shared by every caller.
pub fn close(parent: &mut EditorState, workflow_name: String, popped: EditorState) {
    if popped.dirty {
        parent.dirty = true;
    }
    parent
        .workflow
        .put_nested_workflow(workflow_name.clone(), popped.workflow);

    let affected: Vec<NH> = {
        let node_info_prop = parent.workflow.node_info_prop();
        let Ok(node_infos) = node_info_prop.try_borrow() else {
            return;
        };
        parent
            .workflow
            .node_iter()
            .filter(|&nh| {
                matches!(&node_infos[nh], NodeInfo::NestedCall { workflow_name: n } if *n == workflow_name)
            })
            .collect()
    };
    for nh in affected {
        exec::mark_dirty(parent, nh);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use orc_sdk::{Deck, FuncInfo, IH, OH, PluginSet, Workflow};
    use std::sync::atomic::Ordering;

    fn constant(wf: &mut Workflow, value: f64) -> (NH, OH) {
        let mut handle = OrcHandle {
            handle: crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed),
            ..Default::default()
        };
        let mut deck = Deck::<f64>::default();
        deck.push(value, 0);
        crate::REGISTRY
            .alloc_with_value(Some(deck), &mut handle)
            .unwrap();
        wf.add_constant(handle).unwrap()
    }

    fn make_call_site() -> (EditorState, NH) {
        let mut outer = Workflow::default();
        let inner = Workflow::default();
        let ps = PluginSet::default();
        outer
            .push_nested_workflow("inner".to_string(), inner, &ps)
            .unwrap();
        let mut ihs = [IH::default(); 1];
        let mut ohs = [OH::default(); 1];
        let nh = outer
            .add_nested_workflow_call("inner", &mut ihs, &mut ohs)
            .unwrap();
        (EditorState::from_workflow(outer), nh)
    }

    #[test]
    fn t_open_moves_the_real_workflow_out_leaving_it_unregistered() {
        let (mut caller, nh) = make_call_site();
        assert!(caller.workflow.has_nested_workflow("inner"));

        let (name, _pushed) = open(&mut caller, nh).expect("should open");
        assert_eq!(name, "inner");
        assert!(
            !caller.workflow.has_nested_workflow("inner"),
            "the real workflow must be gone from the caller while checked out"
        );
    }

    #[test]
    fn t_open_twice_in_a_row_fails_the_second_time() {
        let (mut caller, nh) = make_call_site();
        let first = open(&mut caller, nh);
        assert!(first.is_some());
        // Already checked out -- nothing left to take.
        let second = open(&mut caller, nh);
        assert!(second.is_none());
    }

    #[test]
    fn t_open_returns_none_for_a_non_nested_call_node() {
        let mut wf = Workflow::default();
        let mut outs = [OH::default()];
        let nh = wf
            .add_function(FuncInfo::default(), &mut [], &mut outs)
            .unwrap();
        let mut state = EditorState::from_workflow(wf);
        assert!(open(&mut state, nh).is_none());
    }

    #[test]
    fn t_close_restores_the_workflow_and_caller_can_open_it_again() {
        let (mut caller, nh) = make_call_site();
        let (name, pushed) = open(&mut caller, nh).unwrap();
        close(&mut caller, name, pushed);
        assert!(caller.workflow.has_nested_workflow("inner"));
        assert!(open(&mut caller, nh).is_some());
    }

    #[test]
    fn t_close_bubbles_the_dirty_flag_up() {
        let (mut caller, nh) = make_call_site();
        caller.dirty = false;
        let (name, mut pushed) = open(&mut caller, nh).unwrap();
        pushed.dirty = true;
        close(&mut caller, name, pushed);
        assert!(
            caller.dirty,
            "an edit made inside the nested editor must mark the caller dirty too"
        );
    }

    #[test]
    fn t_close_does_not_dirty_the_caller_when_nothing_changed() {
        let (mut caller, nh) = make_call_site();
        caller.dirty = false;
        let (name, pushed) = open(&mut caller, nh).unwrap();
        assert!(!pushed.dirty);
        close(&mut caller, name, pushed);
        assert!(!caller.dirty);
    }

    #[test]
    fn t_close_marks_every_caller_of_the_same_name_dirty() {
        // `tick` polls the shared, process-wide pool's result channel regardless of whether this
        // test dispatched anything itself, so without this guard it can race with (and silently
        // steal a completed result from) any other test that's concurrently mid-dispatch. See
        // `exec::POOL_TEST_LOCK`'s own doc comment.
        let _guard = exec::POOL_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut outer = Workflow::default();
        let inner = Workflow::default();
        let ps = PluginSet::default();
        outer
            .push_nested_workflow("inner".to_string(), inner, &ps)
            .unwrap();
        let mut ihs_a = [IH::default(); 1];
        let mut ohs_a = [OH::default(); 1];
        let a = outer
            .add_nested_workflow_call("inner", &mut ihs_a, &mut ohs_a)
            .unwrap();
        let mut ihs_b = [IH::default(); 1];
        let mut ohs_b = [OH::default(); 1];
        let b = outer
            .add_nested_workflow_call("inner", &mut ihs_b, &mut ohs_b)
            .unwrap();
        let mut state = EditorState::from_workflow(outer);

        // Both call sites have no inputs of their own, so a single `tick` dispatches (and, given
        // `Workflow::run` runs synchronously, immediately settles) each right away -- with an
        // arity-mismatch error in this case (the inner workflow declares 0 outputs, not the 1 the
        // call node was created with), but settled either way, which is all this test needs.
        exec::tick(&mut state);
        assert!(exec::is_settled(&state, a));
        assert!(exec::is_settled(&state, b));

        let (name, pushed) = open(&mut state, a).unwrap();
        close(&mut state, name, pushed);

        assert!(!exec::is_settled(&state, a), "the opened call site");
        assert!(
            !exec::is_settled(&state, b),
            "a different call site referencing the same (now-changed) name"
        );
    }

    #[test]
    fn t_gather_current_inputs_captures_a_constant_value() {
        let mut outer = Workflow::default();
        let (_, out) = constant(&mut outer, 42.0);
        let inner = Workflow::default();
        let ps = PluginSet::default();
        outer
            .push_nested_workflow("inner".to_string(), inner, &ps)
            .unwrap();
        let mut ihs = [IH::default(); 1];
        let mut ohs = [OH::default(); 1];
        let nh = outer
            .add_nested_workflow_call("inner", &mut ihs, &mut ohs)
            .unwrap();
        outer.connect(out, ihs[0]).unwrap();
        let mut state = EditorState::from_workflow(outer);

        let (_, pushed) = open(&mut state, nh).unwrap();
        assert_eq!(pushed.simulated_inputs.len(), 1);
        assert_eq!(
            pushed.simulated_inputs[0].items::<f64>().unwrap(),
            [42.0].as_slice()
        );
    }

    #[test]
    fn t_gather_current_inputs_is_empty_handle_for_a_dangling_pin() {
        let (mut caller, nh) = make_call_site();
        let (_, pushed) = open(&mut caller, nh).unwrap();
        assert_eq!(pushed.simulated_inputs.len(), 1);
        assert!(pushed.simulated_inputs[0].free_fn.is_none());
    }

    /// End-to-end sanity check against the real generated fixture (`workflows/gen_test.py`'s
    /// `nested_demo_workflow`: a `double_add(x, y)` call, `double_add` itself being
    /// `add(add(a, b), add(a, b))`) -- not run by default (the file is git-ignored and only
    /// exists after `python workflows/gen_test.py`, same as every other `workflows/*.orc`
    /// fixture), kept `#[ignore]`d as a manual double-check that opening, editing, and closing a
    /// real nested workflow loaded from disk actually computes, and doesn't just pass in
    /// hand-built unit tests.
    #[test]
    #[ignore = "requires workflows/nested_demo.orc, generated by `python workflows/gen_test.py`"]
    fn t_real_fixture_opens_edits_and_recomputes_end_to_end() {
        let _guard = exec::POOL_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../workflows/nested_demo.orc");
        let workflow = crate::file_menu::open_workflow(&path)
            .expect("run `python workflows/gen_test.py` first");
        assert!(workflow.has_nested_workflow("double_add"));

        let call_nh = {
            let node_info_prop = workflow.node_info_prop();
            let node_infos = node_info_prop.try_borrow().unwrap();
            workflow
                .node_iter()
                .find(|&nh| {
                    matches!(&node_infos[nh], NodeInfo::NestedCall { workflow_name } if workflow_name == "double_add")
                })
                .expect("nested_demo_workflow calls double_add")
        };

        let mut state = EditorState::from_workflow(workflow);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            exec::tick(&mut state);
            if exec::is_settled(&state, call_nh) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "double_add call never settled"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        {
            let err = state.execution_error.try_borrow().unwrap();
            assert!(
                err[call_nh].is_none(),
                "unexpected error: {:?}",
                err[call_nh]
            );
        }
        let output = state
            .workflow
            .node_outputs(call_nh)
            .next()
            .expect("double_add has an output");
        let first_result = {
            let computed = state.computed_outputs.try_borrow().unwrap();
            assert!(
                computed[output].free_fn.is_some(),
                "the call's output must actually be computed"
            );
            computed[output].items::<f64>().unwrap().to_vec()
        };

        // Open it, confirm the simulated inputs reflect the real call-site values, close it
        // without changing anything, and confirm the parent recomputes to the same answer.
        let (name, pushed) = open(&mut state, call_nh).expect("should open");
        assert_eq!(name, "double_add");
        assert_eq!(pushed.simulated_inputs.len(), 2, "double_add takes a, b");
        close(&mut state, name, pushed);
        assert!(
            !exec::is_settled(&state, call_nh),
            "closing must invalidate the call site so it recomputes"
        );

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            exec::tick(&mut state);
            if exec::is_settled(&state, call_nh) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "double_add call never resettled after close"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let recomputed = state.computed_outputs.try_borrow().unwrap();
        assert_eq!(
            recomputed[output].items::<f64>().unwrap(),
            first_result.as_slice(),
            "recomputing the same, unedited definition must give the same answer"
        );
    }
}
