use alloy_primitives::Address;
use alloy_provider::Provider;
use log::{error, info};

use debug_provider::DebugProviderExt;
use defi_actors::{fetch_and_add_pool_by_address, fetch_state_and_add_pool};
use defi_entities::{Market, MarketState, Pool, PoolClass, Token};
use defi_pools::protocols::CurveProtocol;
use defi_pools::CurvePool;
use loom_actors::SharedState;

#[allow(dead_code)]
async fn load_pools<P: Provider + DebugProviderExt + Send + Sync + Clone + 'static>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
) -> eyre::Result<()> {
    if let Ok(curve_contract) =
        CurveProtocol::get_contract_from_code(client.clone(), "0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7".parse().unwrap()).await
    {
        let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
        fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await?
    } else {
        error!("CURVE_POOL_NOT_LOADED");
    }

    if let Ok(curve_contract) =
        CurveProtocol::get_contract_from_code(client.clone(), "0x9c3B46C0Ceb5B9e304FCd6D88Fc50f7DD24B31Bc".parse().unwrap()).await
    {
        let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
        fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await?
    } else {
        error!("CURVE_POOL_NOT_LOADED");
    }

    if let Ok(curve_contract) =
        CurveProtocol::get_contract_from_code(client.clone(), "0xa1F8A6807c402E4A15ef4EBa36528A3FED24E577".parse().unwrap()).await
    {
        let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
        fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await?
    } else {
        error!("CURVE_POOL_NOT_LOADED");
    }

    if let Ok(curve_contract) =
        CurveProtocol::get_contract_from_code(client.clone(), "0x4ebdf703948ddcea3b11f675b4d1fba9d2414a14".parse().unwrap()).await
    {
        let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
        fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await?
    } else {
        error!("CURVE_POOL_NOT_LOADED");
    }

    /*
    if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(client.clone(),"0xf5f5b97624542d72a9e06f04804bf81baa15e2b4".parse().unwrap()).await {
        let curve_pool = CurvePool::from(curve_contract);
        fetch_and_add_pool(client.clone(),market.clone(), market_state.clone(), curve_pool.clone()).await?
    }else{
        error!("Pool not loaded");
        panic!("Pool not loaded");
    }

     */

    /*if let Ok(curve_contract) = CurveProtocol::get_contract_from_code(client.clone(),"0xd51a44d3fae010294c616388b506acda1bfaae46".parse().unwrap()).await {
        let curve_pool = CurvePool::from(curve_contract);
        fetch_and_add_pool(client.clone(),market.clone(), market_state.clone(), curve_pool.clone()).await?
    }else{
        error!("Pool not loaded");
        panic!("Pool not loaded");
    }

     */

    let curve_contracts = CurveProtocol::get_contracts_vec(client.clone());
    for curve_contract in curve_contracts.into_iter() {
        let curve_pool = CurvePool::fetch_pool_data(client.clone(), curve_contract).await?;
        let pool_address = curve_pool.get_address();
        match fetch_state_and_add_pool(client.clone(), market.clone(), market_state.clone(), curve_pool.into()).await {
            Err(e) => {
                error!("Curve pool loading error : {}", e)
            }
            Ok(_) => {
                info!("Curve pool loaded {}", pool_address.to_checksum(None));
            }
        }
    }

    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x8a15b2Dc9c4f295DCEbB0E7887DD25980088fDCB".parse().unwrap(), PoolClass::UniswapV3 ).await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x17C1Ae82D99379240059940093762c5e4539aba5".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x763d3b7296e7c9718ad5b058ac2692a19e5b3638".parse().unwrap(), PoolClass::UniswapV3 ).await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0xee4cf3b78a74affa38c6a926282bcd8b5952818d".parse().unwrap(), PoolClass::UniswapV3 ).await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x3a8414b08ffb128cf1a9da1097b0454e0d4bfa8f".parse().unwrap(), PoolClass::UniswapV2 ).await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x0de0fa91b6dbab8c8503aaa2d1dfa91a192cb149".parse().unwrap(), PoolClass::UniswapV2 ).await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0xa181a4496403491ac406f93593199c704c701976".parse().unwrap(), PoolClass::UniswapV2 ).await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x801ccfae9d2c77893b545e8d0e4637c055cd26cb".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x04c8577958ccc170eb3d2cca76f9d51bc6e42d8f".parse().unwrap(), PoolClass::UniswapV3 ).await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8".parse().unwrap(), PoolClass::UniswapV3 ).await?;
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x9db9e0e53058c89e5b94e29621a205198648425b".parse().unwrap(), PoolClass::UniswapV3 ).await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xfa6e8e97ececdc36302eca534f63439b1e79487b".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xCBCdF9626bC03E24f779434178A73a0B4bad62eD".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //ETH WBTC
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xbb2b8038a1640196fbe3e38816f3e67cba72d940".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; //ETH WBTC
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x919Fa96e88d67499339577Fa202345436bcDaf79".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //ETH CRV
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x3da1313ae46132a397d90d95b1424a9a7e3e0fce".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; //ETH CRV
             //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x9c2dc3d5ffcecf61312c5f4c00660695b32fb3d1".parse().unwrap(), PoolClass::UniswapV2 ).await?;
             //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0xee4cf3b78a74affa38c6a926282bcd8b5952818d".parse().unwrap(), PoolClass::UniswapV3 ).await?;
             //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x20e95253e54490d8d30ea41574b24f741ee70201".parse().unwrap(), PoolClass::UniswapV2 ).await?;
             //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x4ab6702b3ed3877e9b1f203f90cbef13d663b0e8".parse().unwrap(), PoolClass::UniswapV2 ).await?;
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xa84181f223a042949e9040e42b44c50021802db6".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //WETH PEPE
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xaa9b647f42858f2db441f0aa75843a8e7fd5aff2".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; //WETH PEPE

    //
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x177a9b475f55b6b7b25204e2562a39308bba2a54".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; //WETH N
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x90e7a93e0a6514cb0c84fc7acc1cb5c0793352d2".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //WETH N

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x48da0965ab2d2cbf1c17c09cfb5cbe67ad5b1406".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //DAI USDT
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0xebce363564fa8b55d85aaf681156087116b148db".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //USDT
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x2b2a82d50e6e9d5b95ca644b989f9b143ea9ede2".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //USDT
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x2dd35b4da6534230ff53048f7477f17f7f4e7a70".parse().unwrap(),
        PoolClass::UniswapV3,
    )
    .await?; //USDT
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x4a86c01d67965f8cb3d0aaa2c655705e64097c31".parse().unwrap(),
        PoolClass::UniswapV2,
    )
    .await?; //USDT

    Ok(())
}

#[allow(dead_code)]
async fn load_tokens<P: Provider + DebugProviderExt + Send + Sync + Clone + 'static>(
    _client: P,
    market_instance: SharedState<Market>,
) -> eyre::Result<()> {
    let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
    let usdc_address: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
    let usdt_address: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap();
    let dai_address: Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap();
    let wbtc_address: Address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap();
    let threecrv_address: Address = "0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490".parse().unwrap();
    let crv_address: Address = "0xD533a949740bb3306d119CC777fa900bA034cd52".parse().unwrap();

    let weth_token = Token::new_with_data(weth_address, Some("WETH".to_string()), None, Some(18), true, false);
    let usdc_token = Token::new_with_data(usdc_address, Some("USDC".to_string()), None, Some(6), true, false);
    let usdt_token = Token::new_with_data(usdt_address, Some("USDT".to_string()), None, Some(6), true, false);
    let dai_token = Token::new_with_data(dai_address, Some("DAI".to_string()), None, Some(18), true, false);
    let wbtc_token = Token::new_with_data(wbtc_address, Some("WBTC".to_string()), None, Some(8), true, false);
    let threecrv_token = Token::new_with_data(threecrv_address, Some("3Crv".to_string()), None, Some(18), false, true);
    let crv_token = Token::new_with_data(crv_address, Some("Crv".to_string()), None, Some(18), false, false);

    let mut market_guard = market_instance.write().await;

    market_guard.add_token(weth_token)?;
    market_guard.add_token(usdc_token)?;
    market_guard.add_token(usdt_token)?;
    market_guard.add_token(dai_token)?;
    market_guard.add_token(wbtc_token)?;
    market_guard.add_token(threecrv_token)?;
    market_guard.add_token(crv_token)?;

    Ok(())
}
