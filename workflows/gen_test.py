"""Generate a test workflow for dagger development.

Creates a small DAG: add two inputs, multiply the sum by a constant,
then add another constant offset. Saves to workflows/test.orc.

Usage:
    python workflows/gen_test.py
"""

import os
import orc

project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
search_dir = os.path.join(project_root, "build", "debug")
orc.load_plugins(search_dir)


def test_pipeline():
    x = orc.make_deck([1.23])
    y = orc.make_deck([2.34])
    summed = orc.add(x, y)
    scale = orc.make_deck([2.0])
    scaled = orc.multiply(summed, scale)
    offset = orc.make_deck([100.0])
    return orc.add(scaled, offset)


graph = orc.make_workflow(test_pipeline)
out_path = os.path.join(project_root, "workflows", "test.orc")
orc.save_workflow(graph, out_path)
print(f"Saved test workflow to {out_path}")
