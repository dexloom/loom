use alloy_consensus::{SignableTransaction, TxEnvelope};
use alloy_network::{TransactionBuilder, TxSigner as AlloyTxSigner, TxSignerSync};
use alloy_primitives::{hex, Address, Bytes, B256};
use alloy_rpc_types::Transaction;
use alloy_signer_local::PrivateKeySigner;
use eyre::{eyre, OptionExt, Result};
use indexmap::IndexMap;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use rand::prelude::IteratorRandom;
use std::fmt;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub trait LoomTxSigner<LDT: LoomDataTypes>: Send + Sync + Debug {
    fn sign<'a>(&'a self, tx: LDT::TransactionRequest) -> Pin<Box<dyn std::future::Future<Output = Result<LDT::Transaction>> + Send + 'a>>;
    fn sign_sync(&self, tx: LDT::TransactionRequest) -> Result<LDT::Transaction>;
    fn address(&self) -> LDT::Address;
}

#[derive(Clone)]
pub struct TxSignerEth {
    address: Address,
    wallet: PrivateKeySigner,
}

impl Default for TxSignerEth {
    fn default() -> Self {
        let wallet = PrivateKeySigner::random();
        Self { address: wallet.address(), wallet }
    }
}

impl fmt::Debug for TxSignerEth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TxSigner").field("address", &self.address.to_string()).finish()
    }
}

impl LoomTxSigner<LoomDataTypesEthereum> for TxSignerEth {
    fn address(&self) -> <LoomDataTypesEthereum as LoomDataTypes>::Address {
        self.address
    }
    fn sign<'a>(
        &'a self,
        tx_req: <LoomDataTypesEthereum as LoomDataTypes>::TransactionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<<LoomDataTypesEthereum as LoomDataTypes>::Transaction>> + Send + 'a>> {
        let fut = async move {
            let mut typed_tx = tx_req
                .build_typed_tx()
                .map_err(|e| eyre!("TRANSACTION_TYPE_IS_MISSING"))?
                .eip1559()
                .ok_or_eyre("TRANSACTION_IS_NOT_EIP1559")?
                .clone();
            let signature = self.wallet.sign_transaction(&mut typed_tx).await?;
            let signed_tx = typed_tx.clone().into_signed(signature);
            let tx_env: TxEnvelope = signed_tx.into();
            let tx = Transaction {
                inner: tx_env,
                block_hash: None,
                block_number: None,
                transaction_index: None,
                effective_gas_price: None,
                from: self.address(),
            };
            eyre::Result::<Transaction>::Ok(tx)
        };
        Box::pin(fut)

        //let hash = signed_tx.signature_hash();
        //let tx_env: TxEnvelope = signed_tx.into();
        //let tx_data = tx_env.encoded_2718();
        //Ok((hash, Bytes::from(tx_data)))
    }

    fn sign_sync(
        &self,
        tx_req: <LoomDataTypesEthereum as LoomDataTypes>::TransactionRequest,
    ) -> Result<<LoomDataTypesEthereum as LoomDataTypes>::Transaction> {
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
        let tx = Transaction {
            inner: tx_env,
            block_hash: None,
            block_number: None,
            transaction_index: None,
            effective_gas_price: None,
            from: self.address(),
        };
        Ok(tx)
    }
}

impl TxSignerEth {
    pub fn new(wallet: PrivateKeySigner) -> TxSignerEth {
        TxSignerEth { address: wallet.address(), wallet }
    }
}

#[derive(Clone, Default)]
pub struct TxSigners<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    signers: IndexMap<LDT::Address, Arc<dyn LoomTxSigner<LDT>>>,
}

impl TxSigners<LoomDataTypesEthereum> {
    pub fn add_privkey(&mut self, priv_key: Bytes) -> TxSignerEth {
        let wallet = PrivateKeySigner::from_bytes(&B256::from_slice(priv_key.as_ref())).unwrap();
        self.signers.insert(wallet.address(), Arc::new(TxSignerEth::new(wallet.clone())));
        TxSignerEth::new(wallet)
    }

    pub fn add_testkey(&mut self) -> TxSignerEth {
        self.add_privkey(Bytes::from(hex!("507485ea5bcf6864596cb51b2e727bb2d8ed5e64bb4f3d8c77a734d2fd610c6e")))
    }
}

impl<LDT: LoomDataTypes> TxSigners<LDT> {
    pub fn new() -> TxSigners<LDT> {
        TxSigners { signers: IndexMap::new() }
    }

    pub fn len(&self) -> usize {
        self.signers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signers.is_empty()
    }

    pub fn get_random_signer(&self) -> Option<Arc<dyn LoomTxSigner<LDT>>> {
        if self.is_empty() {
            None
        } else {
            let mut rng = rand::thread_rng();
            self.signers.values().choose(&mut rng).cloned()
        }
    }
    pub fn get_signer_by_index(&self, index: usize) -> Result<Arc<dyn LoomTxSigner<LDT>>> {
        match self.signers.get_index(index) {
            Some((_, s)) => Ok(s.clone()),
            None => Err(eyre!("SIGNER_NOT_FOUND")),
        }
    }

    pub fn get_signer_by_address(&self, address: &LDT::Address) -> Result<Arc<dyn LoomTxSigner<LDT>>> {
        match self.signers.get(address) {
            Some(s) => Ok(s.clone()),
            None => Err(eyre!("SIGNER_NOT_FOUND")),
        }
    }

    pub fn get_address_vec(&self) -> Vec<LDT::Address> {
        self.signers.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, TxHash};
    use alloy_rpc_types::TransactionRequest;
    use eyre::Result;
    use loom_types_blockchain::LoomTx;
    // TxSigner tests

    #[test]
    fn test_new_signer() {
        let wallet = PrivateKeySigner::random();
        let signer = TxSignerEth::new(wallet.clone());
        assert_eq!(signer.address(), wallet.address());
    }

    #[test]
    fn test_address() {
        let wallet = PrivateKeySigner::random();
        let signer = TxSignerEth::new(wallet.clone());
        assert_eq!(signer.address(), wallet.address());
    }

    #[tokio::test]
    async fn test_sign() -> Result<()> {
        let wallet = PrivateKeySigner::from_bytes(&B256::repeat_byte(1))?;
        let signer = Box::new(TxSignerEth::new(wallet));
        let tx_req = TransactionRequest::default()
            .with_to(Address::ZERO)
            .with_nonce(1)
            .with_gas_limit(1)
            .with_max_fee_per_gas(1)
            .with_max_priority_fee_per_gas(1);
        let tx = signer.sign(tx_req).await?;
        let tx_hash = tx.tx_hash();
        let tx_rlp = tx.encode();
        assert_eq!(tx_hash, TxHash::from(hex!("a43d09cb299eb6269f5a63fb10ea078c649cbf6a5f159cfd5b6f4be7ad0dfcfd")));
        assert!(!tx_rlp.is_empty());
        Ok(())
    }

    #[test]
    fn test_sign_sync() -> Result<()> {
        let wallet = PrivateKeySigner::from_bytes(&B256::repeat_byte(1))?;
        let signer = TxSignerEth::new(wallet);
        let tx_req = TransactionRequest::default()
            .with_to(Address::ZERO)
            .with_nonce(1)
            .with_gas_limit(1)
            .with_max_fee_per_gas(1)
            .with_max_priority_fee_per_gas(1);
        let tx = signer.sign_sync(tx_req)?;
        let tx_hash = tx.tx_hash();
        let tx_rlp = tx.encode();
        assert_eq!(tx_hash, TxHash::from(hex!("a43d09cb299eb6269f5a63fb10ea078c649cbf6a5f159cfd5b6f4be7ad0dfcfd")));
        assert!(!tx_rlp.is_empty());
        Ok(())
    }

    // TxSigners tests

    #[test]
    fn test_new_signers() {
        let signers: TxSigners<LoomDataTypesEthereum> = TxSigners::new();
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
        assert!(signers.get_random_signer().is_some());
    }

    #[test]
    fn test_get_signer_by_index() {
        let mut signers = TxSigners::new();
        let signer = signers.add_testkey();
        let address = signer.address();
        assert!(signers.get_signer_by_index(0).is_ok());
        // test negative case
        let unknown_address = Address::random();
        assert!(signers.get_signer_by_index(1).is_err());
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
