use crate::protocols::{fetch_uni3_factory, UniswapV3Protocol};
use crate::{pool_loader, MaverickPool, PancakeV3Pool, UniswapV3Pool};
use alloy::primitives::Bytes;
use alloy::primitives::Log as EVMLog;
use alloy::providers::network::Ethereum;
use alloy::sol_types::SolEventInterface;
use eyre::{eyre, ErrReport};
use futures::Stream;
use loom_defi_abi::uniswap3::IUniswapV3Pool::IUniswapV3PoolEvents;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{get_protocol_by_factory, PoolClass, PoolId, PoolLoader, PoolProtocol, PoolWrapper};
use revm::primitives::Env;
use revm::DatabaseRef;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::error;

pool_loader!(UniswapV3PoolLoader);

impl<P> PoolLoader<P, Ethereum, LoomDataTypesEthereum> for UniswapV3PoolLoader<P, Ethereum, LoomDataTypesEthereum>
where
    P: Provider<Ethereum> + Clone + 'static,
{
    fn get_pool_class_by_log(
        &self,
        log_entry: &<LoomDataTypesEthereum as LoomDataTypes>::Log,
    ) -> Option<(PoolId<LoomDataTypesEthereum>, PoolClass)> {
        let log_entry: Option<EVMLog> = EVMLog::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
        match log_entry {
            Some(log_entry) => match IUniswapV3PoolEvents::decode_log(&log_entry, false) {
                Ok(event) => match event.data {
                    IUniswapV3PoolEvents::Swap(_)
                    | IUniswapV3PoolEvents::Mint(_)
                    | IUniswapV3PoolEvents::Burn(_)
                    | IUniswapV3PoolEvents::Initialize(_) => Some((PoolId::Address(log_entry.address), PoolClass::UniswapV3)),
                    _ => None,
                },
                Err(_) => None,
            },
            None => None,
        }
    }

    fn fetch_pool_by_id<'a>(
        &'a self,
        pool_id: PoolId<LoomDataTypesEthereum>,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<PoolWrapper<LoomDataTypesEthereum>>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(provider) = &self.provider {
                self.fetch_pool_by_id_from_provider(pool_id, provider.clone()).await
            } else {
                Err(eyre!("NO_PROVIDER"))
            }
        })
    }

    fn fetch_pool_by_id_from_provider<'a>(
        &'a self,
        pool_id: PoolId<LoomDataTypesEthereum>,
        provider: P,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<PoolWrapper<LoomDataTypesEthereum>>> + Send + 'a>> {
        Box::pin(async move {
            let pool_address = pool_id.address()?;

            let factory_address_result = fetch_uni3_factory(provider.clone(), pool_address).await;
            match factory_address_result {
                Ok(factory_address) => match get_protocol_by_factory(factory_address) {
                    PoolProtocol::PancakeV3 => {
                        Ok(PoolWrapper::new(Arc::new(PancakeV3Pool::fetch_pool_data(provider.clone(), pool_address).await?)))
                    }
                    PoolProtocol::Maverick => {
                        Ok(PoolWrapper::new(Arc::new(MaverickPool::fetch_pool_data(provider.clone(), pool_address).await?)))
                    }
                    _ => Ok(PoolWrapper::new(Arc::new(UniswapV3Pool::fetch_pool_data(provider.clone(), pool_address).await?))),
                },
                Err(e) => {
                    error!("Error fetching factory address at {:#20x}: {}", pool_address, e);
                    Err(eyre!("CANNOT_GET_FACTORY_ADDRESS"))
                }
            }
        })
    }

    fn fetch_pool_by_id_from_evm(
        &self,
        pool_id: PoolId<LoomDataTypesEthereum>,
        db: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
    ) -> eyre::Result<PoolWrapper<LoomDataTypesEthereum>> {
        Ok(PoolWrapper::new(Arc::new(UniswapV3Pool::fetch_pool_data_evm(db, env, pool_id.address()?)?)))
    }

    fn is_code(&self, code: &Bytes) -> bool {
        UniswapV3Protocol::is_code(code)
    }

    fn protocol_loader(&self) -> eyre::Result<Pin<Box<dyn Stream<Item = (PoolId, PoolClass)> + Send>>> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }
}
