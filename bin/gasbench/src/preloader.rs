use alloy_network::Ethereum;
use alloy_provider::Provider;
use loom_core_actors::SharedState;
use loom_defi_address_book::{
    CurveMetapoolAddress, CurvePoolAddress, PancakeV2PoolAddress, PancakeV3PoolAddress, TokenAddressEth, UniswapV2PoolAddress,
    UniswapV3PoolAddress,
};
use loom_defi_market::fetch_and_add_pool_by_pool_id;
use loom_defi_pools::PoolLoadersBuilder;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_entities::pool_config::PoolsLoadingConfig;
use loom_types_entities::{Market, MarketState, PoolClass, Token};
use revm::{Database, DatabaseCommit, DatabaseRef};
use std::sync::Arc;

pub async fn preload_pools<P, DB>(client: P, market: SharedState<Market>, market_state: SharedState<MarketState<DB>>) -> eyre::Result<()>
where
    P: Provider<Ethereum> + DebugProviderExt<Ethereum> + Send + Sync + Clone + 'static,
    DB: DatabaseRef + DatabaseCommit + Database + Send + Sync + Clone + 'static,
{
    let mut market_instance = market.write().await;

    market_instance.add_token(Token::new_with_data(TokenAddressEth::WETH, Some("WETH".to_string()), None, Some(18), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddressEth::USDC, Some("USDC".to_string()), None, Some(6), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddressEth::USDT, Some("USDT".to_string()), None, Some(6), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddressEth::DAI, Some("DAI".to_string()), None, Some(18), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddressEth::WBTC, Some("WBTC".to_string()), None, Some(8), true, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddressEth::THREECRV, Some("3Crv".to_string()), None, Some(18), false, true))?;
    market_instance.add_token(Token::new_with_data(TokenAddressEth::CRV, Some("Crv".to_string()), None, Some(18), false, false))?;
    market_instance.add_token(Token::new_with_data(TokenAddressEth::LUSD, Some("LUSD".to_string()), None, Some(18), false, false))?;

    drop(market_instance);

    let pool_loaders = Arc::new(PoolLoadersBuilder::default_pool_loaders(client.clone(), PoolsLoadingConfig::default()));

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        CurvePoolAddress::ETH_BTC_USD.into(),
        PoolClass::Curve,
    )
    .await?;

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        CurvePoolAddress::USDT_BTC_ETH.into(),
        PoolClass::Curve,
    )
    .await?;

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        CurvePoolAddress::DAI_USDC_USDT.into(),
        PoolClass::Curve,
    )
    .await?;

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        CurveMetapoolAddress::LUSD.into(),
        PoolClass::Curve,
    )
    .await?;

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        UniswapV3PoolAddress::WETH_USDT_3000.into(),
        PoolClass::UniswapV3,
    )
    .await?;

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        PancakeV2PoolAddress::WETH_USDT.into(),
        PoolClass::UniswapV2,
    )
    .await?;
    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        UniswapV2PoolAddress::WETH_USDT.into(),
        PoolClass::UniswapV2,
    )
    .await?;
    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        PancakeV3PoolAddress::USDC_USDT_100.into(),
        PoolClass::UniswapV3,
    )
    .await?;

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        UniswapV3PoolAddress::USDC_WETH_3000.into(),
        PoolClass::UniswapV3,
    )
    .await?;
    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        UniswapV3PoolAddress::USDC_WETH_500.into(),
        PoolClass::UniswapV3,
    )
    .await?;
    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        UniswapV3PoolAddress::WBTC_USDT_3000.into(),
        PoolClass::UniswapV3,
    )
    .await?;
    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        UniswapV3PoolAddress::USDC_USDT_100.into(),
        PoolClass::UniswapV3,
    )
    .await?;

    fetch_and_add_pool_by_pool_id(
        client.clone(),
        market.clone(),
        market_state.clone(),
        pool_loaders.clone(),
        UniswapV2PoolAddress::LUSD_WETH.into(),
        PoolClass::UniswapV2,
    )
    .await?;

    Ok(())
}
