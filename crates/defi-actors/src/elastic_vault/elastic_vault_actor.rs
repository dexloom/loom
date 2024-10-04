use alloy_eips::BlockId;
use alloy_network::{Ethereum, Network};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::TransactionBuilder;
use alloy_primitives::aliases::U24;
use alloy_primitives::private::alloy_rlp::Decodable;
use alloy_primitives::utils::{format_units, parse_units};
use alloy_primitives::{address, Address, Bytes, I256, U160, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{Transaction, TransactionRequest};
use alloy_sol_types::{SolCall, SolInterface, SolValue};
use alloy_transport::Transport;
use chrono::Utc;
use debug_provider::DebugProviderExt;
use defi_abi::elastic_vault::ampl_rebaser::AMPLRebaser::{rebaseCall, AMPLRebaserInstance};
use defi_abi::uniswap_periphery::IQuoterV2;
use defi_abi::uniswap_periphery::IQuoterV2::QuoteExactInputSingleParams;
use defi_abi::IERC20::IERC20Instance;
use defi_blockchain::Blockchain;
use defi_entities::{AccountNonceAndBalanceState, LatestBlock, Swap};
use defi_events::{BlockHeader, MessageBlockHeader, MessageMempoolDataUpdate, MessageTxCompose, TxComposeData};
use defi_pools::state_readers::UniswapV3QuoterStateReader;
use defi_pools::QUOTER_ADDRESS;
use eyre::eyre;
use lazy_static::lazy_static;
use log::{error, info};
use loom_actors::Consumer;
use loom_actors::Producer;
use loom_actors::{run_async, subscribe, Actor, ActorResult, Broadcaster, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_utils::evm::evm_call;
use loom_utils::tokens::WETH_ADDRESS;
use reth_primitives::revm_primitives::{BlockEnv, SHANGHAI};
use revm::db::{AlloyDB, CacheDB};
use revm::primitives::Env;
use revm::Evm;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Sender;
use tokio::sync::RwLock;

lazy_static! {
    pub static ref ELASTIC_FINANCE_TOKEN: Address = address!("857FfC55B1Aa61A7fF847C82072790cAE73cd883");
    pub static ref AMPL_TOKEN: Address = address!("D46bA6D942050d489DBd938a2C909A5d5039A161");
    pub static ref ELASTIC_VAULT_ADDRESS: Address = address!("5557f095556feb36c725f1ffe94a97631adeb770");
}

pub fn fork_db_alloy<T: Clone + Transport, N: Network, P: Provider<T, N>>(provider: P, block_id: BlockId) -> CacheDB<AlloyDB<T, N, P>> {
    CacheDB::new(AlloyDB::new(provider, block_id).unwrap())
}

pub async fn elastic_vault_worker<T, P>(
    client: P,
    block_header_rx: Broadcaster<MessageBlockHeader>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Clone + 'static,
{
    info!("Starting Elastic Vault strategy...");

    subscribe!(block_header_rx);

    let mut last_processed_block_number = 0u64;
    loop {
        let block_header = match block_header_rx.recv().await {
            Ok(b) => b.inner,
            Err(e) => match e {
                RecvError::Closed => {
                    error!("Block header channel closed");
                    break Err(eyre!("BLOCK_HEADER_RX_CLOSED"));
                }
                RecvError::Lagged(lag) => {
                    error!("Block header  channel lagged by {} messages", lag);
                    continue;
                }
            },
        };

        // do not process out-dated-blocks
        if last_processed_block_number > block_header.header.number {
            info!(
                "Ignore outdated block: last_processed_block_number={}, latest_block_number={}",
                last_processed_block_number, block_header.header.number
            );
            continue;
        }
        last_processed_block_number = block_header.header.number;

        let process_result = process_new_block(client.clone(), block_header.clone(), compose_channel_tx.clone()).await;
        if let Err(e) = process_result {
            info!("Process new block error: block={}, {:?}", block_header.header.number, e);
            continue;
        }
    }
}

pub async fn process_new_block<T, P>(
    client: P,
    block_header: BlockHeader,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> eyre::Result<()>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Clone + 'static,
{
    let fixed_bribe = parse_units("1", "gwei")?.get_absolute().to::<u128>();

    let mut mintable = false;

    let contract = AMPLRebaserInstance::new(*ELASTIC_VAULT_ADDRESS, client.clone());
    let last_rebase_call = contract.last_rebase_call().call().block(BlockId::number(block_header.header.number)).await?;
    let next_timeslot = last_rebase_call.last_rebase_call.to::<u64>() + 24 * 60 * 60;
    if next_timeslot > block_header.next_block_timestamp {
        info!(
            "Rebase call was made less than 24 hours ago, next_block={}, last={}, diff={}",
            block_header.next_block_number,
            last_rebase_call.last_rebase_call.to::<u64>(),
            next_timeslot - block_header.next_block_timestamp
        );
    } else {
        info!(
            "Rebase call was made more than 24 hours ago, next_block={}, last={}, diff={}",
            block_header.next_block_number,
            last_rebase_call.last_rebase_call.to::<u64>(),
            block_header.next_block_timestamp - next_timeslot
        );
        mintable = true;
    }

    let last_ampl_supply = contract.last_ampl_supply().call().block(BlockId::number(block_header.header.number)).await?;
    let token_contract = IERC20Instance::new(*AMPL_TOKEN, client.clone());
    let total_supply = token_contract.totalSupply().block(BlockId::number(block_header.header.number)).call().await?;
    if last_ampl_supply.last_ampl_supply == total_supply._0 {
        info!(
            "Last AMPL supply is the same as total supply, skipping latest_block={}, last_ampl_supply={}, total_supply={}",
            block_header.header.number, last_ampl_supply.last_ampl_supply, total_supply._0
        );
    } else {
        info!(
            "Last AMPL supply is different from total supply, latest_block={}, last_ampl_supply={}, total_supply={}",
            block_header.header.number, last_ampl_supply.last_ampl_supply, total_supply._0
        );
        mintable = true;
    }
    if !mintable {
        return Ok(());
    }

    let tx_gas = 43121u128;
    let tx_cost = tx_gas * (block_header.next_block_base_fee + fixed_bribe);
    info!("Tx cost: {:?}", format_units(tx_cost, "ether").unwrap_or_default());
    info!(
        "next_block={:?}, next_base_fee={:?}",
        block_header.header.number,
        format_units(U256::from(block_header.next_block_base_fee), "gwei").unwrap_or_default()
    );
    let amount = parse_units("0.025", "ether")?.get_absolute();

    let fork_db = fork_db_alloy(client.clone(), BlockId::number(block_header.header.number));
    let env = Env {
        block: BlockEnv {
            number: U256::from(block_header.next_block_number),
            timestamp: U256::from(block_header.next_block_timestamp),
            ..BlockEnv::default()
        },
        ..Env::default()
    };
    let (token_value, _gas_used) = UniswapV3QuoterStateReader::quote_exact_input(
        &fork_db,
        env.clone(),
        *QUOTER_ADDRESS,
        *ELASTIC_FINANCE_TOKEN,
        WETH_ADDRESS,
        U24::from(10000),
        amount,
    )?;

    let (_value, gas_used) = evm_call(fork_db, env, *ELASTIC_VAULT_ADDRESS, rebaseCall {}.abi_encode())?;

    let bribe_result = calc_bribe_fixed_prio_fee(gas_used as u128, token_value, block_header.next_block_base_fee, fixed_bribe);

    let extra_gas = (gas_used as f64 * 1.2) as u128;

    if bribe_result.real_profit <= I256::ZERO {
        info!("Skipping, not profitable: real_profit={} eth", format_units(bribe_result.real_profit, "ether")?);
        return Ok(());
    }

    let tx = TransactionRequest::default()
        .transaction_type(2)
        .with_to(*ELASTIC_VAULT_ADDRESS)
        .with_max_priority_fee_per_gas(fixed_bribe)
        .with_value(U256::from(0))
        .with_input(Bytes::from(rebaseCall {}.abi_encode()))
        .gas_limit(extra_gas);

    let tx_compose = TxComposeData {
        signer: None,
        nonce: 0,
        eth_balance: Default::default(),
        value: Default::default(),
        gas: 0,
        gas_fee: 0,
        priority_gas_fee: 0,
        stuffing_txs_hashes: vec![],
        stuffing_txs: vec![],
        block: 0,
        block_timestamp: 0,
        swap: Swap::None,
        opcodes: None,
        tx_bundle: None,
        rlp_bundle: None,
        prestate: None,
        poststate: None,
        poststate_update: None,
        origin: None,
        tips_pct: None,
        tips: None,
    };

    run_async!(compose_channel_tx.send(MessageTxCompose::encode(tx_compose)));

    Ok(())
}

pub struct BribeResult {
    pub raw_profit: I256,
    pub real_profit: I256,
    pub bribe_priority_fee: u128,
    pub total_cost: U256,
}

pub fn calc_bribe_fixed_prio_fee(gas_used: u128, token_value: U256, next_block_base_fee: u128, bribe_priority_fee: u128) -> BribeResult {
    let base_fee = gas_used * next_block_base_fee;
    let raw_profit = I256::from_limbs(*token_value.as_limbs()) - I256::try_from(base_fee).unwrap();
    if raw_profit <= I256::ZERO {
        return BribeResult { raw_profit, real_profit: raw_profit, bribe_priority_fee: 0, total_cost: U256::try_from(base_fee).unwrap() };
    }

    let total_cost = base_fee + bribe_priority_fee * gas_used;

    BribeResult {
        raw_profit,
        real_profit: I256::from_limbs(*token_value.as_limbs()) - I256::try_from(total_cost).unwrap(),
        bribe_priority_fee,
        total_cost: U256::from(total_cost),
    }
}

#[derive(Accessor, Consumer, Producer)]
pub struct ElasticVaultActor<P, T> {
    client: P,
    #[consumer]
    block_header_rx: Option<Broadcaster<MessageBlockHeader>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,

    _t: PhantomData<T>,
}

impl<P, T> ElasticVaultActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> Self {
        Self { client, block_header_rx: None, compose_channel_tx: None, _t: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { block_header_rx: Some(bc.new_block_headers_channel()), compose_channel_tx: Some(bc.compose_channel()), ..self }
    }
}

impl<P, T> Actor for ElasticVaultActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(elastic_vault_worker(
            self.client.clone(),
            self.block_header_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "ElasticVaultActor"
    }
}
