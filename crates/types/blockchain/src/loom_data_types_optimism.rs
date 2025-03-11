use crate::{ChainParameters, GethStateUpdate, LoomBlock, LoomDataTypes, LoomDataTypesEVM, LoomDataTypesEthereum, LoomHeader, LoomTx};
use alloy_consensus::{BlockHeader, Transaction as TransactionTrait};
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Address, BlockHash, Bytes, TxHash, TxKind};
use alloy_provider::network::{TransactionBuilder, TransactionResponse};

use crate::loom_data_types::LoomTransactionRequest;
use alloy_rpc_types_eth::{Block as EthBlock, Header, Log, TransactionRequest};
use op_alloy::rpc_types::{OpTransactionReceipt, OpTransactionRequest, Transaction as OpTransaction};

#[derive(Clone, Debug, Default)]
pub struct LoomDataTypesOptimism {
    _private: (),
}

impl LoomDataTypes for LoomDataTypesOptimism {
    type Transaction = OpTransaction;
    type TransactionRequest = OpTransactionRequest;
    type TransactionReceipt = OpTransactionReceipt;
    type Block = EthBlock<OpTransaction, Header>;
    type Header = Header;
    type Log = Log;
    type StateUpdate = GethStateUpdate;
    type BlockHash = BlockHash;
    type TxHash = TxHash;
    type Address = Address;
}

impl LoomDataTypesEVM for LoomDataTypesOptimism {}

impl LoomTx<LoomDataTypesOptimism> for OpTransaction {
    fn get_gas_price(&self) -> u128 {
        TransactionTrait::max_fee_per_gas(self)
    }

    fn get_gas_limit(&self) -> u64 {
        TransactionTrait::gas_limit(self)
    }

    fn get_tx_hash(&self) -> <LoomDataTypesOptimism as LoomDataTypes>::TxHash {
        TransactionResponse::tx_hash(self)
    }

    fn get_nonce(&self) -> u64 {
        TransactionTrait::nonce(self)
    }

    fn get_from(&self) -> Address {
        TransactionResponse::from(self)
    }

    fn encode(&self) -> Vec<u8> {
        //self.inner.
        //TODO : Fix this
        vec![]
    }

    fn to_transaction_request(&self) -> <LoomDataTypesOptimism as LoomDataTypes>::TransactionRequest {
        let r = self.inner.clone().into_inner();
        r.into()
    }
}

impl LoomBlock<LoomDataTypesOptimism> for EthBlock<OpTransaction, Header> {
    fn get_transactions(&self) -> Vec<<LoomDataTypesOptimism as LoomDataTypes>::Transaction> {
        self.transactions.clone().into_transactions_vec()
    }

    fn get_header(&self) -> <LoomDataTypesOptimism as LoomDataTypes>::Header {
        self.header.clone()
    }
}

impl LoomTransactionRequest<LoomDataTypesOptimism> for OpTransactionRequest {
    fn get_to(&self) -> Option<<LoomDataTypesOptimism as LoomDataTypes>::Address> {
        match &self.clone().build_typed_tx() {
            Ok(tx) => tx.to(),
            _ => None,
        }
    }

    fn build_call(to: <LoomDataTypesEthereum as LoomDataTypes>::Address, data: Bytes) -> OpTransactionRequest {
        OpTransactionRequest::default().with_kind(TxKind::Call(to)).with_input(data)
    }
}
