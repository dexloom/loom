mod curve;
mod maverick;
mod uniswap2;
mod uniswap3;

use crate::loaders::curve::CurvePoolLoader;
use alloy::providers::network::Ethereum;
use alloy::providers::{Network, Provider};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::pool_config::PoolsConfig;
use loom_types_entities::{PoolClass, PoolLoader, PoolLoaders};
pub use maverick::MaverickPoolLoader;
use std::sync::Arc;
pub use uniswap2::UniswapV2PoolLoader;
pub use uniswap3::UniswapV3PoolLoader;

/// creates  pool loader and imports necessary crates
#[macro_export]
macro_rules! pool_loader {
    // This will match the input like MaverickPoolLoader
    ($name:ident) => {
        use alloy::providers::{Network, Provider};
        use std::marker::PhantomData;

        #[derive(Clone)]

        pub struct $name<P, N, LDT = LoomDataTypesEthereum>
        where
            N: Network,
            P: Provider<N> + Clone,
            LDT: LoomDataTypes,
        {
            provider: Option<P>,
            phantom_data: PhantomData<(P, N, LDT)>,
        }

        #[allow(dead_code)]
        impl<P, N, LDT> $name<P, N, LDT>
        where
            N: Network,
            P: Provider<N> + Clone,
            LDT: LoomDataTypes,
        {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn with_provider(provder: P) -> Self {
                Self { provider: Some(provder), phantom_data: PhantomData }
            }
        }

        impl<P, N, LDT> Default for $name<P, N, LDT>
        where
            N: Network,
            P: Provider<N> + Clone,
            LDT: LoomDataTypes,
        {
            fn default() -> Self {
                Self { provider: None, phantom_data: PhantomData }
            }
        }
    };
}

pub struct PoolLoadersBuilder<P, N, LDT = LoomDataTypesEthereum>
where
    N: Network,
    P: Provider<N> + 'static,
    LDT: LoomDataTypes,
{
    inner: PoolLoaders<P, N, LDT>,
}

impl<P, N, LDT> PoolLoadersBuilder<P, N, LDT>
where
    N: Network,
    P: Provider<N> + 'static,
    LDT: LoomDataTypes,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(self, provider: P) -> Self {
        Self { inner: self.inner.with_provider(provider) }
    }

    pub fn with_config(self, config: PoolsConfig) -> Self {
        Self { inner: self.inner.with_config(config) }
    }

    pub fn add_loader(self, pool_class: PoolClass, pool_loader: Arc<dyn PoolLoader<P, N, LDT>>) -> Self {
        Self { inner: self.inner.add_loader(pool_class, pool_loader) }
    }

    pub fn build(self) -> PoolLoaders<P, N, LDT> {
        self.inner
    }
}

impl<P, N, LDT> Default for PoolLoadersBuilder<P, N, LDT>
where
    N: Network,
    P: Provider<N> + 'static,
    LDT: LoomDataTypes,
{
    fn default() -> Self {
        Self { inner: PoolLoaders::new() }
    }
}

impl<P> PoolLoadersBuilder<P, Ethereum, LoomDataTypesEthereum>
where
    P: Provider<Ethereum> + 'static,
{
    pub fn default_pool_loaders(provider: P, config: PoolsConfig) -> PoolLoaders<P, Ethereum, LoomDataTypesEthereum>
    where
        P: Provider<Ethereum> + Clone,
    {
        PoolLoadersBuilder::new()
            .with_provider(provider.clone())
            .with_config(config)
            .add_loader(PoolClass::Maverick, Arc::new(MaverickPoolLoader::new()))
            .add_loader(PoolClass::UniswapV2, Arc::new(UniswapV2PoolLoader::new()))
            .add_loader(PoolClass::UniswapV3, Arc::new(UniswapV3PoolLoader::new()))
            .add_loader(PoolClass::Curve, Arc::new(CurvePoolLoader::new()))
            .build()
    }
}
