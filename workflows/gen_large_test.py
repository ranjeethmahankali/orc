"""Generate a large test workflow for dagger's Barnes-Hut layout benchmarking.

Builds a wide binary-reduction tree of ~500 nodes (well above the
BRUTE_FORCE_NODE_THRESHOLD in rust/dagger/src/layout.rs) so the quadtree
repulsion path is actually exercised when opened in the editor.

Usage:
    python workflows/gen_large_test.py
"""

import os
import orc

project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
search_dir = os.path.join(project_root, "build", "debug")
orc.load_plugins(search_dir)


def big_pipeline(x, y):
    leaves = [orc.add(x, y)]
    for i in range(255):
        leaves.append(orc.multiply(x, orc.make_deck([float(i + 1)])))
    while len(leaves) > 1:
        nxt = []
        for i in range(0, len(leaves) - 1, 2):
            nxt.append(orc.add(leaves[i], leaves[i + 1]))
        if len(leaves) % 2 == 1:
            nxt.append(leaves[-1])
        leaves = nxt
    return leaves[0]


graph = orc.make_workflow(big_pipeline)
out_path = os.path.join(project_root, "workflows", "test_large.orc")
orc.save_workflow(graph, out_path)
print(f"Saved large test workflow to {out_path}")
