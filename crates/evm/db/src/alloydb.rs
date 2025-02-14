use alloy::eips::BlockId;
use alloy::network::primitives::{BlockTransactionsKind, HeaderResponse};
use alloy::providers::{network::BlockResponse, Network, Provider};
use eyre::ErrReport;
use revm::{
    primitives::{AccountInfo, Address, Bytecode, B256, U256},
    Database, DatabaseRef,
};
use std::future::IntoFuture;
use tokio::runtime::{Handle, Runtime};

#[derive(Debug)]
pub(crate) enum HandleOrRuntime {
    Handle(Handle),
    Runtime(Runtime),
}

impl HandleOrRuntime {
    #[inline]
    pub(crate) fn block_on<F>(&self, f: F) -> F::Output
    where
        F: std::future::Future + Send,
        F::Output: Send,
    {
        match self {
            Self::Handle(handle) => tokio::task::block_in_place(move || handle.block_on(f)),
            Self::Runtime(rt) => rt.block_on(f),
        }
    }
}

/// An alloy-powered REVM [Database].
///
/// When accessing the database, it'll use the given provider to fetch the corresponding account's data.
#[derive(Debug)]
pub struct AlloyDB<N: Network, P: Provider<N>> {
    /// The provider to fetch the data from.
    provider: P,
    /// The block number on which the queries will be based on.
    block_number: BlockId,
    /// handle to the tokio runtime
    rt: HandleOrRuntime,
    _marker: std::marker::PhantomData<fn() -> N>,
}

/*impl<T: Transport + Clone, N: Network, P: Provider<T, N>> Default for  AlloyDB<T, N, P> {
    fn default() -> Self {
        let provider = ProviderBuilder::new().with_recommended_fillers().on_anvil();
        let alloy_db = AlloyDB::new(provider, BlockNumberOrTag::Latest.into()).unwrap();
        alloy_db
    }

}

 */

#[allow(dead_code)]
impl<N: Network, P: Provider<N>> AlloyDB<N, P> {
    /// Create a new AlloyDB instance, with a [Provider] and a block.
    ///
    /// Returns `None` if no tokio runtime is available or if the current runtime is a current-thread runtime.
    pub fn new(provider: P, block_number: BlockId) -> Option<Self> {
        let rt = match Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                tokio::runtime::RuntimeFlavor::CurrentThread => return None,
                _ => HandleOrRuntime::Handle(handle),
            },
            Err(_) => return None,
        };
        Some(Self { provider, block_number, rt, _marker: std::marker::PhantomData })
    }

    /// Create a new AlloyDB instance, with a provider and a block and a runtime.
    ///
    /// Refer to [tokio::runtime::Builder] on how to create a runtime if you are in synchronous world.
    /// If you are already using something like [tokio::main], call AlloyDB::new instead.
    pub fn with_runtime(provider: P, block_number: BlockId, runtime: Runtime) -> Self {
        let rt = HandleOrRuntime::Runtime(runtime);
        Self { provider, block_number, rt, _marker: std::marker::PhantomData }
    }

    /// Create a new AlloyDB instance, with a provider and a block and a runtime handle.
    ///
    /// This generally allows you to pass any valid runtime handle, refer to [tokio::runtime::Handle] on how
    /// to obtain a handle. If you are already in asynchronous world, like [tokio::main], use AlloyDB::new instead.
    pub fn with_handle(provider: P, block_number: BlockId, handle: Handle) -> Self {
        let rt = HandleOrRuntime::Handle(handle);
        Self { provider, block_number, rt, _marker: std::marker::PhantomData }
    }

    /// Internal utility function that allows us to block on a future regardless of the runtime flavor.
    #[inline]
    fn block_on<F>(&self, f: F) -> F::Output
    where
        F: std::future::Future + Send,
        F::Output: Send,
    {
        self.rt.block_on(f)
    }

    /// Set the block number on which the queries will be based on.
    pub fn set_block_number(&mut self, block_number: BlockId) {
        self.block_number = block_number;
    }
}

impl<N: Network, P: Provider<N>> DatabaseRef for AlloyDB<N, P> {
    type Error = ErrReport;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let f = async {
            let nonce = self.provider.get_transaction_count(address).block_id(self.block_number);
            let balance = self.provider.get_balance(address).block_id(self.block_number);
            let code = self.provider.get_code_at(address).block_id(self.block_number);
            tokio::join!(nonce.into_future(), balance.into_future(), code.into_future())
        };

        let (nonce, balance, code) = self.block_on(f);

        let balance = balance?;
        let code = Bytecode::new_raw(code?.0.into());
        let code_hash = code.hash_slow();
        let nonce = nonce?;

        Ok(Some(AccountInfo::new(balance, nonce, code_hash, code)))
    }

    fn code_by_hash_ref(&self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        panic!("This should not be called, as the code is already loaded");
        // This is not needed, as the code is already loaded with basic_ref
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let f = self.provider.get_storage_at(address, index).block_id(self.block_number);
        let slot_val = self.block_on(f.into_future())?;
        Ok(slot_val)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        let block = self.block_on(
            self.provider
                // SAFETY: We know number <= u64::MAX, so we can safely convert it to u64
                .get_block_by_number(number.into(), BlockTransactionsKind::Hashes),
        )?;
        // SAFETY: If the number is given, the block is supposed to be finalized, so unwrapping is safe.
        Ok(B256::new(*block.unwrap().header().hash()))
    }
}

impl<N: Network, P: Provider<N>> Database for AlloyDB<N, P> {
    type Error = ErrReport;

    #[inline]
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        <Self as DatabaseRef>::basic_ref(self, address)
    }

    #[inline]
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        <Self as DatabaseRef>::code_by_hash_ref(self, code_hash)
    }

    #[inline]
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        <Self as DatabaseRef>::storage_ref(self, address, index)
    }

    #[inline]
    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        <Self as DatabaseRef>::block_hash_ref(self, number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::providers::ProviderBuilder;
    use std::env;
    use url::Url;

    #[test]
    #[ignore = "flaky RPC"]
    fn can_get_basic() {
        let node_url = env::var("MAINNET_HTTP").unwrap();
        let node_url = Url::parse(node_url.as_str()).unwrap();

        let client = ProviderBuilder::new().on_http(node_url);
        let alloydb = AlloyDB::new(client, BlockId::from(16148323));

        if alloydb.is_none() {
            println!("Alloydb is None");
        }

        // ETH/USDT pair on Uniswap V2
        let address: Address = "0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852".parse().unwrap();

        let acc_info = alloydb.unwrap().basic_ref(address).unwrap().unwrap();
        assert!(acc_info.exists());
    }
}
