use std::fmt::{Display, Formatter};

use alloy_consensus::TxEnvelope;
use alloy_network::eip2718::Encodable2718;
use alloy_network::TransactionResponse;
use alloy_primitives::{keccak256, Address, Bytes, TxHash, U256, U64};
use alloy_rpc_types::{AccessList, Log, Transaction};
use eyre::Result;
use serde::ser::Error as SerdeError;
use serde::{Deserialize, Serialize, Serializer};

use crate::client::utils::{deserialize_optional_h160, deserialize_u256, deserialize_u64};

/// A bundle hash.
pub type BundleHash = TxHash;

/// A transaction that can be added to a bundle.
#[derive(Debug, Clone)]
pub enum BundleTransaction {
    /// A pre-signed transaction.
    Signed(Box<Transaction>),
    /// An RLP encoded signed transaction.
    Raw(Bytes),
}

impl From<TxEnvelope> for BundleTransaction {
    fn from(tx: TxEnvelope) -> Self {
        let rlp = tx.encoded_2718();
        //Self::Signed(Bytes))
        Self::Raw(Bytes::from(rlp))
    }
}

impl From<Bytes> for BundleTransaction {
    fn from(tx: Bytes) -> Self {
        Self::Raw(tx)
    }
}

/// A bundle that can be submitted to a Flashbots relay.
///
/// The bundle can include your own transactions and transactions from
/// the mempool.
///
/// Additionally, this bundle can be simulated through a relay if simulation
/// parameters are provided using [`BundleRequest::set_simulation_block`] and
/// [`BundleRequest::set_simulation_timestamp`].
///
/// Please note that some parameters are required, and submitting a bundle
/// without them will get it rejected pre-flight. The required parameters
/// include:
///
/// - At least one transaction ([`BundleRequest::push_transaction`])
/// - A target block ([`BundleRequest::set_target_block`])
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleRequest {
    #[serde(rename = "txs")]
    #[serde(serialize_with = "serialize_txs")]
    transactions: Vec<BundleTransaction>,
    #[serde(rename = "revertingTxHashes")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    revertible_transaction_hashes: Vec<TxHash>,

    #[serde(rename = "accessListHashList")]
    #[serde(skip_serializing_if = "Option::is_none")]
    access_list_hashes: Option<Vec<TxHash>>,

    #[serde(rename = "blockNumber")]
    #[serde(skip_serializing_if = "Option::is_none")]
    target_block: Option<U64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    min_timestamp: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    max_timestamp: Option<u64>,

    #[serde(rename = "stateBlockNumber")]
    #[serde(skip_serializing_if = "Option::is_none")]
    simulation_block: Option<U64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "timestamp")]
    simulation_timestamp: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "baseFee")]
    simulation_basefee: Option<u64>,
}

pub fn serialize_txs<S>(txs: &[BundleTransaction], s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let raw_txs: Result<Vec<Bytes>> = txs
        .iter()
        .map(|tx| match tx {
            BundleTransaction::Signed(inner) => {
                let tx = inner.as_ref().clone();
                Ok(Bytes::from(tx.inner.encoded_2718()))
            }
            BundleTransaction::Raw(inner) => Ok(inner.clone()),
        })
        .collect();

    raw_txs.map_err(S::Error::custom)?.serialize(s)
}

impl BundleRequest {
    /// Creates an empty bundle request.
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds a transaction to the bundle request.
    ///
    /// Transactions added to the bundle can either be novel transactions,
    /// i.e. transactions that you have crafted, or they can be from
    /// one of the mempool APIs.
    pub fn push_transaction<T: Into<BundleTransaction>>(mut self, tx: T) -> Self {
        self.transactions.push(tx.into());
        self
    }

    /// Adds a transaction to the bundle request.
    ///
    /// This function takes a mutable reference to `self` and adds the specified
    /// transaction to the `transactions` vector. The added transaction can either
    /// be a novel transaction that you have crafted, or it can be from one of the
    /// mempool APIs.
    pub fn add_transaction<T: Into<BundleTransaction>>(&mut self, tx: T) {
        self.transactions.push(tx.into());
    }

    /// Adds a revertible transaction to the bundle request.
    ///
    /// This differs from [`BundleRequest::push_transaction`] in that the bundle will still be
    /// considered valid if the transaction reverts.
    pub fn push_revertible_transaction<T: Into<BundleTransaction>>(mut self, tx: T) -> Self {
        let tx = tx.into();
        self.transactions.push(tx.clone());

        let tx_hash: TxHash = match tx {
            BundleTransaction::Signed(inner) => inner.tx_hash(),
            BundleTransaction::Raw(inner) => keccak256(inner),
        };
        self.revertible_transaction_hashes.push(tx_hash);

        self
    }

    /// Adds a revertible transaction to the bundle request.
    ///
    /// This function takes a mutable reference to `self` and adds the specified
    /// revertible transaction to the `transactions` vector. The added transaction can either
    /// be a novel transaction that you have crafted, or it can be from one of the
    /// mempool APIs. Unlike the `push_transaction` method, the bundle will still be considered
    /// valid even if the added transaction reverts.
    pub fn add_revertible_transaction<T: Into<BundleTransaction>>(&mut self, tx: T) {
        let tx = tx.into();
        self.transactions.push(tx.clone());

        let tx_hash: TxHash = match tx {
            BundleTransaction::Signed(inner) => inner.tx_hash(),
            BundleTransaction::Raw(inner) => keccak256(inner),
        };
        self.revertible_transaction_hashes.push(tx_hash);
    }

    /// Get a reference to the transactions currently in the bundle request.
    pub fn transactions(&self) -> &Vec<BundleTransaction> {
        &self.transactions
    }

    /*
    /// Get a list of transaction hashes in the bundle request.
    pub fn transaction_hashes(&self) -> Vec<TxHash> {
        self.transactions
            .iter()
            .map(|tx| match tx {
                BundleTransaction::Signed(inner) => keccak256(inner.as_ref().rlp()).into(),
                BundleTransaction::Raw(inner) => keccak256(inner).into(),
            })
            .collect()
    }

     */

    /// Get the target block (if any).
    pub fn target_block(&self) -> Option<U64> {
        self.target_block
    }

    /// Set the target block of the bundle.
    pub fn set_target_block(mut self, target_block: U64) -> Self {
        self.target_block = Some(target_block);
        self
    }

    /// Get the block that determines the state for bundle simulation (if any).
    ///
    /// See [`eth_callBundle`][fb_call_bundle] in the Flashbots documentation
    /// for more information on bundle simulations.
    ///
    /// [fb_call_bundle]: https://docs.flashbots.net/flashbots-auction/searchers/advanced/rpc-endpoint#eth_callbundle
    pub fn simulation_block(&self) -> Option<U64> {
        self.simulation_block
    }

    /// Set the block that determines the state for bundle simulation.
    pub fn set_simulation_block(mut self, block: U64) -> Self {
        self.simulation_block = Some(block);
        self
    }

    pub fn set_access_list_hashes(mut self, hashes: Option<Vec<TxHash>>) -> Self {
        self.access_list_hashes = hashes;
        self
    }

    /// Get the UNIX timestamp used for bundle simulation (if any).
    ///
    /// See [`eth_callBundle`][fb_call_bundle] in the Flashbots documentation
    /// for more information on bundle simulations.
    ///
    /// [fb_call_bundle]: https://docs.flashbots.net/flashbots-auction/searchers/advanced/rpc-endpoint#eth_callbundle
    pub fn simulation_timestamp(&self) -> Option<u64> {
        self.simulation_timestamp
    }

    /// Set the UNIX timestamp used for bundle simulation.
    pub fn set_simulation_timestamp(mut self, timestamp: u64) -> Self {
        self.simulation_timestamp = Some(timestamp);
        self
    }

    /// Get the base gas fee for bundle simulation (if any).
    ///
    /// See [`eth_callBundle`][fb_call_bundle] in the Flashbots documentation
    /// for more information on bundle simulations.
    ///
    /// [fb_call_bundle]: https://docs.flashbots.net/flashbots-auction/searchers/advanced/rpc-endpoint#eth_callbundle
    pub fn simulation_basefee(&self) -> Option<u64> {
        self.simulation_basefee
    }

    /// Set the base gas fee for bundle simulation (if any).
    /// Optional: will default to a value chosen by the node if not specified.
    pub fn set_simulation_basefee(mut self, basefee: u64) -> Self {
        self.simulation_basefee = Some(basefee);
        self
    }

    /// Get the minimum timestamp for which this bundle is valid (if any),
    /// in seconds since the UNIX epoch.
    pub fn min_timestamp(&self) -> Option<u64> {
        self.min_timestamp
    }

    /// Set the minimum timestamp for which this bundle is valid (if any),
    /// in seconds since the UNIX epoch.
    pub fn set_min_timestamp(mut self, timestamp: u64) -> Self {
        self.min_timestamp = Some(timestamp);
        self
    }

    /// Get the maximum timestamp for which this bundle is valid (if any),
    /// in seconds since the UNIX epoch.
    pub fn max_timestamp(&self) -> Option<u64> {
        self.max_timestamp
    }

    /// Set the maximum timestamp for which this bundle is valid (if any),
    /// in seconds since the UNIX epoch.
    pub fn set_max_timestamp(mut self, timestamp: u64) -> Self {
        self.max_timestamp = Some(timestamp);
        self
    }
}

/// Details of a simulated transaction.
///
/// Details for a transaction that has been simulated as part of
/// a bundle.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimulatedTransaction {
    /// The transaction hash
    #[serde(rename = "txHash")]
    pub hash: TxHash,
    /// The difference in coinbase's balance due to this transaction.
    ///
    /// This includes tips and gas fees for this transaction.
    #[serde(rename = "coinbaseDiff")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub coinbase_diff: U256,
    /// The amount of Eth sent to coinbase in this transaction.
    #[serde(rename = "ethSentToCoinbase")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub coinbase_tip: U256,
    /// The gas price.
    #[serde(rename = "gasPrice")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub gas_price: U256,
    /// The amount of gas used in this transaction.
    #[serde(rename = "gasUsed")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub gas_used: U256,
    /// The total gas fees for this transaction.
    #[serde(rename = "gasFees")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub gas_fees: U256,
    /// The origin of this transaction.
    #[serde(rename = "fromAddress")]
    pub from: Address,
    /// The destination of this transaction.
    ///
    /// If this is `None`, then the transaction was to a newly
    /// deployed contract.
    #[serde(rename = "toAddress")]
    #[serde(deserialize_with = "deserialize_optional_h160")]
    pub to: Option<Address>,
    /// The return value of the transaction.
    pub value: Option<Bytes>,
    /// The reason this transaction failed (if it did).
    pub error: Option<String>,
    /// The revert reason for this transaction, if available.
    pub revert: Option<String>,

    #[serde(rename = "accessList")]
    pub access_list: Option<AccessList>,
    #[serde(rename = "logs")]
    pub logs: Option<Vec<Log>>,
}

impl Display for SimulatedTransaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:#20x}->{:20x} {} Gas : {}  CB : {} {}",
            self.from,
            self.to.unwrap_or_default(),
            self.hash,
            self.gas_used,
            self.coinbase_tip,
            self.coinbase_diff,
        )
    }
}

impl SimulatedTransaction {
    /// The effective gas price of the transaction,
    /// i.e. `coinbase_diff / gas_used`.
    pub fn effective_gas_price(&self) -> U256 {
        self.coinbase_diff / self.gas_used
    }
}

/// Details of a simulated bundle.
///
/// The details of a bundle that has been simulated.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SimulatedBundle {
    /// The bundle's hash.
    #[serde(rename = "bundleHash")]
    pub hash: BundleHash,
    /// The difference in coinbase's balance due to this bundle.
    ///
    /// This includes total gas fees and coinbase tips.
    #[serde(rename = "coinbaseDiff")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub coinbase_diff: U256,
    /// The amount of Eth sent to coinbase in this bundle.
    #[serde(rename = "ethSentToCoinbase")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub coinbase_tip: U256,
    /// The gas price of the bundle.
    #[serde(rename = "bundleGasPrice")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub gas_price: U256,
    /// The total amount of gas used in this bundle.
    #[serde(rename = "totalGasUsed")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub gas_used: U256,
    /// The total amount of gas fees in this bundle.
    #[serde(rename = "gasFees")]
    #[serde(deserialize_with = "deserialize_u256")]
    pub gas_fees: U256,
    /// The block at which this bundle was simulated.
    #[serde(rename = "stateBlockNumber")]
    #[serde(deserialize_with = "deserialize_u64")]
    pub simulation_block: U64,
    /// The simulated transactions in this bundle.
    #[serde(rename = "results")]
    pub transactions: Vec<SimulatedTransaction>,
}

impl SimulatedBundle {
    /// The effective gas price of the transaction,
    /// i.e. `coinbase_diff / gas_used`.
    ///
    /// Note that this is also an approximation of the
    /// bundle's score.
    pub fn effective_gas_price(&self) -> U256 {
        self.coinbase_diff / self.gas_used
    }

    pub fn find_tx(&self, tx_hash: TxHash) -> Option<&SimulatedTransaction> {
        self.transactions.iter().find(|&item| item.hash == tx_hash)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn bundle_serialize() {
        let bundle = BundleRequest::new()
            .push_transaction(Bytes::from(vec![0x1]))
            .push_revertible_transaction(Bytes::from(vec![0x2]))
            .set_target_block(U64::from(2))
            .set_min_timestamp(1000)
            .set_max_timestamp(2000)
            .set_simulation_timestamp(1000)
            .set_simulation_block(U64::from(1))
            .set_simulation_basefee(333333);

        assert_eq!(
            &serde_json::to_string(&bundle).unwrap(),
            r#"{"txs":["0x01","0x02"],"revertingTxHashes":["0xf2ee15ea639b73fa3db9b34a245bdfa015c260c598b211bf05a1ecc4b3e3b4f2"],"blockNumber":"0x2","minTimestamp":1000,"maxTimestamp":2000,"stateBlockNumber":"0x1","timestamp":1000,"baseFee":333333}"#
        );
    }

    #[test]
    fn bundle_serialize_add_transactions() {
        let mut bundle = BundleRequest::new()
            .push_transaction(Bytes::from(vec![0x1]))
            .push_revertible_transaction(Bytes::from(vec![0x2]))
            .set_target_block(U64::from(2))
            .set_min_timestamp(1000)
            .set_max_timestamp(2000)
            .set_simulation_timestamp(1000)
            .set_simulation_block(U64::from(1))
            .set_simulation_basefee(333333);

        bundle.add_transaction(Bytes::from(vec![0x3]));
        bundle.add_revertible_transaction(Bytes::from(vec![0x4]));

        assert_eq!(
            &serde_json::to_string(&bundle).unwrap(),
            r#"{"txs":["0x01","0x02","0x03","0x04"],"revertingTxHashes":["0xf2ee15ea639b73fa3db9b34a245bdfa015c260c598b211bf05a1ecc4b3e3b4f2","0xf343681465b9efe82c933c3e8748c70cb8aa06539c361de20f72eac04e766393"],"blockNumber":"0x2","minTimestamp":1000,"maxTimestamp":2000,"stateBlockNumber":"0x1","timestamp":1000,"baseFee":333333}"#
        );
    }

    #[test]
    fn simulated_bundle_deserialize() {
        let simulated_bundle: SimulatedBundle = serde_json::from_str(
            r#"{
    "bundleGasPrice": "476190476193",
    "bundleHash": "0x73b1e258c7a42fd0230b2fd05529c5d4b6fcb66c227783f8bece8aeacdd1db2e",
    "coinbaseDiff": "20000000000126000",
    "ethSentToCoinbase": "20000000000000000",
    "gasFees": "126000",
    "results": [
      {
        "coinbaseDiff": "10000000000063000",
        "ethSentToCoinbase": "10000000000000000",
        "fromAddress": "0x02A727155aeF8609c9f7F2179b2a1f560B39F5A0",
        "gasFees": "63000",
        "gasPrice": "476190476193",
        "gasUsed": 21000,
        "toAddress": "0x73625f59CAdc5009Cb458B751b3E7b6b48C06f2C",
        "txHash": "0x669b4704a7d993a946cdd6e2f95233f308ce0c4649d2e04944e8299efcaa098a",
        "value": "0x",
        "error": "execution reverted"
      },
      {
        "coinbaseDiff": "10000000000063000",
        "ethSentToCoinbase": "10000000000000000",
        "fromAddress": "0x02A727155aeF8609c9f7F2179b2a1f560B39F5A0",
        "gasFees": "63000",
        "gasPrice": "476190476193",
        "gasUsed": 21000,
        "toAddress": "0x73625f59CAdc5009Cb458B751b3E7b6b48C06f2C",
        "txHash": "0xa839ee83465657cac01adc1d50d96c1b586ed498120a84a64749c0034b4f19fa",
        "value": "0x01"
      },
      {
        "coinbaseDiff": "10000000000063000",
        "ethSentToCoinbase": "10000000000000000",
        "fromAddress": "0x02A727155aeF8609c9f7F2179b2a1f560B39F5A0",
        "gasFees": "63000",
        "gasPrice": "476190476193",
        "gasUsed": 21000,
        "toAddress": "0x",
        "txHash": "0xa839ee83465657cac01adc1d50d96c1b586ed498120a84a64749c0034b4f19fa",
        "value": "0x"
      }
    ],
    "stateBlockNumber": 5221585,
    "totalGasUsed": 42000
  }"#,
        )
        .unwrap();

        assert_eq!(
            simulated_bundle.hash,
            TxHash::from_str("0x73b1e258c7a42fd0230b2fd05529c5d4b6fcb66c227783f8bece8aeacdd1db2e").expect("could not deserialize hash")
        );
        assert_eq!(simulated_bundle.coinbase_diff, U256::from(20000000000126000u64));
        assert_eq!(simulated_bundle.coinbase_tip, U256::from(20000000000000000u64));
        assert_eq!(simulated_bundle.gas_price, U256::from(476190476193u64));
        assert_eq!(simulated_bundle.gas_used, U256::from(42000));
        assert_eq!(simulated_bundle.gas_fees, U256::from(126000));
        assert_eq!(simulated_bundle.simulation_block, U64::from(5221585));
        assert_eq!(simulated_bundle.transactions.len(), 3);
        assert_eq!(simulated_bundle.transactions[0].value, Some(Bytes::from(vec![])));
        assert_eq!(simulated_bundle.transactions[0].error, Some("execution reverted".into()));
        assert_eq!(simulated_bundle.transactions[1].error, None);
        assert_eq!(simulated_bundle.transactions[1].value, Some(Bytes::from(vec![0x1])));
        assert_eq!(simulated_bundle.transactions[2].to, None);
    }

    #[test]
    fn simulated_transaction_deserialize() {
        let tx: SimulatedTransaction = serde_json::from_str(
            r#"{
        "coinbaseDiff": "10000000000063000",
        "ethSentToCoinbase": "10000000000000000",
        "fromAddress": "0x02A727155aeF8609c9f7F2179b2a1f560B39F5A0",
        "gasFees": "63000",
        "gasPrice": "476190476193",
        "gasUsed": 21000,
        "toAddress": "0x",
        "txHash": "0xa839ee83465657cac01adc1d50d96c1b586ed498120a84a64749c0034b4f19fa",
        "error": "execution reverted"
      }"#,
        )
        .unwrap();
        assert_eq!(tx.error, Some("execution reverted".into()));

        let tx: SimulatedTransaction = serde_json::from_str(
            r#"{
        "coinbaseDiff": "10000000000063000",
        "ethSentToCoinbase": "10000000000000000",
        "fromAddress": "0x02A727155aeF8609c9f7F2179b2a1f560B39F5A0",
        "gasFees": "63000",
        "gasPrice": "476190476193",
        "gasUsed": 21000,
        "toAddress": "0x",
        "txHash": "0xa839ee83465657cac01adc1d50d96c1b586ed498120a84a64749c0034b4f19fa",
        "error": "execution reverted",
        "revert": "transfer failed"
      }"#,
        )
        .unwrap();

        assert_eq!(tx.error, Some("execution reverted".into()));
        assert_eq!(tx.revert, Some("transfer failed".into()));
    }
}
