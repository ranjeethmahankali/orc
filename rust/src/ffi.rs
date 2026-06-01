use crate::{Deck, Mark};

#[repr(u32)]
pub enum OrcTypeId {
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
pub struct OrcTypeInfo {
    type_id: OrcTypeId,
    opaque_id: u32,
}

pub trait TOrcData: Default {
    fn type_info() -> OrcTypeInfo;
}

#[repr(C)]
pub struct DeckHandle {
    handle: u64,
    items: *const std::ffi::c_void,
    n_items: u64,
    marks: *const Mark,
    stride_offset: *const u64,
    n_marks: u64,
    strides: *const u64,
    type_info: OrcTypeInfo,
}

impl<T: TOrcData> From<&Deck<T>> for DeckHandle {
    fn from(deck: &Deck<T>) -> Self {
        let type_info = T::type_info();
        let (items, marks, (stride_offset, strides)) =
            (deck.items(), deck.marks(), deck.stride_info());
        debug_assert_eq!(
            marks.len(),
            stride_offset.len(),
            "Malformed deck datastructure"
        );
        DeckHandle {
            handle: std::ptr::from_ref(deck) as u64,
            items: items.as_ptr().cast(),
            n_items: items.len() as u64,
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
            type_id: OrcTypeId::U8,
            opaque_id: 0,
        }
    }
}
impl TOrcData for u16 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::U16,
            opaque_id: 0,
        }
    }
}
impl TOrcData for u32 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::U32,
            opaque_id: 0,
        }
    }
}
impl TOrcData for u64 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::U64,
            opaque_id: 0,
        }
    }
}
impl TOrcData for i8 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::I8,
            opaque_id: 0,
        }
    }
}
impl TOrcData for i16 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::I16,
            opaque_id: 0,
        }
    }
}
impl TOrcData for i32 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::I32,
            opaque_id: 0,
        }
    }
}
impl TOrcData for i64 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::I64,
            opaque_id: 0,
        }
    }
}
impl TOrcData for f32 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::F32,
            opaque_id: 0,
        }
    }
}
impl TOrcData for f64 {
    fn type_info() -> OrcTypeInfo {
        OrcTypeInfo {
            type_id: OrcTypeId::F64,
            opaque_id: 0,
        }
    }
}
