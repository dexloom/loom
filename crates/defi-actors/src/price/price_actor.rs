use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Div, Mul};
use std::time::Duration;

use alloy_network::Network;
use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use alloy_transport::Transport;
use async_trait::async_trait;
use log::{debug, error, info};

use defi_blockchain::Blockchain;
use defi_entities::{Market, Pool};
use defi_pools::protocols::CurveProtocol;
use defi_pools::CurvePool;
use loom_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_actors_macros::Accessor;

//use market::{CurveProtocol, Market, PoolSetup};
//use market::contracts::CurvePool;

async fn price_worker<N: Network, T: Transport + Clone, P: Provider<T, N> + Clone + 'static>(
    client: P,
    market: SharedState<Market>,
    once: bool,
) -> WorkerResult {
    let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();

    let usdc_address: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
    let usdt_address: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap();
    let dai_address: Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap();
    let wbtc_address: Address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap();

    let curve_tricrypto_usdc =
        CurveProtocol::new_u256_3_eth_to(client.clone(), "0x7F86Bf177Dd4F3494b841a37e810A34dD56c829B".parse().unwrap());
    let curve_tricrypto_usdt =
        CurveProtocol::new_u256_3_eth_to(client.clone(), "0xf5f5b97624542d72a9e06f04804bf81baa15e2b4".parse().unwrap());

    let mut coins_hash_map: HashMap<Address, CurvePool<P, T, N>> = HashMap::new();

    let curve_tricrypto_usdc_pool = CurvePool::fetch_pool_data(client.clone(), curve_tricrypto_usdc).await?;

    let curve_tricrypto_usdt_pool = CurvePool::fetch_pool_data(client.clone(), curve_tricrypto_usdt).await?;

    coins_hash_map.insert(usdc_address, curve_tricrypto_usdc_pool.clone());
    coins_hash_map.insert(wbtc_address, curve_tricrypto_usdc_pool.clone());
    coins_hash_map.insert(usdt_address, curve_tricrypto_usdt_pool.clone());

    let one_ether = U256::from(10).pow(U256::from(18));
    let weth_amount = one_ether.mul(U256::from(5));

    match market.read().await.get_token(&weth_address) {
        Some(token) => {
            token.set_eth_price(Some(one_ether));
        }
        _ => {
            error!("WETH_NOT_FOUND")
        }
    }

    loop {
        for (token_address, curve_pool) in coins_hash_map.iter() {
            debug!("Fetching price of {} at {}", token_address, curve_pool.get_address());

            match curve_pool.fetch_out_amount(weth_address, *token_address, weth_amount).await {
                Ok(out_amount) => {
                    let price = out_amount.mul(one_ether).div(weth_amount);
                    info!("Price of ETH in {token_address:#20x} is {price}");
                    match market.read().await.get_token(token_address) {
                        Some(tkn) => {
                            tkn.set_eth_price(Some(price));
                            debug!("price is set");
                        }
                        _ => {
                            error!("Token {token_address:#20x} not found");
                        }
                    }
                }
                Err(e) => {
                    error!("fetch_out_amount : {e}")
                }
            }
        }

        let usdt_price = market.read().await.get_token_or_default(&usdt_address).get_eth_price();
        let usdc_price = market.read().await.get_token_or_default(&usdc_address).get_eth_price();

        let mut usd_price: Option<U256> = None;
        if let Some(usdc_price) = usdc_price {
            if let Some(usdt_price) = usdt_price {
                usd_price = Some((usdc_price + usdt_price) >> 1);
            }
        }

        if let Some(usd_price) = usd_price {
            match market.read().await.get_token(&dai_address) {
                Some(tkn) => {
                    tkn.set_eth_price(Some(U256::from(10).pow(U256::from(12)).mul(usd_price)));
                }
                _ => {
                    error!("Token {dai_address:#20x} not found");
                }
            }
        }
        if once {
            break;
        }

        let _ = tokio::time::sleep(Duration::new(60, 0)).await;
    }
    Ok("PriceWorker finished".to_string())
}

#[derive(Accessor)]
pub struct PriceActor<P, T, N> {
    client: P,
    only_once: bool,
    #[accessor]
    market: Option<SharedState<Market>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> PriceActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self { client, only_once: false, market: None, _t: PhantomData, _n: PhantomData }
    }

    pub fn only_once(self) -> Self {
        Self { only_once: true, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market()), ..self }
    }
}

#[async_trait]
impl<P, T, N> Actor for PriceActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(price_worker(self.client.clone(), self.market.clone().unwrap(), self.only_once));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PriceActor"
    }
}
