use alloy_network::primitives::BlockTransactionsKind;
use alloy_primitives::{Address, BlockNumber, Bytes, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use clap::Parser;
use colored::*;
use eyre::{OptionExt, Result};
use revm::primitives::Env;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info};

use crate::cli::Cli;
use crate::dto::SwapLineDTO;
use crate::preloader::preload_pools;
use crate::soltest::create_sol_test;
use loom_node_debug_provider::AnvilDebugProviderFactory;

use loom_defi_address_book::UniswapV2PoolAddress;
use loom_types_entities::{Market, MarketState, PoolId, PoolWrapper, Swap, SwapAmountType, SwapLine};

use loom_core_actors::SharedState;
use loom_defi_preloader::preload_market_state;
use loom_evm_db::LoomDBType;
use loom_evm_utils::{BalanceCheater, NWETH};
use loom_execution_multicaller::pool_opcodes_encoder::ProtocolSwapOpcodesEncoderV2;
use loom_execution_multicaller::{
    MulticallerDeployer, MulticallerEncoder, MulticallerSwapEncoder, ProtocolABIEncoderV2, SwapLineEncoder, SwapStepEncoder,
};

mod cli;
mod dto;
mod preloader;
mod soltest;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SwapPathDTO {
    tokens: Vec<Address>,
    pools: Vec<Address>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::try_parse()?;

    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    let block_number = 20089277u64;

    println!("Hello, block {block_number}!");

    let node_url = env::var("MAINNET_WS")?;

    let client = AnvilDebugProviderFactory::from_node_on_block(node_url, BlockNumber::from(block_number)).await?;

    let block_header =
        client.get_block_by_number(BlockNumberOrTag::Number(block_number), BlockTransactionsKind::Hashes).await?.unwrap().header;

    let operator_address = Address::repeat_byte(0x12);
    let multicaller_address = Address::repeat_byte(0x78);

    // Set Multicaller code
    let multicaller_address =
        MulticallerDeployer::new().set_code(client.clone(), multicaller_address).await?.address().ok_or_eyre("MULTICALLER_NOT_DEPLOYED")?;
    info!("Multicaller deployed at {:?}", multicaller_address);

    // Cheat : SET Multicaller WETH balance
    BalanceCheater::set_anvil_token_balance_float(client.clone(), NWETH::ADDRESS, multicaller_address, 1.0).await?;

    // Initialization
    let cache_db = LoomDBType::default();

    let market_instance = Market::default();

    let market_state_instance = MarketState::new(cache_db.clone());

    let market_instance = SharedState::new(market_instance);

    let market_state_instance = SharedState::new(market_state_instance);

    let abi_encoder = ProtocolABIEncoderV2::default();

    let swap_opcodes_encoder = ProtocolSwapOpcodesEncoderV2::default();

    let swap_line_encoder = SwapLineEncoder::new(multicaller_address, Arc::new(abi_encoder), Arc::new(swap_opcodes_encoder));

    let swap_step_encoder = SwapStepEncoder::new(multicaller_address, swap_line_encoder);

    let swap_encoder = Arc::new(MulticallerSwapEncoder::new(multicaller_address, swap_step_encoder));

    //preload state
    preload_market_state(client.clone(), vec![multicaller_address], vec![], vec![], market_state_instance.clone(), None).await?;

    //Preloading market
    preload_pools(client.clone(), market_instance.clone(), market_state_instance.clone()).await?;

    let market = market_instance.read().await;

    // Getting swap directions
    let pool_address: Address = UniswapV2PoolAddress::WETH_USDT;

    let pool = market.get_pool(&PoolId::Address(pool_address)).ok_or_eyre("POOL_NOT_FOUND")?;

    let swap_directions = pool.get_swap_directions();

    let mut btree_map: BTreeMap<PoolWrapper, Vec<(Address, Address)>> = BTreeMap::new();

    btree_map.insert(pool.clone(), swap_directions);

    //let swap_paths = market.build_swap_path_vec(&btree_map)?;

    let swap_paths = market.swap_paths_vec();

    let db = market_state_instance.read().await.state_db.clone();

    let mut env = Env::default();

    env.block.number = U256::from(block_header.number);
    env.block.timestamp = U256::from(block_header.timestamp);
    //env.block.basefee = U256::from(block_header.base_fee_per_gas.unwrap_or_default());

    let in_amount_f64 = 1.0;
    let in_amount = NWETH::from_float(in_amount_f64);

    let mut gas_used_map: HashMap<SwapLineDTO, u64> = HashMap::new();
    let mut calldata_map: HashMap<SwapLineDTO, Bytes> = HashMap::new();

    // Make tests

    for swap_path in swap_paths.iter() {
        if !swap_path.tokens[0].is_weth() {
            continue;
        }

        let sp = swap_path.clone();
        let sp_dto: SwapLineDTO = (&sp).into();
        println!("Checking {}", sp_dto);
        if let Some(filter) = &cli.filter.clone() {
            if !format!("{}", sp_dto).contains(filter) {
                println!("Skipping {}", sp_dto);
                continue;
            }
        }

        let mut swapline = SwapLine { path: sp, amount_in: SwapAmountType::Set(in_amount), ..SwapLine::default() };

        match swapline.calculate_with_in_amount(&db, env.clone(), in_amount) {
            Ok((out_amount, gas_used, _)) => {
                println!("{} gas: {}  amount {} -> {}", sp_dto, gas_used, in_amount_f64, NWETH::to_float(out_amount));
                swapline.amount_out = SwapAmountType::Set(out_amount)
            }
            Err(e) => {
                error!("calculate_with_in_amount error : {:?}", e);
            }
        }
        let swap = Swap::BackrunSwapLine(swapline);

        let calls = swap_encoder.make_calls(&swap)?;
        let (to, payload) = swap_encoder.encode_calls(calls)?;

        calldata_map.insert(swap_path.into(), payload.clone());

        let tx_request = TransactionRequest::default().to(to).from(operator_address).input(TransactionInput::new(payload));

        let gas_used = match client.estimate_gas(&tx_request).await {
            Ok(gas_needed) => {
                //info!("Gas required:  {gas_needed}");
                gas_needed
            }
            Err(e) => {
                error!("Gas estimation error for {sp_dto}, err={e}");
                0
            }
        };

        gas_used_map.insert(swap_path.into(), gas_used);
    }

    if let Some(bench_file) = cli.file {
        if cli.anvil {
            // Save anvil test data
            let mut calldata_vec: Vec<(SwapLineDTO, Bytes)> = calldata_map.into_iter().collect();
            calldata_vec.sort_by(|a, b| a.0.cmp(&b.0));
            let calldata_vec: Vec<(String, Bytes)> = calldata_vec.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            let test_data = create_sol_test(calldata_vec);
            println!("{}", test_data);
            let mut file = File::create(bench_file).await?;
            file.write_all(test_data.as_bytes()).await?;
        } else if cli.save {
            // Save benchmark results
            let results: Vec<(SwapLineDTO, u64)> = gas_used_map.into_iter().collect();

            let json_string = serde_json::to_string_pretty(&results)?;

            let mut file = File::create(bench_file).await?;
            file.write_all(json_string.as_bytes()).await?;
        } else {
            // Compare benchmark results
            let mut file = File::open(bench_file).await?;
            let mut json_string = String::new();
            file.read_to_string(&mut json_string).await?;

            let stored_results: Vec<(SwapLineDTO, u64)> = serde_json::from_str(&json_string)?;

            let stored_gas_map: HashMap<SwapLineDTO, u64> = stored_results.clone().into_iter().collect();

            for (current_entry, gas) in gas_used_map.iter() {
                match stored_gas_map.get(current_entry) {
                    Some(stored_gas) => {
                        let change_i: i64 = *gas as i64 - *stored_gas as i64;
                        let change = format!("{change_i}");

                        let change = if *gas < 40000 {
                            change.red()
                        } else {
                            match change_i {
                                i if i > 0 => change.red(),
                                i if i < 0 => change.green(),
                                _ => change.normal(),
                            }
                        };

                        println!("{} : {} {} - {} ", change, current_entry, gas, stored_gas,);
                    }
                    None => {
                        if *gas < 40000 {
                            println!("{} : {} {}", "FAILED".red(), current_entry, gas,);
                        } else {
                            println!("{} : {} {}", "NO_DATA".green(), current_entry, gas,);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
