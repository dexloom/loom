use alloy::primitives::BlockHash;
use alloy::primitives::ChainId;
use defi_address_book::TokenAddress;
use defi_entities::{AccountNonceAndBalanceState, BlockHistory, LatestBlock, Market, MarketState, Token};
use defi_events::{
    MarketEvents, MempoolEvents, MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate, MessageHealthEvent,
    MessageMempoolDataUpdate, MessageTxCompose, StateUpdateEvent, Task,
};
use defi_types::{ChainParameters, Mempool};
use influxdb::WriteQuery;
use loom_actors::{Broadcaster, SharedState};

#[derive(Clone)]
pub struct Blockchain {
    chain_id: ChainId,
    chain_parameters: ChainParameters,
    market: SharedState<Market>,
    latest_block: SharedState<LatestBlock>,
    market_state: SharedState<MarketState>,
    block_history_state: SharedState<BlockHistory>,
    mempool: SharedState<Mempool>,
    account_nonce_and_balance: SharedState<AccountNonceAndBalanceState>,

    new_block_headers_channel: Broadcaster<MessageBlockHeader>,
    new_block_with_tx_channel: Broadcaster<MessageBlock>,
    new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate>,
    new_block_logs_channel: Broadcaster<MessageBlockLogs>,
    new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate>,
    market_events_channel: Broadcaster<MarketEvents>,
    mempool_events_channel: Broadcaster<MempoolEvents>,
    pool_health_monitor_channel: Broadcaster<MessageHealthEvent>,
    compose_channel: Broadcaster<MessageTxCompose>,
    state_update_channel: Broadcaster<StateUpdateEvent>,
    influxdb_write_channel: Broadcaster<WriteQuery>,
    tasks_channel: Broadcaster<Task>,
}

impl Blockchain {
    pub fn new(chain_id: ChainId) -> Blockchain {
        let new_block_headers_channel: Broadcaster<MessageBlockHeader> = Broadcaster::new(10);
        let new_block_with_tx_channel: Broadcaster<MessageBlock> = Broadcaster::new(10);
        let new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate> = Broadcaster::new(10);
        let new_block_logs_channel: Broadcaster<MessageBlockLogs> = Broadcaster::new(10);

        let new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate> = Broadcaster::new(5000);

        let market_events_channel: Broadcaster<MarketEvents> = Broadcaster::new(100);
        let mempool_events_channel: Broadcaster<MempoolEvents> = Broadcaster::new(2000);
        let pool_health_monitor_channel: Broadcaster<MessageHealthEvent> = Broadcaster::new(1000);
        let compose_channel: Broadcaster<MessageTxCompose> = Broadcaster::new(100);
        let state_update_channel: Broadcaster<StateUpdateEvent> = Broadcaster::new(100);
        let influx_write_channel: Broadcaster<WriteQuery> = Broadcaster::new(1000);
        let tasks_channel: Broadcaster<Task> = Broadcaster::new(1000);

        let mut market_instance = Market::default();

        let weth_token = Token::new_with_data(TokenAddress::WETH, Some("WETH".to_string()), None, Some(18), true, false);
        let usdc_token = Token::new_with_data(TokenAddress::USDC, Some("USDC".to_string()), None, Some(6), true, false);
        let usdt_token = Token::new_with_data(TokenAddress::USDT, Some("USDT".to_string()), None, Some(6), true, false);
        let dai_token = Token::new_with_data(TokenAddress::DAI, Some("DAI".to_string()), None, Some(18), true, false);
        let wbtc_token = Token::new_with_data(TokenAddress::WBTC, Some("WBTC".to_string()), None, Some(8), true, false);
        let threecrv_token = Token::new_with_data(TokenAddress::THREECRV, Some("3Crv".to_string()), None, Some(18), false, true);

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
            influxdb_write_channel: influx_write_channel,
            tasks_channel,
        }
    }

    pub fn with_market_state(&self, market_state: MarketState) -> Blockchain {
        Blockchain { market_state: SharedState::new(market_state), ..self.clone() }
    }

    pub fn chain_id(&self) -> u64 {
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

    pub fn mempool(&self) -> SharedState<Mempool> {
        self.mempool.clone()
    }

    pub fn nonce_and_balance(&self) -> SharedState<AccountNonceAndBalanceState> {
        self.account_nonce_and_balance.clone()
    }

    pub fn new_block_headers_channel(&self) -> Broadcaster<MessageBlockHeader> {
        self.new_block_headers_channel.clone()
    }

    pub fn new_block_with_tx_channel(&self) -> Broadcaster<MessageBlock> {
        self.new_block_with_tx_channel.clone()
    }

    pub fn new_block_state_update_channel(&self) -> Broadcaster<MessageBlockStateUpdate> {
        self.new_block_state_update_channel.clone()
    }

    pub fn new_block_logs_channel(&self) -> Broadcaster<MessageBlockLogs> {
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

    pub fn influxdb_write_channel(&self) -> Broadcaster<WriteQuery> {
        self.influxdb_write_channel.clone()
    }

    pub fn tasks_channel(&self) -> Broadcaster<Task> {
        self.tasks_channel.clone()
    }
}
