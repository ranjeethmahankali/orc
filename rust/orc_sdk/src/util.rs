use crate::{
    Deck, Error, ORC_MSG_LEVEL_DEBUG, ORC_MSG_LEVEL_ERROR, ORC_MSG_LEVEL_FATAL, ORC_MSG_LEVEL_INFO,
    ORC_MSG_LEVEL_WARN, ORC_NUM_DIMS, OrcFuncInfo, OrcHandle, OrcHost, OrcHostCallbackAPI,
    OrcPlugin, OrcTypeId, OrcTypeInfo, deck::fmt_raw_deck, ffi::TOrcData,
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
        atomic::{AtomicPtr, AtomicU64, Ordering},
    },
};

pub fn ptr_from_slice<T>(arr: &[T]) -> *const T {
    if arr.is_empty() {
        std::ptr::null()
    } else {
        arr.as_ptr()
    }
}

pub unsafe fn slice_from_ptr<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

pub unsafe fn slice_from_ptr_mut<'a, T>(ptr: *mut T, len: usize) -> &'a mut [T] {
    if ptr.is_null() || len == 0 {
        &mut []
    } else {
        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }
}

pub fn handle_from_deck<T: TOrcData>(deck: &Deck<T>, id: u64) -> OrcHandle {
    let (items, marks, (stride_offset, strides)) = (deck.items(), deck.marks(), deck.stride_info());
    debug_assert_eq!(
        marks.len(),
        stride_offset.len(),
        "Malformed deck data structure"
    );
    OrcHandle {
        handle: id,
        items: ptr_from_slice(items).cast(),
        n_items: items.len() as u64,
        item_size: size_of::<T>() as u64,
        marks: ptr_from_slice(marks),
        stride_offset: ptr_from_slice(stride_offset),
        n_marks: marks.len() as u64,
        strides: ptr_from_slice(strides),
        type_id: T::TYPE_INFO.type_id,
        dims: [0; ORC_NUM_DIMS as usize],
    }
}

pub fn reset_handle(handle: &mut OrcHandle) {
    *handle = OrcHandle::default();
}

pub struct ObjectRegistry {
    handles: RwLock<HashMap<u64, Arc<RwLock<Box<dyn Any + Send + Sync>>>>>,
    counter: AtomicU64,
}

impl ObjectRegistry {
    pub fn new() -> Self {
        ObjectRegistry {
            handles: RwLock::new(HashMap::new()),
            // Ids start at 1. 0 represents an empty / uninitialized id.
            counter: AtomicU64::new(1),
        }
    }

    pub fn alloc<T: Any + Send + Sync>(&self, obj: T) -> Result<u64, Error> {
        // This can block this thread until write access is available.
        let mut handles = self
            .handles
            .write()
            .map_err(|_e| Error::ConcurrencyProblem)?;
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        handles.insert(id, Arc::new(RwLock::new(Box::new(obj))));
        Ok(id)
    }

    pub fn free(&self, id: u64) -> Result<(), Error> {
        // This can block this thread until write access is available.
        let mut handles = self
            .handles
            .write()
            .map_err(|_e| Error::ConcurrencyProblem)?;
        handles.remove(&id);
        Ok(())
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

    pub fn ensure_alloc_default<T: Default + Any + Send + Sync>(
        &self,
        id: &mut u64,
    ) -> Result<(), Error> {
        if *id == 0 {
            // Valid ids start at 1. This is an empty / invalid id. We need to allocate a new deck and overwrite the id.
            *id = self.alloc(T::default())?;
            return Ok(());
        }
        // Here, the id is valid, so we try to find the previous allocation and reuse it. If it
        // doesn't match the type, or doesn't exist, we just reallocate. The line below can block
        // this thread until write access is available.
        let mut handles = self
            .handles
            .write()
            .map_err(|_e| Error::ConcurrencyProblem)?;
        match handles.entry(*id) {
            Entry::Occupied(mut occupied) => {
                let realloc_needed = {
                    let read_lock = occupied
                        .get()
                        .try_read()
                        .map_err(|_e| Error::ConcurrencyProblem)?;
                    read_lock.downcast_ref::<T>().is_none()
                };
                if realloc_needed {
                    // Drop the old object and overwrite it with a new one.
                    occupied.insert(Arc::new(RwLock::new(Box::new(T::default()))));
                }
            }
            Entry::Vacant(vacant) => {
                vacant.insert(Arc::new(RwLock::new(Box::new(T::default()))));
            }
        };
        Ok(())
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

pub struct TypeInfo {
    pub type_id: OrcTypeId,
    pub name: String,
    pub desc: String,
}

impl From<&OrcTypeInfo> for TypeInfo {
    fn from(info: &OrcTypeInfo) -> Self {
        Self {
            type_id: info.type_id,
            name: unsafe { CStr::from_ptr(info.name) }
                .to_string_lossy()
                .into_owned(),
            desc: unsafe { CStr::from_ptr(info.desc) }
                .to_string_lossy()
                .into_owned(),
        }
    }
}

pub struct FuncInfo {
    pub name: String,
    pub desc: String,
    pub func: unsafe extern "C" fn(
        ctx: u64,
        inputs: *const OrcHandle,
        n_inputs: u64,
        outputs: *mut OrcHandle,
        n_outputs: u64,
    ),
}

impl From<&OrcFuncInfo> for FuncInfo {
    fn from(info: &OrcFuncInfo) -> Self {
        Self {
            name: unsafe { CStr::from_ptr(info.name) }
                .to_string_lossy()
                .into_owned(),
            desc: unsafe { CStr::from_ptr(info.desc) }
                .to_string_lossy()
                .into_owned(),
            func: info.func.expect("NULL function pointer"),
        }
    }
}

pub struct PluginInfo {
    pub types: Box<[TypeInfo]>,
    pub functions: Box<[FuncInfo]>,
}

impl From<&OrcPlugin> for PluginInfo {
    fn from(plugin: &OrcPlugin) -> Self {
        let types = if plugin.n_types == 0 || plugin.types.is_null() {
            Box::default()
        } else {
            unsafe { std::slice::from_raw_parts(plugin.types, plugin.n_types as usize) }
                .iter()
                .map(TypeInfo::from)
                .collect()
        };
        let functions = if plugin.n_functions == 0 || plugin.functions.is_null() {
            Box::default()
        } else {
            unsafe { std::slice::from_raw_parts(plugin.functions, plugin.n_functions as usize) }
                .iter()
                .map(FuncInfo::from)
                .collect()
        };
        Self { types, functions }
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
/// if they're not met, and return immediately.
#[macro_export]
macro_rules! orc_check_return {
    ($host:expr, $cond:expr, $($fmt:tt)+) => {{
        if !($cond) {
            let message = ::std::format!($($fmt)+);
            $host.error(&message);
            return;
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
