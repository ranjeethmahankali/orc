use orc_sdk::{Error, OrcTypeId, TOrcData, orc_fn};
use std::fmt::Display;

use crate::{host_callbacks, registry};

pub const ARC_TYPE_ID: OrcTypeId = 0xfeaaa707a60f5329;

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
        type_id: ARC_TYPE_ID,
        name: c"Arc2d".as_ptr(),
        desc: c"Two dimensional circular arc".as_ptr(),
    };
}

impl Display for Arc2d {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = self.inner.center();
        // write!(f, "Arc2d {{ center: {{{}, {}, {}}}, radius: {} }}", c)
        todo!()
    }
}

impl Arc2d {
    pub fn serialize(items: &[Self], write: &mut impl std::io::Write) -> std::io::Result<usize> {
        todo!()
    }

    pub fn deserialize(
        read: &mut impl std::io::Read,
        n_items: usize,
    ) -> std::io::Result<Vec<Self>> {
        todo!()
    }
}

#[orc_fn]
fn arc_2d_from_three_points() {
    let host_callbacks = host_callbacks();
    let registry: &DeckRegistry = registry();

    fn run(start: &f64, mid: &f64, end: &f64, arc: &mut Arc2d) -> Result<(), Error> {
        todo!();
    }
}
