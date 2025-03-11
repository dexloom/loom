use crate::protocols::{fetch_uni2_factory, UniswapV2Protocol};
use crate::{pool_loader, UniswapV2Pool};
use alloy::primitives::Bytes;
use alloy::primitives::Log as EVMLog;
use alloy::providers::network::Ethereum;
use alloy::sol_types::SolEventInterface;
use eyre::{eyre, ErrReport};
use futures::Stream;
use loom_defi_abi::uniswap2::IUniswapV2Pair::IUniswapV2PairEvents;
use loom_evm_utils::LoomExecuteEvm;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEVM, LoomDataTypesEthereum};
use loom_types_entities::{get_protocol_by_factory, EntityAddress, PoolClass, PoolLoader, PoolProtocol, PoolWrapper};
use revm::DatabaseRef;
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;

pool_loader!(UniswapV2PoolLoader);

impl<P, N, LDT> PoolLoader<P, N, LDT> for UniswapV2PoolLoader<P, N, LDT>
where
    N: Network,
    P: Provider<N> + Clone + 'static,
    LDT: LoomDataTypesEVM + 'static,
{
    fn get_pool_class_by_log(&self, log_entry: &LDT::Log) -> Option<(EntityAddress, PoolClass)> {
        let log_entry: Option<EVMLog> = EVMLog::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
        match log_entry {
            Some(log_entry) => match IUniswapV2PairEvents::decode_log(&log_entry, false) {
                Ok(event) => match event.data {
                    IUniswapV2PairEvents::Swap(_)
                    | IUniswapV2PairEvents::Mint(_)
                    | IUniswapV2PairEvents::Burn(_)
                    | IUniswapV2PairEvents::Sync(_) => Some((EntityAddress::Address(log_entry.address), PoolClass::UniswapV2)),
                    _ => None,
                },
                Err(_) => None,
            },
            None => None,
        }
    }

    fn fetch_pool_by_id<'a>(&'a self, pool_id: EntityAddress) -> Pin<Box<dyn Future<Output = eyre::Result<PoolWrapper>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(provider) = self.provider.clone() {
                self.fetch_pool_by_id_from_provider(pool_id, provider).await
            } else {
                Err(eyre!("NO_PROVIDER"))
            }
        })
    }

    fn fetch_pool_by_id_from_provider<'a>(
        &'a self,
        pool_id: EntityAddress,
        provider: P,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<PoolWrapper>> + Send + 'a>> {
        Box::pin(async move {
            let pool_address = pool_id.address()?;
            let factory_address = fetch_uni2_factory(provider.clone(), pool_address).await?;
            match get_protocol_by_factory(factory_address) {
                PoolProtocol::NomiswapStable
                | PoolProtocol::Miniswap
                | PoolProtocol::Integral
                | PoolProtocol::Safeswap
                | PoolProtocol::AntFarm => Err(eyre!("POOL_PROTOCOL_NOT_SUPPORTED")),
                _ => Ok(PoolWrapper::new(Arc::new(UniswapV2Pool::fetch_pool_data(provider, pool_id.address()?).await?))),
            }
        })
    }

    fn fetch_pool_by_id_from_evm(&self, pool_id: EntityAddress, evm: &mut dyn LoomExecuteEvm) -> eyre::Result<PoolWrapper> {
        Ok(PoolWrapper::new(Arc::new(UniswapV2Pool::fetch_pool_data_evm(evm, pool_id.address()?)?)))
    }

    fn is_code(&self, code: &Bytes) -> bool {
        UniswapV2Protocol::is_code(code)
    }

    fn protocol_loader(&self) -> eyre::Result<Pin<Box<dyn Stream<Item = (EntityAddress, PoolClass)> + Send>>> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }
}
