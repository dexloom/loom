use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;

use debug_provider::DebugProviderExt;
use defi_actors::fetch_and_add_pool_by_address;
use defi_address_book::{
    CurveMetapoolAddress, CurvePoolAddress, PancakeV2PoolAddress, PancakeV3PoolAddress, UniswapV2PoolAddress, UniswapV3PoolAddress,
};
use defi_entities::{Market, MarketState, PoolClass, Token};
use loom_actors::SharedState;
use loom_utils::tokens::*;

pub async fn preload_pools<P, T, N>(client: P, market: SharedState<Market>, market_state: SharedState<MarketState>) -> eyre::Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let weth_token = Token::new_with_data(WETH_ADDRESS, Some("WETH".to_string()), None, Some(18), true, false);
    let usdc_token = Token::new_with_data(USDC_ADDRESS, Some("USDC".to_string()), None, Some(6), true, false);
    let usdt_token = Token::new_with_data(USDT_ADDRESS, Some("USDT".to_string()), None, Some(6), true, false);
    let dai_token = Token::new_with_data(DAI_ADDRESS, Some("DAI".to_string()), None, Some(18), true, false);
    let wbtc_token = Token::new_with_data(WBTC_ADDRESS, Some("WBTC".to_string()), None, Some(8), true, false);
    let threecrv_token = Token::new_with_data(THREECRV_ADDRESS, Some("3Crv".to_string()), None, Some(18), false, true);
    let crv_token = Token::new_with_data(CRV_ADDRESS, Some("Crv".to_string()), None, Some(18), false, false);

    let lusd_token = Token::new_with_data(LUSD_ADDRESS, Some("LUSD".to_string()), None, Some(18), false, false);

    let mut market_instance = market.write().await;

    market_instance.add_token(weth_token)?;
    market_instance.add_token(usdc_token)?;
    market_instance.add_token(usdt_token)?;
    market_instance.add_token(dai_token)?;
    market_instance.add_token(wbtc_token)?;
    market_instance.add_token(threecrv_token)?;
    market_instance.add_token(crv_token)?;
    market_instance.add_token(lusd_token)?;

    drop(market_instance);

    fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(), CurvePoolAddress::ETH_BTC_USD, PoolClass::Curve)
        .await?;

    fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(), CurvePoolAddress::USDT_BTC_ETH, PoolClass::Curve)
        .await?;

    fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(), CurvePoolAddress::DAI_USDC_USDT, PoolClass::Curve)
        .await?;

    fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(), CurveMetapoolAddress::LUSD, PoolClass::Curve)
        .await?;

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        UniswapV3PoolAddress::WETH_USDT_3000,
        PoolClass::UniswapV3,
    )
    .await?;

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        PancakeV2PoolAddress::WETH_USDT,
        PoolClass::UniswapV2,
    )
    .await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        UniswapV2PoolAddress::WETH_USDT,
        PoolClass::UniswapV2,
    )
    .await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        PancakeV3PoolAddress::USDC_USDT_100,
        PoolClass::UniswapV3,
    )
    .await?;

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        UniswapV3PoolAddress::USDC_WETH_3000,
        PoolClass::UniswapV3,
    )
    .await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        UniswapV3PoolAddress::USDC_WETH_500,
        PoolClass::UniswapV3,
    )
    .await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        UniswapV3PoolAddress::WBTC_USDT_3000,
        PoolClass::UniswapV3,
    )
    .await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        UniswapV3PoolAddress::USDC_USDT_100,
        PoolClass::UniswapV3,
    )
    .await?;

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        UniswapV2PoolAddress::LUSD_WETH,
        PoolClass::UniswapV2,
    )
    .await?;

    Ok(())
}
