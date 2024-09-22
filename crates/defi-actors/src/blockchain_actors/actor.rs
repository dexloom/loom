use std::marker::PhantomData;
use std::sync::Arc;

use crate::backrun::BlockStateChangeProcessorActor;
use crate::{
    ArbSwapPathMergerActor, BlockHistoryActor, DiffPathMergerActor, EvmEstimatorActor, FlashbotsBroadcastActor, GethEstimatorActor,
    HistoryPoolLoaderActor, InitializeSignersOneShotActor, MarketStatePreloadedOneShotActor, MempoolActor, NewPoolLoaderActor,
    NodeBlockActor, NodeBlockActorConfig, NodeExExGrpcActor, NodeMempoolActor, NonceAndBalanceMonitorActor,
    PendingTxStateChangeProcessorActor, PoolHealthMonitorActor, PriceActor, ProtocolPoolLoaderActor, RequiredPoolLoaderActor,
    SamePathMergerActor, StateChangeArbSearcherActor, StateHealthMonitorActor, SwapEncoderActor, TxSignersActor,
};
use alloy_network::Ethereum;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::Provider;
use alloy_transport::Transport;
use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::required_state::RequiredState;
use defi_entities::{PoolClass, TxSigners};
use eyre::{eyre, Result};
use flashbots::client::RelayConfig;
use flashbots::Flashbots;
use loom_actors::{Actor, ActorsManager, SharedState};
use loom_multicaller::MulticallerSwapEncoder;
use loom_utils::tokens::{ETH_NATIVE_ADDRESS, WETH_ADDRESS};
use loom_utils::NWETH;

pub struct BlockchainActors<P, T> {
    provider: P,
    bc: Blockchain,
    pub signers: SharedState<TxSigners>,
    actor_manager: ActorsManager,
    encoder: Option<MulticallerSwapEncoder>,
    has_mempool: bool,
    has_state_update: bool,
    has_signers: bool,
    mutlicaller_address: Option<Address>,
    relays: Vec<RelayConfig>,
    _t: PhantomData<T>,
}

impl<P, T> BlockchainActors<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(provider: P, bc: Blockchain, relays: Vec<RelayConfig>) -> Self {
        Self {
            provider,
            bc,
            signers: SharedState::new(TxSigners::new()),
            actor_manager: ActorsManager::new(),
            encoder: None,
            has_mempool: false,
            has_state_update: false,
            has_signers: false,
            mutlicaller_address: None,
            relays,
            _t: PhantomData,
        }
    }

    pub async fn wait(self) {
        self.actor_manager.wait().await
    }

    /// Start a custom actor
    pub fn start(&mut self, actor: impl Actor + 'static) -> Result<&mut Self> {
        self.actor_manager.start(actor)?;
        Ok(self)
    }

    /// Start a custom actor and wait for it to finish
    pub fn start_and_wait(&mut self, actor: impl Actor + Send + Sync + 'static) -> Result<&mut Self> {
        self.actor_manager.start_and_wait(actor)?;
        Ok(self)
    }

    /// Initialize signers with the default anvil Private Key
    pub fn initialize_signers_with_anvil(&mut self) -> Result<&mut Self> {
        let key: B256 = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".parse()?;

        self.actor_manager
            .start_and_wait(InitializeSignersOneShotActor::new(Some(key.to_vec())).with_signers(self.signers.clone()).on_bc(&self.bc))?;
        self.with_signers()?;
        Ok(self)
    }

    /// Initialize signers with the private key. Random key generated if param in None
    pub fn initialize_signers_with_key(&mut self, key: Option<Vec<u8>>) -> Result<&mut Self> {
        self.actor_manager.start_and_wait(InitializeSignersOneShotActor::new(key).with_signers(self.signers.clone()).on_bc(&self.bc))?;
        self.with_signers()?;
        Ok(self)
    }

    /// Initialize signers with multiple private keys
    pub fn initialize_signers_with_keys(&mut self, keys: Vec<Vec<u8>>) -> Result<&mut Self> {
        for key in keys {
            self.actor_manager
                .start_and_wait(InitializeSignersOneShotActor::new(Some(key)).with_signers(self.signers.clone()).on_bc(&self.bc))?;
        }
        self.with_signers()?;
        Ok(self)
    }

    /// Initialize signers with encrypted private key
    pub fn initialize_signers_with_encrypted_key(&mut self, key: Vec<u8>) -> Result<&mut Self> {
        self.actor_manager.start_and_wait(
            InitializeSignersOneShotActor::new_from_encrypted_key(key).with_signers(self.signers.clone()).on_bc(&self.bc),
        )?;
        self.with_signers()?;
        Ok(self)
    }

    /// Initializes signers with encrypted key form DATA env var
    pub fn initialize_signers_with_env(&mut self) -> Result<&mut Self> {
        self.actor_manager
            .start_and_wait(InitializeSignersOneShotActor::new_from_encrypted_env().with_signers(self.signers.clone()).on_bc(&self.bc))?;
        self.with_signers()?;
        Ok(self)
    }

    /// Starts signer actor
    pub fn with_signers(&mut self) -> Result<&mut Self> {
        if !self.has_signers {
            self.has_signers = true;
            self.actor_manager.start(TxSignersActor::new().on_bc(&self.bc))?;
        }
        Ok(self)
    }

    /// Initializes encoder and start encoder actor
    pub fn with_swap_encoder(&mut self, multicaller_address: Option<Address>) -> Result<&mut Self> {
        let multicaller_address = match multicaller_address {
            Some(multicaller) => multicaller,
            None => match self.mutlicaller_address {
                Some(multicaller) => multicaller,
                None => return Err(eyre!("MULTICALLER_ADDRESS_NOT_SET")),
            },
        };

        self.encoder = Some(MulticallerSwapEncoder::new(multicaller_address));
        self.actor_manager.start(SwapEncoderActor::new(multicaller_address).with_signers(self.signers.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts market state preloader
    pub fn with_market_state_preloader(&mut self) -> Result<&mut Self> {
        let mut address_vec = self.signers.inner().try_read()?.get_address_vec();

        if let Some(loom_multicaller) = self.mutlicaller_address {
            address_vec.push(loom_multicaller);
        }

        self.actor_manager.start_and_wait(
            MarketStatePreloadedOneShotActor::new(self.provider.clone()).with_copied_accounts(address_vec).on_bc(&self.bc),
        )?;
        Ok(self)
    }

    /// Starts preloaded virtual artefacts
    pub fn with_market_state_preloader_virtual(&mut self, address_to_copy: Vec<Address>) -> Result<&mut Self> {
        let address_vec = self.signers.inner().try_read()?.get_address_vec();

        let mut market_state_preloader = MarketStatePreloadedOneShotActor::new(self.provider.clone());

        for address in address_vec {
            //            market_state_preloader = market_state_preloader.with_new_account(address, 0, NWETH::from_float(10.0), None);
            market_state_preloader = market_state_preloader.with_copied_account(address).with_token_balance(
                ETH_NATIVE_ADDRESS,
                address,
                NWETH::from_float(10.0),
            );
        }

        market_state_preloader = market_state_preloader.with_copied_accounts(address_to_copy);

        market_state_preloader = market_state_preloader.with_new_account(
            loom_multicaller::DEFAULT_VIRTUAL_ADDRESS,
            0,
            U256::ZERO,
            loom_multicaller::MulticallerDeployer::new().account_info().code,
        );

        market_state_preloader =
            market_state_preloader.with_token_balance(WETH_ADDRESS, loom_multicaller::DEFAULT_VIRTUAL_ADDRESS, NWETH::from_float(10.0));

        self.mutlicaller_address = Some(loom_multicaller::DEFAULT_VIRTUAL_ADDRESS);

        self.actor_manager.start_and_wait(market_state_preloader.on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts nonce and balance monitor
    pub fn with_nonce_and_balance_monitor(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NonceAndBalanceMonitorActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    pub fn with_nonce_and_balance_monitor_only_events(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NonceAndBalanceMonitorActor::new(self.provider.clone()).only_once().on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts block history actor
    pub fn with_block_history(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(BlockHistoryActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts token price calculator
    pub fn with_price_station(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(PriceActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts receiving blocks events through RPC
    pub fn with_block_events(&mut self, config: NodeBlockActorConfig) -> Result<&mut Self> {
        self.actor_manager.start(NodeBlockActor::new(self.provider.clone(), config).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts receiving blocks events through direct Reth DB access
    pub fn reth_node_with_blocks(&mut self, db_path: String, config: NodeBlockActorConfig) -> Result<&mut Self> {
        self.actor_manager.start(NodeBlockActor::new(self.provider.clone(), config).on_bc(&self.bc).with_reth_db(Some(db_path)))?;
        Ok(self)
    }

    /// Starts receiving blocks and mempool events through ExEx GRPC
    pub fn with_exex_events(&mut self) -> Result<&mut Self> {
        self.mempool()?;
        self.actor_manager.start(NodeExExGrpcActor::new("http://[::1]:10000".to_string()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts mempool actor collecting pending txes from all mempools and pulling new tx hashes in mempool_events channel

    pub fn mempool(&mut self) -> Result<&mut Self> {
        if !self.has_mempool {
            self.has_mempool = true;
            self.actor_manager.start(MempoolActor::new().on_bc(&self.bc))?;
        }
        Ok(self)
    }

    /// Starts local node pending tx provider
    pub fn with_local_mempool_events(&mut self) -> Result<&mut Self> {
        self.mempool()?;
        self.actor_manager.start(NodeMempoolActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts remote node pending tx provider
    pub fn with_remote_mempool<PM, TM>(&mut self, provider: PM) -> Result<&mut Self>
    where
        TM: Transport + Clone,
        PM: Provider<TM, Ethereum> + Send + Sync + Clone + 'static,
    {
        self.mempool()?;
        self.actor_manager.start(NodeMempoolActor::new(provider).on_bc(&self.bc))?;
        Ok(self)
    }

    //TODO : Refactor estimators actors encoder type to SwapEncoders
    pub fn with_geth_estimator(&mut self) -> Result<&mut Self> {
        let flashbots = Flashbots::new(self.provider.clone(), "https://relay.flashbots.net", None).with_default_relays();

        self.actor_manager
            .start(GethEstimatorActor::new(Arc::new(flashbots), self.encoder.clone().unwrap().swap_step_encoder.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts EVM gas estimator and tips filler
    pub fn with_evm_estimator(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(EvmEstimatorActor::new(self.encoder.clone().unwrap().swap_step_encoder.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts flashbots broadcaster
    pub fn with_flashbots_broadcaster(&mut self, smart: bool) -> Result<&mut Self> {
        let flashbots = match self.relays.is_empty() {
            true => Flashbots::new(self.provider.clone(), "https://relay.flashbots.net", None).with_default_relays(),
            false => Flashbots::new(self.provider.clone(), "https://relay.flashbots.net", None).with_relays(self.relays.clone()),
        };

        self.actor_manager.start(FlashbotsBroadcastActor::new(flashbots, smart).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start composer : estimator, signer and broadcaster
    pub fn with_composers(&mut self) -> Result<&mut Self> {
        self.with_evm_estimator()?.with_signers()?.with_flashbots_broadcaster(true)
    }

    /// Starts pool health monitor
    pub fn with_health_monitor_pools(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(PoolHealthMonitorActor::new().on_bc(&self.bc))?;
        Ok(self)
    }
    /// Starts state health monitor
    pub fn with_health_monitor_state(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(StateHealthMonitorActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Starts stuffing tx monitor
    pub fn with_health_monitor_stuffing_tx(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(StateHealthMonitorActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start pool loader from new block events
    pub fn with_new_pool_loader(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NewPoolLoaderActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start pool loader for last 10000 blocks
    pub fn with_pool_history_loader(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(HistoryPoolLoaderActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start pool loader for curve + steth + wsteth
    pub fn with_pool_protocol_loader(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(ProtocolPoolLoaderActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start all pool loaders
    pub fn with_pool_loaders(&mut self) -> Result<&mut Self> {
        self.with_new_pool_loader()?.with_pool_history_loader()?.with_pool_protocol_loader()
    }

    pub fn with_preloaded_state(&mut self, pools: Vec<(Address, PoolClass)>, state_required: Option<RequiredState>) -> Result<&mut Self> {
        let mut actor = RequiredPoolLoaderActor::new(self.provider.clone());

        for (pool_address, pool_class) in pools {
            actor = actor.with_pool(pool_address, pool_class);
        }

        if let Some(state_required) = state_required {
            actor = actor.with_required_state(state_required);
        }

        self.actor_manager.start_and_wait(actor.on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start swap path merger
    pub fn with_swap_path_merger(&mut self) -> Result<&mut Self> {
        let mutlicaller_address = self.encoder.clone().ok_or(eyre!("NO_ENCODER"))?.multicaller_address;
        self.actor_manager.start(ArbSwapPathMergerActor::new(mutlicaller_address).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start same path merger
    pub fn with_same_path_merger(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(SamePathMergerActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start diff path merger
    pub fn with_diff_path_merger(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(DiffPathMergerActor::new().on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start all mergers
    pub fn with_mergers(&mut self) -> Result<&mut Self> {
        self.with_swap_path_merger()?.with_same_path_merger()?.with_diff_path_merger()
    }

    /// Start backrun on block
    pub fn with_backrun_block(&mut self) -> Result<&mut Self> {
        if !self.has_state_update {
            self.actor_manager.start(StateChangeArbSearcherActor::new(true).on_bc(&self.bc))?;
            self.has_state_update = true
        }
        self.actor_manager.start(BlockStateChangeProcessorActor::new().on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start backrun for pending txs

    pub fn with_backrun_mempool(&mut self) -> Result<&mut Self> {
        if !self.has_state_update {
            self.actor_manager.start(StateChangeArbSearcherActor::new(true).on_bc(&self.bc))?;
            self.has_state_update = true
        }
        self.actor_manager.start(PendingTxStateChangeProcessorActor::new(self.provider.clone()).on_bc(&self.bc))?;
        Ok(self)
    }

    /// Start backrun for blocks and pending txs
    pub async fn with_backrun(&mut self) -> Result<&mut Self> {
        self.with_backrun_block()?.with_backrun_mempool()
    }
}
