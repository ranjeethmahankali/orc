use crate::quadtree::{Contribution, QuadTree};
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
/// Below this many nodes, summing repulsion directly over every pair is faster than
/// building a quadtree first — confirmed by the before/after numbers in `bench` (see
/// `quadtree.rs`'s module doc for why the tree wins at all above some size: it turns an
/// O(n^2) scan into an O(n log n) one by approximating distant clusters as a single point).
/// Below the crossover the tree's own build cost dominates instead, since it's still
/// O(n log n) work just to construct.
const BRUTE_FORCE_NODE_THRESHOLD: usize = 200;
/// Top-left starting point for the depth-0 layer's seed positions, in canvas units. Purely a
/// visual choice for where the layout starts before the user ever pans.
const SEED_ORIGIN_X: f32 = 100.0;
const SEED_ORIGIN_Y: f32 = 300.0;
/// Vertical spacing between nodes seeded into the same depth layer. Not derived from any
/// measured node height, since sizes aren't known yet at seed time (`measure()` hasn't run on
/// the first frame) — just a reasonable guess the force simulation is free to correct once real
/// sizes are available.
const SEED_ROW_HEIGHT: f32 = 120.0;

/// The nodes feeding a node, via its connected inputs.
fn predecessors(workflow: &Workflow, node: NH) -> impl Iterator<Item = NH> + '_ {
    workflow.node_inputs(node).filter_map(move |ih| {
        workflow
            .input_source(ih)
            .map(|oh| workflow.node_from_output(oh))
    })
}

/// Longest-path depth of every node, plus which nodes take part in a cycle. Both are indexed
/// by `NH::index()`. Only used within this module (by `topological_seed` and its own tests).
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
        let x = SEED_ORIGIN_X + d as f32 * DAG_MIN_GAP;
        let y = SEED_ORIGIN_Y + (idx_in_layer - (layer_size - 1.0) / 2.0) * SEED_ROW_HEIGHT;
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

/// Applies bounding-box repulsion between every pair of nodes to `forces`.
///
/// Below `BRUTE_FORCE_NODE_THRESHOLD`, sums every pair directly, exploiting Newton's third
/// law (the force on `b` is the negation of the force on `a`) to halve the work. Above it,
/// approximates distant clusters via a Barnes-Hut quadtree (`quadtree.rs`) instead of
/// visiting every pair: each node's force becomes an O(log n) tree walk rather than an
/// O(n) scan, so the whole pass is O(n log n) instead of O(n^2). The quadtree only reports
/// geometry (which points are close enough to need an exact comparison, which clusters are
/// far enough to summarize as one mass at their center); the actual repulsion law —
/// including the overlap-margin term, which only ever matters at the close range the tree
/// always resolves down to individual points for — is applied here, identically to the
/// brute-force path.
fn apply_repulsion(positions: &[[f32; 2]], sizes: &[[f32; 2]], forces: &mut [[f32; 2]]) {
    if positions.len() <= BRUTE_FORCE_NODE_THRESHOLD {
        apply_repulsion_brute_force(positions, sizes, forces);
    } else {
        apply_repulsion_quadtree(positions, sizes, forces);
    }
}

fn apply_repulsion_brute_force(positions: &[[f32; 2]], sizes: &[[f32; 2]], forces: &mut [[f32; 2]]) {
    let n = positions.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let f = repulsion_force(positions[i], sizes[i], positions[j], sizes[j]);
            forces[i][0] += f[0];
            forces[i][1] += f[1];
            forces[j][0] -= f[0];
            forces[j][1] -= f[1];
        }
    }
}

fn apply_repulsion_quadtree(positions: &[[f32; 2]], sizes: &[[f32; 2]], forces: &mut [[f32; 2]]) {
    let tree = QuadTree::build(positions);
    for i in 0..positions.len() {
        let mut force = [0.0f32; 2];
        tree.visit(i, positions[i], |c| {
            let f = match c {
                Contribution::Exact(j) => {
                    repulsion_force(positions[i], sizes[i], positions[j], sizes[j])
                }
                Contribution::Approx { com, mass } => {
                    let dx = positions[i][0] - com[0];
                    let dy = positions[i][1] - com[1];
                    let dist_sq = (dx * dx + dy * dy).max(100.0);
                    let dist = dist_sq.sqrt();
                    // The monopole approximation: `mass` coincident nodes at their shared
                    // center behave, at this range, like one node repelling with `mass`
                    // times the strength of one. There's no overlap-margin term here
                    // because a cluster this far away — far enough for `dist` to clear the
                    // opening-angle test against its own width — is never anywhere near
                    // actually overlapping node `i`.
                    let mag = REPULSION_STRENGTH * 0.1 * mass as f32 / dist_sq;
                    [dx / dist * mag, dy / dist * mag]
                }
            };
            force[0] += f[0];
            force[1] += f[1];
        });
        forces[i][0] += force[0];
        forces[i][1] += force[1];
    }
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

    // Read current positions, sizes and velocities into local vecs for fast access. `positions`
    // and `velocities` are mutated in place by the integration step below; `sizes` is read-only
    // throughout.
    let (mut positions, sizes, mut velocities) = {
        let pos = state.node_positions.try_borrow().unwrap();
        let sz = state.node_sizes.try_borrow().unwrap();
        let vel = state.node_velocities.try_borrow().unwrap();
        let positions: Vec<[f32; 2]> = nodes.iter().map(|&nh| pos[nh]).collect();
        let sizes: Vec<[f32; 2]> = nodes.iter().map(|&nh| sz[nh]).collect();
        let velocities: Vec<[f32; 2]> = nodes.iter().map(|&nh| vel[nh]).collect();
        (positions, sizes, velocities)
    };

    let mut forces = vec![[0.0f32; 2]; n];

    // 1. Bounding-box repulsion between all node pairs. See `apply_repulsion` for the
    // brute-force/quadtree split, and `repulsion_force` for why the pairwise formula is
    // one continuous expression rather than a hard switch between "overlapping" and "not".
    apply_repulsion(&positions, &sizes, &mut forces);

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
    for i in 0..n {
        if Some(i) == pinned_idx {
            // Actively dragged this frame: the cursor has full control of its position. Zero
            // its velocity so that letting go doesn't fling it off with whatever velocity it
            // happened to have before the drag started — it resumes from rest.
            velocities[i] = [0.0, 0.0];
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
        velocities[i] = [vx, vy];
        positions[i][0] += vx;
        positions[i][1] += vy;
        max_move = max_move.max(speed);
    }

    // Write back.
    {
        let mut pos = state.node_positions.try_borrow_mut().unwrap();
        let mut vel = state.node_velocities.try_borrow_mut().unwrap();
        for (i, &nh) in nodes.iter().enumerate() {
            pos[nh] = positions[i];
            vel[nh] = velocities[i];
        }
    }

    max_move < CONVERGENCE_THRESHOLD
}

#[cfg(test)]
mod test {
    use super::{
        BRUTE_FORCE_NODE_THRESHOLD, apply_repulsion_brute_force, apply_repulsion_quadtree,
        compute_depths, repulsion_force, step,
    };
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

    /// `topological_seed` (run once, inside `EditorState::from_workflow`) had no test at all
    /// before this: every other test in this module overwrites `node_positions` right after
    /// construction, so a broken seed formula (nodes stacked on top of each other within a
    /// layer, or depth not increasing left to right) would ship unnoticed, even though it's the
    /// very first thing a user sees when opening a workflow. Asserts relative properties of the
    /// formula rather than exact coordinates, so retuning the seed constants doesn't break this.
    #[test]
    fn t_topological_seed_places_layers_left_to_right_and_centers_each_layer() {
        let mut wf = Workflow::default();
        let (a, _, a_out) = node(&mut wf, 0, 3);
        // Three siblings at depth 1, all fed by `a`, none connected to each other.
        let (b1, b1_in, _) = node(&mut wf, 1, 0);
        let (b2, b2_in, _) = node(&mut wf, 1, 0);
        let (b3, b3_in, _) = node(&mut wf, 1, 0);
        wf.connect(a_out[0], b1_in[0]).unwrap();
        wf.connect(a_out[1], b2_in[0]).unwrap();
        wf.connect(a_out[2], b3_in[0]).unwrap();

        let state = EditorState::from_workflow(wf);
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

    /// `apply_repulsion` picks brute force or the quadtree purely based on node count — this
    /// checks the two paths agree on the same input, which is what makes that switch safe.
    /// A scattered (not clustered) layout, unlike `quadtree.rs`'s own approximation-error
    /// test, since that's the shape a real settling layout actually has, and it's a much
    /// harder case for Barnes-Hut than a single tight cluster: many cells end up close to
    /// the opening-angle threshold at once instead of one comfortably far cluster.
    #[test]
    fn t_quadtree_repulsion_matches_brute_force_for_a_scattered_layout() {
        let n = 300;
        let positions: Vec<[f32; 2]> = (0..n)
            .map(|i| {
                [
                    ((i * 37) % 2000) as f32,
                    ((i * 53) % 1500) as f32,
                ]
            })
            .collect();
        let sizes = vec![[160.0f32, 80.0]; n];

        let mut brute = vec![[0.0f32; 2]; n];
        apply_repulsion_brute_force(&positions, &sizes, &mut brute);
        let mut tree = vec![[0.0f32; 2]; n];
        apply_repulsion_quadtree(&positions, &sizes, &mut tree);

        // Aggregate (RMS) error against aggregate magnitude, not a per-node worst-case
        // ratio: individual nodes can have a near-zero net force from cancellation between
        // neighbors on opposite sides, where even a tiny absolute approximation error
        // balloons into a huge relative one despite the layout looking (and behaving)
        // fine. The layout is a visual aid settled over many damped iterations, not a
        // physics simulation with a correctness contract, so what matters is that the
        // approximation is close in aggregate, not bit-exact per node.
        let mut sum_err_sq = 0.0f32;
        let mut sum_mag_sq = 0.0f32;
        for i in 0..n {
            sum_err_sq += (tree[i][0] - brute[i][0]).powi(2) + (tree[i][1] - brute[i][1]).powi(2);
            sum_mag_sq += brute[i][0].powi(2) + brute[i][1].powi(2);
        }
        let rms_rel_err = (sum_err_sq / sum_mag_sq).sqrt();
        assert!(
            rms_rel_err < 0.6,
            "quadtree repulsion drifted too far from brute force in aggregate: rms relative \
             error {rms_rel_err}"
        );
    }

    /// `step` itself must dispatch to the quadtree path once a layout crosses
    /// `BRUTE_FORCE_NODE_THRESHOLD` without blowing up — the two repulsion paths agreeing
    /// in isolation (previous test) doesn't guarantee the switch is wired correctly into
    /// the rest of the integration loop (spring, centering, DAG constraint, damping).
    /// Starts from `topological_seed`'s ordinary starting layout, same as opening a real
    /// workflow. A smoke test, not a physics-quality check: whether this many mutually
    /// repelling, spring-free siblings fully settle within any given step budget is a
    /// property of the existing force model at this scale, not of the quadtree switch.
    #[test]
    fn t_large_layout_above_the_quadtree_threshold_stays_finite_and_bounded() {
        let n = BRUTE_FORCE_NODE_THRESHOLD + 50;
        let mut wf = Workflow::default();
        let mut handles = Vec::with_capacity(n);
        for _ in 0..n {
            let (nh, _, _) = node(&mut wf, 0, 1);
            handles.push(nh);
        }
        let mut state = EditorState::from_workflow(wf);
        {
            let mut sizes = state.node_sizes.try_borrow_mut().unwrap();
            for &nh in &handles {
                sizes[nh] = [160.0, 80.0];
            }
        }

        for _ in 0..500 {
            step(&mut state, None);
        }

        let pos = state.node_positions.try_borrow().unwrap();
        for &nh in &handles {
            assert!(
                pos[nh][0].is_finite() && pos[nh][1].is_finite(),
                "quadtree repulsion must never produce a non-finite position"
            );
            assert!(
                pos[nh][0].abs() < 1_000_000.0 && pos[nh][1].abs() < 1_000_000.0,
                "layout should not blow up to an unbounded position, got {:?}",
                pos[nh]
            );
        }
    }
}
