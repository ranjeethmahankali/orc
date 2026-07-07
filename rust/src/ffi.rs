use crate::Deck;
use std::ffi::{c_char, c_void};

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum OrcPrimitiveTypeId {
    // Unsigned integers.
    U8 = 0x01,
    U16 = 0x02,
    U32 = 0x03,
    U64 = 0x04,
    // Scalars.
    F32 = 0x05,
    F64 = 0x06,
    // Signed integers.
    I8 = 0x11,
    I16 = 0x12,
    I32 = 0x13,
    I64 = 0x14,
    // All custom opaque types defined by a plugin.
    Opaque = u32::MAX,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OrcTypeId {
    pub primitive_id: OrcPrimitiveTypeId,
    pub opaque_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OrcTypeInfo {
    pub type_id: OrcTypeId,
    pub name: *const c_char,
    pub desc: *const c_char,
}

#[repr(C)]
pub struct OrcFuncInfo {
    name: *const c_char,
    desc: *const c_char,
    n_inputs: u64,
    n_outputs: u64,

    func: extern "C" fn(
        ctx: u64,
        inputs: *const OrcHandle,
        n_inputs: u64,
        outputs: *mut OrcHandle,
        n_outputs: u64,
    ),
}

#[repr(C)]
pub struct OrcPlugin {
    types: *const OrcTypeInfo,
    n_types: u64,
    functions: *const OrcFuncInfo,
    n_functions: u64,
}

#[repr(C)]
pub struct OrcHostMemoryApi {
    alloc: unsafe extern "C" fn(nbytes: u64, alignment: u64) -> *mut c_void,
    dealloc: unsafe extern "C" fn(ptr: *mut c_void, nbytes: u64, alignment: u64),
}

#[repr(C)]
pub struct OrcHostCallbacks {
    report_progress: extern "C" fn(ctx: u64, progress: f64),
    report_error: extern "C" fn(ctx: u64, error: *const c_char),
    report_warning: extern "C" fn(ctx: u64, warning: *const c_char),
    check_cancellation: extern "C" fn(ctx: u64) -> bool,
}

#[repr(C)]
pub struct OrcHost {
    memory_api: OrcHostMemoryApi,
    callbacks: OrcHostCallbacks,
}

pub const ORC_NUM_DIMS: usize = 7;

#[derive(PartialEq, Eq)]
#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct OrcDims([i32; ORC_NUM_DIMS]);

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OrcMark {
    pub(crate) depth: u8,
    pub(crate) pos: u64,
}

#[repr(C)]
#[derive(Clone)]
pub struct OrcHandle {
    pub handle: u64,
    pub items: *const std::ffi::c_void,
    pub n_items: u64,
    pub item_size: u64,
    pub marks: *const OrcMark,
    pub stride_offset: *const u64,
    pub n_marks: u64,
    pub strides: *const u64,
    pub type_id: OrcTypeId,
    pub dims: OrcDims,
}

#[repr(C)]
pub struct OrcItemProxy {
    tree: u64,
    item: u64,
}

// ===========================================================
// Functions meant to be implemented by the plugin.
// ===========================================================

#[macro_export]
macro_rules! orc_plugin {
    ($plugin:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_plugin_init(
            host: *const OrcHost,
            plugin_data_out: *mut OrcPlugin,
        ) {
            todo!()
        }

        // This function should be inside a macro, to be invoked (hence defining the function) inside the plugin.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_alloc(_id: OrcTypeId, _out: *mut OrcHandle) {
            todo!()
        }

        // This function should be inside a macro, to be invoked (hence defining the function) inside the plugin.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_free(_handle: *mut OrcHandle) {
            todo!()
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_from_proxy(
            inputs: *const OrcHandle, n_inputs: u64,
            proxy_type: u32, proxy: *const OrcHandle,
            out: *OrcHandle
        ) {
            todo!()
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
            dims: OrcDims([0; ORC_NUM_DIMS]),
        }
    }
}

impl TOrcData for u8 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId {
                primitive_id: OrcPrimitiveTypeId::U8,
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
                primitive_id: OrcPrimitiveTypeId::U16,
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
                primitive_id: OrcPrimitiveTypeId::U32,
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
                primitive_id: OrcPrimitiveTypeId::U64,
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
                primitive_id: OrcPrimitiveTypeId::F32,
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
                primitive_id: OrcPrimitiveTypeId::F64,
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
                primitive_id: OrcPrimitiveTypeId::I8,
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
                primitive_id: OrcPrimitiveTypeId::I16,
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
                primitive_id: OrcPrimitiveTypeId::I32,
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
                primitive_id: OrcPrimitiveTypeId::I64,
                opaque_id: 0,
            },
            name: c"i64".as_ptr(),
            desc: c"Signed 64 bit integer".as_ptr(),
        }
    }
}

impl OrcDims {
    pub fn multiply(&self, other: &OrcDims) -> OrcDims {
        let mut out = OrcDims([0; ORC_NUM_DIMS]);
        for i in 0..ORC_NUM_DIMS {
            out.0[i] = self.0[i] + other.0[i]
        }
        out
    }

    pub fn divide(&self, other: &OrcDims) -> OrcDims {
        let mut out = OrcDims([0; ORC_NUM_DIMS]);
        for i in 0..ORC_NUM_DIMS {
            out.0[i] = self.0[i] - other.0[i]
        }
        out
    }

    pub fn pow(&self, exponent: u32) -> OrcDims {
        let mut out = OrcDims([0; ORC_NUM_DIMS]);
        for i in 0..ORC_NUM_DIMS {
            out.0[i] = self.0[i].pow(exponent)
        }
        out
    }
}
