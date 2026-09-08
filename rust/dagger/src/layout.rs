use crate::state::EditorState;
use orc_sdk::{DagHandle, NH};

const SPRING_K: f32 = 0.3;
const SPRING_REST_LENGTH: f32 = 250.0;
const REPULSION_STRENGTH: f32 = 5000.0;
const REPULSION_MARGIN: f32 = 30.0;
const DAMPING: f32 = 0.85;
const CENTER_Y_STRENGTH: f32 = 0.02;
const DAG_MIN_GAP: f32 = 220.0;
const DAG_CONSTRAINT_STRENGTH: f32 = 0.5;
const CONVERGENCE_THRESHOLD: f32 = 0.5;
const MAX_DISPLACEMENT: f32 = 50.0;

/// Seed node positions using topological depth (left-to-right) with vertical spread.
pub fn topological_seed(state: &mut EditorState) {
    let n_nodes = state.workflow.num_nodes();
    if n_nodes == 0 {
        return;
    }

    // Compute topological depth for each node.
    // depth[nh.index()] = max depth of any input predecessor + 1, or 0 for sources.
    let mut depth = vec![0u32; n_nodes];
    let nodes: Vec<NH> = state.workflow.node_iter().collect();

    // Iterate until stable (simple relaxation — works for DAGs).
    let mut changed = true;
    while changed {
        changed = false;
        for &nh in &nodes {
            for ih in state.workflow.node_inputs(nh) {
                if let Some(src_oh) = state.workflow.input_source(ih) {
                    let src_nh = state.workflow.node_from_output(src_oh);
                    let new_depth = depth[src_nh.index()] + 1;
                    if new_depth > depth[nh.index()] {
                        depth[nh.index()] = new_depth;
                        changed = true;
                    }
                }
            }
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
