use crate::{
    Deck, Error, ORC_NUM_DIMS, OrcFuncInfo, OrcHandle, OrcHost, OrcPlugin, OrcTypeId, OrcTypeInfo,
    ffi::TOrcData,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    any::Any,
    collections::HashMap,
    ffi::{CStr, c_void},
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
    let type_info = T::type_info();
    let (items, marks, (stride_offset, strides)) = (deck.items(), deck.marks(), deck.stride_info());
    debug_assert_eq!(
        marks.len(),
        stride_offset.len(),
        "Malformed deck datastructure"
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
        type_id: type_info.type_id,
        dims: [0; ORC_NUM_DIMS as usize],
    }
}

pub fn reset_handle(handle: &mut OrcHandle) {
    handle.handle = 0;
    handle.items = std::ptr::null();
    handle.n_items = 0;
    handle.item_size = 0;
    handle.marks = std::ptr::null();
    handle.stride_offset = std::ptr::null();
    handle.n_marks = 0;
    handle.strides = std::ptr::null();
    handle.type_id.primitive_id = 0;
    handle.type_id.opaque_id = 0;
    handle.dims.fill(0);
}

pub struct ObjectRegistry {
    handles: RwLock<HashMap<u64, Arc<RwLock<Box<dyn Any + Send + Sync>>>>>,
    counter: AtomicU64,
}

impl ObjectRegistry {
    pub fn new() -> Self {
        ObjectRegistry {
            handles: RwLock::new(HashMap::new()),
            counter: AtomicU64::new(0),
        }
    }

    pub fn alloc<T: Any + Send + Sync>(&self, obj: T) -> Result<u64, Error> {
        // This can block this thread until write access is available.
        let mut handles = self
            .handles
            .write()
            .map_err(|_e| Error::CannotLockRegistry)?;
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        handles.insert(id, Arc::new(RwLock::new(Box::new(obj))));
        Ok(id)
    }

    pub fn free(&self, id: u64) -> Result<(), Error> {
        // This can block this thread until write access is available.
        let mut handles = self
            .handles
            .write()
            .map_err(|_e| Error::CannotLockRegistry)?;
        handles.remove(&id);
        Ok(())
    }

    pub fn with_mut<T, F>(&self, ids: &[u64], callback: F) -> Result<T, Error>
    where
        F: FnOnce(&mut [&mut (dyn Any + Send + Sync)]) -> T,
    {
        let arcs: Vec<_> = {
            // This can block this thread until write access is available.
            let map = self
                .handles
                .read()
                .map_err(|_e| Error::CannotLockRegistry)?;
            ids.iter()
                .map(|id| map.get(id).cloned().ok_or(Error::InvalidHandle))
                .collect::<Result<_, _>>()?
        };
        let mut guards: Vec<_> = arcs
            .iter()
            .map(|arc| arc.try_write().map_err(|_e| Error::DeckBorrowError))
            .collect::<Result<_, _>>()?;
        let mut references: Vec<&mut (dyn Any + Send + Sync)> = guards
            .iter_mut()
            .map(|guard| guard.as_mut() as &mut (dyn Any + Send + Sync))
            .collect();
        Ok(callback(&mut references))
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
// ========= Rust native types for C types  =========
// ==================================================
//
// It is nicer to have Rust native types for these things. Using the C types directly may require
// some assumptions about the lifetimes of the raw pointers. And the users of the SDK are still free
// to use the raw C types if that fits their needs better. But if they prefer Rust types beyond the
// FFI boundary, types below can help. For example, a host program written in Rust can keep track of
// various plugins, their types and functions etc. using below Rust types.

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
