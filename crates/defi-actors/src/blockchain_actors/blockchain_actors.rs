use std::marker::PhantomData;

use alloy_network::Network;
use alloy_provider::Provider;
use alloy_transport::Transport;
use eyre::Result;

use debug_provider::DebugProviderExt;
use defi_blockchain::Blockchain;
use defi_entities::TxSigners;
use loom_actors::{Actor, ActorsManager, SharedState};

use crate::{BlockHistoryActor, GasStationActor, InitializeSignersActor, MarketStatePreloadedActor, NonceAndBalanceMonitorActor, TxSignersActor};

pub struct BlockchainActors<P, T, N> {
    provider: P,
    bc: Blockchain,
    signers: SharedState<TxSigners>,
    actor_manager: ActorsManager,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> BlockchainActors<P, T, N>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,

{
    pub fn new(provider: P, bc: Blockchain) -> Self {
        Self {
            provider,
            bc,
            signers: SharedState::new(TxSigners::new()),
            actor_manager: ActorsManager::new(),
            _t: PhantomData,
            _n: PhantomData,
        }
    }

    pub async fn start(&mut self, actor: impl Actor + 'static) -> Result<()> {
        self.actor_manager.start(actor).await
    }


    pub async fn initialize_signers(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(InitializeSignersActor::new(None).with_signers(self.signers.clone()).on_bc(&self.bc)).await?;
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
    pub async fn with_block_history_actor(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(BlockHistoryActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn with_gas_station(&mut self) -> Result<&mut Self> {
        self.actor_manager.start(GasStationActor::new().on_bc(&self.bc)).await?;
        Ok(self)
    }

    pub async fn wait(self) {
        self.actor_manager.wait().await
    }
}