use crate::{ChainParameters, GethStateUpdate};
use alloy_consensus::{BlockHeader, Transaction as TransactionTrait};
use alloy_primitives::{Address, BlockHash, TxHash};
use alloy_provider::network::TransactionResponse;
use alloy_rpc_types_eth::{Block, Header, Log, Transaction, TransactionReceipt, TransactionRequest};
use std::fmt::{Debug, Display};
use std::hash::Hash;

pub trait LoomTx<LDT: LoomDataTypes> {
    fn gas_price(&self) -> u128;
    fn gas_limit(&self) -> u64;

    fn tx_hash(&self) -> LDT::TxHash;

    fn nonce(&self) -> u64;
    fn from(&self) -> LDT::Address;
}

pub trait LoomHeader<LDT: LoomDataTypes> {
    fn number(&self) -> u64;

    fn hash(&self) -> LDT::BlockHash;

    fn base_fee(&self) -> Option<u128>;

    fn next_base_fee(&self, params: &ChainParameters) -> u128;
}

pub trait LoomBlock<LDT: LoomDataTypes> {
    fn transactions(&self) -> Vec<LDT::Transaction>;

    fn number(&self) -> u64;
}

pub trait LoomDataTypes: Debug + Clone + Send + Sync {
    type Transaction: Debug + Clone + Send + Sync + LoomTx<Self>;
    type TransactionRequest: Debug + Clone + Send + Sync;
    type TransactionReceipt: Debug + Clone + Send + Sync;
    type Block: Default + Debug + Clone + Send + Sync + LoomBlock<Self>;
    type Header: Default + Debug + Clone + Send + Sync + LoomHeader<Self>;
    type Log: Default + Debug + Clone + Send + Sync;
    type StateUpdate: Default + Debug + Clone + Send + Sync;
    type BlockHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type TxHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type Address: Eq + Copy + Hash + Ord + Default + Display + Debug + Clone + Send + Sync;
    const WETH: Self::Address;

    fn is_weth(address: &Self::Address) -> bool;
}

#[derive(Clone, Debug, Default)]
pub struct LoomDataTypesEthereum {
    _private: (),
}

impl LoomDataTypes for LoomDataTypesEthereum {
    type Transaction = Transaction;
    type TransactionRequest = TransactionRequest;
    type TransactionReceipt = TransactionReceipt;
    type Block = Block;
    type Header = Header;
    type Log = Log;
    type StateUpdate = GethStateUpdate;

    type BlockHash = BlockHash;
    type TxHash = TxHash;

    type Address = Address;

    const WETH: Self::Address = Address::ZERO;

    fn is_weth(address: &Self::Address) -> bool {
        address.eq(&Self::WETH)
    }
}

impl LoomTx<LoomDataTypesEthereum> for Transaction {
    fn gas_price(&self) -> u128 {
        TransactionTrait::max_fee_per_gas(self)
    }

    fn gas_limit(&self) -> u64 {
        TransactionTrait::gas_limit(self)
    }

    fn tx_hash(&self) -> <LoomDataTypesEthereum as LoomDataTypes>::TxHash {
        TransactionResponse::tx_hash(self)
    }

    fn nonce(&self) -> u64 {
        TransactionTrait::nonce(self)
    }

    fn from(&self) -> <LoomDataTypesEthereum as LoomDataTypes>::Address {
        TransactionResponse::from(self)
    }
}

impl LoomHeader<LoomDataTypesEthereum> for Header {
    fn number(&self) -> u64 {
        self.number
    }

    fn hash(&self) -> <LoomDataTypesEthereum as LoomDataTypes>::BlockHash {
        self.hash
    }

    fn base_fee(&self) -> Option<u128> {
        self.base_fee_per_gas().map(|s| s as u128)
    }

    fn next_base_fee(&self, params: &ChainParameters) -> u128 {
        params.calc_next_block_base_fee_from_header(self) as u128
    }
}

impl LoomBlock<LoomDataTypesEthereum> for Block {
    fn transactions(&self) -> Vec<<LoomDataTypesEthereum as LoomDataTypes>::Transaction> {
        Block::transactions(self)
    }

    fn number(&self) -> u64 {
        Block::number(self)
    }
}
