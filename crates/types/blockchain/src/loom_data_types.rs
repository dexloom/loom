use crate::{ChainParameters, GethStateUpdate};
use alloy_consensus::BlockHeader;
use alloy_primitives::{Address, BlockHash, TxHash};
use alloy_rpc_types::TransactionTrait;
use alloy_rpc_types_eth::{Header, Log, TransactionRequest};
use op_alloy::rpc_types::OpTransactionRequest;
use std::fmt::{Debug, Display};
use std::hash::Hash;

pub trait LoomTx<LDT: LoomDataTypes> {
    fn gas_price(&self) -> u128;
    fn gas_limit(&self) -> u64;

    fn tx_hash(&self) -> LDT::TxHash;

    fn nonce(&self) -> u64;
    fn from(&self) -> LDT::Address;

    fn encode(&self) -> Vec<u8>;

    fn to_transaction_request(&self) -> LDT::TransactionRequest;
}

pub trait LoomHeader<LDT: LoomDataTypes> {
    fn get_timestamp(&self) -> u64;
    fn get_number(&self) -> u64;

    fn get_hash(&self) -> LDT::BlockHash;
    fn get_parent_hash(&self) -> LDT::BlockHash;

    fn get_base_fee(&self) -> Option<u128>;

    fn get_next_base_fee(&self, params: &ChainParameters) -> u128;

    fn get_beneficiary(&self) -> LDT::Address;
}

pub trait LoomBlock<LDT: LoomDataTypes> {
    fn get_transactions(&self) -> Vec<LDT::Transaction>;

    fn get_header(&self) -> LDT::Header;
}

pub trait LoomTransactionRequest<LDT: LoomDataTypes> {
    fn get_to(&self) -> Option<LDT::Address>;
}

pub trait LoomDataTypes: Debug + Clone + Send + Sync {
    type Transaction: Debug + Clone + Send + Sync + LoomTx<Self> + TransactionTrait;
    type TransactionRequest: Debug + Clone + Send + Sync + LoomTransactionRequest<Self>;
    type TransactionReceipt: Debug + Clone + Send + Sync;
    type Block: Default + Debug + Clone + Send + Sync + LoomBlock<Self>;
    type Header: Default + Debug + Clone + Send + Sync + LoomHeader<Self>;
    type Log: Default + Debug + Clone + Send + Sync;
    type StateUpdate: Default + Debug + Clone + Send + Sync;
    type BlockHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type TxHash: Eq + Copy + Hash + Default + Display + Debug + Clone + Send + Sync;
    type Address: Eq + Copy + Hash + Ord + Default + Display + Debug + Clone + Send + Sync;
}

pub trait LoomDataTypesEVM:
    LoomDataTypes<Header = Header, TxHash = TxHash, BlockHash = BlockHash, Log = Log, StateUpdate = GethStateUpdate, Address = Address>
{
}

impl<LDT> LoomHeader<LDT> for Header
where
    LDT: LoomDataTypes<Header = Header, BlockHash = BlockHash, Address = Address>,
{
    fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    fn get_number(&self) -> u64 {
        self.number
    }

    fn get_hash(&self) -> LDT::BlockHash {
        self.hash
    }

    fn get_parent_hash(&self) -> LDT::BlockHash {
        self.parent_hash
    }

    fn get_base_fee(&self) -> Option<u128> {
        self.base_fee_per_gas().map(|s| s as u128)
    }

    fn get_next_base_fee(&self, params: &ChainParameters) -> u128 {
        params.calc_next_block_base_fee_from_header(self) as u128
    }

    fn get_beneficiary(&self) -> LDT::Address {
        self.beneficiary
    }
}
