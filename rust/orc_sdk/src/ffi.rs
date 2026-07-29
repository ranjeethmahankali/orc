use crate::bindings::*;

// ===========================================================
// Functions meant to be implemented by the plugin.
// ===========================================================

#[macro_export]
macro_rules! orc_plugin {
    ($plugin:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_plugin_init(
            host: *const orc_sdk::OrcHost,
            plugin_data_out: *mut orc_sdk::OrcPlugin,
        ) {
            let (host, plugin_data_out) = unsafe { (&*host, &mut *plugin_data_out) };
            <$plugin as orc_sdk::TOrcPluginAdaptor>::plugin_init(host, plugin_data_out);
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_alloc(
            id: orc_sdk::OrcTypeId,
            out: *mut orc_sdk::OrcHandle,
        ) {
            unsafe {
                *out = <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_alloc(id);
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_free(handle: *mut orc_sdk::OrcHandle) {
            <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_free(unsafe { &mut *handle });
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_from_proxy(
            inputs: *const orc_sdk::OrcHandle,
            n_inputs: u64,
            proxy_type: u32,
            proxy: *const orc_sdk::OrcHandle,
            out: *mut orc_sdk::OrcHandle,
        ) {
            // Convert all the FFI pointers to Rust references.
            let (inputs, proxy, out) = unsafe {
                (
                    orc_sdk::slice_from_ptr(inputs, n_inputs as usize),
                    &*proxy,
                    &mut *out,
                )
            };
            assert!(
                proxy.type_id.primitive_id == orc_sdk::ORC_PROXY,
                "Invalid proxy deck"
            );
            assert!(!inputs.is_empty(), "orc_deck_from_proxy: no inputs");
            let type_id = inputs[0].type_id;
            assert!(
                inputs.iter().all(|i| i.type_id == type_id),
                "orc_deck_from_proxy: all input handles must have the same type_id"
            );
            let proxy_type = match proxy_type {
                orc_sdk::ORC_DECK_PROXY_COPY_ALL => ProxyType::CopyAll,
                orc_sdk::ORC_DECK_PROXY_COPY_ITEMS => ProxyType::CopyItems,
                orc_sdk::ORC_DECK_PROXY_SHUFFLE => ProxyType::Shuffle,
                _ => panic!("Invalid proxy type."),
            };
            let proxies: &[OrcItemProxy] = if proxy.n_items > 0 && !proxy.items.is_null() {
                unsafe { orc_sdk::slice_from_ptr(proxy.items.cast(), proxy.n_items as usize) }
            } else {
                &[]
            };
            let marks: &[OrcMark] = if proxy.n_marks > 0 && !proxy.marks.is_null() {
                unsafe { orc_sdk::slice_from_ptr(proxy.marks, proxy.n_marks as usize) }
            } else {
                &[]
            };
            <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_from_proxy(
                inputs, proxy_type, proxies, marks, out,
            );
        }
    };
}

pub enum ProxyType {
    CopyAll,
    CopyItems,
    Shuffle,
}

pub trait TOrcPluginAdaptor {
    fn plugin_init(host: &OrcHost, out: &mut OrcPlugin);
    fn deck_alloc(id: OrcTypeId) -> OrcHandle;
    fn deck_free(handle: &mut OrcHandle);
    fn deck_from_proxy(
        inputs: &[OrcHandle],
        proxy_type: ProxyType,
        proxies: &[OrcItemProxy],
        marks: &[OrcMark],
        out: &mut OrcHandle,
    );
}

pub trait TOrcData: Default + Clone + Send + Sync + 'static {
    const TYPE_INFO: OrcTypeInfo;
}

impl TOrcData for u8 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_U8,
            opaque_id: 0,
        },
        name: c"u8".as_ptr(),
        desc: c"Unsigned 8 bit integer".as_ptr(),
    };
}
impl TOrcData for u16 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_U16,
            opaque_id: 0,
        },
        name: c"u16".as_ptr(),
        desc: c"Unsigned 16 bit integer".as_ptr(),
    };
}
impl TOrcData for u32 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_U32,
            opaque_id: 0,
        },
        name: c"u32".as_ptr(),
        desc: c"Unsigned 32 bit integer".as_ptr(),
    };
}
impl TOrcData for u64 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_U64,
            opaque_id: 0,
        },
        name: c"u64".as_ptr(),
        desc: c"Unsigned 64 bit integer".as_ptr(),
    };
}
impl TOrcData for f32 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_F32,
            opaque_id: 0,
        },
        name: c"f32".as_ptr(),
        desc: c"32 bit floating point scalar".as_ptr(),
    };
}
impl TOrcData for f64 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_F64,
            opaque_id: 0,
        },
        name: c"f64".as_ptr(),
        desc: c"64 bit floating point scalar".as_ptr(),
    };
}
impl TOrcData for i8 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_I8,
            opaque_id: 0,
        },
        name: c"i8".as_ptr(),
        desc: c"Signed 8 bit integer".as_ptr(),
    };
}
impl TOrcData for i16 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_I16,
            opaque_id: 0,
        },
        name: c"i16".as_ptr(),
        desc: c"Signed 16 bit integer".as_ptr(),
    };
}
impl TOrcData for i32 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_I32,
            opaque_id: 0,
        },
        name: c"i32".as_ptr(),
        desc: c"Signed 32 bit integer".as_ptr(),
    };
}
impl TOrcData for i64 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: OrcTypeId {
            primitive_id: ORC_I64,
            opaque_id: 0,
        },
        name: c"i64".as_ptr(),
        desc: c"Signed 64 bit integer".as_ptr(),
    };
}

// ==================== Dims helper functions ====================

pub fn dims_multiply(dims: &OrcDims, other: &OrcDims) -> OrcDims {
    let mut out = [0; ORC_NUM_DIMS as usize];
    for i in 0..(ORC_NUM_DIMS as usize) {
        out[i] = dims[i] + other[i]
    }
    out
}

pub fn dims_divide(dims: &OrcDims, other: &OrcDims) -> OrcDims {
    let mut out = [0; ORC_NUM_DIMS as usize];
    for i in 0..(ORC_NUM_DIMS as usize) {
        out[i] = dims[i] - other[i]
    }
    out
}

pub fn dims_pow(dims: &OrcDims, exponent: i32) -> OrcDims {
    dims.map(|d| d * exponent)
}
