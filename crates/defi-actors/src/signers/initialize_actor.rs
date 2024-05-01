use alloy_primitives::hex;
use async_trait::async_trait;
use log::{error, info};

use defi_entities::{AccountNonceAndBalanceState, KeyStore, TxSigners};
use loom_actors::{Accessor, Actor, ActorResult, SharedState};
use loom_actors_macros::Accessor;

#[derive(Accessor)]
pub struct InitializeSignersActor {
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    #[accessor]
    monitor: Option<SharedState<AccountNonceAndBalanceState>>,
}

impl InitializeSignersActor {
    pub fn new() -> InitializeSignersActor {
        InitializeSignersActor {
            signers: None,
            monitor: None,
        }
    }
}

#[async_trait]
impl Actor for InitializeSignersActor {
    async fn start(&mut self) -> ActorResult {
        match std::env::var("DATA") {
            Ok(priv_key_enc) => {
                let keystore = KeyStore::new();

                let priv_key = keystore.encrypt_once(hex::decode(priv_key_enc).unwrap().as_slice()).unwrap();

                let new_signer = self.signers.clone().unwrap().write().await.add_privkey(priv_key.into());
                self.monitor.clone().unwrap().write().await.add_account(new_signer.address());
                info!("New signer added {:?}", new_signer.address() );
            }
            _ => {
                error!("No signer keys found");
            }
        }
        Ok(vec![])
    }
}