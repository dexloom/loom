use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;

use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_network::Network;
use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use alloy_rpc_types_trace::geth::AccountState;
use alloy_transport::Transport;
use async_trait::async_trait;
use eyre::Result;
use log::{debug, error};

use defi_entities::{MarketState, TxSigners};
use defi_pools::protocols::UniswapV3Protocol;
use defi_types::GethStateUpdate;
use loom_actors::{Accessor, Actor, ActorResult, SharedState};
use loom_actors_macros::Accessor;
use loom_multicaller::SwapStepEncoder;

pub async fn preload_market_state<P, T, N>(
    client: P,
    address_vec: Vec<Address>,
    signers: Option<SharedState<TxSigners>>,
    market_state: SharedState<MarketState>,
) -> Result<()>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    let mut market_state_guard = market_state.write().await;

    let _ = market_state_guard.add_state(&UniswapV3Protocol::get_quoter_v3_state());

    let mut state: GethStateUpdate = BTreeMap::new();

    for address in address_vec {
        debug!("Loading address : {address}");
        let code = client.get_code_at(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();
        let balance = client.get_balance(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();
        let nonce = client.get_transaction_count(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();

        state.insert(address, AccountState {
            balance: Some(balance),
            code: Some(code),
            nonce: Some(nonce),
            storage: BTreeMap::new(),
        });
    }


    if let Some(signers) = signers {
        let signers_guard = signers.read().await;
        for i in 0..signers_guard.len() {
            match signers_guard.get_signer_by_index(i) {
                Ok(s) => {
                    let signer_address = s.address();
                    let nonce = client.get_transaction_count(signer_address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();
                    let balance = client.get_balance(signer_address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await.unwrap();
                    debug!("Loading signer {signer_address} {nonce} {balance}");

                    state.insert(signer_address, AccountState {
                        balance: Some(balance),
                        code: None,
                        nonce: Some(nonce),
                        storage: BTreeMap::new(),
                    });
                }
                Err(e) => { error!("Cannot get signer {i} : {e}") }
            }
        }
    }

    market_state_guard.add_state(&state);

    Ok(())
}

#[derive(Accessor)]
pub struct MarketStatePreloadedActor<P, T, N>
{
    client: P,
    encoder: Arc<SwapStepEncoder>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    _t: PhantomData<T>,
    _n: PhantomData<N>,
}

impl<P, T, N> MarketStatePreloadedActor<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    pub fn new(client: P, encoder: Arc<SwapStepEncoder>) -> Self {
        Self {
            client,
            encoder,
            market_state: None,
            signers: None,
            _t: PhantomData::default(),
            _n: PhantomData::default(),
        }
    }
}


#[async_trait]
impl<P, T, N> Actor for MarketStatePreloadedActor<P, T, N>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + Send + Sync + Clone + 'static
{
    async fn start(&mut self) -> ActorResult
    {
        preload_market_state(
            self.client.clone(),
            vec![self.encoder.get_multicaller()],
            self.signers.clone(),
            self.market_state.clone().unwrap(),
        ).await?;
        Ok(vec![])
    }
}
