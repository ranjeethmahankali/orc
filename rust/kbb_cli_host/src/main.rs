mod macros;

#[cfg(test)]
mod test;

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
    create_deck_from_proxy: None, // TODO: Implement this later.
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
    let plugin_set = orc_sdk::load_plugins(&exe_dir, &HOST)?;
    println!("Loaded {} plugin(s)\n", plugin_set.num_plugins());
    // Print the loaded plugins and functions.
    for plugin in plugin_set.plugins() {
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
    let handle_counter = std::sync::atomic::AtomicU64::new(0);
    let a_deck: Deck<f64> = deck![[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]];
    let b_deck: Deck<f64> = deck![10.0, 20.0, 30.0];
    let c_deck: Deck<f64> = deck![[1.0, 2.0, 3.0], [4.0, 5.0]];
    let (a, b, c) = {
        let (mut a, mut b, mut c) = (
            OrcHandle::default(),
            OrcHandle::default(),
            OrcHandle::default(),
        );
        a.handle = handle_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        b.handle = handle_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        c.handle = handle_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        unsafe {
            update_handle_from_deck(&a_deck, &mut a);
            update_handle_from_deck(&b_deck, &mut b);
            update_handle_from_deck(&c_deck, &mut c);
        }
        (a, b, c)
    };
    let a_plus_b = kbb_dag!(plugin_set, &handle_counter, {
        (add a b)
    });
    println!(
        "math_plugin add([1,2,3], [10,20,30]):\n{}",
        a_plus_b.display::<f64>()
    );
    let len_a = kbb_dag!(plugin_set, &handle_counter, {
        (list_length a)
    });
    println!("List length output:\n{}", len_a.display::<u64>());
    let flat_c = kbb_dag!(plugin_set, &handle_counter, {
        (flatten_deck c)
    });
    println!(
        "flatten_deck([[1,2,3],[4,5]]):\n{}",
        flat_c.display::<f64>()
    );
    let fmad_abc = kbb_dag!(plugin_set, &handle_counter, {
        (let m (mul a b))
        (add m c)
    });
    println!("mul_add_a_b_c:\n{}", fmad_abc.display::<f64>());
    let complex_result = kbb_dag!(plugin_set, &handle_counter, {
        (let c (create_complex a b))
            (let c2 (create_complex b a))
            (let (real imag) (complex_get_parts (mul_complex c c2)))
            (add real imag)
    });
    println!("complex_resunt:\n{}", complex_result.display::<f64>());
    Ok(())
}
