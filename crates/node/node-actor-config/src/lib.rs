#[derive(Debug, Clone)]
pub struct NodeBlockActorConfig {
    pub block_header: bool,
    pub block_with_tx: bool,
    pub block_logs: bool,
    pub block_state_update: bool,
}

impl NodeBlockActorConfig {
    pub fn all_disabled() -> Self {
        Self { block_header: false, block_with_tx: false, block_logs: false, block_state_update: false }
    }

    pub fn all_enabled() -> Self {
        Self { block_header: true, block_with_tx: true, block_logs: true, block_state_update: true }
    }

    pub fn with_block_header(mut self) -> Self {
        self.block_header = true;
        self
    }

    pub fn with_block_with_tx(mut self) -> Self {
        self.block_with_tx = true;
        self
    }

    pub fn with_block_logs(mut self) -> Self {
        self.block_logs = true;
        self
    }

    pub fn with_block_state_update(mut self) -> Self {
        self.block_state_update = true;
        self
    }
}
