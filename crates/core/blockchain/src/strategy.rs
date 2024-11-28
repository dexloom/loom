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

impl<DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState + DatabaseLoomExt + Send + Sync + Clone + Default + 'static> Default
    for Strategy<DB, LoomDataTypesEthereum>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<DB: DatabaseRef + Database + DatabaseCommit + BlockHistoryState + DatabaseLoomExt + Send + Sync + Clone + Default + 'static>
    Strategy<DB, LoomDataTypesEthereum>
{
    pub fn new() -> Self {
        let compose_channel: Broadcaster<MessageSwapCompose<DB, LoomDataTypesEthereum>> = Broadcaster::new(100);
        let state_update_channel: Broadcaster<StateUpdateEvent<DB, LoomDataTypesEthereum>> = Broadcaster::new(100);
        Strategy { swap_compose_channel: compose_channel, state_update_channel }
    }
}

impl<DB: Send + Sync + Clone + 'static> Strategy<DB, LoomDataTypesEthereum> {
    pub fn swap_compose_channel(&self) -> Broadcaster<MessageSwapCompose<DB, LoomDataTypesEthereum>> {
        self.swap_compose_channel.clone()
    }

    pub fn state_update_channel(&self) -> Broadcaster<StateUpdateEvent<DB, LoomDataTypesEthereum>> {
        self.state_update_channel.clone()
    }
}
