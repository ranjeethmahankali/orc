use crate::state::EditorState;
use orc_sdk::{DagHandle, NH, Workflow};

const SPRING_K: f32 = 0.3;
const SPRING_REST_LENGTH: f32 = 250.0;
const REPULSION_STRENGTH: f32 = 5000.0;
const REPULSION_MARGIN: f32 = 30.0;
const DAMPING: f32 = 0.85;
const CENTER_Y_STRENGTH: f32 = 0.02;
const DAG_MIN_GAP: f32 = 220.0;
const DAG_CONSTRAINT_STRENGTH: f32 = 0.5;
const CONVERGENCE_THRESHOLD: f32 = 0.1;
const MAX_DISPLACEMENT: f32 = 50.0;

/// The nodes feeding a node, via its connected inputs.
fn predecessors(workflow: &Workflow, node: NH) -> impl Iterator<Item = NH> + '_ {
    workflow.node_inputs(node).filter_map(move |ih| {
        workflow
            .input_source(ih)
            .map(|oh| workflow.node_from_output(oh))
    })
}

/// Longest-path depth of every node, plus which nodes take part in a cycle. Both are indexed
/// by `NH::index()`.
pub struct Depths {
    pub depth: Vec<u32>,
    pub in_cycle: Vec<bool>,
}

/// Walk the graph upstream and give each node a depth one past its deepest predecessor.
///
/// This mirrors the Euler tour in `Workflow::run`: each node is pushed twice, and meeting a
/// node that is still on the current path means an edge closes a cycle. Unlike `run`, a cycle
/// is not an error here. The traversal stops at the closing edge and the node keeps whatever
/// depth its other predecessors gave it, so seeding still has an approximate position to work
/// from, and the nodes making up the cycle are reported so they can be drawn as an error.
pub fn compute_depths(workflow: &Workflow) -> Depths {
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

/// Seed node positions using topological depth (left-to-right) with vertical spread, and record
/// which nodes are part of a cycle so they can be drawn as an error.
pub fn topological_seed(state: &mut EditorState) {
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

    // Group nodes by depth layer, assign positions.
    let max_depth = depth.iter().copied().max().unwrap_or(0);
    let mut layer_counts = vec![0u32; (max_depth + 1) as usize];
    let mut layer_indices = vec![0u32; n_nodes];
    for &nh in &nodes {
        let d = depth[nh.index()] as usize;
        layer_indices[nh.index()] = layer_counts[d];
        layer_counts[d] += 1;
    }

    let mut pos = state.node_positions.try_borrow_mut().unwrap();
    for &nh in &nodes {
        let d = depth[nh.index()] as usize;
        let idx_in_layer = layer_indices[nh.index()] as f32;
        let layer_size = layer_counts[d] as f32;
        let x = 100.0 + d as f32 * DAG_MIN_GAP;
        let y = 100.0 + idx_in_layer * 120.0 - (layer_size - 1.0) * 60.0 + 200.0;
        pos[nh] = [x, y];
    }
}

/// Run one step of the force-directed layout simulation.
/// Returns true if the layout has converged (all displacements below threshold).
pub fn step(state: &mut EditorState) -> bool {
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    let n = nodes.len();
    if n == 0 {
        return true;
    }

    // Read current positions and sizes into local vecs for fast access.
    let (positions, sizes) = {
        let pos = state.node_positions.try_borrow().unwrap();
        let sz = state.node_sizes.try_borrow().unwrap();
        let positions: Vec<[f32; 2]> = nodes.iter().map(|&nh| pos[nh]).collect();
        let sizes: Vec<[f32; 2]> = nodes.iter().map(|&nh| sz[nh]).collect();
        (positions, sizes)
    };

    let mut forces = vec![[0.0f32; 2]; n];

    // 1. Bounding-box repulsion between all node pairs.
    for i in 0..n {
        let (ax, ay) = (positions[i][0], positions[i][1]);
        let (aw, ah) = (sizes[i][0], sizes[i][1]);
        for j in (i + 1)..n {
            let (bx, by) = (positions[j][0], positions[j][1]);
            let (bw, bh) = (sizes[j][0], sizes[j][1]);

            // Overlap with margin.
            let overlap_x =
                (aw + bw) / 2.0 + REPULSION_MARGIN - ((ax + aw / 2.0) - (bx + bw / 2.0)).abs();
            let overlap_y =
                (ah + bh) / 2.0 + REPULSION_MARGIN - ((ay + ah / 2.0) - (by + bh / 2.0)).abs();

            if overlap_x > 0.0 && overlap_y > 0.0 {
                // Nodes overlap — push apart.
                let cx_a = ax + aw / 2.0;
                let cy_a = ay + ah / 2.0;
                let cx_b = bx + bw / 2.0;
                let cy_b = by + bh / 2.0;
                let mut dx = cx_a - cx_b;
                let mut dy = cy_a - cy_b;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                dx /= dist;
                dy /= dist;
                let force = REPULSION_STRENGTH / (dist * dist).max(100.0);
                forces[i][0] += dx * force;
                forces[i][1] += dy * force;
                forces[j][0] -= dx * force;
                forces[j][1] -= dy * force;
            } else {
                // Even non-overlapping nodes get a mild repulsion.
                let cx_a = ax + aw / 2.0;
                let cy_a = ay + ah / 2.0;
                let cx_b = bx + bw / 2.0;
                let cy_b = by + bh / 2.0;
                let dx = cx_a - cx_b;
                let dy = cy_a - cy_b;
                let dist_sq = (dx * dx + dy * dy).max(100.0);
                let force = REPULSION_STRENGTH * 0.1 / dist_sq;
                let dist = dist_sq.sqrt();
                forces[i][0] += (dx / dist) * force;
                forces[i][1] += (dy / dist) * force;
                forces[j][0] -= (dx / dist) * force;
                forces[j][1] -= (dy / dist) * force;
            }
        }
    }

    // Build a NH index lookup (NH -> index in nodes vec).
    let max_idx = nodes.iter().map(|nh| nh.index()).max().unwrap_or(0);
    let mut nh_to_idx = vec![usize::MAX; max_idx + 1];
    for (i, &nh) in nodes.iter().enumerate() {
        nh_to_idx[nh.index()] = i;
    }

    // 2. Spring attraction along edges.
    for (i, &nh) in nodes.iter().enumerate() {
        for ih in state.workflow.node_inputs(nh) {
            if let Some(src_oh) = state.workflow.input_source(ih) {
                let src_nh = state.workflow.node_from_output(src_oh);
                let j = nh_to_idx[src_nh.index()];
                if j == usize::MAX {
                    continue;
                }

                let dx = positions[i][0] - positions[j][0];
                let dy = positions[i][1] - positions[j][1];
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let displacement = dist - SPRING_REST_LENGTH;
                let fx = SPRING_K * displacement * (dx / dist);
                let fy = SPRING_K * displacement * (dy / dist);
                forces[i][0] -= fx;
                forces[i][1] -= fy;
                forces[j][0] += fx;
                forces[j][1] += fy;
            }
        }
    }

    // 3. Vertical centering — pull all nodes gently toward a common y center.
    let avg_y: f32 = positions.iter().map(|p| p[1]).sum::<f32>() / n as f32;
    for i in 0..n {
        forces[i][1] += (avg_y - positions[i][1]) * CENTER_Y_STRENGTH;
    }

    // 4. DAG flow constraint — if a source node is not sufficiently left of its target,
    //    push both apart horizontally.
    for (i, &nh) in nodes.iter().enumerate() {
        for ih in state.workflow.node_inputs(nh) {
            if let Some(src_oh) = state.workflow.input_source(ih) {
                let src_nh = state.workflow.node_from_output(src_oh);
                let j = nh_to_idx[src_nh.index()];
                if j == usize::MAX {
                    continue;
                }
                // src (j) should be left of dst (i).
                let src_right = positions[j][0] + sizes[j][0];
                let dst_left = positions[i][0];
                let gap = dst_left - src_right;
                if gap < DAG_MIN_GAP * 0.3 {
                    let push = (DAG_MIN_GAP * 0.3 - gap) * DAG_CONSTRAINT_STRENGTH;
                    forces[i][0] += push;
                    forces[j][0] -= push;
                }
            }
        }
    }

    // Apply forces with damping and max displacement.
    let mut max_move: f32 = 0.0;
    let mut new_positions = positions.clone();
    for i in 0..n {
        let mut dx = forces[i][0] * DAMPING;
        let mut dy = forces[i][1] * DAMPING;
        let mag = (dx * dx + dy * dy).sqrt();
        if mag > MAX_DISPLACEMENT {
            let scale = MAX_DISPLACEMENT / mag;
            dx *= scale;
            dy *= scale;
        }
        new_positions[i][0] += dx;
        new_positions[i][1] += dy;
        max_move = max_move.max(mag);
    }

    // Write back.
    {
        let mut pos = state.node_positions.try_borrow_mut().unwrap();
        for (i, &nh) in nodes.iter().enumerate() {
            pos[nh] = new_positions[i];
        }
    }

    max_move < CONVERGENCE_THRESHOLD
}

#[cfg(test)]
mod test {
    use super::compute_depths;
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
