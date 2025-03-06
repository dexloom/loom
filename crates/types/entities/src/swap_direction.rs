use crate::PoolId;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Clone, Debug)]
pub struct SwapDirection<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    token_from: LDT::Address,
    token_to: LDT::Address,
}

impl<LDT: LoomDataTypes> SwapDirection<LDT> {
    #[inline]
    pub fn new(token_from: LDT::Address, token_to: LDT::Address) -> Self {
        Self { token_from, token_to }
    }

    #[inline]
    pub fn from(&self) -> &LDT::Address {
        &self.token_from
    }
    #[inline]
    pub fn to(&self) -> &LDT::Address {
        &self.token_to
    }

    #[inline]
    pub fn get_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    #[inline]
    pub fn get_hash_with_pool(&self, pool_id: &PoolId<LDT>) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        pool_id.hash(&mut hasher);
        hasher.finish()
    }
}

impl<LDT: LoomDataTypes> Hash for SwapDirection<LDT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.token_from.hash(state);
        self.token_to.hash(state);
    }
}

impl<LDT: LoomDataTypes> PartialEq for SwapDirection<LDT> {
    fn eq(&self, other: &Self) -> bool {
        self.token_from.eq(&other.token_from) && self.token_to.eq(&other.token_to)
    }
}

impl<LDT: LoomDataTypes> Eq for SwapDirection<LDT> {}

impl<LDT: LoomDataTypes> From<(LDT::Address, LDT::Address)> for SwapDirection<LDT> {
    fn from(value: (LDT::Address, LDT::Address)) -> Self {
        Self { token_from: value.0, token_to: value.1 }
    }
}
