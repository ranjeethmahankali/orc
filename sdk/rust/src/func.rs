use std::ffi::c_void;

pub enum Arity {
    View(u8),
    Deck,
}

pub trait TFunc<const NIn: usize, const NOut: usize> {
    fn input_arities() -> [Arity; NIn];
    fn output_arities() -> [Arity; NOut];
}

#[unsafe(no_mangle)]
pub extern "C" fn test_c_binding(a: *const c_void, b: *const c_void, c: *mut c_void) {}

pub fn test_impl(a: &f64, b: &f64, out: &mut f64) {
    *out = a + b;
}
