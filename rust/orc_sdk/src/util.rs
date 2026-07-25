use crate::{Deck, Error, ORC_NUM_DIMS, OrcHandle, ffi::TOrcData};
use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
};

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

pub struct DeckRegistry {
    handles: HashMap<usize, RefCell<Box<dyn Any>>>,
}

impl DeckRegistry {
    pub fn alloc<T: TOrcData>(&mut self) -> OrcHandle {
        let id = self.handles.len();
        let deck: Box<Deck<T>> = Box::new(Deck::default());
        let handle: OrcHandle = handle_from_deck(deck.as_ref(), id as u64);
        self.handles.insert(id, RefCell::new(deck));
        handle
    }

    pub fn free(&mut self, handle: &mut OrcHandle) {
        let id = handle.handle as usize;
        self.handles.remove(&id);
        reset_handle(handle);
    }

    pub fn get<T: TOrcData>(&self, handle: &OrcHandle) -> Result<Ref<'_, Deck<T>>, Error> {
        let id = handle.handle as usize;
        let cell = self.handles.get(&id).ok_or(Error::InvalidHandle)?;
        let borrow = cell.try_borrow().map_err(|_| Error::DeckBorrowError)?;
        if borrow.downcast_ref::<Deck<T>>().is_none() {
            return Err(Error::DeckTypeMismatch);
        }
        Ok(Ref::map(borrow, |b| b.downcast_ref::<Deck<T>>().unwrap()))
    }

    pub fn get_mut<T: TOrcData>(&self, handle: &OrcHandle) -> Result<RefMut<'_, Deck<T>>, Error> {
        let id = handle.handle as usize;
        let cell = self.handles.get(&id).ok_or(Error::InvalidHandle)?;
        let borrow = cell.try_borrow_mut().map_err(|_| Error::DeckBorrowError)?;
        if borrow.downcast_ref::<Deck<T>>().is_none() {
            return Err(Error::DeckTypeMismatch);
        }
        Ok(RefMut::map(borrow, |b| {
            b.downcast_mut::<Deck<T>>().unwrap()
        }))
    }
}
