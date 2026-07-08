use crate::Deck;
use crate::bindings::*;

// ===========================================================
// Functions meant to be implemented by the plugin.
// ===========================================================

#[macro_export]
macro_rules! orc_plugin {
    ($plugin:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_plugin_init(
            host: *const $crate::bindings::OrcHost,
            plugin_data_out: *mut $crate::bindings::OrcPlugin,
        ) {
            todo!();
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_alloc(
            id: $crate::bindings::OrcTypeId,
            out: *mut $crate::bindings::OrcHandle,
        ) {
            todo!();
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_free(handle: *mut $crate::bindings::OrcHandle) {
            todo!();
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_from_proxy(
            inputs: *const $crate::bindings::OrcHandle,
            n_inputs: u64,
            proxy_type: u32,
            proxy: *const $crate::bindings::OrcHandle,
            out: *mut $crate::bindings::OrcHandle,
        ) {
            todo!();
        }
    };
}

pub trait TOrcPlugin {
    fn deck_alloc(id: OrcTypeId) -> OrcHandle;
    fn deck_free(handle: &mut OrcHandle);
    fn plugin_init(host: &OrcHost, out: &mut OrcPlugin);
}

pub trait TOrcData: Default {
    fn type_info() -> OrcTypeInfo;
}

impl<T: TOrcData> From<&Deck<T>> for OrcHandle {
    fn from(deck: &Deck<T>) -> Self {
        let type_info = T::type_info();
        let (items, marks, (stride_offset, strides)) =
            (deck.items(), deck.marks(), deck.stride_info());
        debug_assert_eq!(
            marks.len(),
            stride_offset.len(),
            "Malformed deck datastructure"
        );
        OrcHandle {
            handle: std::ptr::from_ref(deck) as u64,
            items: items.as_ptr().cast(),
            n_items: items.len() as u64,
            item_size: size_of::<T>() as u64,
            marks: marks.as_ptr(),
            stride_offset: stride_offset.as_ptr(),
            n_marks: marks.len() as u64,
            strides: strides.as_ptr(),
            type_id: type_info.type_id,
            dims: [0; ORC_NUM_DIMS as usize],
        }
    }
}

impl TOrcData for u8 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_U8,
                opaque_id: 0,
            },
            name: c"u8".as_ptr(),
            desc: c"Unsigned 8 bit integer".as_ptr(),
        }
    }
}
impl TOrcData for u16 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_U16,
                opaque_id: 0,
            },
            name: c"u16".as_ptr(),
            desc: c"Unsigned 16 bit integer".as_ptr(),
        }
    }
}
impl TOrcData for u32 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_U32,
                opaque_id: 0,
            },
            name: c"u32".as_ptr(),
            desc: c"Unsigned 32 bit integer".as_ptr(),
        }
    }
}
impl TOrcData for u64 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_U64,
                opaque_id: 0,
            },
            name: c"u64".as_ptr(),
            desc: c"Unsigned 64 bit integer".as_ptr(),
        }
    }
}
impl TOrcData for f32 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_F32,
                opaque_id: 0,
            },
            name: c"f32".as_ptr(),
            desc: c"32 bit floating point scalar".as_ptr(),
        }
    }
}
impl TOrcData for f64 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_F64,
                opaque_id: 0,
            },
            name: c"f64".as_ptr(),
            desc: c"64 bit floating point scalar".as_ptr(),
        }
    }
}
impl TOrcData for i8 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_I8,
                opaque_id: 0,
            },
            name: c"i8".as_ptr(),
            desc: c"Signed 8 bit integer".as_ptr(),
        }
    }
}
impl TOrcData for i16 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_I16,
                opaque_id: 0,
            },
            name: c"i16".as_ptr(),
            desc: c"Signed 16 bit integer".as_ptr(),
        }
    }
}
impl TOrcData for i32 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_I32,
                opaque_id: 0,
            },
            name: c"i32".as_ptr(),
            desc: c"Signed 32 bit integer".as_ptr(),
        }
    }
}
impl TOrcData for i64 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: ORC_I64,
                opaque_id: 0,
            },
            name: c"i64".as_ptr(),
            desc: c"Signed 64 bit integer".as_ptr(),
        }
    }
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
    let mut out = [0; ORC_NUM_DIMS as usize];
    for i in 0..(ORC_NUM_DIMS as usize) {
        out[i] = dims[i] * exponent
    }
    out
}
