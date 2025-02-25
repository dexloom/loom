use crate::{pool_loader, MaverickPool};
use alloy::primitives::Bytes;
use alloy::primitives::Log as EVMLog;
use alloy::providers::network::Ethereum;
use alloy::sol_types::SolEventInterface;
use eyre::{eyre, ErrReport, Result};
use loom_defi_abi::maverick::IMaverickPool::IMaverickPoolEvents;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{PoolClass, PoolId, PoolLoader, PoolWrapper};
use revm::primitives::Env;
use revm::DatabaseRef;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;

pool_loader!(MaverickPoolLoader);

impl<P> PoolLoader<P, Ethereum, LoomDataTypesEthereum> for MaverickPoolLoader<P, Ethereum, LoomDataTypesEthereum>
where
    P: Provider<Ethereum> + Clone + 'static,
{
    fn get_pool_class_by_log(
        &self,
        log_entry: &<LoomDataTypesEthereum as LoomDataTypes>::Log,
    ) -> Option<(PoolId<LoomDataTypesEthereum>, PoolClass)> {
        let log_entry: Option<EVMLog> = EVMLog::new(log_entry.address(), log_entry.topics().to_vec(), log_entry.data().data.clone());
        match log_entry {
            Some(log_entry) => match IMaverickPoolEvents::decode_log(&log_entry, false) {
                Ok(event) => match event.data {
                    IMaverickPoolEvents::Swap(_) | IMaverickPoolEvents::AddLiquidity(_) | IMaverickPoolEvents::RemoveLiquidity(_) => {
                        Some((PoolId::Address(log_entry.address), PoolClass::Maverick))
                    }
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
    ) -> Pin<Box<dyn Future<Output = Result<PoolWrapper<LoomDataTypesEthereum>>> + Send + 'a>> {
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
        pool_id: PoolId<LoomDataTypesEthereum>,
        provider: P,
    ) -> Pin<Box<dyn Future<Output = Result<PoolWrapper<LoomDataTypesEthereum>>> + Send>> {
        Box::pin(async move { Ok(PoolWrapper::new(Arc::new(MaverickPool::fetch_pool_data(provider.clone(), pool_id.address()?).await?))) })
    }

    fn fetch_pool_by_id_from_evm(
        &self,
        pool_id: PoolId<LoomDataTypesEthereum>,
        db: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
    ) -> Result<PoolWrapper<LoomDataTypesEthereum>> {
        Ok(PoolWrapper::new(Arc::new(MaverickPool::fetch_pool_data_evm(db, env, pool_id.address()?)?)))
    }

    fn is_code(&self, _code: &Bytes) -> bool {
        false
    }

    fn protocol_loader(&self) -> Result<Pin<Box<dyn Stream<Item = (PoolId, PoolClass)> + Send>>> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }
}
