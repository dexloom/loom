use alloy_network::TransactionBuilderError::Signer;
use alloy_primitives::{B256, Bytes, hex};
use async_trait::async_trait;
use log::{error, info};

use defi_blockchain::Blockchain;
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
        let key = key.unwrap_or_else(|| B256::random().to_vec());

        InitializeSignersActor {
            key: Some(key),
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

    pub fn new_from_encrypted_key(priv_key_enc: Vec<u8>) -> InitializeSignersActor {
        let keystore = KeyStore::new();
        let key = keystore.encrypt_once(priv_key_enc.as_slice()).unwrap();

        InitializeSignersActor {
            key: Some(key),
            signers: None,
            monitor: None,
        }
    }


    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            monitor: Some(bc.nonce_and_balance()),
            ..self
        }
    }

    pub fn with_signers(self, signers: SharedState<TxSigners>) -> Self {
        Self {
            signers: Some(signers),
            ..self
        }
    }
}

#[async_trait]
impl Actor for InitializeSignersActor {
    async fn start(&self) -> ActorResult {
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

    fn name(&self) -> &'static str {
        "InitializeSignersActor"
    }
}