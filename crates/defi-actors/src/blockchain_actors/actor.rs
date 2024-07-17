use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Ethereum;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use eyre::{eyre, Result};

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::TxSigners;
use flashbots::Flashbots;
use loom_actors::{Actor, ActorsManager, SharedState};
use loom_multicaller::MulticallerSwapEncoder;

use crate::backrun::BlockStateChangeProcessorActor;
use crate::{
    ArbSwapPathEncoderActor, ArbSwapPathMergerActor, BlockHistoryActor, DiffPathMergerActor, EvmEstimatorActor, FlashbotsBroadcastActor,
    GasStationActor, GethEstimatorActor, HistoryPoolLoaderActor, InitializeSignersActor, MarketStatePreloadedActor, MempoolActor,
    NewPoolLoaderActor, NodeBlockActor, NodeExExGrpcActor, NodeMempoolActor, NonceAndBalanceMonitorActor,
    PendingTxStateChangeProcessorActor, PoolHealthMonitorActor, PriceActor, ProtocolPoolLoaderActor, SamePathMergerActor,
    StateChangeArbSearcherActor, StateHealthMonitorActor, TxSignersActor,
};

pub struct BlockchainActors<P, T> {
    provider: P,
    bc: Blockchain,
    signers: SharedState<TxSigners>,
    actor_manager: ActorsManager,
    encoder: Option<MulticallerSwapEncoder>,
    has_mempool: bool,
    has_state_update: bool,
    _t: PhantomData<T>,
}

impl<P, T> BlockchainActors<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + DebugProviderExt<T, Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(provider: P, bc: Blockchain) -> Self {
        Self {
            provider,
            bc,
            signers: SharedState::new(TxSigners::new()),
            actor_manager: ActorsManager::new(),
            encoder: None,
            has_mempool: false,
            has_state_update: false,
            _t: PhantomData,
        }
    }

    pub async fn wait(self) {
        self.actor_manager.wait().await
    }

    /// Start an actor
    pub async fn start(&mut self, actor: impl Actor + 'static) -> Result<()> {
        self.actor_manager.start(actor).await
    }

    /// Initialize signers with the private key. Random key generated if param in None
    pub async fn initialize_signers_with_key(&mut self, key: Option<Vec<u8>>) -> Result<&mut Self> {
        self.actor_manager.start(InitializeSignersActor::new(key).with_signers(self.signers.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }
    pub async fn initialize_signers_with_encrypted_key(&mut self, key: Vec<u8>) -> Result<&mut Self> {
        self.actor_manager
            .start(InitializeSignersActor::new_from_encrypted_key(key).with_signers(self.signers.clone()).on_bc(&self.bc))
            .await?;
        Ok(self)
    }

    /// Initializes signers with encrypted key form DATA env var
    pub async fn initialize_signers_with_env(&mut self) -> Result<&mut Self> {
        self.actor_manager
            .start(InitializeSignersActor::new_from_encrypted_env().with_signers(self.signers.clone()).on_bc(&self.bc))
            .await?;
        Ok(self)
    }

    /// Starts signer actor
    pub async fn with_signers(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(TxSignersActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Initializes encoder and start encoder actor
    pub async fn with_encoder(&mut self, multicaller_address: Address) -> Result<&mut Self> {
        self.encoder = Some(MulticallerSwapEncoder::new(multicaller_address));
        self.actor_manager
            .start(ArbSwapPathEncoderActor::new(multicaller_address).with_signers(self.signers.clone()).on_bc(&self.bc))
            .await?;

        Ok(self)
    }

    /// Starts market state preloader
    pub async fn with_market_state_preloader(&mut self) -> Result<&mut Self> {
        let mut address_vec = self.signers.read().await.get_address_vec();
        let multicaller_address = self.encoder.clone().unwrap().multicaller_address;
        address_vec.push(multicaller_address);

        self.actor_manager
            .start(MarketStatePreloadedActor::new(self.provider.clone()).with_address_vec(address_vec).on_bc(&self.bc))
            .await?;
        Ok(self)
    }

    /// Starts nonce and balance monitor
    pub async fn with_nonce_and_balance_monitor(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NonceAndBalanceMonitorActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts block history actor
    pub async fn with_block_history(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(BlockHistoryActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts gas station
    pub async fn with_gas_station(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(GasStationActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts token price calculator
    pub async fn with_price_station(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(PriceActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts receiving blocks events through RPC
    pub async fn with_block_events(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NodeBlockActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts receiving blocks events through direct Reth DB access
    pub async fn reth_node_with_blocks(&mut self, db_path: String) -> Result<&mut Self> {
        self.actor_manager.start(NodeBlockActor::new(self.provider.clone()).on_bc(&self.bc).with_reth_db(Some(db_path))).await?;
        Ok(self)
    }

    /// Starts receiving blocks and mempool events through ExEx GRPC
    pub async fn with_exex_events(&mut self) -> Result<&mut Self> {
        self.mempool().await?;
        self.actor_manager.start(NodeExExGrpcActor::new("http://[::1]:10000".to_string()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts mempool actor collecting pending txes from all mempools and pulling new tx hashes in mempool_events channel

    pub async fn mempool(&mut self) -> Result<&mut Self> {
        if !self.has_mempool {
            self.has_mempool = true;
            self.actor_manager.start(MempoolActor::new(self.bc.chain_parameters()).on_bc(&self.bc)).await?;
        }
        Ok(self)
    }

    /// Starts local node pending tx provider
    pub async fn with_local_mempool_events(&mut self) -> Result<&mut Self> {
        self.mempool().await?;
        self.actor_manager.start(NodeMempoolActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts remote node pending tx provider
    pub async fn with_remote_mempool<PM, TM>(&mut self, provider: PM) -> Result<&mut Self>
    where
        TM: Transport + Clone,
        PM: Provider<TM, Ethereum> + Send + Sync + Clone + 'static,
    {
        self.mempool().await?;
        self.actor_manager.start(NodeMempoolActor::new(provider).on_bc(&self.bc)).await?;
        Ok(self)
    }

    //TODO : Refactor estimators actors encoder type to SwapEncoders
    pub async fn with_geth_estimator(&mut self) -> Result<&mut Self> {
        let flashbots = Flashbots::new(self.provider.clone(), "https://relay.flashbots.net");

        self.actor_manager
            .start(GethEstimatorActor::new(Arc::new(flashbots), self.encoder.clone().unwrap().swap_step_encoder.clone()).on_bc(&self.bc))
            .await?;
        Ok(self)
    }

    /// Starts EVM gas estimator and tips filler
    pub async fn with_evm_estimator(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(EvmEstimatorActor::new(self.encoder.clone().unwrap().swap_step_encoder.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts flashbots broadcaster
    pub async fn with_flashbots_broadcaster(&mut self, smart: bool) -> Result<&mut Self> {
        let flashbots = Flashbots::new(self.provider.clone(), "https://relay.flashbots.net");
        self.actor_manager.start(FlashbotsBroadcastActor::new(flashbots, smart).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start composer : estimator, signer and broadcaster
    pub async fn with_composers(&mut self) -> Result<&mut Self> {
        self.with_evm_estimator().await?.with_signers().await?.with_flashbots_broadcaster(true).await
    }

    /// Starts pool health monitor
    pub async fn with_health_monitor_pools(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(PoolHealthMonitorActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }
    /// Starts state health monitor
    pub async fn with_health_monitor_state(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(StateHealthMonitorActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Starts stuffing tx monitor
    pub async fn with_health_monitor_stuffing_tx(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(StateHealthMonitorActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start pool loader from new block events
    pub async fn with_new_pool_loader(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NewPoolLoaderActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start pool loader for last 10000 blocks
    pub async fn with_pool_history_loader(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(HistoryPoolLoaderActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start pool loader for curve + steth + wsteth
    pub async fn with_pool_protocol_loader(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(ProtocolPoolLoaderActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start all pool loaders
    pub async fn with_pool_loaders(&mut self) -> Result<&mut Self> {
        self.with_new_pool_loader().await?.with_pool_history_loader().await?.with_pool_protocol_loader().await
    }

    /// Start swap path merger
    pub async fn with_swap_path_merger(&mut self) -> Result<&mut Self> {
        let mutlicaller_address = self.encoder.clone().ok_or(eyre!("NO_ENCODER"))?.multicaller_address;
        self.actor_manager.start(ArbSwapPathMergerActor::new(mutlicaller_address).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start same path merger
    pub async fn with_same_path_merger(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(SamePathMergerActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start diff path merger
    pub async fn with_diff_path_merger(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(DiffPathMergerActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start all mergers
    pub async fn with_mergers(&mut self) -> Result<&mut Self> {
        self.with_swap_path_merger().await?.with_same_path_merger().await?.with_diff_path_merger().await
    }

    /// Start backrun on block
    pub async fn with_backrun_block(&mut self) -> Result<&mut Self> {
        if !self.has_state_update {
            self.actor_manager.start(StateChangeArbSearcherActor::new(true).on_bc(&self.bc)).await?;
            self.has_state_update = true
        }
        self.actor_manager.start(BlockStateChangeProcessorActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start backrun for pending txs

    pub async fn with_backrun_mempool(&mut self) -> Result<&mut Self> {
        if !self.has_state_update {
            self.actor_manager.start(StateChangeArbSearcherActor::new(true).on_bc(&self.bc)).await?;
            self.has_state_update = true
        }
        self.actor_manager.start(PendingTxStateChangeProcessorActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    /// Start backrun for blocks and pending txs
    pub async fn with_backrun(&mut self) -> Result<&mut Self> {
        self.with_backrun_block().await?.with_backrun_mempool().await
    }
}
