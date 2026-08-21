/*! # Resources with context handles

Many parts of the ABI use a u64 ctx to recognize the caller. The host will often need to allocate
some resources for a particular ctx key, and pass that ctx to the plugin. The host might then need
to reacquire the same resource based on the ctx, inside the callback. This file provides useful
things for implementing this pattern.
 */

use std::sync::{Mutex, RwLock};

use crate::Error;

#[derive(Default)]
pub struct Arena<T: Default> {
    slots: RwLock<Vec<Mutex<T>>>,
    free: Mutex<Vec<u64>>,
}

impl<T: Default> Arena<T> {
    pub fn insert(&mut self, init: impl Fn(&mut T)) -> Result<u64, Error> {
        let mut free = self.free.lock().map_err(|_| Error::ConcurrencyProblem)?;
        match free.pop() {
            Some(last) => {
                self.visit_mut(last, init)?;
                Ok(last)
            }
            None => {
                let mut slots = self
                    .slots
                    .try_write()
                    .map_err(|_| Error::ConcurrencyProblem)?;
                let idx = slots.len();
                let mut value = T::default();
                init(&mut value);
                slots.push(Mutex::new(value));
                Ok(idx as u64)
            }
        }
    }

    pub fn visit_mut<R>(&self, ctx: u64, vis: impl Fn(&mut T) -> R) -> Result<R, Error> {
        let slots = self
            .slots
            .try_read()
            .map_err(|_| Error::ConcurrencyProblem)?;
        let slot_mx = slots.get(ctx as usize).ok_or(Error::InvalidContext)?;
        let mut slot = slot_mx.lock().map_err(|_| Error::ConcurrencyProblem)?;
        Ok(vis(&mut slot))
    }

    pub fn consume<R>(&mut self, ctx: u64, vis: impl Fn(&mut T) -> R) -> Result<R, Error> {
        let mut free = self.free.lock().map_err(|_| Error::ConcurrencyProblem)?;
        free.push(ctx);
        self.visit_mut(ctx, vis)
    }
}
