use crate::{
    Deck, DeckView, Error, ORC_ARGS_VARIADIC, ORC_MSG_LEVEL_DEBUG, ORC_MSG_LEVEL_ERROR,
    ORC_MSG_LEVEL_FATAL, ORC_MSG_LEVEL_INFO, ORC_MSG_LEVEL_WARN, ORC_NUM_DIMS, OrcFuncInfo,
    OrcHandle, OrcHost, OrcHostCallbackAPI, OrcItemProxy, OrcMark, OrcPluginFunction, OrcTypeId,
    OrcTypeInfo, ProxyType, deck::fmt_raw_deck, ffi::TOrcData,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    any::Any,
    collections::{HashMap, hash_map::Entry},
    ffi::{CStr, CString, c_void},
    fmt::Display,
    marker::PhantomData,
    sync::{
        Arc, RwLock,
        atomic::{AtomicPtr, Ordering},
    },
};

pub fn ptr_from_slice<T>(arr: &[T]) -> *const T {
    if arr.is_empty() {
        std::ptr::null()
    } else {
        arr.as_ptr()
    }
}

/// Create a slice from a pointer and length. Meant for working with data coming across the FFI
/// boundary. Checks for null pointers and empty slices and handles them gracefully.
///
/// # SAFETY
/// The caller must ensure the pointer and the length are valid.
pub unsafe fn slice_from_ptr<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// Create a mutable slice from a pointer and length. Meant for working with data coming across the
/// FFI boundary. Checks for null pointers and empty slices and handles them gracefully.
///
/// # SAFETY
/// The caller must ensure the pointer and the length are valid.
pub unsafe fn slice_from_ptr_mut<'a, T>(ptr: *mut T, len: usize) -> &'a mut [T] {
    if ptr.is_null() || len == 0 {
        &mut []
    } else {
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }
}

pub fn string_from_ffi(ptr: *const std::ffi::c_char) -> String {
    if ptr.is_null() {
        String::default()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

/// # SAFETY
///
/// The caller must ensure the handle they're passing in, uniquely owns the deck they're passing. If
/// this invariant is not met, this could lead to use after free and other sorts of bugs.
pub unsafe fn update_handle_from_deck<T: TOrcData>(deck: &Deck<T>, handle: &mut OrcHandle) {
    // We should not touch the handle.handle. We only reassign things that we can infer from the Deck<T>.
    let (items, marks, (stride_offset, strides)) = (deck.items(), deck.marks(), deck.stride_info());
    handle.items = ptr_from_slice(items).cast();
    handle.n_items = items.len() as u64;
    handle.item_size = size_of::<T>() as u64;
    handle.marks = ptr_from_slice(marks);
    handle.stride_offset = ptr_from_slice(stride_offset);
    handle.n_marks = marks.len() as u64;
    handle.strides = ptr_from_slice(strides);
    handle.type_id = T::TYPE_INFO.type_id;
}

pub fn reset_handle(handle: &mut OrcHandle) {
    handle.items = std::ptr::null();
    handle.n_items = 0;
    handle.item_size = 0;
    handle.marks = std::ptr::null();
    handle.stride_offset = std::ptr::null();
    handle.n_marks = 0;
    handle.strides = std::ptr::null();
    handle.type_id = 0;
    handle.dims = [0; ORC_NUM_DIMS as usize];
    handle.free_fn = None;
}

type ObjEntry = Arc<RwLock<Box<dyn Any + Send + Sync>>>;

pub struct DeckRegistry {
    handles: RwLock<HashMap<u64, ObjEntry>>,
}

impl Default for DeckRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeckRegistry {
    pub fn new() -> Self {
        DeckRegistry {
            handles: RwLock::new(HashMap::new()),
        }
    }

    pub fn alloc<T: TOrcData + Any + Send + Sync>(
        &self,
        handle: &mut OrcHandle,
    ) -> Result<(), Error> {
        self.alloc_with_value(Deck::<T>::default(), handle)
    }

    pub fn alloc_with_value<T: TOrcData + Any + Send + Sync>(
        &self,
        value: Deck<T>,
        handle: &mut OrcHandle,
    ) -> Result<(), Error> {
        // Here, the id is valid, so we try to find the previous allocation and reuse it. If it
        // doesn't match the type, or doesn't exist, we just reallocate. The line below can block
        // this thread until write access is available.
        let mut handles = self
            .handles
            .write()
            .map_err(|_e| Error::ConcurrencyProblem)?;
        match handles.entry(handle.handle) {
            Entry::Occupied(mut occupied) => {
                let type_matches = {
                    let read_lock = occupied
                        .get()
                        .try_read()
                        .map_err(|_e| Error::ConcurrencyProblem)?;
                    read_lock.downcast_ref::<Deck<T>>().is_some()
                };
                if type_matches {
                    // Same type — reuse the allocation but clear the deck so the next write
                    // starts fresh and doesn't see stale data from the previous call.
                    let mut write_lock = occupied
                        .get()
                        .try_write()
                        .map_err(|_e| Error::ConcurrencyProblem)?;
                    let deck = write_lock
                        .downcast_mut::<Deck<T>>()
                        .ok_or(Error::DeckTypeMismatch)?;
                    deck.clear();
                    unsafe { update_handle_from_deck(deck, handle) };
                } else {
                    // Different type — drop the old deck and insert a fresh one.
                    unsafe { update_handle_from_deck(&value, handle) };
                    occupied.insert(Arc::new(RwLock::new(Box::new(value))));
                }
                handle.free_fn = Some(crate::orc_deck_free);
            }
            Entry::Vacant(vacant) => {
                // This handle could be pointing to data inside another plugin. So we have to free that data first, before reassigning.
                handle.free();
                unsafe { update_handle_from_deck(&value, handle) };
                vacant.insert(Arc::new(RwLock::new(Box::new(value))));
                handle.free_fn = Some(crate::orc_deck_free);
            }
        };
        Ok(())
    }

    pub fn free(&self, id: u64) -> Result<(), Error> {
        // This can block this thread until write access is available.
        let mut handles = self
            .handles
            .write()
            .map_err(|_e| Error::ConcurrencyProblem)?;
        handles.remove(&id).ok_or(Error::InvalidHandle).map(|_| ())
    }

    pub fn with_mut<TResult, F>(&self, ids: &[u64], callback: F) -> Result<TResult, Error>
    where
        F: FnOnce(&mut [&mut (dyn Any + Send + Sync)]) -> TResult,
    {
        let arcs: Vec<_> = {
            // This can block this thread until read access is available.
            let map = self
                .handles
                .read()
                .map_err(|_e| Error::ConcurrencyProblem)?;
            ids.iter()
                .map(|id| map.get(id).cloned().ok_or(Error::InvalidHandle))
                .collect::<Result<_, _>>()?
        };
        let mut guards: Vec<_> = arcs
            .iter()
            .map(|arc| arc.try_write().map_err(|_e| Error::ConcurrencyProblem))
            .collect::<Result<_, _>>()?;
        let mut references: Vec<&mut (dyn Any + Send + Sync)> = guards
            .iter_mut()
            .map(|guard| guard.as_mut() as &mut (dyn Any + Send + Sync))
            .collect();
        Ok(callback(&mut references))
    }

    pub fn with_refs<TResult, F>(&self, ids: &[u64], callback: F) -> Result<TResult, Error>
    where
        F: FnOnce(&[&(dyn Any + Send + Sync)]) -> TResult,
    {
        let arcs: Vec<_> = {
            // This can block this thread until read access is available.
            let map = self
                .handles
                .read()
                .map_err(|_e| Error::ConcurrencyProblem)?;
            ids.iter()
                .map(|id| map.get(id).cloned().ok_or(Error::InvalidHandle))
                .collect::<Result<_, _>>()?
        };
        let mut guards: Vec<_> = arcs
            .iter()
            .map(|arc| arc.try_read().map_err(|_e| Error::ConcurrencyProblem))
            .collect::<Result<_, _>>()?;
        let references: Vec<&(dyn Any + Send + Sync)> = guards
            .iter_mut()
            .map(|guard| guard.as_ref() as &(dyn Any + Send + Sync))
            .collect();
        Ok(callback(&references))
    }

    pub fn count(&self) -> Result<usize, Error> {
        self.handles
            .read()
            .map(|map| map.len())
            .map_err(|_| Error::ConcurrencyProblem)
    }
}

// ==================================================
// ================= Allocators =====================
// ==================================================

type HostAllocFn = unsafe extern "C" fn(size: u64, alignment: u64) -> *mut c_void;
type HostDeallocFn = unsafe extern "C" fn(ptr: *mut c_void, size: u64, alignment: u64);

unsafe extern "C" fn system_alloc(size: u64, alignment: u64) -> *mut c_void {
    unsafe {
        let layout = Layout::from_size_align_unchecked(size as usize, alignment as usize);
        System.alloc(layout) as *mut c_void
    }
}

unsafe extern "C" fn system_dealloc(ptr: *mut c_void, size: u64, alignment: u64) {
    unsafe {
        let layout = Layout::from_size_align_unchecked(size as usize, alignment as usize);
        System.dealloc(ptr as *mut u8, layout);
    }
}

pub struct PluginAllocator {
    alloc_fn: AtomicPtr<()>,
    dealloc_fn: AtomicPtr<()>,
}

impl Default for PluginAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginAllocator {
    pub const fn new() -> Self {
        PluginAllocator {
            alloc_fn: AtomicPtr::new(system_alloc as *mut ()),
            dealloc_fn: AtomicPtr::new(system_dealloc as *mut ()),
        }
    }

    pub fn init_from_host(&self, host: &OrcHost) {
        if let Some(f) = host.memory_api.alloc {
            self.alloc_fn.store(f as *mut (), Ordering::Release);
        }
        if let Some(f) = host.memory_api.dealloc {
            self.dealloc_fn.store(f as *mut (), Ordering::Release);
        }
    }
}

unsafe impl Sync for PluginAllocator {}

unsafe impl GlobalAlloc for PluginAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let f: HostAllocFn = unsafe { std::mem::transmute(self.alloc_fn.load(Ordering::Relaxed)) };
        unsafe { f(layout.size() as u64, layout.align() as u64) as *mut u8 }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let f: HostDeallocFn =
            unsafe { std::mem::transmute(self.dealloc_fn.load(Ordering::Relaxed)) };
        unsafe {
            f(
                ptr as *mut c_void,
                layout.size() as u64,
                layout.align() as u64,
            )
        }
    }
}

// ==================================================
// ========= Rust native types for C types =========
// ==================================================
//
// It is nicer to have Rust native types for these things. Using the C types directly may require
// some assumptions about the lifetimes of the raw pointers. And the users of the SDK are still free
// to use the raw C types if that fits their needs better. But if they prefer Rust types beyond the
// FFI boundary, types below can help. For example, a host program written in Rust can keep track of
// various plugins, their types and functions etc. using the Rust types below.

#[derive(Clone, Debug)]
pub struct TypeInfo {
    pub type_id: OrcTypeId,
    pub name: String,
    pub desc: String,
}

impl From<&OrcTypeInfo> for TypeInfo {
    fn from(info: &OrcTypeInfo) -> Self {
        Self {
            type_id: info.type_id,
            name: string_from_ffi(info.name.cast()),
            desc: string_from_ffi(info.desc.cast()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FuncInfo {
    pub name: String,
    pub desc: String,
    pub n_inputs: Option<usize>,
    pub n_outputs: Option<usize>,
    pub func: OrcPluginFunction,
}

impl From<&OrcFuncInfo> for FuncInfo {
    fn from(info: &OrcFuncInfo) -> Self {
        Self {
            name: string_from_ffi(info.name.cast()),
            desc: string_from_ffi(info.desc.cast()),
            n_inputs: if info.n_inputs == ORC_ARGS_VARIADIC {
                None
            } else {
                Some(info.n_inputs as usize)
            },
            n_outputs: if info.n_outputs == ORC_ARGS_VARIADIC {
                None
            } else {
                Some(info.n_outputs as usize)
            },
            func: info.func,
        }
    }
}

/// This is a helper for displaying handle data.
pub struct HandleDisplayWrapper<'a, T: TOrcData + Display> {
    handle: &'a OrcHandle,
    _phantom: PhantomData<T>,
}

impl<'a, T: TOrcData + Display> Display for HandleDisplayWrapper<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "handle: {}", self.handle.handle)?;
        writeln!(f, "type_id: {:?}", self.handle.type_id)?;
        writeln!(f, "dims: {:?}", self.handle.dims)?;
        let (items, marks) = unsafe {
            (
                slice_from_ptr::<T>(self.handle.items.cast(), self.handle.n_items as usize),
                slice_from_ptr(self.handle.marks, self.handle.n_marks as usize),
            )
        };
        fmt_raw_deck(items, marks, f)?;
        Ok(())
    }
}

impl OrcHandle {
    pub fn display<'a, T: TOrcData + Display>(&'a self) -> HandleDisplayWrapper<'a, T> {
        HandleDisplayWrapper {
            handle: self,
            _phantom: PhantomData,
        }
    }
}

// ==================================================
// ========= Safe wrapper for host callbacks =======
// ==================================================

pub struct HostCallbacks {
    pub inner: OrcHostCallbackAPI,
    pub context: u64,
}

impl HostCallbacks {
    pub const DUMMY_CALLBACKS: OrcHostCallbackAPI = OrcHostCallbackAPI {
        report_progress: None,
        report_message: None,
        check_cancellation: None,
        report_intermediate_output: None,
    };

    pub fn report_progress(&self, progress: f64) {
        if let Some(callback) = self.inner.report_progress {
            // SAFETY: We just checked to make sure the function is not None.
            unsafe { callback(self.context, progress) }
        }
    }

    pub fn debug(&self, message: &str) {
        if let Some(callback) = self.inner.report_message {
            let cstr = CString::new(message).unwrap_or_default();
            // SAFETY: We just checked to make sure the function is not None.
            unsafe { callback(self.context, ORC_MSG_LEVEL_DEBUG, cstr.as_ptr()) }
        }
    }

    pub fn info(&self, message: &str) {
        if let Some(callback) = self.inner.report_message {
            let cstr = CString::new(message).unwrap_or_default();
            // SAFETY: We just checked to make sure the function is not None.
            unsafe { callback(self.context, ORC_MSG_LEVEL_INFO, cstr.as_ptr()) }
        }
    }

    pub fn warn(&self, message: &str) {
        if let Some(callback) = self.inner.report_message {
            let cstr = CString::new(message).unwrap_or_default();
            // SAFETY: We just checked to make sure the function is not None.
            unsafe { callback(self.context, ORC_MSG_LEVEL_WARN, cstr.as_ptr()) }
        }
    }

    pub fn error(&self, message: &str) {
        if let Some(callback) = self.inner.report_message {
            let cstr = CString::new(message).unwrap_or_default();
            // SAFETY: We just checked to make sure the function is not None.
            unsafe { callback(self.context, ORC_MSG_LEVEL_ERROR, cstr.as_ptr()) }
        }
    }

    pub fn fatal(&self, message: &str) {
        if let Some(callback) = self.inner.report_message {
            let cstr = CString::new(message).unwrap_or_default();
            // SAFETY: We just checked to make sure the function is not None.
            unsafe { callback(self.context, ORC_MSG_LEVEL_FATAL, cstr.as_ptr()) }
        }
    }

    pub fn check_cancellation(&self) -> bool {
        match self.inner.check_cancellation {
            // SAFETY: We just checked to make sure the function is not None.
            Some(callback) => unsafe { callback(self.context) },
            None => false,
        }
    }

    pub fn report_intermediate_output(&self, handle: &OrcHandle) {
        if let Some(callback) = self.inner.report_intermediate_output {
            // SAFETY: We just checked to make sure the function is not None.
            unsafe { callback(self.context, handle) }
        }
    }
}

/// This is meant to be used by the plugin function to check for invariants, report them to the host
/// if they're not met, and return the given error code immediately.
#[macro_export]
macro_rules! orc_check_return {
    ($host:expr, $err:expr, $cond:expr, $($fmt:tt)+) => {{
        if !($cond) {
            let message = ::std::format!($($fmt)+);
            $host.error(&message);
            return $err;
        }
    }};
}

/// This is meant to be used by the plugin function to check for invariants, report them to the host
/// if they're not met.
#[macro_export]
macro_rules! orc_check_warn {
    ($host:expr, $cond:expr, $($fmt:tt)+) => {{
        if !($cond) {
            let message = ::std::format!($($fmt)+);
            $host.warn(&message);
        }
    }};
}

/// Helper to quickly access all the built_in_types.
pub const PRIMITIVE_TYPES: &[OrcTypeInfo] = &[
    u8::TYPE_INFO,
    u16::TYPE_INFO,
    u32::TYPE_INFO,
    u64::TYPE_INFO,
    f32::TYPE_INFO,
    f64::TYPE_INFO,
    i8::TYPE_INFO,
    i16::TYPE_INFO,
    i32::TYPE_INFO,
    i64::TYPE_INFO,
];

//==================== Helper to create proxy decks ====================

pub fn deck_from_proxy<T: TOrcData>(
    inputs: &[OrcHandle],
    proxy_type: ProxyType,
    proxy: &OrcHandle,
    out: &mut OrcHandle,
    registry: &DeckRegistry,
) -> Result<(), Error> {
    let type_id = match inputs.first() {
        Some(input) => input.type_id,
        None => return Err(Error::InvalidProxy),
    };
    if inputs.iter().skip(1).any(|h| h.type_id != type_id) {
        // All inputs must be of the same type. This is a problem.
        return Err(Error::InvalidProxy);
    }
    out.dims = proxy.dims;
    registry.alloc::<T>(out)?;
    registry
        .with_mut(&[out.handle], |out_decks| -> Result<(), Error> {
            let out_deck = out_decks[0]
                .downcast_mut::<Deck<T>>()
                .ok_or(Error::DeckTypeMismatch)?;
            let (items, marks) = match proxy_type {
                ProxyType::CopyAll => {
                    // We expect exactly one input, and we will make a full clone of that data.
                    if inputs.len() != 1 {
                        return Err(Error::InvalidProxy);
                    }
                    let input_handle = unsafe { inputs.get_unchecked(0) }; // SAFETY: we just checked above.
                    let input = DeckView::<T>::from_handle(input_handle)?;
                    (input.items().to_vec(), input.marks().to_vec())
                }
                ProxyType::CopyItems => {
                    // We expect exactly one input. We will copy the items of the input, but the marks from the proxy.
                    if inputs.len() != 1 {
                        return Err(Error::InvalidProxy);
                    }
                    let input_handle = unsafe { inputs.get_unchecked(0) }; // SAFETY: we just checked above.
                    let input = DeckView::<T>::from_handle(input_handle)?;
                    let proxy = DeckView::<OrcItemProxy>::from_handle(proxy)?;
                    (input.items().to_vec(), proxy.marks().to_vec())
                }
                ProxyType::Shuffle => {
                    let proxy = DeckView::<OrcItemProxy>::from_handle(proxy)?;
                    let inputs = inputs
                        .iter()
                        .map(|input| DeckView::<T>::from_handle(input))
                        .collect::<Result<Box<[DeckView<T>]>, Error>>()?;
                    (
                        proxy
                            .items()
                            .iter()
                            .map(|ii| inputs[ii.tree as usize].items()[ii.item as usize].clone())
                            .collect::<Vec<T>>(),
                        proxy.marks().to_vec(),
                    )
                }
            };
            out_deck.assign_from_raw_data(items, marks);
            unsafe { update_handle_from_deck(out_deck, out) }; // SAFETY: we pulled the deck out of the same handle.
            Ok(())
        })
        .flatten()
}

// ==================== Serialization ====================

pub struct SerialWrite {
    ctx: u64,
    write_func: crate::OrcSerializeWriteFn,
    buf: Vec<u8>,
}

impl SerialWrite {
    pub fn new(ctx: u64, write_func: crate::OrcSerializeWriteFn) -> Self {
        Self {
            ctx,
            write_func,
            buf: Default::default(),
        }
    }
}

impl std::io::Write for SerialWrite {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let func = self
            .write_func
            .ok_or(std::io::Error::from(std::io::ErrorKind::NotFound))?;
        let err_kind =
            match unsafe { (func)(self.ctx, self.buf.as_ptr().cast(), self.buf.len() as u64) } {
                crate::ORC_ERROR_NONE => return Ok(()),
                crate::ORC_ERROR_ABI_VERSION_MISMATCH | crate::ORC_ERROR_MISSING_CAPABILITY => {
                    std::io::ErrorKind::Unsupported
                }
                crate::ORC_ERROR_INVALID_HANDLE
                | crate::ORC_ERROR_INVALID_DIMENSIONS
                | crate::ORC_ERROR_TYPE_MISMATCH
                | crate::ORC_ERROR_INVALID_COMBINATIONS
                | crate::ORC_ERROR_INVALID_PROXY
                | crate::ORC_ERROR_INVALID_FUNCTION
                | crate::ORC_ERROR_INVALID_ARGUMENTS => std::io::ErrorKind::InvalidData,
                crate::ORC_ERROR_PLUGIN_ALREADY_INITIALIZED => std::io::ErrorKind::AlreadyExists,
                crate::ORC_ERROR_CONCURRENCY_PROBLEM => std::io::ErrorKind::Other,
                crate::ORC_ERROR_CANNOT_LOAD_PLUGINS | crate::ORC_ERROR_NULL_PTR => {
                    std::io::ErrorKind::NotFound
                }
                crate::ORC_ERROR_OUT_OF_BOUNDS => std::io::ErrorKind::FileTooLarge,
                crate::ORC_ERROR_ALLOC_FAILED => std::io::ErrorKind::OutOfMemory,
                _ => std::io::ErrorKind::Other,
            };
        Err(std::io::Error::from(err_kind))
    }
}

// Helper functions to serialize the structure of an OrcHandle, without the data. All SDKs that want
// to work with the same data must comply with this plain text format.

pub fn write_orc_handle_structure(
    handle: &OrcHandle,
    f: &mut impl std::io::Write,
) -> std::io::Result<()> {
    writeln!(
        f,
        "type_id: {}\nn_items: {}\nitem_size: {}",
        handle.type_id, handle.n_items, handle.item_size
    )?;
    writeln!(
        f,
        "dims: {} {} {} {} {} {} {}",
        handle.dims[0],
        handle.dims[1],
        handle.dims[2],
        handle.dims[3],
        handle.dims[4],
        handle.dims[5],
        handle.dims[6]
    )?;
    writeln!(f, "n_marks: {}", handle.n_marks)?;
    let marks = unsafe { slice_from_ptr(handle.marks, handle.n_marks as usize) };
    if !marks.is_empty() {
        writeln!(f, "marks:")?;
        for m in marks {
            writeln!(f, "    {} {}", m.pos, m.depth)?;
        }
    }
    Ok(())
}

pub fn read_orc_handle_structure(
    handle: &mut OrcHandle,
    r: &mut impl std::io::BufRead,
) -> std::io::Result<Vec<OrcMark>> {
    let err_fn = || std::io::Error::from(std::io::ErrorKind::InvalidData);
    let mut marks = Vec::new();
    let mut in_marks = false;
    let mut line = String::new();
    while r.read_line(&mut line)? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }
        if in_marks {
            // Mark lines are indented: "    pos depth"
            if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
                let mut parts = trimmed.split_whitespace();
                let pos: u64 = parts
                    .next()
                    .ok_or_else(err_fn)?
                    .parse()
                    .map_err(|_| err_fn())?;
                let depth: u8 = parts
                    .next()
                    .ok_or_else(err_fn)?
                    .parse()
                    .map_err(|_| err_fn())?;
                marks.push(OrcMark { depth, pos });
                line.clear();
                continue;
            }
            in_marks = false;
        }
        if let Some(val) = trimmed.strip_prefix("type_id:") {
            handle.type_id = val.trim().parse().map_err(|_| err_fn())?;
        } else if let Some(val) = trimmed.strip_prefix("n_items:") {
            handle.n_items = val.trim().parse().map_err(|_| err_fn())?;
        } else if let Some(val) = trimmed.strip_prefix("item_size:") {
            handle.item_size = val.trim().parse().map_err(|_| err_fn())?;
        } else if let Some(val) = trimmed.strip_prefix("n_marks:") {
            handle.n_marks = val.trim().parse().map_err(|_| err_fn())?;
        } else if let Some(val) = trimmed.strip_prefix("dims:") {
            let nums: Vec<i32> = val
                .split_whitespace()
                .map(|s| s.parse().map_err(|_| err_fn()))
                .collect::<std::io::Result<_>>()?;
            if nums.len() != crate::ORC_NUM_DIMS as usize {
                return Err(err_fn());
            }
            handle.dims.copy_from_slice(&nums);
        } else if trimmed == "marks:" {
            in_marks = true;
        } else {
            return Err(err_fn());
        }
        line.clear();
    }
    handle.n_marks = marks.len() as u64;
    handle.marks = ptr_from_slice(&marks);
    Ok(marks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Deck, ORC_ERROR_NONE, ORC_NUM_DIMS, OrcError, OrcHandle, ffi::TOrcData};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    // Provide orc_deck_free for the test binary. Each plugin normally defines this via
    // orc_plugin!. In tests we manage the registry directly, so this is a no-op.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn orc_deck_free(_handle: *mut OrcHandle) -> OrcError {
        ORC_ERROR_NONE
    }

    fn fresh_handle(id: u64) -> OrcHandle {
        OrcHandle {
            handle: id,
            ..Default::default()
        }
    }

    // Helper: suppress the no-op Drop call on a handle we already cleaned up.
    fn disarm(h: &mut OrcHandle) {
        h.free_fn = None;
    }

    // ==================== DeckRegistry ====================

    #[test]
    fn alloc_fresh() {
        let reg = DeckRegistry::new();
        let mut h = fresh_handle(1);
        reg.alloc::<f32>(&mut h).unwrap();
        assert!(h.free_fn.is_some());
        assert_eq!(h.type_id, f32::TYPE_INFO.type_id);
        assert_eq!(h.handle, 1);
        disarm(&mut h);
    }

    #[test]
    fn alloc_reuse_same_type() {
        // Second alloc with the same type should be a no-op: no error, type and ID unchanged.
        // We verify the old free_fn is NOT called by checking the handle still has a valid entry
        // in the registry after the second alloc.
        let reg = DeckRegistry::new();
        let mut h = fresh_handle(2);
        reg.alloc::<f64>(&mut h).unwrap();
        let type_id_before = h.type_id;
        reg.alloc::<f64>(&mut h).unwrap();
        assert_eq!(h.type_id, type_id_before);
        assert_eq!(h.handle, 2);
        // Entry must still be accessible (not freed).
        reg.with_mut::<(), _>(&[2], |_| ()).unwrap();
        disarm(&mut h);
    }

    #[test]
    fn alloc_type_change() {
        // Second alloc with a different type: old deck replaced, new type reflected in handle.
        let reg = DeckRegistry::new();
        let mut h = fresh_handle(3);
        reg.alloc::<i32>(&mut h).unwrap();
        assert_eq!(h.type_id, i32::TYPE_INFO.type_id);
        reg.alloc::<f64>(&mut h).unwrap();
        assert_eq!(h.type_id, f64::TYPE_INFO.type_id);
        assert_eq!(h.handle, 3);
        // New type is accessible.
        reg.with_mut(&[3], |decks| {
            assert!(decks[0].downcast_mut::<Deck<f64>>().is_some());
        })
        .unwrap();
        disarm(&mut h);
    }

    #[test]
    fn alloc_eviction_foreign() {
        // A handle whose free_fn belongs to a foreign plugin: alloc must evict it first.
        static FOREIGN_CALLED: AtomicUsize = AtomicUsize::new(0);
        unsafe extern "C" fn mock_foreign(_handle: *mut OrcHandle) -> OrcError {
            FOREIGN_CALLED.fetch_add(1, Ordering::Relaxed);
            ORC_ERROR_NONE
        }
        let reg = DeckRegistry::new();
        let mut h = OrcHandle {
            handle: 4,
            free_fn: Some(mock_foreign),
            ..Default::default()
        };
        let before = FOREIGN_CALLED.load(Ordering::Relaxed);
        reg.alloc::<u32>(&mut h).unwrap();
        assert_eq!(FOREIGN_CALLED.load(Ordering::Relaxed), before + 1);
        assert_eq!(h.handle, 4);
        assert_eq!(h.type_id, u32::TYPE_INFO.type_id);
        disarm(&mut h);
    }

    #[test]
    fn free_success() {
        let reg = DeckRegistry::new();
        let mut h = fresh_handle(5);
        reg.alloc::<f32>(&mut h).unwrap();
        reg.free(5).unwrap();
        // A plugin calls reset_handle after registry.free to clear the handle fields.
        reset_handle(&mut h);
        assert!(h.free_fn.is_none());
        assert!(h.items.is_null());
        assert_eq!(h.handle, 5);
    }

    #[test]
    fn free_unregistered() {
        let reg = DeckRegistry::new();
        assert!(matches!(reg.free(999), Err(Error::InvalidHandle)));
    }

    #[test]
    fn free_does_not_clear_handle_id() {
        let reg = DeckRegistry::new();
        let mut h = fresh_handle(6);
        reg.alloc::<u8>(&mut h).unwrap();
        reg.free(6).unwrap();
        assert_eq!(h.handle, 6);
        disarm(&mut h);
    }

    #[test]
    fn with_mut_borrows_and_mutates() {
        let reg = DeckRegistry::new();
        let mut h = fresh_handle(10);
        reg.alloc::<f64>(&mut h).unwrap();
        reg.with_mut(&[10], |decks| {
            let deck = decks[0].downcast_mut::<Deck<f64>>().expect("wrong type");
            deck.push(3.1234, 1);
        })
        .unwrap();
        reg.with_mut(&[10], |decks| {
            let deck = decks[0].downcast_mut::<Deck<f64>>().expect("wrong type");
            assert_eq!(deck.items(), &[3.1234]);
        })
        .unwrap();
        disarm(&mut h);
    }

    #[test]
    fn with_mut_missing_id() {
        let reg = DeckRegistry::new();
        assert!(matches!(
            reg.with_mut::<(), _>(&[42], |_| ()),
            Err(Error::InvalidHandle)
        ));
    }

    #[test]
    fn concurrent_alloc_free() {
        use std::thread;
        let reg = Arc::new(DeckRegistry::new());
        let threads: Vec<_> = (0u64..16)
            .map(|id| {
                let reg = Arc::clone(&reg);
                thread::spawn(move || {
                    let mut h = OrcHandle {
                        handle: id,
                        ..Default::default()
                    };
                    reg.alloc::<f32>(&mut h).unwrap();
                    // Write a value that encodes this thread's id.
                    reg.with_mut(&[id], |decks| {
                        let deck = decks[0].downcast_mut::<Deck<f32>>().unwrap();
                        deck.push(id as f32, 1);
                    })
                    .unwrap();
                    // Read it back — another thread must not have overwritten it.
                    reg.with_mut(&[id], |decks| {
                        let deck = decks[0].downcast_mut::<Deck<f32>>().unwrap();
                        assert_eq!(deck.items(), &[id as f32]);
                    })
                    .unwrap();
                    reg.free(id).unwrap();
                    disarm(&mut h);
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        // All entries freed — none should be accessible.
        assert!(matches!(
            reg.with_mut::<(), _>(&[0], |_| ()),
            Err(Error::InvalidHandle)
        ));
    }

    // ==================== update_handle_from_deck ====================

    #[test]
    fn fields_populated() {
        let mut deck: Deck<f32> = Deck::default();
        deck.push(1.0, 1);
        deck.push(2.0, 0);
        deck.push(3.0, 0);
        let mut h = OrcHandle {
            handle: 99,
            free_fn: None,
            ..Default::default()
        };
        unsafe { update_handle_from_deck(&deck, &mut h) };
        assert_eq!(h.type_id, f32::TYPE_INFO.type_id);
        assert_eq!(h.n_items, 3);
        assert!(!h.items.is_null());
        assert_eq!(h.handle, 99);
        assert!(h.free_fn.is_none());
    }

    #[test]
    fn empty_deck_gives_null_items() {
        let deck: Deck<u64> = Deck::default();
        let mut h = OrcHandle {
            handle: 55,
            ..Default::default()
        };
        unsafe { update_handle_from_deck(&deck, &mut h) };
        assert!(h.items.is_null());
        assert_eq!(h.n_items, 0);
        assert_eq!(h.type_id, u64::TYPE_INFO.type_id);
    }

    #[test]
    fn preserves_handle_id() {
        let mut deck: Deck<i32> = Deck::default();
        deck.push(42, 1);
        let mut h = OrcHandle {
            handle: 77,
            ..Default::default()
        };
        unsafe { update_handle_from_deck(&deck, &mut h) };
        assert_eq!(h.handle, 77);
    }

    #[test]
    fn preserves_free_fn() {
        let deck: Deck<i32> = Deck::default();
        let mut h = OrcHandle {
            handle: 88,
            free_fn: None,
            ..Default::default()
        };
        unsafe { update_handle_from_deck(&deck, &mut h) };
        assert!(h.free_fn.is_none());
    }

    // ==================== reset_handle ====================

    #[test]
    fn clears_data_fields_not_id() {
        let mut h = OrcHandle {
            handle: 42,
            n_items: 5,
            n_marks: 2,
            type_id: 1,
            free_fn: Some(orc_deck_free),
            ..Default::default()
        };
        reset_handle(&mut h);
        assert_eq!(h.handle, 42);
        assert_eq!(h.n_items, 0);
        assert_eq!(h.n_marks, 0);
        assert_eq!(h.type_id, 0);
        assert!(h.free_fn.is_none());
        assert!(h.items.is_null());
        assert!(h.marks.is_null());
        assert_eq!(h.dims, [0i32; ORC_NUM_DIMS as usize]);
    }

    #[test]
    fn reset_idempotent() {
        let mut h = OrcHandle {
            handle: 11,
            n_items: 3,
            ..Default::default()
        };
        reset_handle(&mut h);
        reset_handle(&mut h);
        assert_eq!(h.handle, 11);
        assert_eq!(h.n_items, 0);
        assert!(h.items.is_null());
    }

    // ==================== ptr helpers ====================

    #[test]
    fn empty_slice_gives_null() {
        let empty: &[u32] = &[];
        assert!(ptr_from_slice(empty).is_null());
    }

    #[test]
    fn nonempty_round_trips() {
        let data = [1u32, 2, 3];
        let ptr = ptr_from_slice(&data);
        assert!(!ptr.is_null());
        let back = unsafe { slice_from_ptr(ptr, 3) };
        assert_eq!(back, &data);
    }

    #[test]
    fn null_ptr_gives_empty_slice() {
        let s: &[u32] = unsafe { slice_from_ptr(std::ptr::null(), 5) };
        assert!(s.is_empty());
    }

    #[test]
    fn zero_len_gives_empty_slice() {
        let data = [42u32];
        let s: &[u32] = unsafe { slice_from_ptr(data.as_ptr(), 0) };
        assert!(s.is_empty());
    }
}
