use crate::protocols::CurveProtocol;
use crate::{pool_loader, CurvePool};
use alloy::primitives::Bytes;
use alloy::providers::network::Ethereum;
use async_stream::stream;
use eyre::{eyre, ErrReport};
use futures::Stream;
use loom_evm_utils::LoomExecuteEvm;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEVM, LoomDataTypesEthereum};
use loom_types_entities::{EntityAddress, PoolClass, PoolLoader, PoolWrapper};
use revm::DatabaseRef;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::error;

pool_loader!(CurvePoolLoader);

impl<P, N, LDT> PoolLoader<P, N, LDT> for CurvePoolLoader<P, N, LDT>
where
    N: Network,
    P: Provider<N> + Clone + 'static,
    LDT: LoomDataTypesEVM + 'static,
{
    fn get_pool_class_by_log(&self, _log_entry: &LDT::Log) -> Option<(EntityAddress, PoolClass)> {
        None
    }

    fn fetch_pool_by_id<'a>(&'a self, pool_id: EntityAddress) -> Pin<Box<dyn Future<Output = eyre::Result<PoolWrapper>> + Send + 'a>> {
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
        pool_id: EntityAddress,
        provider: P,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<PoolWrapper>> + Send + 'a>> {
        Box::pin(async move {
            let pool_address = pool_id.address()?;
            match CurveProtocol::get_contract_from_code(provider.clone(), pool_address).await {
                Ok(curve_contract) => {
                    let curve_pool = CurvePool::<P, N>::fetch_pool_data_with_default_encoder(provider.clone(), curve_contract).await?;

                    Ok(PoolWrapper::new(Arc::new(curve_pool)))
                }
                Err(e) => {
                    error!("Error getting curve contract from code {} : {} ", pool_address, e);
                    Err(e)
                }
            }
        })
    }

    fn fetch_pool_by_id_from_evm(&self, _pool_id: EntityAddress, evm: &mut dyn LoomExecuteEvm) -> eyre::Result<PoolWrapper> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    fn is_code(&self, _code: &Bytes) -> bool {
        false
    }

    fn protocol_loader(&self) -> eyre::Result<Pin<Box<dyn Stream<Item = (EntityAddress, PoolClass)> + Send>>> {
        let provider_clone = self.provider.clone();

        if let Some(client) = provider_clone {
            Ok(Box::pin(stream! {
                let curve_contracts = CurveProtocol::get_contracts_vec(client.clone());
                for curve_contract in curve_contracts.iter() {
                    yield (EntityAddress::Address(curve_contract.get_address()), PoolClass::Curve)
                }

                for factory_idx in 0..10 {
                    if let Ok(factory_address) = CurveProtocol::get_factory_address(client.clone(), factory_idx).await {
                        if let Ok(pool_count) = CurveProtocol::get_pool_count(client.clone(), factory_address).await {
                            for pool_id in 0..pool_count {
                                if let Ok(addr) = CurveProtocol::get_pool_address(client.clone(), factory_address, pool_id).await {
                                    yield (EntityAddress::Address(addr), PoolClass::Curve)
                                }
                            }
                        }
                    }
                }
            }))
        } else {
            Err(eyre!("NO_PROVIDER"))
        }
    }
}
