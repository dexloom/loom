use alloy_primitives::{Address, B256, U256};
use eyre::eyre;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Shl;

#[derive(Clone, Debug, Serialize)]
pub enum PoolId<LDT: LoomDataTypes = LoomDataTypesEthereum>
where
    LDT::Address: Eq + Hash,
{
    Address(LDT::Address),
    Bytes32(B256),
}

impl<'de> Deserialize<'de> for PoolId<LoomDataTypesEthereum> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let as_u256 = U256::deserialize(deserializer)?;

        // Convert B256 to U256 for comparison
        // let as_u256 = U256::try_from(b256).map_err(|_| D::Error::custom("ERROR"))?;

        // If below the boundary, interpret it as an address
        if as_u256 < U256::from(1).shl(160) {
            // Take the rightmost 20 bytes as the address
            let mut addr_bytes = [0u8; 20];
            addr_bytes.copy_from_slice(&as_u256.to_be_bytes_vec()[12..]);
            Ok(PoolId::Address(Address::from(addr_bytes)))
        } else {
            // Otherwise, treat it as a full 32‑byte hash
            Ok(PoolId::Bytes32(B256::from(as_u256)))
        }
    }
}

impl<LDT: LoomDataTypes> PoolId<LDT> {
    pub fn address(&self) -> eyre::Result<LDT::Address> {
        if let Self::Address(addr) = self {
            Ok(*addr)
        } else {
            Err(eyre!("NOT_ADDRESS"))
        }
    }

    pub fn bytes32(&self) -> eyre::Result<B256> {
        if let Self::Bytes32(bytes32) = self {
            Ok(*bytes32)
        } else {
            Err(eyre!("NOT_BYTES32"))
        }
    }

    pub fn address_or_zero(&self) -> LDT::Address {
        if let Self::Address(addr) = self {
            *addr
        } else {
            LDT::Address::default()
        }
    }

    pub fn bytes_or_zero(&self) -> B256 {
        if let Self::Bytes32(addr) = self {
            *addr
        } else {
            B256::ZERO
        }
    }
}

impl<LDT: LoomDataTypes> Copy for PoolId<LDT> {}

impl<LDT: LoomDataTypes> Hash for PoolId<LDT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Address(addr) => addr.hash(state),
            Self::Bytes32(addr) => addr.hash(state),
        }
    }
}

impl<LDT: LoomDataTypes> PartialEq<Self> for PoolId<LDT> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Address(a), Self::Address(b)) => a == b,
            (Self::Bytes32(a), Self::Bytes32(b)) => a == b,
            _ => false,
        }
    }
}

impl<LDT: LoomDataTypes> PartialOrd for PoolId<LDT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<LDT: LoomDataTypes> Ord for PoolId<LDT> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PoolId::Address(a), PoolId::Address(b)) => a.cmp(b),
            (PoolId::Bytes32(a), PoolId::Bytes32(b)) => a.cmp(b),
            (PoolId::Address(a), PoolId::Bytes32(b)) => Ordering::Less,
            (PoolId::Bytes32(a), PoolId::Address(b)) => Ordering::Greater,
        }
    }
}

impl<LDT: LoomDataTypes> Display for PoolId<LDT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Address(a) => write!(f, "{}", a),
            Self::Bytes32(a) => write!(f, "{}", a),
        }
    }
}

impl<LDT: LoomDataTypes> Eq for PoolId<LDT> {}

impl<LDT: LoomDataTypes> Default for PoolId<LDT> {
    fn default() -> Self {
        Self::Address(Default::default())
    }
}

impl From<Address> for PoolId {
    fn from(addr: Address) -> Self {
        Self::Address(addr)
    }
}

impl From<B256> for PoolId {
    fn from(bytes: B256) -> Self {
        Self::Bytes32(bytes)
    }
}

impl From<[u8; 32]> for PoolId {
    fn from(bytes: [u8; 32]) -> Self {
        Self::Bytes32(B256::from(bytes))
    }
}
