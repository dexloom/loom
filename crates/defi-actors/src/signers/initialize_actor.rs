use alloy_primitives::{Bytes, hex};
use async_trait::async_trait;
use log::{error, info};

use defi_entities::{AccountNonceAndBalanceState, KeyStore, TxSigners};
use loom_actors::{Accessor, Actor, ActorResult, SharedState};
use loom_actors_macros::Accessor;

#[derive(Accessor)]
pub struct InitializeSignersActor {
    key: Option<Vec<u8>>,
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    #[accessor]
    monitor: Option<SharedState<AccountNonceAndBalanceState>>,
}

impl InitializeSignersActor {
    pub fn new(key: Option<Vec<u8>>) -> InitializeSignersActor {
        InitializeSignersActor {
            key,
            signers: None,
            monitor: None,
        }
    }

    pub fn new_from_encrypted_env() -> InitializeSignersActor {
        let key = match std::env::var("DATA") {
            Ok(priv_key_enc) => {
                let keystore = KeyStore::new();
                let key = keystore.encrypt_once(hex::decode(priv_key_enc).unwrap().as_slice()).unwrap();
                Some(key)
            }
            _ => None
        };

        InitializeSignersActor {
            key,
            signers: None,
            monitor: None,
        }
    }
}

#[async_trait]
impl Actor for InitializeSignersActor {
    async fn start(&mut self) -> ActorResult {
        match self.key.clone() {
            Some(key) => {
                let new_signer = self.signers.clone().unwrap().write().await.add_privkey(Bytes::from(key));
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