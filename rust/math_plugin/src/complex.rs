use crate::{host_callbacks, registry};
use orc_sdk::{Error, HostCallbacks, TOrcData, orc_fn};

#[derive(Default, Clone)]
struct Complex {
    real: f64,
    imag: f64,
}

impl TOrcData for Complex {
    const TYPE_INFO: orc_sdk::OrcTypeInfo = orc_sdk::OrcTypeInfo {
        type_id: 0xd17d7399a9b11a54,
        name: c"complex number".as_ptr(),
        desc: c"3 + 2i type of stuff.".as_ptr(),
    };
}

orc_fn!(add_complex, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(
        _host: &HostCallbacks,
        lhs: &Complex,
        rhs: &Complex,
        out: &mut Complex,
    ) -> Result<(), Error> {
        *out = Complex {
            real: lhs.real + rhs.real,
            imag: lhs.imag + rhs.imag,
        };
        Ok(())
    }
});

orc_fn!(mul_complex, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(
        _host: &HostCallbacks,
        lhs: &Complex,
        rhs: &Complex,
        out: &mut Complex,
    ) -> Result<(), Error> {
        *out = Complex {
            real: lhs.real * rhs.real - lhs.imag * rhs.imag,
            imag: lhs.real * rhs.imag + rhs.real * lhs.imag,
        };
        Ok(())
    }
});
