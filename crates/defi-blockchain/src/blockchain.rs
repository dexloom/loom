use alloy::{
    primitives::{Address, BlockHash},
    rpc::types::{Block, Header},
};

use defi_entities::{AccountNonceAndBalanceState, BlockHistory, GasStation, LatestBlock, Market, MarketState, Token};
use defi_events::{
    BlockLogs, BlockStateUpdate, MarketEvents, MempoolEvents, MessageHealthEvent, MessageMempoolDataUpdate, MessageTxCompose,
    StateUpdateEvent,
};
use defi_types::{ChainParameters, Mempool};
use loom_actors::{Broadcaster, SharedState};

#[derive(Clone)]
pub struct Blockchain {
    chain_id: i64,
    chain_parameters: ChainParameters,
    market: SharedState<Market>,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState>,
    gas_station_state: SharedState<GasStation>,
    block_history_state: SharedState<BlockHistory>,
    mempool: SharedState<Mempool>,
    account_nonce_and_balance: SharedState<AccountNonceAndBalanceState>,

    new_block_headers_channel: Broadcaster<Header>,
    new_block_with_tx_channel: Broadcaster<Block>,
    new_block_state_update_channel: Broadcaster<BlockStateUpdate>,
    new_block_logs_channel: Broadcaster<BlockLogs>,
    new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate>,
    market_events_channel: Broadcaster<MarketEvents>,
    mempool_events_channel: Broadcaster<MempoolEvents>,
    pool_health_monitor_channel: Broadcaster<MessageHealthEvent>,
    compose_channel: Broadcaster<MessageTxCompose>,
    state_update_channel: Broadcaster<StateUpdateEvent>,
}

impl Blockchain {
    pub fn new(chain_id: i64) -> Blockchain {
        let new_block_headers_channel: Broadcaster<Header> = Broadcaster::new(10);
        let new_block_with_tx_channel: Broadcaster<Block> = Broadcaster::new(10);
        let new_block_state_update_channel: Broadcaster<BlockStateUpdate> = Broadcaster::new(10);
        let new_block_logs_channel: Broadcaster<BlockLogs> = Broadcaster::new(10);

        let new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate> = Broadcaster::new(1000);

        let market_events_channel: Broadcaster<MarketEvents> = Broadcaster::new(1000);
        let mempool_events_channel: Broadcaster<MempoolEvents> = Broadcaster::new(1000);
        let pool_health_monitor_channel: Broadcaster<MessageHealthEvent> = Broadcaster::new(1000);
        let compose_channel: Broadcaster<MessageTxCompose> = Broadcaster::new(1000);
        let state_update_channel: Broadcaster<StateUpdateEvent> = Broadcaster::new(1000);

        let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let usdc_address: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
        let usdt_address: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap();
        let dai_address: Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap();
        let wbtc_address: Address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap();
        let threecrv_address: Address = "0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490".parse().unwrap();

        let mut market_instance = Market::default();

        let weth_token = Token::new_with_data(weth_address, Some("WETH".to_string()), None, Some(18), true, false);
        let usdc_token = Token::new_with_data(usdc_address, Some("USDC".to_string()), None, Some(6), true, false);
        let usdt_token = Token::new_with_data(usdt_address, Some("USDT".to_string()), None, Some(6), true, false);
        let dai_token = Token::new_with_data(dai_address, Some("DAI".to_string()), None, Some(18), true, false);
        let wbtc_token = Token::new_with_data(wbtc_address, Some("WBTC".to_string()), None, Some(8), true, false);
        let threecrv_token = Token::new_with_data(threecrv_address, Some("3Crv".to_string()), None, Some(18), false, true);

        market_instance.add_token(weth_token).unwrap();
        market_instance.add_token(usdc_token).unwrap();
        market_instance.add_token(usdt_token).unwrap();
        market_instance.add_token(dai_token).unwrap();
        market_instance.add_token(wbtc_token).unwrap();
        market_instance.add_token(threecrv_token).unwrap();

        Blockchain {
            chain_id,
            chain_parameters: ChainParameters::ethereum(),
            market: SharedState::new(market_instance),
            market_state: SharedState::new(MarketState::new(Default::default())),
            mempool: SharedState::new(Mempool::new()),
            latest_block: SharedState::new(LatestBlock::new(0, BlockHash::ZERO)),
            block_history_state: SharedState::new(BlockHistory::new(10)),
            gas_station_state: SharedState::new(GasStation::new()),
            account_nonce_and_balance: SharedState::new(AccountNonceAndBalanceState::new()),
            new_block_headers_channel,
            new_block_with_tx_channel,
            new_block_state_update_channel,
            new_block_logs_channel,
            new_mempool_tx_channel,
            market_events_channel,
            mempool_events_channel,
            pool_health_monitor_channel,
            compose_channel,
            state_update_channel,
        }
    }

    pub fn chain_id(&self) -> i64 {
        self.chain_id
    }

    pub fn chain_parameters(&self) -> ChainParameters {
        self.chain_parameters.clone()
    }

    pub fn market(&self) -> SharedState<Market> {
        self.market.clone()
    }

    pub fn latest_block(&self) -> SharedState<LatestBlock> {
        self.latest_block.clone()
    }

    pub fn market_state(&self) -> SharedState<MarketState> {
        self.market_state.clone()
    }

    pub fn block_history(&self) -> SharedState<BlockHistory> {
        self.block_history_state.clone()
    }

    pub fn gas_station(&self) -> SharedState<GasStation> {
        self.gas_station_state.clone()
    }

    pub fn mempool(&self) -> SharedState<Mempool> {
        self.mempool.clone()
    }

    pub fn nonce_and_balance(&self) -> SharedState<AccountNonceAndBalanceState> {
        self.account_nonce_and_balance.clone()
    }

    pub fn new_block_headers_channel(&self) -> Broadcaster<Header> {
        self.new_block_headers_channel.clone()
    }

    pub fn new_block_with_tx_channel(&self) -> Broadcaster<Block> {
        self.new_block_with_tx_channel.clone()
    }

    pub fn new_block_state_update_channel(&self) -> Broadcaster<BlockStateUpdate> {
        self.new_block_state_update_channel.clone()
    }

    pub fn new_block_logs_channel(&self) -> Broadcaster<BlockLogs> {
        self.new_block_logs_channel.clone()
    }

    pub fn new_mempool_tx_channel(&self) -> Broadcaster<MessageMempoolDataUpdate> {
        self.new_mempool_tx_channel.clone()
    }

    pub fn market_events_channel(&self) -> Broadcaster<MarketEvents> {
        self.market_events_channel.clone()
    }

    pub fn mempool_events_channel(&self) -> Broadcaster<MempoolEvents> {
        self.mempool_events_channel.clone()
    }
    pub fn pool_health_monitor_channel(&self) -> Broadcaster<MessageHealthEvent> {
        self.pool_health_monitor_channel.clone()
    }

    pub fn compose_channel(&self) -> Broadcaster<MessageTxCompose> {
        self.compose_channel.clone()
    }

    pub fn state_update_channel(&self) -> Broadcaster<StateUpdateEvent> {
        self.state_update_channel.clone()
    }
}
