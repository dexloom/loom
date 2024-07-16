use alloy_network::Network;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use lazy_static::lazy_static;

use debug_provider::DebugProviderExt;
use defi_actors::fetch_and_add_pool_by_address;
use defi_entities::{Market, MarketState, PoolClass, Token};
use loom_actors::SharedState;

lazy_static! {
    pub static ref WETH_ADDRESS: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
    pub static ref USDC_ADDRESS: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
    pub static ref USDT_ADDRESS: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap();
    pub static ref DAI_ADDRESS: Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap();
    pub static ref WBTC_ADDRESS: Address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap();
    pub static ref THREECRV_ADDRESS: Address = "0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490".parse().unwrap();
    pub static ref CRV_ADDRESS: Address = "0xD533a949740bb3306d119CC777fa900bA034cd52".parse().unwrap();
    pub static ref LUSD_ADDRESS: Address = "0x5f98805A4E8be255a32880FDeC7F6728C6568bA0".parse().unwrap();
}

pub async fn preload_pools<P, T, N>(client: P, market: SharedState<Market>, market_state: SharedState<MarketState>) -> eyre::Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let weth_token = Token::new_with_data(*WETH_ADDRESS, Some("WETH".to_string()), None, Some(18), true, false);
    let usdc_token = Token::new_with_data(*USDC_ADDRESS, Some("USDC".to_string()), None, Some(6), true, false);
    let usdt_token = Token::new_with_data(*USDT_ADDRESS, Some("USDT".to_string()), None, Some(6), true, false);
    let dai_token = Token::new_with_data(*DAI_ADDRESS, Some("DAI".to_string()), None, Some(18), true, false);
    let wbtc_token = Token::new_with_data(*WBTC_ADDRESS, Some("WBTC".to_string()), None, Some(8), true, false);
    let threecrv_token = Token::new_with_data(*THREECRV_ADDRESS, Some("3Crv".to_string()), None, Some(18), false, true);
    let crv_token = Token::new_with_data(*CRV_ADDRESS, Some("Crv".to_string()), None, Some(18), false, false);

    let lusd_token = Token::new_with_data(*LUSD_ADDRESS, Some("LUSD".to_string()), None, Some(18), false, false);

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

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x7f86bf177dd4f3494b841a37e810a34dd56c829b".parse().unwrap(),
        PoolClass::Curve,
    )
    .await?; // Tricrypto USDC

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xd51a44d3fae010294c616388b506acda1bfaae46".parse().unwrap(),
        PoolClass::Curve,
    )
    .await?; // Tricrypto2

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".parse().unwrap(),
        PoolClass::Curve,
    )
    .await?; // 3Crv pool

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xed279fdd11ca84beef15af5d39bb4d4bee23f0ca".parse().unwrap(),
        PoolClass::Curve,
    )
    .await?; // LUSD Metapool

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; // USDT USDC +

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x17C1Ae82D99379240059940093762c5e4539aba5".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; // Pancake USDT WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; // uni2 USDT WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x04c8577958ccc170eb3d2cca76f9d51bc6e42d8f".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; // USDT USDC +

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; // USDC WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; // USDC WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x9db9e0e53058c89e5b94e29621a205198648425b".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; // USDT WBTC +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x3416cF6C708Da44DB2624D63ea0AAef7113527C6".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; // USDT USDC +

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xF20EF17b889b437C151eB5bA15A47bFc62bfF469".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; // LUSD ETH

    Ok(())
}
