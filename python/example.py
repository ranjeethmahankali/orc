"""
Minimal Python Host.

Loads plugin shared libraries via the orc PyO3 module and exercises
immediate mode, zero-copy numpy, complex numbers, and the DAG builder.
"""

import os
import numpy as np
import orc


def simple_experiment():
    """Run example script that uses orc plugins."""
    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    search_dir = os.path.join(project_root, "build", "debug")
    print(f"Searching for plugins in: {search_dir}")
    orc.load_plugins(search_dir)

    a = orc.make_deck([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
    b = orc.make_deck([[10.0, 20.0, 30.0], [3.0, 5.0, 7.0, 11.0]])

    out = orc.add(a, b)

    print(f"Before flattening: {orc.read_deck(out)}")

    flat_out = orc.flatten_deck(out)

    result = orc.read_deck(flat_out)
    print(f"After flattening: {result}")
    assert result == [11.0, 22.0, 33.0, 5.0, 9.0, 13.0, 19.0], f"Unexpected: {result}"

    # Zero-copy numpy view of the same data.
    np_arr = np.asarray(flat_out)

    print(f"numpy (zero copy): {np_arr}")
    assert list(np_arr) == [11, 22, 33, 5, 9, 13, 19]
    assert (np_arr.ctypes.data == flat_out.__array_interface__["data"][0]
            ), "Confirm that numpy is using the same pointer."
    doubled_np_arr = np_arr * 2.0
    print(f"Output after doubling: {doubled_np_arr}")

    # Complex numbers.
    comp = orc.create_complex(a, b)
    comp2 = orc.mul_complex(comp, comp)
    real, imag = orc.complex_get_parts(comp2)

    print("==========\nComplex Numbers\n==========")
    print("Real part: ", orc.read_deck(real))
    print("Imag part: ", orc.read_deck(imag))

    # --- DAG construction ---
    def my_pipeline(x, y):
        offset = orc.make_deck([100.0])
        total = orc.add(x, y)
        return orc.add(total, offset)

    graph = orc.make_workflow(my_pipeline)

    result = graph.run(a, b)
    print(f"\nDAG result: {orc.read_deck(result)}")

    result2 = graph.run(y=b, x=a)
    print(f"DAG result (named): {orc.read_deck(result2)}")

    # Interleave immediate and DAG calls freely.
    doubled = orc.mul(result, orc.make_deck([2.0]))
    print(f"Doubled: {orc.read_deck(doubled)}")

    # Serialize / deserialize.
    orc.save_workflow(graph, "my_pipeline.mpk")
    graph2 = orc.load_workflow("my_pipeline.mpk")
    result3 = graph2.run(a, b)
    print(f"Deserialized DAG result: {orc.read_deck(result3)}")
    os.unlink("my_pipeline.mpk")

    print("PASS")


def collatz_parallel_experiment():
    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    search_dir = os.path.join(project_root, "build", "debug")
    print(f"Searching for plugins in: {search_dir}")
    orc.load_plugins(search_dir)

    nums = list(range(500))
    print(f"Inputs: {nums}")
    nums = orc.make_deck(nums, type_id=orc.ORC_TYPE_U64)
    iterations = orc.make_deck(128, type_id=orc.ORC_TYPE_U64)
    outputs = orc.collatz_parallel_experiment(nums, iterations)
    outputs = orc.read_deck(outputs)
    print(f"Outputs: {outputs}")


if __name__ == "__main__":
    simple_experiment()
    # collatz_parallel_experiment()
