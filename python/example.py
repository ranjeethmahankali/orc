"""
Minimal Python Host.

Minimal Python host that loads a plugin shared library via ctypes
and calls a function (e.g. 'add') on two f64 decks.
"""

import os
import sys
import ctypes
from orc import (default_host, load_plugins, get_function, make_handle,
                 next_handle_id, OrcHandle, read_handle, as_numpy)

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    """Run example script that uses orc plugins."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)
    search_dir = os.path.join(project_root, "build", "debug")
    host = default_host()
    # Load all plugins from the search directory
    print(f"Searching for plugins in: {search_dir}")
    plugins = load_plugins(search_dir, host)
    if not plugins:
        print("No plugins found.")
        sys.exit(1)

    print(f"\nLoaded {len(plugins)} plugin(s):")
    for _lib, plugin in plugins:
        print(f"  {plugin.name.decode()}: {plugin.desc.decode()}")
        for i in range(plugin.n_functions):
            fi = plugin.functions[i]
            print(f"    - {fi.name.decode()}: {fi.desc.decode()}")
    print()

    # Call 'add' on two f64 arrays
    add_fn = get_function(plugins, "add")
    flatten_fn = get_function(plugins, "flatten_deck")

    a = make_handle([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
    b = make_handle([10.0, 20.0, 30.0])
    inputs = (OrcHandle * 2)(a, b)

    out = OrcHandle()
    ctypes.memset(ctypes.addressof(out), 0, ctypes.sizeof(out))
    out.handle = next_handle_id()

    add_fn.func(0, inputs, 2, ctypes.byref(out), 1)
    flat_out = OrcHandle()
    ctypes.memset(ctypes.addressof(flat_out), 0, ctypes.sizeof(flat_out))
    flat_out.handle = next_handle_id()
    flatten_fn.func(0, ctypes.byref(out), 1, ctypes.byref(flat_out), 1)

    result = read_handle(flat_out)
    print(f"flatten(add([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])) = {result}")
    assert result == [11, 22, 33, 12, 24, 36, 38], f"Unexpected: {result}"

    # Zero-copy numpy view of the same data
    np_arr = as_numpy(flat_out)
    print(f"numpy (zero copy): {np_arr}")
    assert list(np_arr) == [11, 22, 33, 12, 24, 36, 38]
    assert (np_arr.ctypes.data == flat_out.items
            ), "Confirm that numpy is using the same pointer."
    doubled_np_arr = np_arr * 2.0
    print(f"Output after doubling: {doubled_np_arr}")
    print("PASS")


if __name__ == "__main__":
    main()
