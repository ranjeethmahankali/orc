use orc_sdk::{
    Deck, ORC_ABI_VERSION, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcHostMemoryAPI, deck,
    handle_from_deck,
};
use std::ffi::CStr;

const HOST: OrcHost = OrcHost {
    abi_version: ORC_ABI_VERSION,
    memory_api: OrcHostMemoryAPI {
        alloc: None,
        dealloc: None,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        println!("{} function(s):", plugin.functions().len());
        for func in plugin.functions().iter() {
            println!("  - {}\n    {}", func.name, func.desc);
        }
    }
    // Test the add function.
    let math_plugin = &plugins[0];
    let add_fn = math_plugin.functions()[0].func;
    let a: Deck<f64> = deck![1.0, 2.0, 3.0];
    let b: Deck<f64> = deck![10.0, 20.0, 30.0];
    let mut out_handle = math_plugin.alloc_deck(orc_sdk::ORC_TYPE_F64)?;
    let inputs: &[OrcHandle] = &[handle_from_deck(&a, 0), handle_from_deck(&b, 1)];
    unsafe {
        add_fn(0, inputs.as_ptr(), inputs.len() as u64, &mut out_handle, 1);
    }
    // Print the output data.
    println!("Output deck: \n{}", out_handle.display::<f64>());
    math_plugin.free_deck(&mut out_handle)?;
    Ok(())
}
