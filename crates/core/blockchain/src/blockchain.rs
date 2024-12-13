use alloy::primitives::BlockHash;
use alloy::primitives::ChainId;
use influxdb::WriteQuery;
use loom_core_actors::{Broadcaster, SharedState};
use loom_defi_address_book::TokenAddressEth;
use loom_types_blockchain::{ChainParameters, Mempool};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{AccountNonceAndBalanceState, LatestBlock, Market, Token};
use loom_types_events::{
    LoomTask, MarketEvents, MempoolEvents, MessageBlock, MessageBlockHeader, MessageBlockLogs, MessageBlockStateUpdate, MessageHealthEvent,
    MessageMempoolDataUpdate, MessageTxCompose,
};

#[derive(Clone)]
pub struct Blockchain<LDT: LoomDataTypes + 'static = LoomDataTypesEthereum> {
    chain_id: ChainId,
    chain_parameters: ChainParameters,
    market: SharedState<Market<LDT>>,
    latest_block: SharedState<LatestBlock<LDT>>,
    mempool: SharedState<Mempool<LDT>>,
    account_nonce_and_balance: SharedState<AccountNonceAndBalanceState<LDT>>,

    new_block_headers_channel: Broadcaster<MessageBlockHeader<LDT>>,
    new_block_with_tx_channel: Broadcaster<MessageBlock<LDT>>,
    new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate<LDT>>,
    new_block_logs_channel: Broadcaster<MessageBlockLogs<LDT>>,
    new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate<LDT>>,
    market_events_channel: Broadcaster<MarketEvents<LDT>>,
    mempool_events_channel: Broadcaster<MempoolEvents<LDT>>,
    tx_compose_channel: Broadcaster<MessageTxCompose<LDT>>,

    pool_health_monitor_channel: Broadcaster<MessageHealthEvent<LDT>>,
    influxdb_write_channel: Broadcaster<WriteQuery>,
    tasks_channel: Broadcaster<LoomTask>,
}

impl Blockchain<LoomDataTypesEthereum> {
    pub fn new(chain_id: ChainId) -> Blockchain<LoomDataTypesEthereum> {
        let new_block_headers_channel: Broadcaster<MessageBlockHeader> = Broadcaster::new(10);
        let new_block_with_tx_channel: Broadcaster<MessageBlock> = Broadcaster::new(10);
        let new_block_state_update_channel: Broadcaster<MessageBlockStateUpdate> = Broadcaster::new(10);
        let new_block_logs_channel: Broadcaster<MessageBlockLogs> = Broadcaster::new(10);

        let new_mempool_tx_channel: Broadcaster<MessageMempoolDataUpdate> = Broadcaster::new(5000);

        let market_events_channel: Broadcaster<MarketEvents> = Broadcaster::new(100);
        let mempool_events_channel: Broadcaster<MempoolEvents> = Broadcaster::new(2000);
        let tx_compose_channel: Broadcaster<MessageTxCompose> = Broadcaster::new(2000);

        let pool_health_monitor_channel: Broadcaster<MessageHealthEvent> = Broadcaster::new(1000);
        let influx_write_channel: Broadcaster<WriteQuery> = Broadcaster::new(1000);
        let tasks_channel: Broadcaster<LoomTask> = Broadcaster::new(1000);

        let mut market_instance = Market::default();

        let weth_token = Token::new_with_data(TokenAddressEth::WETH, Some("WETH".to_string()), None, Some(18), true, false);
        let usdc_token = Token::new_with_data(TokenAddressEth::USDC, Some("USDC".to_string()), None, Some(6), true, false);
        let usdt_token = Token::new_with_data(TokenAddressEth::USDT, Some("USDT".to_string()), None, Some(6), true, false);
        let dai_token = Token::new_with_data(TokenAddressEth::DAI, Some("DAI".to_string()), None, Some(18), true, false);
        let wbtc_token = Token::new_with_data(TokenAddressEth::WBTC, Some("WBTC".to_string()), None, Some(8), true, false);
        let threecrv_token = Token::new_with_data(TokenAddressEth::THREECRV, Some("3Crv".to_string()), None, Some(18), false, true);

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
            mempool: SharedState::new(Mempool::<LoomDataTypesEthereum>::new()),
            latest_block: SharedState::new(LatestBlock::new(0, BlockHash::ZERO)),
            account_nonce_and_balance: SharedState::new(AccountNonceAndBalanceState::new()),
            new_block_headers_channel,
            new_block_with_tx_channel,
            new_block_state_update_channel,
            new_block_logs_channel,
            new_mempool_tx_channel,
            market_events_channel,
            mempool_events_channel,
            pool_health_monitor_channel,
            tx_compose_channel,
            influxdb_write_channel: influx_write_channel,
            tasks_channel,
        }
    }
}

impl<LDT: LoomDataTypes> Blockchain<LDT> {
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn chain_parameters(&self) -> ChainParameters {
        self.chain_parameters.clone()
    }

    pub fn market(&self) -> SharedState<Market<LDT>> {
        self.market.clone()
    }

    pub fn latest_block(&self) -> SharedState<LatestBlock<LDT>> {
        self.latest_block.clone()
    }

    pub fn mempool(&self) -> SharedState<Mempool<LDT>> {
        self.mempool.clone()
    }

    pub fn nonce_and_balance(&self) -> SharedState<AccountNonceAndBalanceState<LDT>> {
        self.account_nonce_and_balance.clone()
    }

    pub fn new_block_headers_channel(&self) -> Broadcaster<MessageBlockHeader<LDT>> {
        self.new_block_headers_channel.clone()
    }

    pub fn new_block_with_tx_channel(&self) -> Broadcaster<MessageBlock<LDT>> {
        self.new_block_with_tx_channel.clone()
    }

    pub fn new_block_state_update_channel(&self) -> Broadcaster<MessageBlockStateUpdate<LDT>> {
        self.new_block_state_update_channel.clone()
    }

    pub fn new_block_logs_channel(&self) -> Broadcaster<MessageBlockLogs<LDT>> {
        self.new_block_logs_channel.clone()
    }

    pub fn new_mempool_tx_channel(&self) -> Broadcaster<MessageMempoolDataUpdate<LDT>> {
        self.new_mempool_tx_channel.clone()
    }

    pub fn market_events_channel(&self) -> Broadcaster<MarketEvents<LDT>> {
        self.market_events_channel.clone()
    }

    pub fn mempool_events_channel(&self) -> Broadcaster<MempoolEvents<LDT>> {
        self.mempool_events_channel.clone()
    }

    pub fn tx_compose_channel(&self) -> Broadcaster<MessageTxCompose<LDT>> {
        self.tx_compose_channel.clone()
    }

    pub fn pool_health_monitor_channel(&self) -> Broadcaster<MessageHealthEvent<LDT>> {
        self.pool_health_monitor_channel.clone()
    }

    pub fn influxdb_write_channel(&self) -> Broadcaster<WriteQuery> {
        self.influxdb_write_channel.clone()
    }

    pub fn tasks_channel(&self) -> Broadcaster<LoomTask> {
        self.tasks_channel.clone()
    }
}
