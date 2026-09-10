//! Dagger's own incremental, multi-threaded executor. Separate from `Workflow::run` (which
//! other hosts depend on as-is): walks every node rather than just declared workflow outputs,
//! only recomputes what's actually stale, and dispatches independent branches onto a small
//! worker pool. See PROJECT.org's "Incremental Execution" section for the full rationale.

use crate::state::EditorState;
use crate::{CANCEL_ARENA, HANDLE_COUNTER};
use orc_sdk::{
    DagHandle, Error, NH, NodeInfo, NodeProperty, OrcHandle, OrcPluginFunction, Workflow,
};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;

/// Everything a worker needs to run one node's function, without ever touching the live
/// `Workflow` (whose `Rc`/`RefCell`-based properties are not `Send`). Inputs are cloned
/// `Arc<OrcHandle>`s, not raw borrows — see "Handle lifetime across threads" in PROJECT.org for
/// why that's enough to keep them alive for exactly as long as the job needs them, with no
/// wrapper type and no `orc_sdk` changes.
struct NodeJob {
    node: NH,
    func: OrcPluginFunction,
    inputs: Vec<Arc<OrcHandle>>,
    n_outputs: usize,
    ctx: u64,
    launched_version: u64,
}

enum JobOutcome {
    Ok(Vec<OrcHandle>),
    Cancelled,
    Failed(Error),
}

struct NodeResult {
    node: NH,
    ctx: u64,
    launched_version: u64,
    outcome: JobOutcome,
}

struct Pool {
    job_tx: mpsc::Sender<NodeJob>,
    result_rx: Mutex<mpsc::Receiver<NodeResult>>,
}

/// A fixed-size pool, spawned once and shared for the process's whole lifetime. No thread
/// creation or teardown happens during interactive use.
fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(|| {
        let (job_tx, job_rx) = mpsc::channel::<NodeJob>();
        let (result_tx, result_rx) = mpsc::channel::<NodeResult>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let n_workers = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .saturating_sub(1)
            .max(1);
        for _ in 0..n_workers {
            let job_rx = Arc::clone(&job_rx);
            let result_tx = result_tx.clone();
            thread::spawn(move || worker_loop(&job_rx, &result_tx));
        }
        Pool {
            job_tx,
            result_rx: Mutex::new(result_rx),
        }
    })
}

fn worker_loop(job_rx: &Arc<Mutex<mpsc::Receiver<NodeJob>>>, result_tx: &mpsc::Sender<NodeResult>) {
    loop {
        let job = {
            let rx = job_rx.lock().unwrap();
            match rx.recv() {
                Ok(job) => job,
                // The sending half (the pool itself) only ever drops at process exit.
                Err(_) => return,
            }
        };
        if result_tx.send(run_job(job)).is_err() {
            return;
        }
    }
}

/// Runs one node's function to completion (or until it aborts via `check_cancellation`, if it
/// checks at all — entirely the plugin author's choice). `OrcHandleBorrowed` is derived from the
/// `Arc<OrcHandle>` inputs right here, locally, an ordinary compiler-checked borrow scoped to
/// this stack frame — it never has to cross a thread boundary itself.
fn run_job(job: NodeJob) -> NodeResult {
    let NodeJob {
        node,
        func,
        inputs,
        n_outputs,
        ctx,
        launched_version,
    } = job;
    let borrowed: Vec<orc_sdk::OrcHandleBorrowed> = inputs.iter().map(|h| h.borrowed()).collect();
    let mut outputs: Vec<OrcHandle> = (0..n_outputs)
        .map(|_| OrcHandle {
            handle: HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed),
            ..Default::default()
        })
        .collect();
    let outcome = match func {
        Some(f) => {
            let err = unsafe {
                f(
                    ctx,
                    borrowed.as_ptr().cast(),
                    borrowed.len() as u64,
                    outputs.as_mut_ptr(),
                    outputs.len() as u64,
                )
            };
            match Error::from_raw(err) {
                Ok(()) => JobOutcome::Ok(outputs),
                Err(Error::OperationCancelled) => JobOutcome::Cancelled,
                Err(e) => JobOutcome::Failed(e),
            }
        }
        None => JobOutcome::Failed(Error::InvalidFunction),
    };
    NodeResult {
        node,
        ctx,
        launched_version,
        outcome,
    }
}

#[derive(Clone, Copy)]
struct InFlightJob {
    ctx: u64,
    launched_version: u64,
}

/// Scheduling bookkeeping that isn't part of the plan's directly-named `EditorState` fields
/// (`computed_outputs`, `dirty_version`) — kept internal to this module.
pub(crate) struct ExecState {
    /// The `dirty_version` a node's current `computed_outputs`/`execution_error` reflects.
    /// Settled (nothing to do) iff this matches `dirty_version`.
    computed_version: NodeProperty<u64>,
    in_flight: NodeProperty<Option<InFlightJob>>,
    /// Nodes to (re)check for dispatch readiness. Populated by edits and by job completions —
    /// never scanned from a full pass over every node.
    pending: VecDeque<NH>,
}

impl ExecState {
    pub(crate) fn new(workflow: &mut Workflow) -> Self {
        Self {
            computed_version: workflow.create_node_property(),
            in_flight: workflow.create_node_property(),
            pending: VecDeque::new(),
        }
    }
}

/// Recomputes which nodes are part of a cycle and refreshes `state.node_in_cycle` (used both for
/// error styling in `render.rs` and to keep the executor from trying to schedule a cycle).
/// `layout::compute_layout` only ever does this once, at the first `measure`; edits after that
/// need their own refresh, which is why this lives here rather than being reused as-is.
fn recompute_cycles(state: &mut EditorState) {
    let depths = crate::layout::compute_depths(&state.workflow);
    if let Ok(mut flags) = state.node_in_cycle.try_borrow_mut() {
        for nh in state.workflow.node_iter() {
            flags[nh] = depths.in_cycle[nh.index()];
        }
    }
}

/// Bump every node's dirty version once, so a freshly loaded/created workflow actually gets
/// executed. Without this, `dirty_version` and `computed_version` both start at their common
/// default (0), and every node would look trivially "settled" despite having never run.
pub fn mark_all_dirty(state: &mut EditorState) {
    recompute_cycles(state);
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    if let Ok(mut dv) = state.dirty_version.try_borrow_mut() {
        for &nh in &nodes {
            dv[nh] += 1;
        }
    }
    state.exec.pending.extend(nodes);
}

/// Bump `nh`'s dirty version (something about it was just directly edited — connect, disconnect,
/// a new node, a constant's value) and propagate the invalidation forward to every downstream
/// node, so nothing keeps showing data computed before the edit. Also recomputes cycles, since
/// this is called on every topology edit.
///
/// Call this *before* deleting a node whose removal is what's invalidating its downstream
/// neighbors — `Workflow::delete_node` disconnects those links as part of deleting, so downstream
/// adjacency has to be captured first (see `interaction::delete_selected`).
pub fn mark_dirty(state: &mut EditorState, nh: NH) {
    recompute_cycles(state);
    let mut visited = HashSet::new();
    let mut stack = vec![nh];
    while let Some(n) = stack.pop() {
        if !visited.insert(n) {
            continue;
        }
        if let Ok(mut dv) = state.dirty_version.try_borrow_mut() {
            dv[n] += 1;
        }
        state.exec.pending.push_back(n);
        let outputs: Vec<orc_sdk::OH> = state.workflow.node_outputs(n).collect();
        for oh in outputs {
            stack.extend(state.workflow.downstream_nodes(oh));
        }
    }
}

pub fn is_settled(state: &EditorState, nh: NH) -> bool {
    match (
        state.dirty_version.try_borrow(),
        state.exec.computed_version.try_borrow(),
    ) {
        (Ok(dv), Ok(cv)) => dv[nh] == cv[nh],
        _ => false,
    }
}

/// A node is ready to dispatch once every input it actually has a source for is fully resolved:
/// a constant (always available), or a function/nested-call node that has already settled (not
/// merely "has some value" — dispatching against an upstream that's about to be superseded
/// anyway would just be wasted work redone a moment later). This is what makes a fast, cheap
/// chain recompute continuously while nothing blocks it, and a chain behind a slow node wait for
/// that node to actually finish rather than restarting redundantly every frame.
fn is_ready(state: &EditorState, nh: NH) -> bool {
    let in_flight = match state.exec.in_flight.try_borrow() {
        Ok(f) => f,
        Err(_) => return false,
    };
    if in_flight[nh].is_some() {
        return false;
    }
    let in_cycle = match state.node_in_cycle.try_borrow() {
        Ok(c) => c,
        Err(_) => return false,
    };
    if in_cycle[nh] {
        return false;
    }
    drop(in_flight);
    drop(in_cycle);

    let node_infos = state.workflow.node_info_prop();
    let node_infos = match node_infos.try_borrow() {
        Ok(n) => n,
        Err(_) => return false,
    };
    for ih in state.workflow.node_inputs(nh) {
        let Some(oh) = state.workflow.input_source(ih) else {
            continue;
        };
        let upstream = state.workflow.node_from_output(oh);
        match &node_infos[upstream] {
            NodeInfo::Constant(_) => {}
            NodeInfo::Function(_) => {
                if !is_settled(state, upstream) {
                    return false;
                }
            }
            // Nesting isn't executed by this engine yet, and an upstream Inspect node is
            // invalid (Inspect has no outputs, so this shouldn't be reachable) -- either way,
            // there's nothing for this input to ever resolve to, so never call it ready.
            NodeInfo::NestedCall { .. } | NodeInfo::Inspect { .. } => return false,
        }
    }
    true
}

/// Marks `nh` settled with no job at all -- used for node kinds that have nothing to compute
/// (Inspect) but still need to participate in staleness tracking, so anything watching them
/// (e.g. the Inspect display) can tell "caught up" apart from "still stale" instead of looking
/// permanently unsettled the moment `mark_dirty` ever touches them.
fn settle_trivially(state: &mut EditorState, nh: NH) {
    let Ok(dv) = state.dirty_version.try_borrow() else {
        return;
    };
    let version = dv[nh];
    drop(dv);
    if let Ok(mut cv) = state.exec.computed_version.try_borrow_mut() {
        cv[nh] = version;
    }
}

/// Gathers this node's current inputs into cloned `Arc<OrcHandle>`s and sends it to the pool.
fn dispatch(state: &mut EditorState, nh: NH) {
    let node_info_prop = state.workflow.node_info_prop();
    let Ok(node_infos) = node_info_prop.try_borrow() else {
        return;
    };
    let is_inspect = matches!(node_infos[nh], NodeInfo::Inspect { .. });
    let func = match &node_infos[nh] {
        NodeInfo::Function(info) => info.func,
        _ => {
            drop(node_infos);
            // `is_ready` already confirmed this node's (only) input, if any, is itself settled --
            // an Inspect node has nothing further to compute, so settling is immediate.
            if is_inspect {
                settle_trivially(state, nh);
            }
            return;
        }
    };
    if func.is_none() {
        return;
    }

    let Ok(computed_outputs) = state.computed_outputs.try_borrow() else {
        return;
    };
    let empty = Arc::<OrcHandle>::default();
    let mut inputs = Vec::new();
    for ih in state.workflow.node_inputs(nh) {
        let value = match state.workflow.input_source(ih) {
            // A Constant's value never goes through `computed_outputs` (nothing ever dispatches
            // a job for it), so it's cloned fresh for this job -- a real copy, unlike the
            // zero-copy `Arc` share used for a Function upstream's cached output.
            Some(oh) => match &node_infos[state.workflow.node_from_output(oh)] {
                NodeInfo::Constant(handle) => match crate::host_clone_orc_handle(handle.borrowed())
                {
                    Ok(cloned) => Arc::new(cloned),
                    Err(_) => return,
                },
                _ => Arc::clone(&computed_outputs[oh]),
            },
            None => Arc::clone(&empty),
        };
        inputs.push(value);
    }
    drop(computed_outputs);
    drop(node_infos);

    let n_outputs = state.workflow.node_outputs(nh).count();
    let launched_version = match state.dirty_version.try_borrow() {
        Ok(dv) => dv[nh],
        Err(_) => return,
    };
    let Ok(ctx) = CANCEL_ARENA.insert(|cancelled| *cancelled = false) else {
        return;
    };
    if let Ok(mut in_flight) = state.exec.in_flight.try_borrow_mut() {
        in_flight[nh] = Some(InFlightJob {
            ctx,
            launched_version,
        });
    } else {
        return;
    }

    let job = NodeJob {
        node: nh,
        func,
        inputs,
        n_outputs,
        ctx,
        launched_version,
    };
    // If the pool's receiving half is somehow gone, there's nothing left to do — the in_flight
    // marker will just sit there; this only happens during process shutdown.
    let _ = pool().job_tx.send(job);
}

fn poll_results(state: &mut EditorState) {
    let results: Vec<NodeResult> =
        std::iter::from_fn(|| pool().result_rx.lock().unwrap().try_recv().ok()).collect();
    for result in results {
        let _ = CANCEL_ARENA.consume(result.ctx, |_| {});
        if let Ok(mut in_flight) = state.exec.in_flight.try_borrow_mut() {
            // Only one job per node is ever in flight (`is_ready` won't dispatch a second one
            // until this slot clears), so a result should always match the job this node is
            // currently tracked as running.
            debug_assert!(
                in_flight[result.node]
                    .is_none_or(|job| job.ctx == result.ctx
                        && job.launched_version == result.launched_version),
                "a result arrived that doesn't match this node's tracked in-flight job"
            );
            in_flight[result.node] = None;
        }

        match result.outcome {
            JobOutcome::Ok(outputs) => {
                if let Ok(mut computed_outputs) = state.computed_outputs.try_borrow_mut() {
                    for (oh, handle) in state.workflow.node_outputs(result.node).zip(outputs) {
                        computed_outputs[oh] = Arc::new(handle);
                    }
                }
                if let Ok(mut cv) = state.exec.computed_version.try_borrow_mut() {
                    cv[result.node] = result.launched_version;
                }
                if let Ok(mut err) = state.execution_error.try_borrow_mut() {
                    err[result.node] = None;
                }
                for oh in state.workflow.node_outputs(result.node).collect::<Vec<_>>() {
                    state
                        .exec
                        .pending
                        .extend(state.workflow.downstream_nodes(oh));
                }
            }
            JobOutcome::Cancelled => {
                // Nothing to commit; the node is already marked dirty (that's why it got
                // superseded), so it's simply eligible for redispatch right away.
                state.exec.pending.push_back(result.node);
            }
            JobOutcome::Failed(e) => {
                if let Ok(mut cv) = state.exec.computed_version.try_borrow_mut() {
                    cv[result.node] = result.launched_version;
                }
                if let Ok(mut err) = state.execution_error.try_borrow_mut() {
                    err[result.node] = Some(e.to_string());
                }
                // This node still "settled" (just with a fault) -- anything downstream that was
                // waiting on it (e.g. an Inspect node, which only settles once its upstream
                // does) needs a chance to notice and re-check readiness, same as a real success.
                for oh in state.workflow.node_outputs(result.node).collect::<Vec<_>>() {
                    state
                        .exec
                        .pending
                        .extend(state.workflow.downstream_nodes(oh));
                }
            }
        }
    }
}

fn dispatch_ready(state: &mut EditorState) {
    while let Some(nh) = state.exec.pending.pop_front() {
        if is_settled(state, nh) {
            continue;
        }
        if !is_ready(state, nh) {
            continue;
        }
        dispatch(state, nh);
    }
}

/// True while any node has a job in flight -- gates the "in-progress" pulse in `render.rs` and
/// the continuous repaint that animates it and drains the results channel.
pub fn any_in_flight(state: &EditorState) -> bool {
    state
        .exec
        .in_flight
        .try_borrow()
        .map(|f| f.iter().any(|slot| slot.is_some()))
        .unwrap_or(false)
}

pub fn is_node_in_flight(state: &EditorState, nh: NH) -> bool {
    state
        .exec
        .in_flight
        .try_borrow()
        .map(|f| f[nh].is_some())
        .unwrap_or(false)
}

/// Drains completed results and dispatches whatever just became ready. Called once per frame.
pub fn tick(state: &mut EditorState) {
    poll_results(state);
    dispatch_ready(state);
}

/// `pool()` is a process-wide singleton -- correct for the real app (one `EditorState` per
/// process, for now), but Rust's default test harness runs tests concurrently within the same
/// process, and results carry only an `NH`, which is just a small index that collides across
/// separate `Workflow` instances. Two tests dispatching onto the real pool at the same time can
/// silently steal each other's results. Any test that touches `pool()` (directly or via
/// `tick`/`dispatch`) must hold this for its whole duration; tests that don't touch the pool are
/// unaffected and still run fully in parallel.
#[cfg(test)]
pub(crate) static POOL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod test {
    use super::*;
    use orc_sdk::{Deck, FuncInfo, IH, OH};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    fn node(wf: &mut Workflow, n_in: usize, n_out: usize) -> (NH, Vec<IH>, Vec<OH>) {
        let mut ins = vec![IH::default(); n_in];
        let mut outs = vec![OH::default(); n_out];
        let nh = wf
            .add_function(FuncInfo::default(), &mut ins, &mut outs)
            .unwrap();
        (nh, ins, outs)
    }

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

    #[test]
    fn t_mark_all_dirty_marks_every_node_unsettled() {
        let mut wf = Workflow::default();
        let (a, ..) = node(&mut wf, 0, 1);
        let (b, ..) = node(&mut wf, 1, 0);
        let mut state = EditorState::from_workflow(wf);
        assert!(!is_settled(&state, a));
        assert!(!is_settled(&state, b));
        // Idempotent: calling it again shouldn't un-settle an already-settled node in some
        // surprising way -- just bumps dirty_version further, still unsettled either way.
        mark_all_dirty(&mut state);
        assert!(!is_settled(&state, a));
    }

    #[test]
    fn t_mark_dirty_propagates_downstream_but_not_upstream_or_sideways() {
        // A -> B, C is unrelated.
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 1);
        let (b, b_in, _) = node(&mut wf, 1, 0);
        let (c, ..) = node(&mut wf, 0, 1);
        wf.connect(a_out[0], b_in[0]).unwrap();
        let mut state = EditorState::from_workflow(wf);

        // Settle everything by hand (as if a previous execution pass already ran to
        // completion), so the propagation below is the only thing that unsettles anything.
        {
            let dv = state.dirty_version.try_borrow().unwrap();
            let (dv_a, dv_b, dv_c) = (dv[a], dv[b], dv[c]);
            drop(dv);
            let mut cv = state.exec.computed_version.try_borrow_mut().unwrap();
            cv[a] = dv_a;
            cv[b] = dv_b;
            cv[c] = dv_c;
        }
        assert!(is_settled(&state, a));
        assert!(is_settled(&state, b));
        assert!(is_settled(&state, c));

        mark_dirty(&mut state, a);
        assert!(!is_settled(&state, a));
        assert!(!is_settled(&state, b), "downstream of the edited node");
        assert!(is_settled(&state, c), "unrelated node must be left alone");
    }

    #[test]
    fn t_is_ready_true_for_a_node_with_no_inputs() {
        let mut wf = Workflow::default();
        let (a, ..) = node(&mut wf, 0, 1);
        let state = EditorState::from_workflow(wf);
        assert!(is_ready(&state, a));
    }

    #[test]
    fn t_is_ready_true_when_upstream_is_a_constant() {
        let mut wf = Workflow::default();
        let (c, c_out) = constant(&mut wf, 1.0);
        let (b, b_in, _) = node(&mut wf, 1, 0);
        wf.connect(c_out, b_in[0]).unwrap();
        let state = EditorState::from_workflow(wf);
        let _ = c;
        assert!(is_ready(&state, b));
    }

    #[test]
    fn t_is_ready_false_when_upstream_function_is_not_settled() {
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 1);
        let (b, b_in, _) = node(&mut wf, 1, 0);
        wf.connect(a_out[0], b_in[0]).unwrap();
        let state = EditorState::from_workflow(wf);
        // Freshly loaded: nothing has executed yet, so A is unsettled and B must wait.
        assert!(!is_settled(&state, a));
        assert!(!is_ready(&state, b));
    }

    #[test]
    fn t_is_ready_false_for_a_node_in_a_cycle() {
        let mut wf = Workflow::default();
        let (a, a_in, a_out) = node(&mut wf, 1, 1);
        let (b, b_in, b_out) = node(&mut wf, 1, 1);
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], a_in[0]).unwrap();
        let mut state = EditorState::from_workflow(wf);
        mark_dirty(&mut state, a); // refreshes node_in_cycle
        assert!(!is_ready(&state, a));
        assert!(!is_ready(&state, b));
    }

    /// End to end: dispatches a real plugin function (`add`, if a plugin providing it is loaded
    /// next to the test binary -- see the identical skip in `main.rs`'s own ABI tests) across
    /// the real worker pool, and polls `tick` until it settles.
    #[test]
    fn t_dispatch_computes_a_real_function_via_the_pool() {
        let _guard = POOL_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(add) = crate::PLUGIN_SET.get_function("add").cloned() else {
            println!("skipping: no plugin providing `add` was loaded");
            return;
        };
        let mut wf = Workflow::default();
        let (lhs, lhs_out) = constant(&mut wf, 3.0);
        let (rhs, rhs_out) = constant(&mut wf, 4.0);
        let mut ins = vec![IH::default(); 2];
        let mut outs = vec![OH::default()];
        let sum = wf.add_function(add, &mut ins, &mut outs).unwrap();
        wf.connect(lhs_out, ins[0]).unwrap();
        wf.connect(rhs_out, ins[1]).unwrap();
        let _ = (lhs, rhs);

        let mut state = EditorState::from_workflow(wf);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            tick(&mut state);
            if is_settled(&state, sum) {
                break;
            }
            assert!(Instant::now() < deadline, "sum never settled");
            std::thread::sleep(Duration::from_millis(1));
        }
        let computed = state.computed_outputs.try_borrow().unwrap();
        let result = &computed[outs[0]];
        assert!(result.free_fn.is_some(), "a real value should be committed");
        assert_eq!(result.items::<f64>(), [7.0].as_slice());
    }
}
