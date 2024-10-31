use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use loom_types_entities::{PoolProtocol, SwapPath};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapLineDTO {
    pub pool_types: Vec<PoolProtocol>,
    pub token_symbols: Vec<String>,
    pub pools: Vec<Address>,
    pub tokens: Vec<Address>,
}

// Implement the Ord and PartialOrd traits for X
impl Ord for SwapLineDTO {
    fn cmp(&self, other: &Self) -> Ordering {
        let len_cmp = self.pools.len().cmp(&other.pools.len());
        if len_cmp != Ordering::Equal {
            return len_cmp;
        }

        // If lengths are equal, compare element-wise
        for (a, b) in self.pool_types.iter().zip(other.pool_types.iter()) {
            let adr_cmp = a.to_string().cmp(&b.to_string());
            if adr_cmp != Ordering::Equal {
                return adr_cmp;
            }
        }

        Ordering::Equal
    }
}

impl PartialOrd for SwapLineDTO {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Implement Eq and PartialEq for completeness
impl PartialEq for SwapLineDTO {
    fn eq(&self, other: &Self) -> bool {
        self.pools == other.pools
    }
}

impl Eq for SwapLineDTO {}

impl Display for SwapLineDTO {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let symbols = self.token_symbols.join("->");
        write!(f, "{:?} [{}]", self.pool_types, symbols)
    }
}

impl From<&SwapPath> for SwapLineDTO {
    fn from(value: &SwapPath) -> Self {
        Self {
            pool_types: value.pools.iter().map(|x| x.get_protocol()).collect(),
            token_symbols: value.tokens.iter().map(|x| x.get_symbol()).collect(),
            pools: value.pools.iter().map(|x| x.get_address()).collect(),
            tokens: value.tokens.iter().map(|x| x.get_address()).collect(),
        }
    }
}

impl From<SwapPath> for SwapLineDTO {
    fn from(value: SwapPath) -> Self {
        Self {
            pool_types: value.pools.iter().map(|x| x.get_protocol()).collect(),
            token_symbols: value.tokens.iter().map(|x| x.get_symbol()).collect(),
            pools: value.pools.iter().map(|x| x.get_address()).collect(),
            tokens: value.tokens.iter().map(|x| x.get_address()).collect(),
        }
    }
}

impl Hash for SwapLineDTO {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
        self.pools.hash(state);
    }
}
