use crate::state::EditorState;
use orc_sdk::{DagHandle, NH, Workflow};

/// Top-left starting point for the depth-0 layer, in canvas units. Purely a visual choice for
/// where the layout starts before the user ever pans.
const LAYOUT_ORIGIN_X: f32 = 100.0;
const LAYOUT_ORIGIN_Y: f32 = 300.0;
/// Breathing room between adjacent layers' bounding boxes, beyond their actual half-widths.
const LAYOUT_LAYER_GAP: f32 = 100.0;
/// Breathing room between adjacent siblings within a layer, beyond their actual half-heights.
const LAYOUT_ROW_GAP: f32 = 30.0;
/// Nominal row spacing used only to order depth-0 nodes before they have anything upstream to
/// align to (see the `d == 0` branch below) — every layer, including this one, still gets
/// resolved to real per-node spacing by the overlap-removal pass right after, so this only
/// needs to be a reasonable rough guess, not an accurate one.
const LAYOUT_INITIAL_ROW_STEP: f32 = 90.0;

/// The nodes feeding a node, via its connected inputs.
fn predecessors(workflow: &Workflow, node: NH) -> impl Iterator<Item = NH> + '_ {
    workflow.node_inputs(node).filter_map(move |ih| {
        workflow
            .input_source(ih)
            .map(|oh| workflow.node_from_output(oh))
    })
}

/// Longest-path depth of every node, plus which nodes take part in a cycle. Both are indexed
/// by `NH::index()`. Only used within this module (by `compute_layout` and its own tests).
struct Depths {
    depth: Vec<u32>,
    in_cycle: Vec<bool>,
}

/// Walk the graph upstream and give each node a depth one past its deepest predecessor.
///
/// This mirrors the Euler tour in `Workflow::run`: each node is pushed twice, and meeting a
/// node that is still on the current path means an edge closes a cycle. Unlike `run`, a cycle
/// is not an error here. The traversal stops at the closing edge and the node keeps whatever
/// depth its other predecessors gave it, so seeding still has an approximate position to work
/// from, and the nodes making up the cycle are reported so they can be drawn as an error.
fn compute_depths(workflow: &Workflow) -> Depths {
    let n_nodes = workflow.num_nodes();
    let mut depth = vec![0u32; n_nodes];
    let mut in_cycle = vec![false; n_nodes];
    let mut finished = vec![false; n_nodes];
    let mut on_current_path = vec![false; n_nodes];
    // The nodes on the current path, so that a closing edge can name the cycle it closes.
    // Mirrors `on_current_path`, but ordered.
    let mut path = Vec::<NH>::new();
    let mut stack = Vec::<(NH, bool)>::new();

    // Every node is a starting point, so that arms not reachable from the workflow outputs are
    // laid out too. `Workflow::run` only needs to start from the outputs.
    for root in workflow.node_iter() {
        if finished[root.index()] {
            continue;
        }
        stack.push((root, false));
        while let Some((node, visited_children)) = stack.pop() {
            if finished[node.index()] {
                continue;
            }
            if visited_children {
                // Every predecessor we were willing to walk has finished, so its depth is final.
                let deepest = predecessors(workflow, node)
                    .map(|pred| depth[pred.index()] + 1)
                    .max()
                    .unwrap_or(0);
                depth[node.index()] = deepest;
                finished[node.index()] = true;
                on_current_path[node.index()] = false;
                path.pop();
            } else if on_current_path[node.index()] {
                // This edge closes a cycle. Flag the nodes it runs through and stop here, since
                // walking into it would not terminate.
                let start = path.iter().rposition(|&n| n == node).unwrap_or(0);
                for &member in &path[start..] {
                    in_cycle[member.index()] = true;
                }
            } else {
                stack.push((node, true));
                on_current_path[node.index()] = true;
                path.push(node);
                stack.extend(predecessors(workflow, node).map(|pred| (pred, false)));
            }
        }
    }

    Depths { depth, in_cycle }
}

/// Compute the entire node layout: a static, one-shot Sugiyama-style layered placement. Nodes
/// are grouped into depth layers left to right, and within a layer each node's y is the
/// average y of its already-placed predecessors (its "barycenter"), so it lands close to
/// where its actual neighbors are rather than at some index-driven position independent of the
/// graph's shape. Also records which nodes are part of a cycle so they can be drawn as an
/// error.
///
/// This is the *only* layout mechanism — there is no ongoing force simulation to iron out a
/// rough placement afterward, so this has to be the final answer. Dragging moves a node
/// directly and nothing else reacts; newly created nodes are placed at the click position and
/// stay there. This only runs when the whole graph needs laying out from scratch: once from
/// `EditorState::measure`, right after real sizes are measured (which needs a live frame, so
/// it can't happen at construction) — never in response to a drag, a delete, or a new node,
/// since those are meant to leave every other node's position alone. Assumes `state.node_sizes`
/// already holds real sizes for every node; callers (tests included) must measure or set them
/// first.
pub fn compute_layout(state: &mut EditorState) {
    let n_nodes = state.workflow.num_nodes();
    if n_nodes == 0 {
        return;
    }

    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    let Depths { depth, in_cycle } = compute_depths(&state.workflow);

    if let Ok(mut flags) = state.node_in_cycle.try_borrow_mut() {
        for &nh in &nodes {
            flags[nh] = in_cycle[nh.index()];
        }
    }

    let max_depth = depth.iter().copied().max().unwrap_or(0) as usize;
    let mut layers: Vec<Vec<NH>> = vec![Vec::new(); max_depth + 1];
    for &nh in &nodes {
        layers[depth[nh.index()] as usize].push(nh);
    }

    let sizes = state.node_sizes.try_borrow().unwrap();
    let mut pos = state.node_positions.try_borrow_mut().unwrap();
    let mut x = LAYOUT_ORIGIN_X;
    let mut prev_half_width = 0.0f32;
    for (d, layer) in layers.iter().enumerate() {
        if layer.is_empty() {
            continue;
        }
        let half_width = layer.iter().map(|&nh| sizes[nh][0]).fold(0.0f32, f32::max) / 2.0;
        if d > 0 {
            x += prev_half_width + LAYOUT_LAYER_GAP + half_width;
        }
        prev_half_width = half_width;

        // Barycenter: each node starts at the average y of its predecessors, which were
        // seeded in an earlier iteration of this same loop (depth strictly increases).
        // Depth-0 nodes have no predecessors at all, so they fall back to an even spread
        // around a shared center — there's nothing upstream to align to yet.
        let mut targets: Vec<(NH, f32)> = layer
            .iter()
            .map(|&nh| {
                let mut sum = 0.0f32;
                let mut count = 0u32;
                for pred in predecessors(&state.workflow, nh) {
                    sum += pos[pred][1];
                    count += 1;
                }
                let y = if count > 0 {
                    sum / count as f32
                } else {
                    0.0
                };
                (nh, y)
            })
            .collect();
        if d == 0 {
            let n = targets.len() as f32;
            for (i, (_, y)) in targets.iter_mut().enumerate() {
                *y = LAYOUT_ORIGIN_Y
                    + (i as f32 - (n - 1.0) / 2.0) * (LAYOUT_INITIAL_ROW_STEP + LAYOUT_ROW_GAP);
            }
        }
        let target_mean = targets.iter().map(|&(_, y)| y).sum::<f32>() / targets.len() as f32;

        // Resolve overlaps: two nodes that share a parent start at the same barycenter, so
        // without this pass they'd be seeded exactly on top of each other. Sorting by the
        // barycenter and pushing each node just far enough below the previous one keeps
        // siblings in their natural relative order while guaranteeing real separation.
        targets.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let mut prev_bottom: Option<f32> = None;
        for (nh, y) in targets.iter_mut() {
            let half_height = sizes[*nh][1] / 2.0;
            if let Some(bottom) = prev_bottom {
                let floor = bottom + LAYOUT_ROW_GAP + half_height;
                if *y < floor {
                    *y = floor;
                }
            }
            prev_bottom = Some(*y + half_height);
        }
        // The push-down pass only ever moves nodes later (down), which drifts the whole
        // layer away from where its parents actually pointed. Re-center on the original
        // barycenter mean so relative spacing (just established above) is preserved but the
        // layer as a whole stays where its neighbors expect it.
        let adjusted_mean = targets.iter().map(|&(_, y)| y).sum::<f32>() / targets.len() as f32;
        let recenter = target_mean - adjusted_mean;

        for (nh, y) in targets {
            pos[nh] = [x, y + recenter];
        }
    }
}

#[cfg(test)]
mod test {
    use super::{compute_depths, compute_layout};
    use crate::state::EditorState;
    use orc_sdk::{DagHandle, FuncInfo, IH, NH, OH, Workflow};

    /// Add a function node with the given pin counts, returning its handle and pins.
    fn node(wf: &mut Workflow, n_in: usize, n_out: usize) -> (NH, Vec<IH>, Vec<OH>) {
        let mut ins = vec![IH::default(); n_in];
        let mut outs = vec![OH::default(); n_out];
        let nh = wf
            .add_function(FuncInfo::default(), &mut ins, &mut outs)
            .unwrap();
        (nh, ins, outs)
    }

    /// `compute_layout` is the *only* layout mechanism — there's no force simulation to correct
    /// a bad placement afterward — so asserting its shape here matters more than it used to.
    /// Asserts relative properties of the formula rather than exact coordinates, so retuning
    /// the layout constants doesn't break this.
    #[test]
    fn t_compute_layout_places_layers_left_to_right_and_centers_each_layer() {
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 3);
        // Three siblings at depth 1, all fed by `a`, none connected to each other.
        let (b1, b1_in, _) = node(&mut wf, 1, 0);
        let (b2, b2_in, _) = node(&mut wf, 1, 0);
        let (b3, b3_in, _) = node(&mut wf, 1, 0);
        wf.connect(a_out[0], b1_in[0]).unwrap();
        wf.connect(a_out[1], b2_in[0]).unwrap();
        wf.connect(a_out[2], b3_in[0]).unwrap();

        let mut state = EditorState::from_workflow(wf);
        {
            let mut sizes = state.node_sizes.try_borrow_mut().unwrap();
            for nh in [a, b1, b2, b3] {
                sizes[nh] = [160.0, 80.0];
            }
        }
        compute_layout(&mut state);
        let pos = state.node_positions.try_borrow().unwrap();

        assert!(
            pos[b1][0] > pos[a][0],
            "depth 1 must be seeded to the right of depth 0"
        );
        assert_eq!(
            pos[b1][0], pos[b2][0],
            "siblings in the same layer share an x"
        );
        assert_eq!(pos[b2][0], pos[b3][0]);

        let ys = [pos[b1][1], pos[b2][1], pos[b3][1]];
        assert_ne!(ys[0], ys[1], "siblings must not be stacked on top of each other");
        assert_ne!(ys[1], ys[2]);
        let center = (ys[0] + ys[2]) / 2.0;
        assert!(
            (ys[1] - center).abs() < 0.01,
            "the middle of three siblings should sit on the layer's center, got {ys:?}"
        );
    }

    #[test]
    fn t_chain_depths_increase_downstream() {
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 1);
        let (b, b_in, b_out) = node(&mut wf, 1, 1);
        let (c, c_in, _) = node(&mut wf, 1, 0);
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], c_in[0]).unwrap();

        let depths = compute_depths(&wf);
        assert_eq!(depths.depth[a.index()], 0);
        assert_eq!(depths.depth[b.index()], 1);
        assert_eq!(depths.depth[c.index()], 2);
        assert!(depths.in_cycle.iter().all(|flagged| !flagged));
    }

    #[test]
    fn t_depth_follows_the_longest_path() {
        // A feeds both B and the far side of C, so C sits one past B, not one past A.
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 1);
        let (b, b_in, b_out) = node(&mut wf, 1, 1);
        let (c, c_in, _) = node(&mut wf, 2, 0);
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], c_in[0]).unwrap();
        wf.connect(a_out[0], c_in[1]).unwrap();

        let depths = compute_depths(&wf);
        assert_eq!(depths.depth[a.index()], 0);
        assert_eq!(depths.depth[b.index()], 1);
        assert_eq!(depths.depth[c.index()], 2);
        assert!(depths.in_cycle.iter().all(|flagged| !flagged));
    }

    #[test]
    fn t_two_node_cycle_terminates_and_is_flagged() {
        let mut wf = Workflow::default();
        let (a, a_in, a_out) = node(&mut wf, 1, 1);
        let (b, b_in, b_out) = node(&mut wf, 1, 1);
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], a_in[0]).unwrap();

        let depths = compute_depths(&wf);
        assert!(depths.in_cycle[a.index()]);
        assert!(depths.in_cycle[b.index()]);
    }

    #[test]
    fn t_self_loop_is_flagged() {
        let mut wf = Workflow::default();
        let (a, a_in, a_out) = node(&mut wf, 1, 1);
        wf.connect(a_out[0], a_in[0]).unwrap();

        let depths = compute_depths(&wf);
        assert!(depths.in_cycle[a.index()]);
    }

    /// Only the nodes the cycle actually runs through are flagged, not the acyclic arm feeding
    /// into it. This is what separates the reported cycle from the whole search path.
    #[test]
    fn t_nodes_feeding_a_cycle_are_not_flagged() {
        let mut wf = Workflow::default();
        let (feeder, _, feeder_out) = node(&mut wf, 0, 1);
        let (a, a_in, a_out) = node(&mut wf, 2, 1);
        let (b, b_in, b_out) = node(&mut wf, 1, 1);
        wf.connect(feeder_out[0], a_in[0]).unwrap();
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], a_in[1]).unwrap();

        let depths = compute_depths(&wf);
        assert!(depths.in_cycle[a.index()]);
        assert!(depths.in_cycle[b.index()]);
        assert!(
            !depths.in_cycle[feeder.index()],
            "the acyclic feeder must not be reported as part of the cycle"
        );
    }

    /// A cycle sitting in an arm that no workflow output reaches is still found, because seeding
    /// has to place those nodes too. `Workflow::run` would never visit them.
    #[test]
    fn t_cycle_in_a_dangling_arm_is_found() {
        let mut wf = Workflow::default();
        let (reachable, _, _) = node(&mut wf, 0, 1);
        let (a, a_in, a_out) = node(&mut wf, 1, 1);
        let (b, b_in, b_out) = node(&mut wf, 1, 1);
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], a_in[0]).unwrap();

        let depths = compute_depths(&wf);
        assert!(!depths.in_cycle[reachable.index()]);
        assert!(depths.in_cycle[a.index()]);
        assert!(depths.in_cycle[b.index()]);
    }

}

