# State management
Loom provides multiple ways to fetch the state and keep it up to date. The state can be fetched using different methods as described in the following section.

## Receiving new state
Loom is allow to fetch the state using three different methods:
- **WS/IPC based**: Subscribing to new events using WebSocket or IPC. For each new event a `debug_trace_block` call is made to get the state diff.
- **Direct DB**: Subscribing to new events like before using WebSocket or IPC, but fetching the state diff directly from the DB.
- **ExEx**: Subscribing to new ExEx events and reading the execution outcome from reth.

<div align="center">

![Receiving new state](../images/receive_new_state.svg)

</div>


## Adding new state to the DB
Loom keeps all required state in-memory and optionally fetches missing state from an external database provider. The `LoomDB` is split in three parts to be efficient cloneable. The first part is mutable where every new or changed state will be added.

With each new block a background task will be spawned that merges all state to the inner read-only `LoomDB`. This inner `LoomDB` lives inside an `Arc`. The motivation is here to not wait for the merge and save costs for not cloning the whole state all the time.

The third part in a `DatabaseRef` to an external database provider. This is used to fetch missing state that was not prefetched. Both parts are optional e.g. for testing if the prefetched state is working correct.

<div align="center">

![Receiving new state](../images/loom_db.svg)

</div>