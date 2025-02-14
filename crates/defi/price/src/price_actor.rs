use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Div, Mul};
use std::time::Duration;

use alloy_network::Network;
use alloy_primitives::{address, Address, U256};
use alloy_provider::Provider;
use loom_core_actors::{Accessor, Actor, ActorResult, SharedState, WorkerResult};
use loom_core_actors_macros::Accessor;
use loom_core_blockchain::Blockchain;
use loom_defi_address_book::TokenAddressEth;
use loom_defi_pools::protocols::CurveProtocol;
use loom_defi_pools::CurvePool;
use loom_types_entities::{Market, Pool};
use tracing::{debug, error, info};

async fn price_worker<N: Network, P: Provider<N> + Clone + 'static>(client: P, market: SharedState<Market>, once: bool) -> WorkerResult {
    let curve_tricrypto_usdc = CurveProtocol::new_u256_3_eth_to(client.clone(), address!("7F86Bf177Dd4F3494b841a37e810A34dD56c829B"));
    let curve_tricrypto_usdt = CurveProtocol::new_u256_3_eth_to(client.clone(), address!("f5f5b97624542d72a9e06f04804bf81baa15e2b4"));

    let mut coins_hash_map: HashMap<Address, CurvePool<P, N>> = HashMap::new();

    let curve_tricrypto_usdc_pool = CurvePool::fetch_pool_data(client.clone(), curve_tricrypto_usdc).await?;

    let curve_tricrypto_usdt_pool = CurvePool::fetch_pool_data(client.clone(), curve_tricrypto_usdt).await?;

    coins_hash_map.insert(TokenAddressEth::USDC, curve_tricrypto_usdc_pool.clone());
    coins_hash_map.insert(TokenAddressEth::WBTC, curve_tricrypto_usdc_pool.clone());
    coins_hash_map.insert(TokenAddressEth::USDT, curve_tricrypto_usdt_pool.clone());

    let one_ether = U256::from(10).pow(U256::from(18));
    let weth_amount = one_ether.mul(U256::from(5));

    match market.read().await.get_token(&TokenAddressEth::WETH) {
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

            match curve_pool.fetch_out_amount(TokenAddressEth::WETH, *token_address, weth_amount).await {
                Ok(out_amount) => {
                    let price = out_amount.mul(one_ether).div(weth_amount);
                    info!("Price of ETH in {token_address:#20x} is {price}");
                    match market.read().await.get_token(token_address) {
                        Some(tkn) => {
                            tkn.set_eth_price(Some(price));
                            debug!("Price is set");
                        }
                        _ => {
                            error!(address=%token_address, "Token not found");
                        }
                    }
                }
                Err(error) => {
                    error!(%error, "fetch_out_amount")
                }
            }
        }

        let usdt_price = market.read().await.get_token_or_default(&TokenAddressEth::USDT).get_eth_price();
        let usdc_price = market.read().await.get_token_or_default(&TokenAddressEth::USDC).get_eth_price();

        let mut usd_price: Option<U256> = None;
        if let Some(usdc_price) = usdc_price {
            if let Some(usdt_price) = usdt_price {
                usd_price = Some((usdc_price + usdt_price) >> 1);
            }
        }

        if let Some(usd_price) = usd_price {
            match market.read().await.get_token(&TokenAddressEth::DAI) {
                Some(tkn) => {
                    tkn.set_eth_price(Some(U256::from(10).pow(U256::from(12)).mul(usd_price)));
                }
                _ => {
                    error!("Token {:#20x} not found", TokenAddressEth::DAI);
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
pub struct PriceActor<P, N> {
    client: P,
    only_once: bool,
    #[accessor]
    market: Option<SharedState<Market>>,
    _n: PhantomData<N>,
}

impl<P, N> PriceActor<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self { client, only_once: false, market: None, _n: PhantomData }
    }

    pub fn only_once(self) -> Self {
        Self { only_once: true, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market: Some(bc.market()), ..self }
    }
}

impl<P, N> Actor for PriceActor<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(price_worker(self.client.clone(), self.market.clone().unwrap(), self.only_once));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "PriceActor"
    }
}
