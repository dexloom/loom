use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use defi_entities::{PoolProtocol, SwapPath};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SwapLineDTO {
    pub pool_types: Vec<PoolProtocol>,
    pub token_symbols: Vec<String>,
    pub pools: Vec<Address>,
    pub tokens: Vec<Address>,
}


impl Display for SwapLineDTO {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {:?}", self.pool_types, self.token_symbols)
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

impl Hash for SwapLineDTO {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
        self.pools.hash(state);
    }
}