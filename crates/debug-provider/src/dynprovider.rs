use std::ops::Deref;
use std::sync::Arc;

use alloy::{
    network::Ethereum,
    providers::{Provider, ProviderBuilder, RootProvider},
    transports::{BoxTransport, http::Http, Transport},
};
use reqwest::Client;

#[derive(Clone)]
struct DynPrv<T> {
    pub provider: Arc<Box<dyn Provider<T>>>,
}

impl<T> Deref for DynPrv<T>
where
    T: Transport + Clone,
{
    type Target = Arc<Box<dyn Provider<T> + 'static>>;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}


struct DynProvider {
    inner: DynPrv<BoxTransport>,
}

impl From<DynPrv<Http<Client>>> for DynProvider {
    fn from(value: DynPrv<Http<Client>>) -> Self {
        let provider = DynPrv { provider: Arc::new(Box::new(ProviderBuilder::new().on_provider(value.root().clone().boxed()))) };
        Self { inner: provider }
    }
}

impl Provider for DynProvider {
    fn root(&self) -> &RootProvider<BoxTransport, Ethereum> {
        self.inner.root()
    }
}


#[cfg(test)]
mod test {
    use std::sync::Arc;

    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_provider::ext::AnvilApi;
    use eyre::Result;

    use super::*;

    #[tokio::test]
    async fn test_dyn_provider() -> Result<()> {
        let provider = ProviderBuilder::new().with_recommended_fillers().on_anvil();

        let dyn_prv = Arc::new(Box::new(provider) as Box<dyn Provider<_>>);

        let dyn_prv = DynPrv {
            provider: dyn_prv.clone()
        };

        let dyn_prv = DynProvider::from(dyn_prv);

        dyn_prv.anvil_drop_all_transactions().await;

        Ok(())
    }
}
