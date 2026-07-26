use crate::{Deck, Error, ORC_NUM_DIMS, OrcHandle, ffi::TOrcData};
use std::{
    any::Any,
    collections::HashMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

#[inline]
fn ptr_from_slice<T>(arr: &[T]) -> *const T {
    if arr.is_empty() {
        std::ptr::null()
    } else {
        arr.as_ptr()
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
        F: FnOnce(&[&mut (dyn Any + Send + Sync)]) -> T,
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
        let references: Vec<&mut (dyn Any + Send + Sync)> = guards
            .iter_mut()
            .map(|guard| guard.as_mut() as &mut (dyn Any + Send + Sync))
            .collect();
        Ok(callback(&references))
    }
}
