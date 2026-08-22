"""
Minimal Python Host.

Minimal Python host that loads a plugin shared library via ctypes
and calls a function (e.g. 'add') on two f64 decks.
"""

import os
import sys
import orc
import random

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def simple_experiment():
    """Run example script that uses orc plugins."""
    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    search_dir = os.path.join(project_root, "build", "debug")
    # Load all plugins from the search directory
    print(f"Searching for plugins in: {search_dir}")
    orc.load_plugins(search_dir)

    a = orc.make_deck([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
    b = orc.make_deck([[10.0, 20.0, 30.0], [3.0, 5.0, 7.0, 11.0]])

    out = orc.add(a, b)

    print(f"Before flattening: {orc.read_deck(out)}")

    flat_out = orc.flatten_deck(out)

    result = orc.read_deck(flat_out)
    print(f"After flattening: {result}")
    assert result == [11, 22, 33, 5, 9, 13, 19], f"Unexpected: {result}"

    # Zero-copy numpy view of the same data
    np_arr = orc.as_numpy(flat_out)

    print(f"numpy (zero copy): {np_arr}")
    assert list(np_arr) == [11, 22, 33, 5, 9, 13, 19]
    assert (np_arr.ctypes.data == flat_out.items
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

    print("PASS")


def collatz_parallel_experiment():
    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    search_dir = os.path.join(project_root, "build", "debug")
    # Load all plugins from the search directory
    print(f"Searching for plugins in: {search_dir}")
    orc.load_plugins(search_dir)

    nums = [i for i in range(500)]
    print(f"Inputs: {nums}")
    nums = orc.make_deck(nums, type_id=orc.ORC_TYPE_U64)
    iterations = orc.make_deck(128, type_id=orc.ORC_TYPE_U64)
    outputs = orc.collatz_parallel_experiment(nums, iterations)
    outputs = orc.read_deck(outputs)
    print(f"Outputs: {outputs}")


if __name__ == "__main__":
    simple_experiment()
    # collatz_parallel_experiment()
