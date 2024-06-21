use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use alloy::primitives::{Address, BlockNumber, U256};
use alloy_network::{Ethereum, Network};
use alloy_primitives::eip191_hash_message;
use alloy_provider::Provider;
use alloy_rpc_types::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_transport::Transport;
use clap::Parser;
use colored::*;
use eyre::{eyre, OptionExt, Result};
use lazy_static::lazy_static;
use log::{debug, error, info};
use revm::db::EmptyDB;
use revm::InMemoryDB;
use revm::primitives::Env;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use debug_provider::{AnvilControl, AnvilDebugProvider, AnvilProviderExt, DebugProviderExt};
use defi_abi::IERC20::IERC20Instance;
use defi_actors::{fetch_and_add_pool_by_address, preload_market_state};
use defi_entities::{Market, MarketState, NWETH, PoolClass, PoolWrapper, Swap, SwapAmountType, SwapLine, SwapPath, Token};
use loom_actors::SharedState;
use loom_multicaller::{MulticallerDeployer, MulticallerSwapEncoder, SwapEncoder};
use loom_utils::db_direct_access::calc_hashmap_cell;
use loom_utils::evm::evm_call;

use crate::cli::Cli;
use crate::dto::SwapLineDTO;

mod cli;
mod dto;

lazy_static! {
    static ref WETH_ADDRESS: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        .parse()
        .unwrap();
    static ref USDC_ADDRESS: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        .parse()
        .unwrap();
    static ref USDT_ADDRESS: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
        .parse()
        .unwrap();
    static ref DAI_ADDRESS: Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F"
        .parse()
        .unwrap();
    static ref WBTC_ADDRESS: Address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"
        .parse()
        .unwrap();
    static ref THREECRV_ADDRESS: Address = "0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490"
        .parse()
        .unwrap();
    static ref CRV_ADDRESS: Address = "0xD533a949740bb3306d119CC777fa900bA034cd52"
        .parse()
        .unwrap();
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SwapPathDTO {
    tokens: Vec<Address>,
    pools: Vec<Address>,
}

async fn preload_pools<P, T, N>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
) -> Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let mut weth_token = Token::new_with_data(
        *WETH_ADDRESS,
        Some("WETH".to_string()),
        None,
        Some(18),
        true,
        false,
    );
    let mut usdc_token = Token::new_with_data(
        *USDC_ADDRESS,
        Some("USDC".to_string()),
        None,
        Some(6),
        true,
        false,
    );
    let mut usdt_token = Token::new_with_data(
        *USDT_ADDRESS,
        Some("USDT".to_string()),
        None,
        Some(6),
        true,
        false,
    );
    let mut dai_token = Token::new_with_data(
        *DAI_ADDRESS,
        Some("DAI".to_string()),
        None,
        Some(18),
        true,
        false,
    );
    let mut wbtc_token = Token::new_with_data(
        *WBTC_ADDRESS,
        Some("WBTC".to_string()),
        None,
        Some(8),
        true,
        false,
    );
    let mut threecrv_token = Token::new_with_data(
        *THREECRV_ADDRESS,
        Some("3Crv".to_string()),
        None,
        Some(18),
        false,
        true,
    );
    let mut crv_token = Token::new_with_data(
        *CRV_ADDRESS,
        Some("Crv".to_string()),
        None,
        Some(18),
        false,
        false,
    );

    let mut market_instance = market.write().await;

    market_instance.add_token(weth_token)?;
    market_instance.add_token(usdc_token)?;
    market_instance.add_token(usdt_token)?;
    market_instance.add_token(dai_token)?;
    market_instance.add_token(wbtc_token)?;
    market_instance.add_token(threecrv_token)?;
    market_instance.add_token(crv_token)?;

    drop(market_instance);

    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x17C1Ae82D99379240059940093762c5e4539aba5"
            .parse()
            .unwrap(),
        PoolClass::UniswapV2,
    )
        .await?; // Pancake USDT WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852"
            .parse()
            .unwrap(),
        PoolClass::UniswapV2,
    )
        .await?; // USDT WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x04c8577958ccc170eb3d2cca76f9d51bc6e42d8f"
            .parse()
            .unwrap(),
        PoolClass::UniswapV3,
    )
        .await?; // USDT USDC +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8"
            .parse()
            .unwrap(),
        PoolClass::UniswapV3,
    )
        .await?; // USDC WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640"
            .parse()
            .unwrap(),
        PoolClass::UniswapV3,
    )
        .await?; // USDC WETH +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x9db9e0e53058c89e5b94e29621a205198648425b"
            .parse()
            .unwrap(),
        PoolClass::UniswapV3,
    )
        .await?; // USDT WBTC +
    fetch_and_add_pool_by_address(
        client.clone(),
        market.clone(),
        market_state.clone(),
        "0x3416cF6C708Da44DB2624D63ea0AAef7113527C6"
            .parse()
            .unwrap(),
        PoolClass::UniswapV3,
    )
        .await?; // USDT USDC +
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0xCBCdF9626bC03E24f779434178A73a0B4bad62eD".parse().unwrap(), PoolClass::UniswapV3 ).await?; //ETH WBTC +
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0xbb2b8038a1640196fbe3e38816f3e67cba72d940".parse().unwrap(), PoolClass::UniswapV2 ).await?; //ETH WBTC +
    //fetch_and_add_pool_by_address(client.clone(), market.clone(), market_state.clone(),"0x4ab6702b3ed3877e9b1f203f90cbef13d663b0e8".parse().unwrap(), PoolClass::UniswapV2 ).await?; // pancake WBTC WETH +-

    Ok(())
}

async fn preset_balances<P, T, N>(client: P) -> Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let uni_pool_address: Address = "0xCBCdF9626bC03E24f779434178A73a0B4bad62eD".parse()?;

    let balance_storage_cell = calc_hashmap_cell(U256::from(3u32), U256::from_be_slice(uni_pool_address.as_slice()));

    let value = client
        .get_storage_at(*WETH_ADDRESS, balance_storage_cell)
        .await?;

    if value.is_zero() {
        Err(eyre!("BAD_BALANCE_CELL"))
    } else {
        debug!("Balance at cell balance_storage_cell {balance_storage_cell} is {value}");
        Ok(())
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::try_parse()?;

    env_logger::init_from_env(
        env_logger::Env::default().default_filter_or("debug,alloy_rpc_client=off"),
    );
    let block_number = 20089277u64;

    println!("Hello, block {block_number}!");
    let client = AnvilControl::from_node_on_block(
        "ws://falcon.loop:8008/looper".to_string(),
        BlockNumber::from(block_number),
    ).await?;


    //preset_balances(client.clone()).await?;

    let block_header = client
        .get_block_by_number(BlockNumberOrTag::Number(block_number), false)
        .await?
        .unwrap()
        .header;

    let operator_address = Address::repeat_byte(0x12);
    let multicaller_address = Address::repeat_byte(0x78);

    // Set Multicaller code
    let multicaller_address = MulticallerDeployer::new()
        .set_code(client.clone(), multicaller_address)
        .await?
        .address()
        .ok_or_eyre("MULTICALLER_NOT_DEPLOYED")?;
    info!("Multicaller deployed at {:?}", multicaller_address);

    // SET Multicaller WETH balance
    let weth_balance = NWETH::from_float(1.0);

    let balance_cell = calc_hashmap_cell(U256::from(3), U256::from_be_slice(multicaller_address.as_slice()));

    match client.set_storage(*WETH_ADDRESS, balance_cell.into(), weth_balance.into()).await {
        Err(e) => {
            error!("{e}");
            panic!("{e}")
        }
        _ => {}
    }


    let new_storage = client.get_storage_at(*WETH_ADDRESS, balance_cell).await?;

    if weth_balance != new_storage {
        error!("{weth_balance} != {new_storage}");
        panic!("STORAGE_NOT_SET")
    }


    let weth_instance = IERC20Instance::new(*WETH_ADDRESS, client.clone());

    let balance = weth_instance.balanceOf(multicaller_address).call().await?;
    if balance._0 != NWETH::from_float(1.0) {
        panic!("BALANCE_NOT_SET")
    }

    // Initialization
    let mut cache_db = InMemoryDB::new(EmptyDB::new());

    let mut market_instance = Market::default();

    let mut market_state_instance = MarketState::new(cache_db.clone());

    let mut market_instance = SharedState::new(market_instance);

    let mut market_state_instance = SharedState::new(market_state_instance);

    let encoder = Arc::new(MulticallerSwapEncoder::new(multicaller_address));

    //preload state
    preload_market_state(
        client.clone(),
        vec![multicaller_address],
        None,
        market_state_instance.clone(),
    )
        .await?;

    //Preloading market
    preload_pools(
        client.clone(),
        market_instance.clone(),
        market_state_instance.clone(),
    )
        .await?;

    let market = market_instance.read().await;

    // Getting swap directions
    let pool_address: Address = "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852".parse()?;

    let pool = market
        .get_pool(&pool_address)
        .ok_or_eyre("POOL_NOT_FOUND")?;

    let swap_directions = pool.get_swap_directions();

    let mut btree_map: BTreeMap<PoolWrapper, Vec<(Address, Address)>> = BTreeMap::new();

    btree_map.insert(pool.clone(), swap_directions);

    let swap_paths = market.build_swap_path_vec(&btree_map)?;

    let swap_path_map: HashMap<SwapPath, u64> = HashMap::new();

    let db = market_state_instance.read().await.state_db.clone();

    let mut env = Env::default();

    env.block.number = U256::from(block_header.number.unwrap_or_default());
    env.block.timestamp = U256::from(block_header.timestamp);
    //env.block.basefee = U256::from(block_header.base_fee_per_gas.unwrap_or_default());

    let in_amount_f64 = 1.0;
    let in_amount = NWETH::from_float(in_amount_f64);

    let mut gas_used_map: HashMap<SwapLineDTO, u64> = HashMap::new();

    for (i, s) in swap_paths.iter().enumerate() {
        if !s.tokens[0].is_weth() {
            continue;
        }
        let sp = s.as_ref().clone();
        println!("{} : {:?}", i, sp);

        let mut swapline = SwapLine {
            path: sp,
            amount_in: SwapAmountType::Set(in_amount),
            ..SwapLine::default()
        };

        match swapline.calculate_with_in_amount(&db, env.clone(), in_amount) {
            Ok((out_amount, gas_used)) => {
                info!(
                    "gas: {}  amount {} -> {}",
                    gas_used,
                    in_amount_f64,
                    NWETH::to_float(out_amount)
                );
                swapline.amount_out = SwapAmountType::Set(out_amount)
            }
            Err(e) => {
                error!("calculate_with_in_amount error : {:?}", e);
            }
        }
        let swap = Swap::BackrunSwapLine(swapline);

        let calls = encoder.make_calls(&swap)?;
        let (to, payload) = encoder.encode_calls(calls)?;

        let tx_request = TransactionRequest::default()
            .to(to)
            .from(operator_address)
            .input(TransactionInput::new(payload));

        let gas_used = match client.estimate_gas(&tx_request).await {
            Ok(gas_needed) => {
                info!("Gas required:  {gas_needed}");
                gas_needed as u64
            }
            Err(e) => {
                error!("Gas estimation error : {e}");
                0
            }
        };

        gas_used_map.insert(s.clone().as_ref().into(), gas_used);
    }


    if cli.save {
        let results: Vec<(SwapLineDTO, u64)> = gas_used_map.into_iter().collect();

        let json_string = serde_json::to_string_pretty(&results)?;

        let mut file = File::create(cli.file).await?;
        file.write_all(json_string.as_bytes()).await?;
    } else {
        let mut file = File::open(cli.file).await?;
        let mut json_string = String::new();
        file.read_to_string(&mut json_string).await?;

        let stored_results: Vec<(SwapLineDTO, u64)> = serde_json::from_str(&json_string)?;


        let stored_gas_map: HashMap<SwapLineDTO, u64> = stored_results.clone().into_iter().map(|(k, v)| (k, v)).collect();

        for (current_entry, gas) in gas_used_map.iter() {
            match stored_gas_map.get(current_entry) {
                Some(stored_gas) => {
                    let change_i: i64 = *gas as i64 - *stored_gas as i64;
                    let change = format!("{change_i}");
                    let change = if change_i > 0 {
                        change.red()
                    } else if change_i < 0 {
                        change.green()
                    } else {
                        change.normal()
                    };
                    println!("{} : {} {} - {} ", change, current_entry, gas, stored_gas, );
                }
                None => {
                    println!("{} : {} {}", "NO_DATA".green(), current_entry, gas, );
                }
            }
        }
    }


    Ok(())
}
