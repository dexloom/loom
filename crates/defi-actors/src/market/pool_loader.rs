use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;

use alloy_primitives::Address;
use alloy_provider::Provider;
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use log::{debug, error, info};

use debug_provider::DebugProviderExt;
use defi_entities::{Market, MarketState, Pool, PoolClass, PoolProtocol, PoolWrapper};
use defi_entities::required_state::RequiredStateReader;
use defi_pools::{UniswapV2Pool, UniswapV3Pool};
use defi_pools::protocols::{fetch_uni2_factory, fetch_uni3_factory};
use loom_actors::SharedState;

lazy_static! {
    static ref uniswapv2_factory: Address = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse().unwrap();
    static ref nomiswap_stable_factory: Address = "0x818339b4E536E707f14980219037c5046b049dD4".parse().unwrap();
    static ref sushiswap_factory: Address = "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse().unwrap();
    static ref dooarswap_factory: Address = "0x1e895bFe59E3A5103e8B7dA3897d1F2391476f3c".parse().unwrap();
    static ref safeswap_factory: Address = "0x7F09d4bE6bbF4b0fF0C97ca5c486a166198aEAeE".parse().unwrap();
    static ref uniswapv3_factory :Address = "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse().unwrap();
    static ref pancakev3_factory: Address =  "0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865".parse().unwrap();
    static ref miniswap_factory: Address =  "0x2294577031F113DF4782B881cF0b140e94209a6F".parse().unwrap();
    static ref shibaswap_factory: Address =  "0x115934131916C8b277DD010Ee02de363c09d037c".parse().unwrap();
    static ref maverick_factory: Address =  "0xEb6625D65a0553c9dBc64449e56abFe519bd9c9B".parse().unwrap();
}



pub fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
    if factory_address == *uniswapv2_factory {
        PoolProtocol::UniswapV2
    } else if factory_address == *uniswapv3_factory {
        PoolProtocol::UniswapV3
    } else if factory_address == *pancakev3_factory {
        PoolProtocol::PancakeV3
    } else if factory_address == *nomiswap_stable_factory {
        PoolProtocol::NomiswapStable
    } else if factory_address == *sushiswap_factory {
        PoolProtocol::Sushiswap
    } else if factory_address == *dooarswap_factory {
        PoolProtocol::DooarSwap
    } else if factory_address == *safeswap_factory {
        PoolProtocol::Safeswap
    } else if factory_address == *miniswap_factory {
        PoolProtocol::Miniswap
    } else if factory_address == *shibaswap_factory {
        PoolProtocol::Shibaswap
    } else if factory_address == *maverick_factory {
        PoolProtocol::Maverick
    } else {
        PoolProtocol::Unknown
    }
}


pub async fn fetch_and_add_pool_by_address<P>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    pool_address: Address,
    pool_class: PoolClass,
) -> Result<()>
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    info!("Fetching pool {:#20x}", pool_address);

    match pool_class {
        PoolClass::UniswapV2 => {
            //TODO fix
            let factory_address = fetch_uni2_factory(client.clone(), pool_address).await?;
            let fetch_result = match get_protocol_by_factory(factory_address) {
                /*PoolProtocol::NomiswapStable => {
                    fetch_and_add_pool(client, market, market_state, NomiswapStablePool::new(pool_address)).await
                }
                PoolProtocol::Miniswap | PoolProtocol::Integral | PoolProtocol::Safeswap => {
                    Err(eyre!("POOL_PROTOCOL_NOT_SUPPORTED"))
                }
                 */
                _ => {
                    let pool = UniswapV2Pool::fetch_pool_data(client.clone(), pool_address).await?;
                    fetch_and_add_pool(client.clone(), market.clone(), market_state.clone(), PoolWrapper::new(Arc::new(pool))).await
                }
            };

            match fetch_result {
                Err(e) => { error!("fetch_and_add_pool uni2 error {:#20x} : {}", pool_address, e) }
                _ => {}
            }
        }
        PoolClass::UniswapV3 => {
            let factory_address_result = fetch_uni3_factory(client.clone(), pool_address).await;
            match factory_address_result {
                Ok(factory_address) => {
                    let fetch_result = match get_protocol_by_factory(factory_address) {
                        /*PoolProtocol::PancakeV3 => {
                            fetch_and_add_pool(client, market, market_state, PancakeV3Pool::new(pool_address)).await
                        }
                        PoolProtocol::Maverick => {
                            fetch_and_add_pool(client, market, market_state, MaverickPool::new(pool_address)).await
                        }*/
                        _ => {
                            let pool = UniswapV3Pool::fetch_pool_data(client.clone(), pool_address).await?;
                            fetch_and_add_pool(client, market, market_state, PoolWrapper::new(Arc::new(pool))).await
                        }
                    };
                    match fetch_result {
                        Err(e) => { error!("fetch_and_add_pool uni3 error {:#20x} : {}", pool_address, e) }
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("Error fetching factory address at {:#20x}: {}",pool_address, e);
                    return Err(eyre!("CANNOT_GET_FACTORY_ADDRESS"));
                }
            }
        }
        _ => {
            error!("Error pool not supported at {:#20x}",pool_address);
            return Err(eyre!("POOL_CLASS_NOT_SUPPORTED"));
        }
    }
    Ok(())
}

pub async fn fetch_and_add_pool<P>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    pool_wrapped: PoolWrapper,
) -> Result<()>
    where
        P: Provider + DebugProviderExt + Send + Sync + Clone + 'static,
{
    match pool_wrapped.get_state_required() {
        Ok(required_state) => {
            match RequiredStateReader::fetch_calls_and_slots(client, required_state, None).await {
                Ok(state) => {
                    //info!("Pool added {} {:?} {:?} accs :{} , storage: {}", pool.get_protocol(), pool.get_address(), pool.get_tokens() ,accs, storage );
                    {
                        let pool_address = pool_wrapped.get_address();
                        {
                            let mut market_state_write_guard = market_state.write().await;
                            market_state_write_guard.add_state(&state);
                            market_state_write_guard.add_force_insert(pool_address);
                            market_state_write_guard.disable_cell_vec(pool_address, pool_wrapped.get_read_only_cell_vec());

                            drop(market_state_write_guard);
                        }

                        let directions_vec = pool_wrapped.get_swap_directions();
                        let mut directions_tree: BTreeMap<PoolWrapper, Vec<(Address, Address)>> = BTreeMap::new();

                        directions_tree.insert(pool_wrapped.clone(), directions_vec);

                        //todo directions

                        let mut market_write_guard = market.write().await;
                        market_write_guard.add_pool(pool_wrapped);

                        let swap_paths = market_write_guard.build_swap_path_vec(&directions_tree)?;
                        market_write_guard.add_paths(swap_paths);

                        drop(market_write_guard)
                    }
                }
                Err(e) => {
                    error!("{}",e);
                    return Err(e);
                }
            }
        }
        Err(e) => {
            error!("{}",e);
            return Err(e);
        }
    }

    Ok(())
}

