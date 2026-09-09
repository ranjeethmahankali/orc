//! A Barnes-Hut quadtree over 2D points.
//!
//! `layout::step`'s repulsion force used to sum every pair of nodes directly, which is
//! O(n^2) per step and dominates frame cost once a workflow has a few hundred nodes. The
//! Barnes-Hut technique (from N-body gravity/electrostatics simulation) instead groups
//! distant points into cells and, once a cell is small enough relative to its distance
//! from the point asking about it, treats the whole cell as one point at its center of
//! mass rather than visiting its contents individually. That turns each point's force
//! query into an O(log n) tree walk instead of an O(n) scan, for O(n log n) overall.
//!
//! This module only knows about geometry — grouping points into cells and deciding when a
//! cell is "far enough" to approximate. It has no idea what a node, a force, or a charge
//! is; `visit` reports individual points and approximated clusters back to the caller via
//! callbacks, and `layout::step` supplies the actual physics.

/// Cells narrower than this stop subdividing and fall back to visiting every point inside
/// individually, rather than partitioning further — otherwise near-coincident points
/// (or literally identical ones) would recurse toward ever-smaller cells without the
/// point count ever shrinking. `MAX_DEPTH` is the backstop for the same case.
const MIN_CELL_SIZE: f32 = 1.0;
const MAX_DEPTH: u32 = 24;
/// Barnes-Hut opening-angle threshold: a cell is treated as a single point once its width
/// is less than `THETA` times its distance from the query point. Smaller is more accurate
/// and slower (0 degenerates to brute force), larger is faster and coarser. 0.75 is the
/// standard textbook default.
const THETA: f32 = 0.75;

#[derive(Clone, Copy)]
struct Bounds {
    center: [f32; 2],
    half_size: f32,
}

impl Bounds {
    fn quadrant_of(&self, p: [f32; 2]) -> usize {
        let right = (p[0] >= self.center[0]) as usize;
        let bottom = (p[1] >= self.center[1]) as usize;
        bottom * 2 + right
    }

    fn child(&self, quadrant: usize) -> Bounds {
        let h = self.half_size / 2.0;
        let dx = if quadrant % 2 == 0 { -h } else { h };
        let dy = if quadrant < 2 { -h } else { h };
        Bounds {
            center: [self.center[0] + dx, self.center[1] + dy],
            half_size: h,
        }
    }
}

struct Node {
    /// Geometric center of this cell's bounding square — distinct from `com`, the
    /// mass-weighted center of the points inside it. `visit_node` needs both: `com` for
    /// where the approximated force originates, `center`/`half_size` to check whether the
    /// query point itself falls inside this cell (see the comment there for why that
    /// matters).
    center: [f32; 2],
    half_size: f32,
    mass: u32,
    com: [f32; 2],
    /// `None` for a leaf. Entries are `u32::MAX` for an empty quadrant.
    children: Option<[u32; 4]>,
    /// Populated for leaves only: every point index that landed in this cell.
    points: Vec<u32>,
}

pub(crate) struct QuadTree {
    nodes: Vec<Node>,
    root: u32,
}

/// One piece of what `QuadTree::visit` found relevant to a query point's force.
pub(crate) enum Contribution {
    /// An individual point close enough to need a direct pairwise comparison.
    Exact(usize),
    /// A cell far enough away to summarize as `mass` coincident points at `com`.
    Approx { com: [f32; 2], mass: u32 },
}

fn bounding_square(positions: &[[f32; 2]]) -> Bounds {
    let mut min = positions[0];
    let mut max = positions[0];
    for p in positions {
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[1]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[1]);
    }
    let half_size = ((max[0] - min[0]).max(max[1] - min[1]) / 2.0).max(1.0);
    Bounds {
        center: [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0],
        // A hair larger than the tightest fit so points exactly on the boundary still land
        // strictly inside a quadrant.
        half_size: half_size * 1.001,
    }
}

fn build_node(
    positions: &[[f32; 2]],
    indices: Vec<u32>,
    bounds: Bounds,
    depth: u32,
    nodes: &mut Vec<Node>,
) -> u32 {
    let mass = indices.len() as u32;
    let mut com = [0.0f32; 2];
    for &i in &indices {
        com[0] += positions[i as usize][0];
        com[1] += positions[i as usize][1];
    }
    com[0] /= mass as f32;
    com[1] /= mass as f32;

    if mass <= 1 || depth >= MAX_DEPTH || bounds.half_size <= MIN_CELL_SIZE {
        let idx = nodes.len() as u32;
        nodes.push(Node {
            center: bounds.center,
            half_size: bounds.half_size,
            mass,
            com,
            children: None,
            points: indices,
        });
        return idx;
    }

    let mut quads: [Vec<u32>; 4] = Default::default();
    for &i in &indices {
        quads[bounds.quadrant_of(positions[i as usize])].push(i);
    }

    let idx = nodes.len() as u32;
    // Reserve the slot before recursing so `idx` is stable while children are built.
    nodes.push(Node {
        center: bounds.center,
        half_size: bounds.half_size,
        mass,
        com,
        children: Some([u32::MAX; 4]),
        points: Vec::new(),
    });
    let mut children = [u32::MAX; 4];
    for (q, pts) in quads.into_iter().enumerate() {
        if !pts.is_empty() {
            children[q] = build_node(positions, pts, bounds.child(q), depth + 1, nodes);
        }
    }
    nodes[idx as usize].children = Some(children);
    idx
}

impl QuadTree {
    pub(crate) fn build(positions: &[[f32; 2]]) -> Self {
        assert!(!positions.is_empty());
        let bounds = bounding_square(positions);
        let indices: Vec<u32> = (0..positions.len() as u32).collect();
        let mut nodes = Vec::new();
        let root = build_node(positions, indices, bounds, 0, &mut nodes);
        QuadTree { nodes, root }
    }

    /// Visit everything relevant to the total force on `query` (whose position is
    /// `query_pos`). Reports `Contribution::Exact(j)` for every individual point close
    /// enough to need a direct pairwise comparison (never `query` itself), and
    /// `Contribution::Approx { com, mass }` for every cell far enough away to summarize as
    /// one point. Combining either into an actual force is the caller's job — this type
    /// only decides *which* points and clusters matter, not how they push. A single
    /// callback (rather than one per variant) so callers can accumulate into one plain
    /// local instead of juggling two closures that both need to mutate it.
    pub(crate) fn visit(&self, query: usize, query_pos: [f32; 2], mut on: impl FnMut(Contribution)) {
        self.visit_node(self.root, query, query_pos, &mut on);
    }

    fn visit_node(
        &self,
        node_idx: u32,
        query: usize,
        query_pos: [f32; 2],
        on: &mut impl FnMut(Contribution),
    ) {
        let node = &self.nodes[node_idx as usize];
        if node.mass == 0 {
            return;
        }
        match &node.children {
            None => {
                for &idx in &node.points {
                    if idx as usize != query {
                        on(Contribution::Exact(idx as usize));
                    }
                }
            }
            Some(children) => {
                let dx = query_pos[0] - node.com[0];
                let dy = query_pos[1] - node.com[1];
                let dist_sq = dx * dx + dy * dy;
                let width = node.half_size * 2.0;
                // The ratio test alone assumes the query point sits outside the cell being
                // considered — true for a typical, roughly evenly spread point set, but
                // not guaranteed: a lone point far from an otherwise tight cluster forces
                // a bounding square that spans both, and that root cell's center of mass
                // then includes the query itself. Approximating it would make a point
                // repel itself. So a cell can only ever be approximated if the query
                // point provably lies outside its bounds, regardless of what the ratio
                // says. The margin (rather than a bare `> half_size`) exists because a
                // point can land almost exactly on a cell boundary: `half_size` here is
                // recomputed independently at this level, while the point's actual
                // quadrant was decided during `build` by comparison against every
                // ancestor's center, and those two paths can disagree by a float ULP or
                // two right at the edge — this widens the cell slightly so that
                // disagreement always resolves toward "still inside" (more recursion)
                // rather than risking a false "outside" (a wrong approximation).
                let margin = node.half_size * (1.0 + 1e-4) + 1e-3;
                let query_outside_cell = (query_pos[0] - node.center[0]).abs() > margin
                    || (query_pos[1] - node.center[1]).abs() > margin;
                if query_outside_cell && width * width < THETA * THETA * dist_sq {
                    on(Contribution::Approx {
                        com: node.com,
                        mass: node.mass,
                    });
                } else {
                    for &child in children {
                        if child != u32::MAX {
                            self.visit_node(child, query, query_pos, on);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Contribution, QuadTree};

    /// Brute-force reference: sum of a unary "charge" (1.0 per point) at every other
    /// point, inverse-square, matching the monopole approximation `visit` is meant to
    /// reproduce. Used to check the tree's `approx` callback against ground truth.
    fn brute_force(query: usize, positions: &[[f32; 2]]) -> [f32; 2] {
        let mut total = [0.0f32; 2];
        for (j, &p) in positions.iter().enumerate() {
            if j == query {
                continue;
            }
            let dx = positions[query][0] - p[0];
            let dy = positions[query][1] - p[1];
            let dist_sq = (dx * dx + dy * dy).max(1e-6);
            let dist = dist_sq.sqrt();
            total[0] += dx / dist / dist_sq;
            total[1] += dy / dist / dist_sq;
        }
        total
    }

    fn visited_force(tree: &QuadTree, query: usize, positions: &[[f32; 2]]) -> [f32; 2] {
        let mut total = [0.0f32; 2];
        tree.visit(query, positions[query], |c| {
            let (dx, dy, mass) = match c {
                Contribution::Exact(j) => (
                    positions[query][0] - positions[j][0],
                    positions[query][1] - positions[j][1],
                    1.0,
                ),
                Contribution::Approx { com, mass } => (
                    positions[query][0] - com[0],
                    positions[query][1] - com[1],
                    mass as f32,
                ),
            };
            let dist_sq = (dx * dx + dy * dy).max(1e-6);
            let dist = dist_sq.sqrt();
            total[0] += mass * dx / dist / dist_sq;
            total[1] += mass * dy / dist / dist_sq;
        });
        total
    }

    /// A cluster of points far from the query should be summarizable as one mass with
    /// only a small relative error against the true pairwise sum — this is the entire
    /// premise the layout step relies on to skip individual comparisons.
    #[test]
    fn t_approximation_matches_brute_force_within_a_few_percent_for_a_distant_cluster() {
        let mut positions = vec![[0.0f32, 0.0]]; // the query point, index 0
        // A tight cluster of 40 points far away, so it should mostly get approximated.
        for i in 0..40 {
            let angle = i as f32 * 0.37;
            positions.push([
                2000.0 + angle.cos() * 10.0,
                2000.0 + angle.sin() * 10.0,
            ]);
        }
        let tree = QuadTree::build(&positions);
        let expected = brute_force(0, &positions);
        let actual = visited_force(&tree, 0, &positions);

        let err = ((actual[0] - expected[0]).powi(2) + (actual[1] - expected[1]).powi(2)).sqrt();
        let mag = (expected[0].powi(2) + expected[1].powi(2)).sqrt();
        assert!(
            err / mag < 0.05,
            "approximation drifted too far from brute force: expected {expected:?}, got \
             {actual:?} (relative error {})",
            err / mag
        );
    }

    /// Every point must be accounted for exactly once, whether visited directly or folded
    /// into a cluster's mass — losing or double-counting a point would silently weaken or
    /// strengthen repulsion instead of erroring.
    #[test]
    fn t_every_other_point_is_covered_exactly_once() {
        let positions: Vec<[f32; 2]> = (0..50)
            .map(|i| [((i * 37) % 500) as f32, ((i * 53) % 500) as f32])
            .collect();
        let tree = QuadTree::build(&positions);

        for query in 0..positions.len() {
            let mut covered = 0u32;
            tree.visit(query, positions[query], |c| {
                covered += match c {
                    Contribution::Exact(_) => 1,
                    Contribution::Approx { mass, .. } => mass,
                };
            });
            assert_eq!(
                covered,
                positions.len() as u32 - 1,
                "query {query} did not account for exactly the other points"
            );
        }
    }

    /// Two points on top of each other must not make the tree recurse without bound —
    /// bounded by `MIN_CELL_SIZE`/`MAX_DEPTH`, and covered here so a regression shows up
    /// as a slow test hang rather than a stack overflow surprise later.
    #[test]
    fn t_coincident_points_do_not_cause_unbounded_recursion() {
        let mut positions = vec![[0.0f32, 0.0]; 20];
        positions.push([500.0, 500.0]);
        let tree = QuadTree::build(&positions);
        let mut covered = 0u32;
        tree.visit(0, positions[0], |c| {
            covered += match c {
                Contribution::Exact(_) => 1,
                Contribution::Approx { mass, .. } => mass,
            };
        });
        assert_eq!(covered, positions.len() as u32 - 1);
    }

    #[test]
    fn t_single_point_has_nothing_to_visit() {
        let positions = vec![[0.0f32, 0.0]];
        let tree = QuadTree::build(&positions);
        let mut covered = 0u32;
        tree.visit(0, positions[0], |c| {
            covered += match c {
                Contribution::Exact(_) => 1,
                Contribution::Approx { mass, .. } => mass,
            };
        });
        assert_eq!(covered, 0);
    }
}
