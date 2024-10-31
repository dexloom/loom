use alloy_primitives::TxHash;

#[derive(Debug, Clone, Default)]
pub struct AccountNonceAndTransactions {
    pub nonce: Option<u64>,
    pub txs: Vec<TxHash>,
}

impl AccountNonceAndTransactions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_tx_hash(&mut self, tx_hash: TxHash) -> &mut Self {
        self.txs.push(tx_hash);
        self
    }

    pub fn set_nonce(&mut self, nonce: Option<u64>) -> &mut Self {
        if let Some(cur_nonce) = self.nonce {
            if let Some(some_nonce) = nonce {
                if cur_nonce < some_nonce {
                    self.nonce = Some(some_nonce);
                }
                return self;
            }
        }
        self.nonce = nonce;
        self
    }
}
