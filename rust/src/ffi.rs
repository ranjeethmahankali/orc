use crate::Deck;
use std::ffi::c_char;

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum OrcPrimitiveTypeId {
    // Unsigned integers.
    U8 = 0x00,
    U16 = 0x01,
    U32 = 0x02,
    U64 = 0x03,
    // Scalars.
    F32 = 0x04,
    F64 = 0x05,
    // Signed integers.
    I8 = 0x10,
    I16 = 0x11,
    I32 = 0x12,
    I64 = 0x13,
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
    pub opaque_id: u32,
}

// ========== Units ==========

pub const ORC_NUM_DIMS: usize = 7;

#[derive(PartialEq, Eq)]
#[repr(C)]
#[derive(Default)]
pub struct Dims([i32; ORC_NUM_DIMS]);

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OrcMark {
    pub(crate) depth: u8,
    pub(crate) pos: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OrcHandle {
    pub handle: u64,
    pub items: *const std::ffi::c_void,
    pub n_items: u64,
    pub item_size: u64,
    pub marks: *const OrcMark,
    pub stride_offset: *const u64,
    pub n_marks: u64,
    pub strides: *const u64,
    pub type_info: OrcTypeInfo,
}

#[repr(C)]
pub struct OrcFuncInfo {
    name: *const c_char,
    desc: *const c_char,
    inputs: *const OrcTypeInfo,
    n_inputs: u64,
    outputs: *const OrcFuncInfo,
    n_outputs: u64,
}

#[repr(C)]
pub struct OrcPlugin {
    types: *const OrcTypeInfo,
    n_types: u64,
    functions: *const OrcFuncInfo,
    n_functions: u64,
}

#[repr(C)]
pub struct OrcHost {
    /*TODO: This doesn't match the C ABI yet, because the contents of this
     * struct are a work in progress. This should be updated later.*/
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

pub trait TOrcPlugin {
    fn deck_alloc(id: OrcTypeId, out: *mut OrcHandle);
    fn deck_free(handle: *mut OrcHandle);
    fn init(host: *const OrcHost, out: *mut OrcPlugin);
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
            type_info,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
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
            opaque_id: 0,
        }
    }
}

impl Dims {
    pub fn multiply(&self, other: &Dims) -> Dims {
        let mut out = Dims([0; ORC_NUM_DIMS]);
        for i in 0..ORC_NUM_DIMS {
            out.0[i] = self.0[i] + other.0[i]
        }
        out
    }

    pub fn divide(&self, other: &Dims) -> Dims {
        let mut out = Dims([0; ORC_NUM_DIMS]);
        for i in 0..ORC_NUM_DIMS {
            out.0[i] = self.0[i] - other.0[i]
        }
        out
    }

    pub fn pow(&self, exponent: u32) -> Dims {
        let mut out = Dims([0; ORC_NUM_DIMS]);
        for i in 0..ORC_NUM_DIMS {
            out.0[i] = self.0[i].pow(exponent)
        }
        out
    }
}
