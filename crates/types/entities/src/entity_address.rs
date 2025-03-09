use alloy_primitives::{Address, B256, U256};
use eyre::eyre;
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Shl;

#[derive(Clone, Debug, Serialize)]
pub enum EntityAddress {
    Address(Address),
    Bytes32(B256),
}

impl<'de> Deserialize<'de> for EntityAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let as_u256 = U256::deserialize(deserializer)?;

        if as_u256 < U256::from(1).shl(160) {
            let mut addr_bytes = [0u8; 20];
            addr_bytes.copy_from_slice(&as_u256.to_be_bytes_vec()[12..]);
            Ok(EntityAddress::Address(Address::from(addr_bytes)))
        } else {
            Ok(EntityAddress::Bytes32(B256::from(as_u256)))
        }
    }
}

impl EntityAddress {
    pub fn address(&self) -> eyre::Result<Address> {
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

    pub fn address_or_zero(&self) -> Address {
        if let Self::Address(addr) = self {
            *addr
        } else {
            Address::default()
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

impl Copy for EntityAddress {}

impl Hash for EntityAddress {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Address(addr) => addr.hash(state),
            Self::Bytes32(addr) => addr.hash(state),
        }
    }
}

impl PartialEq<Self> for EntityAddress {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Address(a), Self::Address(b)) => a == b,
            (Self::Bytes32(a), Self::Bytes32(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialOrd for EntityAddress {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EntityAddress {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (EntityAddress::Address(a), EntityAddress::Address(b)) => a.cmp(b),
            (EntityAddress::Bytes32(a), EntityAddress::Bytes32(b)) => a.cmp(b),
            (EntityAddress::Address(a), EntityAddress::Bytes32(b)) => Ordering::Less,
            (EntityAddress::Bytes32(a), EntityAddress::Address(b)) => Ordering::Greater,
        }
    }
}

impl Display for EntityAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Address(a) => write!(f, "{}", a),
            Self::Bytes32(a) => write!(f, "{}", a),
        }
    }
}

impl Eq for EntityAddress {}

impl Default for EntityAddress {
    fn default() -> Self {
        Self::Address(Default::default())
    }
}

impl From<Address> for EntityAddress {
    fn from(addr: Address) -> Self {
        Self::Address(addr)
    }
}

impl From<&Address> for EntityAddress {
    fn from(addr: &Address) -> Self {
        Self::Address(*addr)
    }
}

impl From<B256> for EntityAddress {
    fn from(bytes: B256) -> Self {
        Self::Bytes32(bytes)
    }
}

impl From<[u8; 32]> for EntityAddress {
    fn from(bytes: [u8; 32]) -> Self {
        Self::Bytes32(B256::from(bytes))
    }
}

// impl Into<Address> for EntityAddress {
//     fn into(self) -> Address {
//         self.address_or_zero()
//     }
// }

impl From<EntityAddress> for alloy_primitives::Address {
    #[inline]
    fn from(addr: EntityAddress) -> Self {
        (&addr).into()
    }
}

impl From<&EntityAddress> for alloy_primitives::Address {
    #[inline]
    fn from(addr: &EntityAddress) -> Self {
        addr.address_or_zero()
    }
}
