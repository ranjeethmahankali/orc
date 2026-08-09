"""
Minimal Python Host.

Minimal Python host that loads a plugin shared library via ctypes
and calls a function (e.g. 'add') on two f64 decks.
"""

from orc import *

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)
    search_dir = os.path.join(project_root, "build", "debug")

    # Build the host
    host = OrcHost()
    host.abi_version = ORC_ABI_VERSION
    host.memory_api.alloc = host_alloc
    host.memory_api.dealloc = host_dealloc
    host.callbacks.report_message = report_message

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

    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    inputs = (OrcHandle * 2)(a, b)

    out = OrcHandle()
    ctypes.memset(ctypes.addressof(out), 0, ctypes.sizeof(out))
    out.handle = next_handle_id()

    add_fn.func(0, inputs, 2, ctypes.byref(out), 1)

    result = read_handle(out)
    print(f"add([1, 2, 3], [10, 20, 30]) = {result}")
    assert result == [11.0, 22.0, 33.0], f"Unexpected: {result}"
    print("PASS")

    # Free output via its free_fn if set
    if out.free_fn:
        out.free_fn(ctypes.byref(out))


if __name__ == "__main__":
    main()
