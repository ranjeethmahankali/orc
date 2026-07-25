use crate::{
    Deck, Error, OrcHandle,
    ffi::{TOrcData, handle_from_deck, reset_handle},
};
use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
};

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
