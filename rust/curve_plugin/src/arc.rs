use orc_sdk::{DeckItemDisplay, Error, HostCallbacks, OrcTypeId, TOrcData, orc_fn};
use std::fmt::Display;

use crate::{host_callbacks, registry};

pub const ARC_2D_TYPE_ID: OrcTypeId = 0xfeaaa707a60f5329;

#[derive(Clone)]
pub struct Arc2d {
    inner: loke::Arc2d,
}

impl Default for Arc2d {
    fn default() -> Self {
        Self {
            inner: loke::Arc2d::unit_quadrant_arc(1),
        }
    }
}

impl TOrcData for Arc2d {
    const TYPE_INFO: orc_sdk::OrcTypeInfo = orc_sdk::OrcTypeInfo {
        type_id: ARC_2D_TYPE_ID,
        name: c"Arc2d".as_ptr(),
        desc: c"Two dimensional circular arc".as_ptr(),
    };
}

impl Display for Arc2d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = self.inner.center();
        write!(
            f,
            "Arc2d {{ center: ({}, {}), radius: {} }}",
            c[0],
            c[1],
            self.inner.radius()
        )
    }
}

impl DeckItemDisplay for Arc2d {
    fn item_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Arc2d {
    pub fn serialize(items: &[Self], mut write: impl std::io::Write) -> std::io::Result<usize> {
        for item in items {
            item.inner.serialize(&mut write)?;
        }
        Ok(items.len())
    }

    pub fn deserialize(mut read: impl std::io::Read, n_items: usize) -> std::io::Result<Vec<Self>> {
        let mut out = Vec::with_capacity(n_items);
        for _ in 0..n_items {
            out.push(Self {
                inner: loke::Arc2d::deserialize(&mut read)?,
            });
        }
        Ok(out)
    }
}

#[orc_fn]
fn arc_2d_from_three_points() {
    let host_callbacks = host_callbacks();
    let registry: &DeckRegistry = registry();

    fn run(
        host: &HostCallbacks,
        start: &[f64; 2],
        mid: &[f64; 2],
        end: &[f64; 2],
        arc: &mut Arc2d,
    ) -> Result<(), Error> {
        match loke::Arc2d::from_three_points(loke::DVec(*start), loke::DVec(*mid), loke::DVec(*end))
        {
            Ok(curve) => {
                arc.inner = curve;
                Ok(())
            }
            Err(e) => {
                host.error(&e.to_string());
                Err(Error::Unknown)
            }
        }
    }
}
