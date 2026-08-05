use orc_sdk::{
    Deck, Error, ORC_ABI_VERSION, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcHostMemoryAPI, deck,
    update_handle_from_deck,
};
use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{CStr, c_void};

unsafe extern "C" fn host_alloc(size: u64, alignment: u64) -> *mut c_void {
    let layout = Layout::from_size_align(size as usize, alignment as usize).unwrap();
    unsafe { alloc(layout) as *mut c_void }
}

unsafe extern "C" fn host_dealloc(ptr: *mut c_void, size: u64, alignment: u64) {
    let layout = Layout::from_size_align(size as usize, alignment as usize).unwrap();
    unsafe { dealloc(ptr as *mut u8, layout) }
}

const HOST: OrcHost = OrcHost {
    abi_version: ORC_ABI_VERSION,
    memory_api: OrcHostMemoryAPI {
        alloc: Some(host_alloc),
        dealloc: Some(host_dealloc),
    },
    callbacks: OrcHostCallbackAPI {
        report_progress: None,
        report_message: Some(report_message),
        check_cancellation: None,
        report_intermediate_output: None,
    },
};

unsafe extern "C" fn report_message(
    ctx: u64,
    level: orc_sdk::OrcMessageLevel,
    msg: *const std::ffi::c_char,
) {
    let msg = if msg.is_null() {
        ""
    } else {
        &unsafe { CStr::from_ptr(msg) }.to_string_lossy()
    };
    println!(
        "[{}][{}] {}",
        match level {
            orc_sdk::ORC_MSG_LEVEL_DEBUG => "DEBUG",
            orc_sdk::ORC_MSG_LEVEL_INFO => "INFO",
            orc_sdk::ORC_MSG_LEVEL_WARN => "WARN",
            orc_sdk::ORC_MSG_LEVEL_ERROR => "ERROR",
            orc_sdk::ORC_MSG_LEVEL_FATAL => "FATAL",
            _ => "FATAL",
        },
        ctx,
        msg
    );
}

fn main() -> Result<(), Error> {
    let exe_dir = std::env::current_exe()
        .expect("Cannot determine executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf();
    println!("Loading plugins from {}", exe_dir.display());
    let plugins = orc_sdk::load_plugins(&exe_dir, &HOST)?;
    println!("Loaded {} plugin(s)\n", plugins.len());
    // Print the loaded plugins and functions.
    for plugin in &plugins {
        println!(
            "{} plugin has {} function(s):",
            plugin.name(),
            plugin.functions().len()
        );
        for func in plugin.functions().iter() {
            println!("  - {}\n    {}", func.name, func.desc);
        }
        println!();
    }
    // --- Test some plugin functions ---

    let add_fn = plugins
        .iter()
        .flat_map(|p| p.functions().iter())
        .find(|f| f.name == "add")
        .expect("add function not found in math_plugin");
    let a: Deck<f64> = deck![[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]];
    let b: Deck<f64> = deck![10.0, 20.0, 30.0];
    {
        let mut a_handle = OrcHandle {
            handle: 0,
            ..Default::default()
        };
        let mut b_handle = OrcHandle {
            handle: 1,
            ..Default::default()
        };
        let mut out_handle = OrcHandle {
            handle: 2,
            ..Default::default()
        };
        unsafe {
            update_handle_from_deck(&a, &mut a_handle);
            update_handle_from_deck(&b, &mut b_handle);
        }
        let inputs: &[OrcHandle] = &[a_handle, b_handle];
        unsafe {
            (add_fn.func)(0, inputs.as_ptr(), inputs.len() as u64, &mut out_handle, 1);
        }
        println!(
            "math_plugin add([1,2,3], [10,20,30]):\n{}",
            out_handle.display::<f64>()
        );
    }
    let list_length_fn = plugins
        .iter()
        .flat_map(|p| p.functions().iter())
        .find(|f| f.name == "list_length")
        .expect("list_length function not found in math_plugin");
    {
        let mut a_handle = OrcHandle {
            handle: 0,
            ..Default::default()
        };
        let mut out_handle = OrcHandle {
            handle: 2,
            ..Default::default()
        };
        unsafe {
            update_handle_from_deck(&a, &mut a_handle);
        }
        let inputs: &[OrcHandle] = &[a_handle];
        unsafe {
            (list_length_fn.func)(0, inputs.as_ptr(), inputs.len() as u64, &mut out_handle, 1);
        }
        println!("List length output:\n{}", out_handle.display::<u64>());
    }
    Ok(())
}
