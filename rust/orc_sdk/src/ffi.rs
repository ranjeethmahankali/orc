use crate::{Error, bindings::*, slice_from_ptr};
use std::marker::PhantomData;

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
        ) -> orc_sdk::OrcError {
            let (host, plugin_data_out) = unsafe { (&*host, &mut *plugin_data_out) };
            match <$plugin as orc_sdk::TOrcPluginAdaptor>::plugin_init(host, plugin_data_out) {
                Ok(()) => orc_sdk::ORC_ERROR_NONE,
                Err(e) => e.into(),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_alloc(
            type_id: orc_sdk::OrcTypeId,
            out: *mut orc_sdk::OrcHandle,
        ) -> orc_sdk::OrcError {
            if out.is_null() {
                return orc_sdk::ORC_ERROR_INVALID_HANDLE;
            }
            let out = unsafe { &mut *out };
            match <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_alloc(type_id, out) {
                Ok(()) => {
                    unsafe {
                        out.free_fn = Some(orc_deck_free);
                    }
                    orc_sdk::ORC_ERROR_NONE
                }
                Err(e) => e.into(),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_free(
            handle: *mut orc_sdk::OrcHandle,
        ) -> orc_sdk::OrcError {
            if handle.is_null() {
                return orc_sdk::ORC_ERROR_NONE; // Nothing to free.
            }
            match <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_free(unsafe { &mut *handle }) {
                Ok(()) => orc_sdk::ORC_ERROR_NONE,
                Err(e) => e.into(),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_from_proxy(
            inputs: *const orc_sdk::OrcHandle,
            n_inputs: u64,
            proxy_type: orc_sdk::OrcProxyType,
            proxy: *const orc_sdk::OrcHandle,
            out: *mut orc_sdk::OrcHandle,
        ) -> orc_sdk::OrcError {
            if inputs.is_null() || proxy.is_null() || out.is_null() {
                return orc_sdk::ORC_ERROR_INVALID_HANDLE;
            }
            // Convert all the FFI pointers to Rust references.
            let (inputs, proxy, out) = unsafe {
                (
                    orc_sdk::slice_from_ptr(inputs, n_inputs as usize),
                    &*proxy,
                    &mut *out,
                )
            };
            let proxy_type = match proxy_type {
                orc_sdk::ORC_DECK_PROXY_COPY_ALL => ProxyType::CopyAll,
                orc_sdk::ORC_DECK_PROXY_COPY_ITEMS => ProxyType::CopyItems,
                orc_sdk::ORC_DECK_PROXY_SHUFFLE => ProxyType::Shuffle,
                _ => return orc_sdk::ORC_ERROR_INVALID_PROXY,
            };
            match <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_from_proxy(
                inputs, proxy_type, proxy, out,
            ) {
                Ok(_) => {
                    out.free_fn = Some(orc_deck_free);
                    orc_sdk::ORC_ERROR_NONE
                }
                Err(e) => e.into(),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_serialize(
            ctx: u64,
            handle: *const OrcHandle,
        ) -> orc_sdk::OrcError {
            let callbacks = <$plugin as orc_sdk::TOrcPluginAdaptor>::host_callbacks();
            let mut writer = orc_sdk::SerialWrite::new(ctx, callbacks.serial_write);
            match <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_serialize(
                ctx,
                unsafe { &*handle },
                &mut writer,
            ) {
                Ok(_) => match std::io::Write::flush(&mut writer) {
                    Ok(_) => orc_sdk::ORC_ERROR_NONE,
                    Err(_) => orc_sdk::ORC_ERROR_SERIALIZATION_ERROR,
                },
                Err(e) => e.into(),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_deserialize(
            ctx: u64,
            buf: *const ::std::os::raw::c_void,
            buf_len: u64,
            out: *mut OrcHandle,
        ) -> orc_sdk::OrcError {
            let bytes = unsafe { orc_sdk::slice_from_ptr(buf.cast::<u8>(), buf_len as usize) };
            let mut reader = std::io::Cursor::new(bytes);
            let out = unsafe { &mut *out };
            match <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_deserialize(ctx, &mut reader, out) {
                Ok(_) => orc_sdk::ORC_ERROR_NONE,
                Err(e) => e.into(),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orc_deck_to_str(
            input: *const orc_sdk::OrcHandle,
            output: *mut orc_sdk::OrcHandle,
        ) -> orc_sdk::OrcError {
            let (input, output) = unsafe { (&*input, &mut *output) };
            match <$plugin as orc_sdk::TOrcPluginAdaptor>::deck_to_str(input, output) {
                Ok(_) => orc_sdk::ORC_ERROR_NONE,
                Err(e) => e.into(),
            }
        }
    };
}

impl OrcHandle {
    pub fn free(&mut self) {
        if let Some(free_fn) = self.free_fn {
            let err = unsafe { free_fn(self as *mut OrcHandle) };
            if err != ORC_ERROR_NONE && !std::thread::panicking() {
                eprintln!("Unable to free OrcHandle: error {:#x}", err);
                std::process::abort();
            }
        }
    }

    /// Empty handles, could represent empty / optional inputs passed to a function.
    pub fn is_empty(&self) -> bool {
        self.items.is_null()
    }

    pub fn borrowed(&self) -> OrcHandleBorrowed<'_> {
        OrcHandleBorrowed {
            inner: OrcHandle {
                handle: self.handle,
                items: self.items,
                n_items: self.n_items,
                item_size: self.item_size,
                marks: self.marks,
                stride_offset: self.stride_offset,
                n_marks: self.n_marks,
                strides: self.strides,
                type_id: self.type_id,
                dims: self.dims,
                free_fn: None,
            },
            _borrow: PhantomData,
        }
    }

    pub fn items<T: TOrcData>(&self) -> &[T] {
        // SAFETY; We're using the pointer and the length from the same pointer.
        unsafe { slice_from_ptr(self.items.cast(), self.n_items as usize) }
    }

    pub fn items_as_bytes(&self) -> &[u8] {
        // SAFETY; We're using the pointer and the length from the same pointer.
        unsafe { slice_from_ptr(self.items.cast(), (self.n_items * self.item_size) as usize) }
    }

    pub fn marks(&self) -> &[OrcMark] {
        // SAFETY: We're using the pointer and the length from the same handle.
        unsafe { slice_from_ptr(self.marks, self.n_marks as usize) }
    }
}

/// # SAFETY
///
/// `OrcHandle` is a handle to data that crosses the FFI boundary between the host and plugins.
/// The handle data must be treated carefully: `Drop` provides a convenience for freeing the
/// backing memory, and `borrowed()` lets the borrow checker enforce lifetimes at the call site,
/// but beyond that it is the caller's responsibility to uphold safety. Transferring an
/// `OrcHandle` between threads is safe as long as the caller maintains exclusive ownership —
/// the ABI contract assumes no concurrent mutable access to the same handle.
unsafe impl Send for OrcHandle {}

/// # SAFETY
///
/// `OrcHandle` is a handle to data that crosses the FFI boundary between the host and plugins.
/// Sharing `&OrcHandle` across threads is safe because it only exposes metadata fields
/// (type_id, n_items, item_size, dims) and the raw `items` pointer value — no mutation occurs
/// through a shared reference. The caller remains responsible for ensuring no mutable aliasing
/// via the `items` pointer occurs concurrently.
unsafe impl Sync for OrcHandle {}

impl Drop for OrcHandle {
    fn drop(&mut self) {
        self.free();
    }
}

impl Default for OrcHost {
    fn default() -> Self {
        Self {
            abi_version: ORC_ABI_VERSION,
            memory_api: Default::default(),
            callbacks: Default::default(),
            create_deck_from_proxy: None,
        }
    }
}

#[repr(transparent)]
pub struct OrcHandleBorrowed<'a> {
    inner: OrcHandle,
    _borrow: PhantomData<&'a OrcHandle>,
}

const _: () = {
    assert!(
        size_of::<OrcHandleBorrowed>() == size_of::<OrcHandle>()
            && align_of::<OrcHandleBorrowed>() == align_of::<OrcHandle>(),
        "
The borrowed wrapper should be a bitwise identical wrapper to the handle.
It's job is to provide a borrow checked copy of the original handle, without it's free function.
"
    );
};

impl<'a> Clone for OrcHandleBorrowed<'a> {
    fn clone(&self) -> Self {
        Self {
            inner: OrcHandle {
                handle: self.inner.handle,
                items: self.inner.items,
                n_items: self.inner.n_items,
                item_size: self.inner.item_size,
                marks: self.inner.marks,
                stride_offset: self.inner.stride_offset,
                n_marks: self.inner.n_marks,
                strides: self.inner.strides,
                type_id: self.inner.type_id,
                dims: self.inner.dims,
                free_fn: None,
            },
            _borrow: self._borrow,
        }
    }
}

// SAFETY: OrcHandleBorrowed is a read-only view whose lifetime is tied to the
// original OrcHandle.  The raw pointers it contains (items, marks, etc.) are
// only ever read through shared references, so sending / sharing across threads
// is safe as long as the source handle outlives the borrow — which the lifetime
// parameter already guarantees.
unsafe impl Send for OrcHandleBorrowed<'_> {}
unsafe impl Sync for OrcHandleBorrowed<'_> {}

impl<'a> OrcHandleBorrowed<'a> {
    pub fn inner(&self) -> &OrcHandle {
        &self.inner
    }

    /// Empty handles, could represent empty / optional inputs passed to a function.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

pub type PluginInitFn = unsafe extern "C" fn(*const OrcHost, *mut OrcPlugin) -> OrcError;
pub type DeckAllocFn = unsafe extern "C" fn(OrcTypeId, *mut OrcHandle) -> OrcError;
pub type DeckFreeFn = unsafe extern "C" fn(*mut OrcHandle) -> OrcError;
pub type DeckFromProxyFn = unsafe extern "C" fn(
    inputs: *const OrcHandle,
    n_inputs: u64,
    proxy_type: OrcProxyType,
    proxy: *const OrcHandle,
    out: *mut OrcHandle,
) -> OrcError;
pub type DeckSerializeFn = unsafe extern "C" fn(ctx: u64, handle: *const OrcHandle) -> OrcError;
pub type DeckDeserializeFn = unsafe extern "C" fn(
    ctx: u64,
    buf: *const ::std::os::raw::c_void,
    buf_len: u64,
    out: *mut OrcHandle,
) -> OrcError;

// Compile-time checks to keep these type aliases in sync with the bindings.
const _: PluginInitFn = orc_plugin_init;
const _: DeckAllocFn = orc_deck_alloc;
const _: DeckFreeFn = orc_deck_free;
const _: DeckFromProxyFn = orc_deck_from_proxy;
const _: DeckSerializeFn = orc_deck_serialize;
const _: DeckDeserializeFn = orc_deck_deserialize;

pub enum ProxyType {
    CopyAll,
    CopyItems,
    Shuffle,
}

pub trait TOrcPluginAdaptor {
    fn host_callbacks() -> &'static OrcHostCallbackAPI;
    fn plugin_init(host: &OrcHost, out: &mut OrcPlugin) -> Result<(), Error>;
    fn deck_alloc(id: OrcTypeId, handle: &mut OrcHandle) -> Result<(), Error>;
    fn deck_free(handle: &mut OrcHandle) -> Result<(), Error>;
    fn deck_from_proxy(
        inputs: &[OrcHandle],
        proxy_type: ProxyType,
        proxy: &OrcHandle,
        out: &mut OrcHandle,
    ) -> Result<(), Error>;
    fn deck_serialize(
        ctx: u64,
        handle: &OrcHandle,
        write: &mut impl std::io::Write,
    ) -> Result<(), Error>;
    fn deck_deserialize(
        ctx: u64,
        read: &mut impl std::io::Read,
        out: &mut OrcHandle,
    ) -> Result<(), Error>;
    fn deck_to_str(input: &OrcHandle, out: &mut OrcHandle) -> Result<(), Error>;
}

pub trait TOrcData: Default + Clone + Send + Sync + 'static {
    const TYPE_INFO: OrcTypeInfo;
}

impl TOrcData for u8 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_U8,
        name: c"u8".as_ptr(),
        desc: c"Unsigned 8 bit integer".as_ptr(),
    };
}
impl TOrcData for u16 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_U16,
        name: c"u16".as_ptr(),
        desc: c"Unsigned 16 bit integer".as_ptr(),
    };
}
impl TOrcData for u32 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_U32,
        name: c"u32".as_ptr(),
        desc: c"Unsigned 32 bit integer".as_ptr(),
    };
}
impl TOrcData for u64 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_U64,
        name: c"u64".as_ptr(),
        desc: c"Unsigned 64 bit integer".as_ptr(),
    };
}
impl TOrcData for f32 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_F32,
        name: c"f32".as_ptr(),
        desc: c"32 bit floating point scalar".as_ptr(),
    };
}
impl TOrcData for f64 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_F64,
        name: c"f64".as_ptr(),
        desc: c"64 bit floating point scalar".as_ptr(),
    };
}
impl TOrcData for i8 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_I8,
        name: c"i8".as_ptr(),
        desc: c"Signed 8 bit integer".as_ptr(),
    };
}
impl TOrcData for i16 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_I16,
        name: c"i16".as_ptr(),
        desc: c"Signed 16 bit integer".as_ptr(),
    };
}
impl TOrcData for i32 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_I32,
        name: c"i32".as_ptr(),
        desc: c"Signed 32 bit integer".as_ptr(),
    };
}
impl TOrcData for i64 {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_I64,
        name: c"i64".as_ptr(),
        desc: c"Signed 64 bit integer".as_ptr(),
    };
}
impl TOrcData for OrcItemProxy {
    const TYPE_INFO: OrcTypeInfo = OrcTypeInfo {
        type_id: crate::ORC_TYPE_PROXY,
        name: c"item_proxy".as_ptr(),
        desc: c"Proxy indices that can be used to point to an element of another deck.".as_ptr(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_handle_from_deck;

    #[test]
    fn t_items_as_bytes_round_trip() {
        let data: [f32; 3] = [1.0, 2.5, -3.0];
        let mut deck = crate::Deck::<f32>::default();
        for &v in &data {
            deck.push(v, 0);
        }
        let mut h = OrcHandle::default();
        unsafe { update_handle_from_deck(&deck, &mut h) };
        let bytes = h.items_as_bytes();
        assert_eq!(bytes.len(), 3 * size_of::<f32>());
        // Re-interpret the bytes back to f32s.
        let (chunks, remainder) = bytes.as_chunks::<{ size_of::<f32>() }>();
        assert!(remainder.is_empty());
        let reconstructed: Vec<f32> = chunks.iter().map(|c| f32::from_ne_bytes(*c)).collect();
        assert_eq!(reconstructed, &data);
    }

    #[test]
    fn t_items_as_bytes_empty() {
        let h = OrcHandle::default();
        assert!(h.items_as_bytes().is_empty());
    }

    #[test]
    fn t_items_as_bytes_length_matches_n_items_times_item_size() {
        let mut deck = crate::Deck::<u64>::default();
        deck.push(42, 1);
        deck.push(99, 0);
        let mut h = OrcHandle::default();
        unsafe { update_handle_from_deck(&deck, &mut h) };
        assert_eq!(
            h.items_as_bytes().len(),
            h.n_items as usize * h.item_size as usize
        );
    }
}
