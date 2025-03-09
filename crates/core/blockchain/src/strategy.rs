use loom_core_actors::Broadcaster;
use loom_evm_db::DatabaseLoomExt;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::BlockHistoryState;
use loom_types_events::{MessageSwapCompose, StateUpdateEvent};
use revm::{Database, DatabaseCommit, DatabaseRef};

#[derive(Clone)]
pub struct Strategy<DB: Clone + Send + Sync + 'static, LDT: LoomDataTypes + 'static = LoomDataTypesEthereum> {
    swap_compose_channel: Broadcaster<MessageSwapCompose<DB, LDT>>,
    state_update_channel: Broadcaster<StateUpdateEvent<DB, LDT>>,
}

impl<
        DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState<LDT> + DatabaseLoomExt + Send + Sync + Clone + Default + 'static,
        LDT: LoomDataTypes,
    > Strategy<DB, LDT>
{
    pub fn new() -> Self {
        let compose_channel: Broadcaster<MessageSwapCompose<DB, LDT>> = Broadcaster::new(100);
        let state_update_channel: Broadcaster<StateUpdateEvent<DB, LDT>> = Broadcaster::new(100);
        Strategy { swap_compose_channel: compose_channel, state_update_channel }
    }
}

impl<DB: Send + Sync + Clone + 'static, LDT: LoomDataTypes> Strategy<DB, LDT> {
    pub fn swap_compose_channel(&self) -> Broadcaster<MessageSwapCompose<DB, LDT>> {
        self.swap_compose_channel.clone()
    }

    pub fn state_update_channel(&self) -> Broadcaster<StateUpdateEvent<DB, LDT>> {
        self.state_update_channel.clone()
    }
}
