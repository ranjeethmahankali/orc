use std::ffi::c_void;

pub enum Arity {
    View(u8),
    Deck,
}

pub trait TFunc<const NIN: usize, const NOUT: usize> {
    fn input_arities() -> [Arity; NIN];
    fn output_arities() -> [Arity; NOUT];
}

#[unsafe(no_mangle)]
pub extern "C" fn test_c_binding(a: *const c_void, b: *const c_void, c: *mut c_void) {}

pub fn test_impl(a: &f64, b: &f64, out: &mut f64) {
    *out = a + b;
}
