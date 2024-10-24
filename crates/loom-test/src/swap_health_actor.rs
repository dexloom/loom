use alloy_network::Ethereum;
use alloy_primitives::{address, Address, U160, U256};
use alloy_provider::Provider;
use alloy_rpc_types::BlockId;
use alloy_transport::Transport;
use debug_provider::DebugProviderExt;
use defi_abi::uniswap3::IUniswapV3Pool;
use defi_abi::uniswap_periphery::IQuoterV2;
use defi_abi::uniswap_periphery::IQuoterV2::{QuoteExactInputSingleParams, QuoteExactOutputSingleParams};
use defi_address_book::PeripheryAddress;
use defi_blockchain::Blockchain;
use defi_entities::{Market, MarketState};
use defi_events::StateUpdateEvent;
use eyre::eyre;
use loom_actors::{subscribe, Actor, ActorResult, Broadcaster, SharedState, WorkerResult};
use revm::primitives::Env;
use std::marker::PhantomData;
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info};

const GOAT_POOL_ADDRESS: Address = address!("8682fc63dc2525fd2e5ed4e28e207a2fd9f36dab");

async fn fetch_original_contract_amounts<P, T>(
    client: P,
    pool_address: Address,
    token_in: Address,
    token_out: Address,
    amount: U256,
    block_number: u64,
    is_amount_out: bool,
) -> eyre::Result<U256>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    let router_contract = IQuoterV2::new(PeripheryAddress::UNISWAP_V3_QUOTER_V2, client.clone());
    let pool_contract = IUniswapV3Pool::new(pool_address, client.clone());
    let pool_fee = pool_contract.fee().call().block(BlockId::from(block_number)).await?._0;

    if is_amount_out {
        let contract_amount_out = router_contract
            .quoteExactInputSingle(QuoteExactInputSingleParams {
                tokenIn: token_in,
                tokenOut: token_out,
                amountIn: amount,
                fee: pool_fee,
                sqrtPriceLimitX96: U160::ZERO,
            })
            .call()
            .block(BlockId::from(block_number))
            .await?;
        Ok(contract_amount_out.amountOut)
    } else {
        let contract_amount_in = router_contract
            .quoteExactOutputSingle(QuoteExactOutputSingleParams {
                tokenIn: token_in,
                tokenOut: token_out,
                amount,
                fee: pool_fee,
                sqrtPriceLimitX96: U160::ZERO,
            })
            .call()
            .block(BlockId::from(block_number))
            .await?;
        Ok(contract_amount_in.amountIn)
    }
}

async fn test_goat_swap<P, T>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    block_number: u64,
) -> eyre::Result<()>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    let market_lock = market.read().await;
    let pool = market_lock.get_pool(&GOAT_POOL_ADDRESS).unwrap();
    let state_db = market_state.read().await.state_db.clone();

    let (token0, token1) = (*pool.get_tokens().first().unwrap(), *pool.get_tokens().last().unwrap());

    //// CASE: token1 -> token0
    let amount_in = U256::from(193_399_997_926_998_016u128);
    let contract_amount_out =
        fetch_original_contract_amounts(client.clone(), GOAT_POOL_ADDRESS, token1, token0, amount_in, block_number, true).await?;

    // under test
    let (amount_out, _gas_used) = match pool.calculate_out_amount(&state_db, Env::default(), &token1, &token0, amount_in) {
        Ok((amount_out, gas_used)) => (amount_out, gas_used),
        Err(e) => {
            error!("Calculation error for pool={:?}, amount_in={}, e={:?}", GOAT_POOL_ADDRESS, amount_in, e);
            return Ok(());
        }
    };
    if amount_out != contract_amount_out {
        error!("Missmatch for pool={:?}, token_out={}, amount_in={}", GOAT_POOL_ADDRESS, token0, amount_in);
    } else {
        info!("Goat ok!");
    }

    Ok(())
}

async fn swap_health_monitor_worker<P, T>(
    client: P,
    market: SharedState<Market>,
    market_state: SharedState<MarketState>,
    state_update_event_rx: Broadcaster<StateUpdateEvent>,
) -> WorkerResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    subscribe!(state_update_event_rx);

    loop {
        let msg = match state_update_event_rx.recv().await {
            Ok(msg) => msg,
            Err(e) => match e {
                RecvError::Closed => {
                    error!("State update channel closed");
                    return Err(eyre!("STATE_UPDATE_RX_CLOSED"));
                }
                RecvError::Lagged(lag) => {
                    error!("State update channel lagged by {} messages", lag);
                    continue;
                }
            },
        };
        if market.read().await.get_pool(&GOAT_POOL_ADDRESS).is_none() {
            info!("Goat pool not found");
            continue;
        }
        info!("Goat pool found block_number={}", msg.next_block_number - 1);

        if let Err(e) = test_goat_swap(client.clone(), market.clone(), market_state.clone(), msg.next_block_number - 1).await {
            error!("Error in test_goat_swap: {:?}", e);
        }
    }
}

pub struct SwapHealthMonitorActor<P, T> {
    client: P,
    market: Option<SharedState<Market>>,
    market_state: Option<SharedState<MarketState>>,
    state_update_event_rx: Option<Broadcaster<StateUpdateEvent>>,
    _t: PhantomData<T>,
}

impl<P, T> SwapHealthMonitorActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(client: P) -> SwapHealthMonitorActor<P, T> {
        SwapHealthMonitorActor { client, market: None, market_state: None, state_update_event_rx: None, _t: PhantomData }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            market: Some(bc.market()),
            market_state: Some(bc.market_state()),
            state_update_event_rx: Some(bc.state_update_channel()),
            ..self
        }
    }
}

impl<P, T> Actor for SwapHealthMonitorActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(swap_health_monitor_worker(
            self.client.clone(),
            self.market.clone().unwrap(),
            self.market_state.clone().unwrap(),
            self.state_update_event_rx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SwapHealthMonitorActor"
    }
}
