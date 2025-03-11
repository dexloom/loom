use crate::{pool_loader, MaverickPool};
use alloy::primitives::Bytes;
use alloy::primitives::Log as EVMLog;
use alloy::providers::network::Ethereum;
use alloy::sol_types::SolEventInterface;
use eyre::{eyre, ErrReport, Result};
use loom_defi_abi::maverick::IMaverickPool::IMaverickPoolEvents;
use loom_evm_utils::LoomExecuteEvm;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEVM, LoomDataTypesEthereum};
use loom_types_entities::{EntityAddress, PoolClass, PoolLoader, PoolWrapper};
use revm::DatabaseRef;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;

pool_loader!(MaverickPoolLoader);

impl<P, N, LDT> PoolLoader<P, N, LDT> for MaverickPoolLoader<P, N, LDT>
where
    N: Network,
    P: Provider<N> + Clone + 'static,
    LDT: LoomDataTypesEVM + 'static,
{
    fn get_pool_class_by_log(&self, log_entry: &LDT::Log) -> Option<(EntityAddress, PoolClass)> {
        let log_entry: Option<EVMLog> = EVMLog::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
        match log_entry {
            Some(log_entry) => match IMaverickPoolEvents::decode_log(&log_entry, false) {
                Ok(event) => match event.data {
                    IMaverickPoolEvents::Swap(_) | IMaverickPoolEvents::AddLiquidity(_) | IMaverickPoolEvents::RemoveLiquidity(_) => {
                        Some((EntityAddress::Address(log_entry.address), PoolClass::Maverick))
                    }
                    _ => None,
                },
                Err(_) => None,
            },
            None => None,
        }
    }

    fn fetch_pool_by_id<'a>(&'a self, pool_id: EntityAddress) -> Pin<Box<dyn Future<Output = Result<PoolWrapper>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(provider) = self.provider.clone() {
                self.fetch_pool_by_id_from_provider(pool_id, provider).await
            } else {
                Err(eyre!("NO_PROVIDER"))
            }
        })
    }

    fn fetch_pool_by_id_from_provider(
        &self,
        pool_id: EntityAddress,
        provider: P,
    ) -> Pin<Box<dyn Future<Output = Result<PoolWrapper>> + Send>> {
        Box::pin(async move { Ok(PoolWrapper::new(Arc::new(MaverickPool::fetch_pool_data(provider.clone(), pool_id.address()?).await?))) })
    }

    fn fetch_pool_by_id_from_evm(&self, pool_id: EntityAddress, evm: &mut dyn LoomExecuteEvm) -> Result<PoolWrapper> {
        Ok(PoolWrapper::new(Arc::new(MaverickPool::fetch_pool_data_evm(evm, pool_id.address()?)?)))
    }

    fn is_code(&self, _code: &Bytes) -> bool {
        false
    }

    fn protocol_loader(&self) -> Result<Pin<Box<dyn Stream<Item = (EntityAddress, PoolClass)> + Send>>> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }
}
