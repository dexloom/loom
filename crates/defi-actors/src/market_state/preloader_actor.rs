use std::any::type_name;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_network::Network;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_rpc_types_trace::geth::AccountState;
use alloy_transport::Transport;
use async_trait::async_trait;
use eyre::Result;
use log::debug;

use defi_blockchain::Blockchain;
use defi_entities::{MarketState, TxSigners};
use defi_pools::protocols::UniswapV3Protocol;
use defi_types::GethStateUpdate;
use loom_actors::{Accessor, Actor, ActorResult, SharedState};
use loom_actors_macros::Accessor;
use loom_multicaller::SwapStepEncoder;

pub async fn preload_market_state<P, T, N>(client: P, address_vec: Vec<Address>, market_state: SharedState<MarketState>) -> Result<()>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    let mut market_state_guard = market_state.write().await;

    market_state_guard.add_state(&UniswapV3Protocol::get_quoter_v3_state());

    let mut state: GethStateUpdate = BTreeMap::new();

    for address in address_vec {
        debug!("Loading address : {address}");
        let code = client.get_code_at(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();
        let balance = client.get_balance(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();
        let nonce = client.get_transaction_count(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();

        state.insert(address, AccountState { balance: Some(balance), code: Some(code), nonce: Some(nonce), storage: BTreeMap::new() });
    }

    market_state_guard.add_state(&state);

    Ok(())
}

#[allow(dead_code)]
#[derive(Accessor)]
pub struct MarketStatePreloadedActor<P, T, N> {
    name: &'static str,
    client: P,
    addresses: Vec<Address>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

#[allow(dead_code)]
impl<P, T, N> MarketStatePreloadedActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }

    pub fn new(client: P) -> Self {
        Self { name: "MarketStatePreloadedActor", client, addresses: Vec::new(), market_state: None, _t: PhantomData, _n: PhantomData }
    }

    pub fn with_name(self, name: &'static str) -> Self {
        Self { name, ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { market_state: Some(bc.market_state()), ..self }
    }

    pub fn with_signers(self, tx_signers: SharedState<TxSigners>) -> Self {
        if let Ok(signers) = tx_signers.try_read() {
            let mut addresses = self.addresses;
            addresses.extend(signers.get_address_vec());
            Self { addresses, ..self }
        } else {
            self
        }
    }

    pub fn with_encoder(self, encoder: &SwapStepEncoder) -> Self {
        let mut addresses = self.addresses;
        addresses.extend(vec![encoder.get_multicaller()]);
        Self { addresses, ..self }
    }

    pub fn with_address_vec(self, address_vec: Vec<Address>) -> Self {
        let mut addresses = self.addresses;
        addresses.extend(address_vec);
        Self { addresses, ..self }
    }
}

#[async_trait]
impl<P, T, N> Actor for MarketStatePreloadedActor<P, T, N>
where
    T: Transport + Clone,
    N: Network,
    P: Provider<T, N> + Send + Sync + Clone + 'static,
{
    fn name(&self) -> &'static str {
        let full_name = type_name::<Self>();
        full_name.split("::").last().unwrap_or(full_name)
    }
    async fn start(&self) -> ActorResult {
        preload_market_state(self.client.clone(), self.addresses.clone(), self.market_state.clone().unwrap()).await?;
        Ok(vec![])
    }
}
