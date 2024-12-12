use crate::{ChainParameters, GethStateUpdate, LoomBlock, LoomDataTypes, LoomHeader, LoomTx};
use alloy_consensus::{BlockHeader, Transaction as TransactionTrait};
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{hex, Address, BlockHash, TxHash};
use alloy_provider::network::TransactionResponse;
use alloy_rpc_types_eth::{Block as EthBlock, Header, Log, Transaction, TransactionReceipt, TransactionRequest};

#[derive(Clone, Debug, Default)]
pub struct LoomDataTypesEthereum {
    _private: (),
}

impl LoomDataTypes for LoomDataTypesEthereum {
    type Transaction = Transaction;
    type TransactionRequest = TransactionRequest;
    type TransactionReceipt = TransactionReceipt;
    type Block = EthBlock;
    type Header = Header;
    type Log = Log;
    type StateUpdate = GethStateUpdate;

    type BlockHash = BlockHash;
    type TxHash = TxHash;

    type Address = Address;

    const WETH: Self::Address = Address::new(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));

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

    fn from(&self) -> Address {
        TransactionResponse::from(self)
    }

    fn encode(&self) -> Vec<u8> {
        self.inner.encoded_2718()
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

impl LoomBlock<LoomDataTypesEthereum> for EthBlock {
    fn transactions(&self) -> Vec<<LoomDataTypesEthereum as LoomDataTypes>::Transaction> {
        self.transactions.as_transactions().unwrap_or_default().to_vec()
    }

    fn number(&self) -> u64 {
        self.header.number
    }
}
