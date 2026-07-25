use orc_sdk::{OrcHandle, OrcHost, OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId, TOrcPluginAdaptor};

struct Adaptor; // Unit struct to implement the adaptor.

impl TOrcPluginAdaptor for Adaptor {
    fn plugin_init(host: &OrcHost, out: &mut OrcPlugin) {
        todo!()
    }

    fn deck_alloc(id: OrcTypeId) -> OrcHandle {
        todo!()
    }

    fn deck_free(handle: &mut OrcHandle) {
        todo!()
    }

    fn deck_from_proxy(
        inputs: &[OrcHandle],
        proxy_type: orc_sdk::ProxyType,
        proxies: &[OrcItemProxy],
        marks: &[OrcMark],
        out: &mut OrcHandle,
    ) {
        todo!()
    }
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
