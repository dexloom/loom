use std::marker::PhantomData;
use std::sync::Arc;

use alloy_network::Ethereum;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use eyre::Result;

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::TxSigners;
use flashbots::Flashbots;
use loom_actors::{Actor, ActorsManager, SharedState};
use loom_multicaller::MulticallerSwapEncoder;

use crate::{BlockHistoryActor, EvmEstimatorActor, FlashbotsBroadcastActor, GasStationActor, GethEstimatorActor, InitializeSignersActor, MarketStatePreloadedActor, MempoolActor, NodeBlockActor, NodeMempoolActor, NonceAndBalanceMonitorActor, TxSignersActor};

pub struct BlockchainActors<P, T> {
    provider: P,
    bc: Blockchain,
    signers: SharedState<TxSigners>,
    actor_manager: ActorsManager,
    encoder: Option<MulticallerSwapEncoder>,
    has_mempool: bool,
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
            _t: PhantomData,
        }
    }


    pub async fn wait(self) {
        self.actor_manager.wait().await
    }

    pub async fn start(&mut self, actor: impl Actor + 'static) -> Result<()> {
        self.actor_manager.start(actor).await
    }

    pub fn with_encoder(&mut self, multicaller_address: Address) -> Result<&mut Self> {
        self.encoder = Some(MulticallerSwapEncoder::new(multicaller_address));
        Ok(self)
    }


    pub async fn initialize_signers_with_key(&mut self, key: Option<Vec<u8>>) -> Result<&mut Self> {
        self.actor_manager.start(InitializeSignersActor::new(key).with_signers(self.signers.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn initialize_signers_with_env(&mut self, key: Option<Vec<u8>>) -> Result<&mut Self> {
        self.actor_manager.start(InitializeSignersActor::new_from_encrypted_env().with_signers(self.signers.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }


    pub async fn with_signers(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(TxSignersActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn with_market_state_preoloader(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(MarketStatePreloadedActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn with_nonce_and_balance_monitor(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NonceAndBalanceMonitorActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }
    pub async fn with_block_history(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(BlockHistoryActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn with_gas_station(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(GasStationActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn node_with_blocks(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NodeBlockActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }
    pub async fn reth_node_with_blocks(&mut self, db_path: String) -> Result<&mut Self> {
        self.actor_manager.start(NodeBlockActor::new(self.provider.clone()).on_bc(&self.bc).with_reth_db(Some(db_path))).await?;
        Ok(self)
    }


    pub async fn mempool(&mut self) -> Result<&mut Self> {
        if !self.has_mempool {
            self.has_mempool = true;
            self.actor_manager.start(MempoolActor::new(self.bc.chain_parameters()).on_bc(&self.bc)).await?;
        }
        Ok(self)
    }
    pub async fn with_local_mempool(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(NodeMempoolActor::new(self.provider.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn with_remote_mempool<PM, TM>(&mut self, provider: PM) -> Result<&mut Self>
    where
        TM: Transport + Clone,
        PM: Provider<TM, Ethereum> + Send + Sync + Clone + 'static,
    {
        self.actor_manager.start(NodeMempoolActor::new(provider).on_bc(&self.bc)).await?;
        Ok(self)
    }

    //TODO : Refactor estimators actors encoder type to SwapEncoders
    pub async fn with_geth_estimator<PM>(&mut self, flashbots: Flashbots<PM>) -> Result<&mut Self>
    where
        PM: Provider + Send + Sync + Clone + 'static,
    {
        self.actor_manager.start(GethEstimatorActor::new(Arc::new(flashbots), self.encoder.clone().unwrap().swap_step_encoder.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn with_evm_estimator(&mut self) -> Result<&mut Self>
    {
        self.actor_manager.start(EvmEstimatorActor::new(self.encoder.clone().unwrap().swap_step_encoder.clone()).on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn with_flashbots_broadcaster<PM>(&mut self, flashbots: Flashbots<PM>, smart: bool) -> Result<&mut Self>
    where
        PM: Provider + Send + Sync + Clone + 'static,
    {
        self.actor_manager.start(FlashbotsBroadcastActor::new(flashbots, smart).on_bc(&self.bc)).await?;
        Ok(self)
    }
}