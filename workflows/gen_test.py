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


def simple_math_workflow():
    x = orc.make_deck([1.23, 2.23, 3.23, 4.23])
    y = orc.make_deck([2.34, 3.45])
    summed = orc.add(x, y)
    scale = orc.make_deck([2.0])
    scaled = orc.multiply(summed, scale)
    offset = orc.make_deck([100.0])
    return orc.add(scaled, offset)

def collatz_experiment_workflow():
    nums = list(range(500))
    nums = orc.make_deck(nums, dtype="u64")
    iterations = orc.make_deck(128, dtype="u64")
    outputs = orc.collatz_parallel_experiment(nums, iterations)
    return outputs

def large_math_workflow(x, y):
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


if __name__ == "__main__":
    graph = orc.make_workflow(simple_math_workflow)
    out_path = os.path.join(project_root, "workflows", "simple_math.orc")
    orc.save_workflow(graph, out_path)
    print(f"Saved test workflow to {out_path}")

    graph = orc.make_workflow(collatz_experiment_workflow)
    out_path = os.path.join(project_root, "workflows", "collatz_experiment.orc")
    orc.save_workflow(graph, out_path)
    print(f"Saved test workflow to {out_path}")

    graph = orc.make_workflow(large_math_workflow)
    out_path = os.path.join(project_root, "workflows", "large_math.orc")
    orc.save_workflow(graph, out_path)
    print(f"Saved test workflow to {out_path}")
