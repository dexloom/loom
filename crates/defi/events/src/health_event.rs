use alloy_primitives::TxHash;

use loom_defi_types::SwapError;

use crate::Message;

#[derive(Clone, Debug)]
pub enum HealthEvent {
    PoolSwapError(SwapError),
    MonitorTx(TxHash),
}

pub type MessageHealthEvent = Message<HealthEvent>;
