use std::fmt;

use alloy_consensus::{SignableTransaction, TxEnvelope};
use alloy_network::{TransactionBuilder, TxSigner as AlloyTxSigner, TxSignerSync};
use alloy_network::eip2718::Encodable2718;
use alloy_primitives::{Address, B256, Bytes, TxHash};
use alloy_primitives::private::alloy_rlp::Encodable;
use alloy_rpc_types::TransactionRequest;
use alloy_signer_local::LocalWallet;
use eyre::{eyre, OptionExt, Result};
use rand::Rng;

#[derive(Clone)]
pub struct TxSigner {
    address: Address,
    wallet: LocalWallet,
}

impl Default for TxSigner {
    fn default() -> Self {
        let wallet = LocalWallet::random();
        Self {
            address: wallet.address(),
            wallet,
        }
    }
}

impl fmt::Debug for TxSigner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TxSigner")
            .field("address", &self.address.to_string())
            .finish()
    }
}


impl TxSigner {
    pub fn new(wallet: LocalWallet) -> TxSigner {
        TxSigner {
            address: wallet.address(),
            wallet,
        }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub async fn sign(&self, tx_req: TransactionRequest) -> Result<(TxHash, Bytes)> {
        let mut typed_tx = tx_req.build_typed_tx().map_err(|_| eyre!("CANNOT_BUILD_TX"))?.eip1559().ok_or_eyre("CANNOT_BUILD_EIP1559")?.clone();
        let signature = self.wallet.sign_transaction(&mut typed_tx).await?;
        let signed_tx = typed_tx.clone().into_signed(signature);

        let hash = signed_tx.signature_hash();
        let mut tx_data: Vec<u8> = Vec::new();
        typed_tx.encode(&mut tx_data);
        Ok((hash, Bytes::from(tx_data)))
    }

    pub fn sign_sync(&self, tx_req: TransactionRequest) -> Result<(TxHash, Bytes)> {
        let mut typed_tx = tx_req.build_unsigned().map_err(|e| {
            eyre!("CANNOT_BUILD_UNSIGNED")
        })?.eip1559().ok_or_eyre("NOT_EIP_1599")?.clone();

        let signature = self.wallet.sign_transaction_sync(&mut typed_tx)?;
        let signed_tx = typed_tx.clone().into_signed(signature);


        let hash = signed_tx.signature_hash();
        let tx_env: TxEnvelope = signed_tx.try_into()?;
        let tx_data = tx_env.encoded_2718();
        Ok((hash, Bytes::from(tx_data)))
    }
}

#[derive(Clone, Default)]
pub struct TxSigners {
    signers_vec: Vec<TxSigner>,
}


impl TxSigners {
    pub fn new() -> TxSigners {
        TxSigners {
            signers_vec: Vec::new()
        }
    }

    pub fn len(&self) -> usize {
        self.signers_vec.len()
    }
    pub fn is_empty(&self) -> bool {
        self.signers_vec.is_empty()
    }
    pub fn add_privkey(&mut self, priv_key: Bytes) -> TxSigner {
        let wallet = LocalWallet::from_bytes(&B256::from_slice(priv_key.as_ref())).unwrap();
        if self.signers_vec.iter().find(|&item| item.address() == wallet.address()).is_none() {
            self.signers_vec.push(TxSigner::new(wallet.clone()));
        }
        TxSigner::new(wallet)
    }

    pub fn add_testkey(&mut self) -> TxSigner {
        self.add_privkey(Bytes::from(hex::decode("507485ea5bcf6864596cb51b2e727bb2d8ed5e64bb4f3d8c77a734d2fd610c6e").unwrap()))
    }


    pub fn get_randon_signer(&self) -> Option<TxSigner> {
        if self.len() == 0 {
            None
        } else {
            let rnd: usize = rand::thread_rng().gen();
            self.signers_vec.get(rnd % self.len()).cloned()
        }
    }

    pub fn get_signer_by_index(&self, index: usize) -> Result<TxSigner> {
        match self.signers_vec.get(index) {
            Some(s) => Ok(s.clone()),
            None => Err(eyre!("SIGNER_NOT_FOUND"))
        }
    }

    pub fn get_address_vec(&self) -> Vec<Address> {
        self.signers_vec.iter().map(|s| s.address).collect()
    }
}

