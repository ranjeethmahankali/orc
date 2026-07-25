use orc_sdk::{
    OrcHandle, OrcHost, OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId, ProxyType, TOrcPluginAdaptor,
    orc_plugin,
};

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

orc_plugin!(Adaptor);
