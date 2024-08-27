use std::collections::HashMap;
use std::fmt;

use alloy_consensus::{SignableTransaction, TxEnvelope};
use alloy_network::eip2718::Encodable2718;
use alloy_network::{TransactionBuilder, TxSigner as AlloyTxSigner, TxSignerSync};
use alloy_primitives::{hex, Address, Bytes, TxHash, B256};
use alloy_rpc_types::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use eyre::{eyre, OptionExt, Result};
use rand::prelude::IteratorRandom;

#[derive(Clone)]
pub struct TxSigner {
    address: Address,
    wallet: PrivateKeySigner,
}

impl Default for TxSigner {
    fn default() -> Self {
        let wallet = PrivateKeySigner::random();
        Self { address: wallet.address(), wallet }
    }
}

impl fmt::Debug for TxSigner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TxSigner").field("address", &self.address.to_string()).finish()
    }
}

impl TxSigner {
    pub fn new(wallet: PrivateKeySigner) -> TxSigner {
        TxSigner { address: wallet.address(), wallet }
    }

    pub fn address(&self) -> Address {
        self.address
    }

    pub async fn sign(&self, tx_req: TransactionRequest) -> Result<(TxHash, Bytes)> {
        let mut typed_tx = tx_req
            .build_typed_tx()
            .map_err(|e| eyre!("TRANSACTION_TYPE_IS_MISSING"))?
            .eip1559()
            .ok_or_eyre("TRANSACTION_IS_NOT_EIP1559")?
            .clone();
        let signature = self.wallet.sign_transaction(&mut typed_tx).await?;
        let signed_tx = typed_tx.clone().into_signed(signature);

        let hash = signed_tx.signature_hash();
        let tx_env: TxEnvelope = signed_tx.into();
        let tx_data = tx_env.encoded_2718();
        Ok((hash, Bytes::from(tx_data)))
    }

    pub fn sign_sync(&self, tx_req: TransactionRequest) -> Result<(TxHash, Bytes)> {
        let mut typed_tx = tx_req
            .build_unsigned()
            .map_err(|e| eyre!(format!("CANNOT_BUILD_UNSIGNED with error: {}", e)))?
            .eip1559()
            .ok_or_eyre("TRANSACTION_IS_NOT_EIP1559")?
            .clone();

        let signature = self.wallet.sign_transaction_sync(&mut typed_tx)?;
        let signed_tx = typed_tx.clone().into_signed(signature);

        let hash = signed_tx.signature_hash();
        let tx_env: TxEnvelope = signed_tx.into();
        let tx_data = tx_env.encoded_2718();
        Ok((hash, Bytes::from(tx_data)))
    }
}

#[derive(Clone, Default)]
pub struct TxSigners {
    signers: HashMap<Address, TxSigner>,
}

impl TxSigners {
    pub fn new() -> TxSigners {
        TxSigners { signers: HashMap::new() }
    }

    pub fn len(&self) -> usize {
        self.signers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signers.is_empty()
    }

    pub fn add_privkey(&mut self, priv_key: Bytes) -> TxSigner {
        let wallet = PrivateKeySigner::from_bytes(&B256::from_slice(priv_key.as_ref())).unwrap();
        self.signers.insert(wallet.address(), TxSigner::new(wallet.clone()));
        TxSigner::new(wallet)
    }

    pub fn add_testkey(&mut self) -> TxSigner {
        self.add_privkey(Bytes::from(hex!("507485ea5bcf6864596cb51b2e727bb2d8ed5e64bb4f3d8c77a734d2fd610c6e")))
    }

    pub fn get_randon_signer(&self) -> Option<TxSigner> {
        if self.is_empty() {
            None
        } else {
            let mut rng = rand::thread_rng();
            self.signers.values().choose(&mut rng).cloned()
        }
    }

    pub fn get_signer_by_address(&self, address: &Address) -> Result<TxSigner> {
        match self.signers.get(address) {
            Some(s) => Ok(s.clone()),
            None => Err(eyre!("SIGNER_NOT_FOUND")),
        }
    }

    pub fn get_address_vec(&self) -> Vec<Address> {
        self.signers.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use eyre::Result;

    // TxSigner tests

    #[test]
    fn test_new_signer() {
        let wallet = PrivateKeySigner::random();
        let signer = TxSigner::new(wallet.clone());
        assert_eq!(signer.address(), wallet.address());
    }

    #[test]
    fn test_address() {
        let wallet = PrivateKeySigner::random();
        let signer = TxSigner::new(wallet.clone());
        assert_eq!(signer.address(), wallet.address());
    }

    #[tokio::test]
    async fn test_sign() -> Result<()> {
        let wallet = PrivateKeySigner::random();
        let signer = TxSigner::new(wallet);
        let tx_req = TransactionRequest::default()
            .with_to(Address::ZERO)
            .with_nonce(1)
            .with_gas_limit(1)
            .with_max_fee_per_gas(1)
            .with_max_priority_fee_per_gas(1);
        let (hash, bytes) = signer.sign(tx_req).await?;
        assert_eq!(hash, TxHash::from(hex!("25a2e3f10d76b0d9dad0f49068362e7c85a3ee5622cccae107640b9f085985c0")));
        assert!(!bytes.is_empty());
        Ok(())
    }

    #[test]
    fn test_sign_sync() -> Result<()> {
        let wallet = PrivateKeySigner::random();
        let signer = TxSigner::new(wallet);
        let tx_req = TransactionRequest::default()
            .with_to(Address::ZERO)
            .with_nonce(1)
            .with_gas_limit(1)
            .with_max_fee_per_gas(1)
            .with_max_priority_fee_per_gas(1);
        let (hash, bytes) = signer.sign_sync(tx_req)?;
        assert_eq!(hash, TxHash::from(hex!("25a2e3f10d76b0d9dad0f49068362e7c85a3ee5622cccae107640b9f085985c0")));
        assert!(!bytes.is_empty());
        Ok(())
    }

    // TxSigners tests

    #[test]
    fn test_new_signers() {
        let signers = TxSigners::new();
        assert!(signers.is_empty());
    }

    #[test]
    fn test_len() {
        let mut signers = TxSigners::new();
        assert_eq!(signers.len(), 0);
        signers.add_testkey();
        assert_eq!(signers.len(), 1);
    }

    #[test]
    fn test_is_empty() {
        let mut signers = TxSigners::new();
        assert!(signers.is_empty());
        signers.add_testkey();
        assert!(!signers.is_empty());
    }

    #[test]
    fn test_add_privkey() {
        let mut signers = TxSigners::new();
        let priv_key = Bytes::from(hex!("507485ea5bcf6864596cb51b2e727bb2d8ed5e64bb4f3d8c77a734d2fd610c6e"));
        let signer = signers.add_privkey(priv_key);
        assert_eq!(signers.len(), 1);
        assert_eq!(signer.address(), signers.get_address_vec()[0]);
        assert_eq!(signer.address(), address!("16Df4b25e4E37A9116eb224799c1e0Fb17fd8d30"));
    }

    #[test]
    fn test_add_testkey() {
        let mut signers = TxSigners::new();
        let signer = signers.add_testkey();
        assert_eq!(signers.len(), 1);
        assert_eq!(signer.address(), signers.get_address_vec()[0]);
    }

    #[test]
    fn test_get_random_signer() {
        let mut signers = TxSigners::new();
        signers.add_testkey();
        assert!(signers.get_randon_signer().is_some());
    }

    #[test]
    fn test_get_signer_by_address() {
        let mut signers = TxSigners::new();
        let signer = signers.add_testkey();
        let address = signer.address();
        assert!(signers.get_signer_by_address(&address).is_ok());
        // test negative case
        let unknown_address = Address::random();
        assert!(signers.get_signer_by_address(&unknown_address).is_err());
    }

    #[test]
    fn test_get_address_vec() {
        let mut signers = TxSigners::new();
        assert!(signers.get_address_vec().is_empty());
        let signer = signers.add_testkey();
        let addresses = signers.get_address_vec();
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0], signer.address());
    }
}
