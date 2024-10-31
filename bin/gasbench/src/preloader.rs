use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;

use loom_core_actors::SharedState;
use loom_defi_entities::{Market, MarketState, PoolClass, Token};
use loom_defi_market::fetch_and_add_pool_by_address;
use loom_node_debug_provider::DebugProviderExt;
use loom_protocol_address_book::{
    CurveMetapoolAddress, CurvePoolAddress, PancakeV2PoolAddress, PancakeV3PoolAddress, TokenAddress, UniswapV2PoolAddress,
    UniswapV3PoolAddress,
};

pub async fn preload_pools<P, T, N>(client: P, market: SharedState<Market>, market_state: SharedState<MarketState>) -> eyre::Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let mut market_instance = market.write().await;

    market_instance.add_token(Token::new_with_data(TokenAddress::WETH, Some("WETH".to_string()), None, Some(18), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddress::USDC, Some("USDC".to_string()), None, Some(6), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddress::USDT, Some("USDT".to_string()), None, Some(6), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddress::DAI, Some("DAI".to_string()), None, Some(18), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddress::WBTC, Some("WBTC".to_string()), None, Some(8), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddress::THREECRV, Some("3Crv".to_string()), None, Some(18), false, true))?;
    market_instance.add_token(Token::new_with_data(TokenAddress::CRV, Some("Crv".to_string()), None, Some(18), false, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddress::LUSD, Some("LUSD".to_string()), None, Some(18), false, false))?;

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
