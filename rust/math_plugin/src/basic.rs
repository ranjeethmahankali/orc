use crate::{host_callbacks, registry};
use orc_sdk::{Error, HostCallbacks, OrcDims, TOrcData, orc_fn};
use std::ops::{Add, Div, Mul, Sub};

orc_fn!(add, {
    let host_callbacks: &HostCallbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (
        Case<f32>,
        Case<f64>,
        Case<u8>,
        Case<u16>,
        Case<u32>,
        Case<u64>,
        Case<i8>,
        Case<i16>,
        Case<i32>,
        Case<i64>,
    );

    /// Adds two inputs values, assigns result to the output. This function supports any integer or
    /// floating point scalar types. The two inputs must be of the same type. The output produced
    /// will be of the same type also.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Add<Output = T> + Copy,
    {
        *out = *lhs + *rhs;
        Ok(())
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
});

orc_fn!(mul, {
    let host_callbacks: &HostCallbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (
        Case<f32>,
        Case<f64>,
        Case<u8>,
        Case<u16>,
        Case<u32>,
        Case<u64>,
        Case<i8>,
        Case<i16>,
        Case<i32>,
        Case<i64>,
    );

    /// Multiplies two inputs values, and assigns the result to the output. This function supports
    /// any integer or floating point scalar types. The two inputs must be of the same type. The
    /// output produced will be of the same type also.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Mul<Output = T> + Copy,
    {
        *out = *lhs * *rhs;
        Ok(())
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
});

orc_fn!(sub, {
    let host_callbacks: &HostCallbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (Case<f32>, Case<f64>);

    /// Subtracts the second operand from the first, and assigns to the output. The input types must
    /// be the same, matching the output type. This function supports floating point scalar types.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Sub<Output = T> + Copy,
    {
        *out = *lhs - *rhs;
        Ok(())
    }
});

orc_fn!(div, {
    let host_callbacks: &HostCallbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (Case<f32>, Case<f64>);

    /// Divides the first input with the second input, and assign to the output. All inputs must be
    /// of the same type, matching the output type. This function supports floating point scalar
    /// types.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Div<Output = T> + Copy,
    {
        *out = *lhs / *rhs;
        Ok(())
    }
});
