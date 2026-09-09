use crate::state::EditorState;
use orc_sdk::{DagHandle, NH, Workflow};

const SPRING_K: f32 = 0.3;
const SPRING_REST_LENGTH: f32 = 250.0;
const REPULSION_STRENGTH: f32 = 5000.0;
const REPULSION_MARGIN: f32 = 30.0;
/// Coefficient for the extra `overlap^2` push once two node boxes (plus margin) actually
/// overlap. See `repulsion_force` for why this is squared rather than linear.
const OVERLAP_PUSH_STRENGTH: f32 = 0.03;
const DAMPING: f32 = 0.5;
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

/// Repulsion between one pair of node boxes, as the force to apply to `a` (the force on `b` is
/// its negation).
///
/// A weak inverse-square background repulsion applies at every separation, so nodes never drift
/// arbitrarily close under some other attractive force (e.g. up/down neighbors at the same DAG
/// depth, which — unlike DAG-connected neighbors — have no spring pulling them apart and only
/// this repulsion to keep them from coasting together). On top of that, once the boxes (plus
/// margin) actually overlap, an extra push ramps in *quadratically* with how deep the
/// penetration is.
///
/// The quadratic ramp matters, not just its zero value at the boundary: a linear ramp is
/// continuous too, but starts contributing at its full slope the instant `overlap` turns
/// positive, which is still a sharp kink relative to the tiny background slope right next to
/// it — close enough to a jump in practice that neighbors could still be seen to clip and snap.
/// A quadratic starts with *zero* slope as well as zero value at the boundary, so the curve
/// really is smooth there, and only grows firm once the penetration is significant. A previous
/// version used an entirely different, ~10x stronger formula the instant boxes overlapped
/// instead of blending anything in, which is what caused the clip-and-snap bug this replaces.
#[inline]
fn repulsion_force(pos_a: [f32; 2], size_a: [f32; 2], pos_b: [f32; 2], size_b: [f32; 2]) -> [f32; 2] {
    let cx_a = pos_a[0] + size_a[0] / 2.0;
    let cy_a = pos_a[1] + size_a[1] / 2.0;
    let cx_b = pos_b[0] + size_b[0] / 2.0;
    let cy_b = pos_b[1] + size_b[1] / 2.0;

    let dx = cx_a - cx_b;
    let dy = cy_a - cy_b;
    let dist_sq = (dx * dx + dy * dy).max(100.0);
    let dist = dist_sq.sqrt();

    let mut force = REPULSION_STRENGTH * 0.1 / dist_sq;

    let overlap_x = (size_a[0] + size_b[0]) / 2.0 + REPULSION_MARGIN - dx.abs();
    let overlap_y = (size_a[1] + size_b[1]) / 2.0 + REPULSION_MARGIN - dy.abs();
    let overlap = overlap_x.min(overlap_y);
    if overlap > 0.0 {
        force += OVERLAP_PUSH_STRENGTH * overlap * overlap;
    }

    [dx / dist * force, dy / dist * force]
}

/// Run one step of the force-directed layout simulation.
///
/// `pinned` is the node being actively dragged this frame, if any. Its position still feeds
/// into the forces on every other node, so neighbours keep reacting to it live, but it is
/// excluded from the displacement step itself — otherwise the simulation would keep pulling it
/// back toward equilibrium in the same frame the cursor is pushing it away, and the drag would
/// feel like a tug-of-war instead of tracking the mouse.
///
/// Returns true if the layout has converged (all displacements below threshold).
pub fn step(state: &mut EditorState, pinned: Option<NH>) -> bool {
    let nodes: Vec<NH> = state.workflow.node_iter().collect();
    let n = nodes.len();
    if n == 0 {
        return true;
    }

    // Read current positions, sizes and velocities into local vecs for fast access.
    let (positions, sizes, velocities) = {
        let pos = state.node_positions.try_borrow().unwrap();
        let sz = state.node_sizes.try_borrow().unwrap();
        let vel = state.node_velocities.try_borrow().unwrap();
        let positions: Vec<[f32; 2]> = nodes.iter().map(|&nh| pos[nh]).collect();
        let sizes: Vec<[f32; 2]> = nodes.iter().map(|&nh| sz[nh]).collect();
        let velocities: Vec<[f32; 2]> = nodes.iter().map(|&nh| vel[nh]).collect();
        (positions, sizes, velocities)
    };

    let mut forces = vec![[0.0f32; 2]; n];

    // 1. Bounding-box repulsion between all node pairs. See `repulsion_force` for why this is
    // one continuous formula rather than a hard switch between "overlapping" and "not".
    for i in 0..n {
        for j in (i + 1)..n {
            let f = repulsion_force(positions[i], sizes[i], positions[j], sizes[j]);
            forces[i][0] += f[0];
            forces[i][1] += f[1];
            forces[j][0] -= f[0];
            forces[j][1] -= f[1];
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

    // Integrate velocity, then position from velocity, rather than moving directly by
    // `force * DAMPING` each step. A position-only update has no memory of which way a node was
    // already headed, so once forces roughly balance near equilibrium it has nothing to smooth
    // out small frame-to-frame imbalances in the force calculation — it reacts to each one
    // independently, which shows up as a persistent low-amplitude jitter that's easy to miss
    // while there's large sweeping motion elsewhere, but is the only thing left to see once
    // everything else has settled. Carrying a damped velocity between steps means a node's
    // motion is an average over recent forces instead of a direct copy of the latest one, so
    // that noise gets smoothed out instead of re-appearing every frame.
    let pinned_idx = pinned.and_then(|nh| {
        let idx = nh_to_idx[nh.index()];
        (idx != usize::MAX).then_some(idx)
    });
    let mut max_move: f32 = 0.0;
    let mut new_positions = positions.clone();
    let mut new_velocities = velocities.clone();
    for i in 0..n {
        if Some(i) == pinned_idx {
            // Actively dragged this frame: the cursor has full control of its position. Zero
            // its velocity so that letting go doesn't fling it off with whatever velocity it
            // happened to have before the drag started — it resumes from rest.
            new_velocities[i] = [0.0, 0.0];
            continue;
        }
        let mut vx = (velocities[i][0] + forces[i][0]) * DAMPING;
        let mut vy = (velocities[i][1] + forces[i][1]) * DAMPING;
        let speed = (vx * vx + vy * vy).sqrt();
        if speed > MAX_DISPLACEMENT {
            let scale = MAX_DISPLACEMENT / speed;
            vx *= scale;
            vy *= scale;
        }
        new_velocities[i] = [vx, vy];
        new_positions[i][0] += vx;
        new_positions[i][1] += vy;
        max_move = max_move.max(speed);
    }

    // Write back.
    {
        let mut pos = state.node_positions.try_borrow_mut().unwrap();
        let mut vel = state.node_velocities.try_borrow_mut().unwrap();
        for (i, &nh) in nodes.iter().enumerate() {
            pos[nh] = new_positions[i];
            vel[nh] = new_velocities[i];
        }
    }

    max_move < CONVERGENCE_THRESHOLD
}

#[cfg(test)]
mod test {
    use super::{compute_depths, repulsion_force, step};
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

    /// A node being dragged must track the cursor exactly. If the physics step also moved it,
    /// a drag away from equilibrium would fight the large spring force pulling it back, and the
    /// drag would feel jerky instead of tracking the mouse 1:1.
    #[test]
    fn t_pinned_node_is_excluded_from_the_displacement() {
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 1);
        let (b, b_in, _) = node(&mut wf, 1, 0);
        wf.connect(a_out[0], b_in[0]).unwrap();
        let mut state = EditorState::from_workflow(wf);
        {
            let mut pos = state.node_positions.try_borrow_mut().unwrap();
            pos[a] = [0.0, 0.0];
            pos[b] = [2000.0, 0.0];
        }
        step(&mut state, Some(a));
        let pos = state.node_positions.try_borrow().unwrap();
        assert_eq!(
            pos[a],
            [0.0, 0.0],
            "the pinned node must not move under physics"
        );
        assert_ne!(
            pos[b],
            [2000.0, 0.0],
            "the unpinned node should still react to the force"
        );
    }

    /// With 160x80 boxes and no x offset, the vertical overlap-with-margin boundary sits at a
    /// gap of exactly `(80 + 80) / 2 + REPULSION_MARGIN` = 110. Sampling the force right on
    /// either side of that boundary directly (bypassing `step`'s spring/centering forces, which
    /// would otherwise swamp the tiny repulsion values involved) catches the exact bug this
    /// guards against: the old formula switched to a completely different, ~10x stronger
    /// expression the instant `overlap` turned positive, so two starting points barely a canvas
    /// unit apart produced wildly different pushes.
    #[test]
    fn t_repulsion_has_no_jump_at_the_overlap_boundary() {
        let size = [160.0, 80.0];
        let just_inside = repulsion_force([0.0, 0.0], size, [0.0, 109.0], size)[1].abs();
        let just_outside = repulsion_force([0.0, 0.0], size, [0.0, 111.0], size)[1].abs();
        let ratio = just_inside.max(just_outside) / just_inside.min(just_outside).max(0.001);
        assert!(
            ratio < 3.0,
            "force should vary smoothly across the overlap boundary, got {just_inside} vs \
             {just_outside} (ratio {ratio})"
        );
    }

    /// Two nodes with no edge between them (siblings at the same depth) have nothing but the
    /// bounding-box repulsion keeping them apart, since neither the spring nor the DAG-flow
    /// constraint reaches unconnected nodes. Companion to the boundary test above: starting deep
    /// inside the overlap, the nodes must actually separate and settle rather than oscillate.
    #[test]
    fn t_overlapping_siblings_settle_without_oscillating() {
        let mut wf = Workflow::default();
        let (a, _, _) = node(&mut wf, 0, 1);
        let (b, _, _) = node(&mut wf, 0, 1);
        let mut state = EditorState::from_workflow(wf);
        {
            let mut sizes = state.node_sizes.try_borrow_mut().unwrap();
            sizes[a] = [160.0, 80.0];
            sizes[b] = [160.0, 80.0];
            let mut pos = state.node_positions.try_borrow_mut().unwrap();
            // Deeply overlapping to start: centers only 10 units apart, same x.
            pos[a] = [0.0, 0.0];
            pos[b] = [0.0, 10.0];
        }

        let mut converged = false;
        for _ in 0..500 {
            if step(&mut state, None) {
                converged = true;
                break;
            }
        }
        assert!(converged, "layout must settle instead of oscillating forever");

        let pos = state.node_positions.try_borrow().unwrap();
        let gap = (pos[b][1] - pos[a][1]).abs();
        assert!(
            gap >= 80.0,
            "siblings should end up clear of each other, gap was {gap}"
        );
    }

    /// A position-only relaxation (no carried velocity) has nothing to smooth out small
    /// per-frame imbalances between the spring, repulsion and centering forces once they're
    /// roughly in balance, so `layout_converged` can flap between true and false forever even
    /// though nothing is visibly settling any further — the app would keep repainting on a
    /// jitter too small to see as motion but large enough to keep tripping the threshold. Once
    /// `step` reports converged, it must stay converged.
    #[test]
    fn t_stays_converged_once_settled() {
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 2);
        let (b, b_in, b_out) = node(&mut wf, 1, 1);
        let (c, c_in, _) = node(&mut wf, 1, 0);
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], c_in[0]).unwrap();
        // A sibling of b with no edge to anything, so repulsion and the centering force are
        // also in play, not just the spring.
        let (d, _, _) = node(&mut wf, 0, 1);
        let mut state = EditorState::from_workflow(wf);
        {
            let mut sizes = state.node_sizes.try_borrow_mut().unwrap();
            for nh in [a, b, c, d] {
                sizes[nh] = [160.0, 80.0];
            }
        }

        let mut converged_at = None;
        for i in 0..1000 {
            if step(&mut state, None) {
                converged_at = Some(i);
                break;
            }
        }
        let converged_at = converged_at.expect("layout must settle");

        for i in 0..50 {
            assert!(
                step(&mut state, None),
                "layout reported converged at step {converged_at} but flapped back to \
                 unconverged {} step(s) later — a residual jitter, not a settled layout",
                i + 1
            );
        }
    }
}
