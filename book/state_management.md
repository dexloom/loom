# State management
Loom provides multiple ways to fetch the state and keep it up to date. The state can be fetched using different methods as described in the following section.

## Receiving new state
Loom is allow to fetch the state using three different methods:
- **WS/IPC based**: Subscribing to new events using WebSocket or IPC. For each new event a `debug_trace_block` call is made to get the state diff.
- **Direct DB**: Subscribing to new events like before using WebSocket or IPC, but fetching the state diff directly from the DB.
- **ExEx**: Subscribing to new ExEx events and reading the execution outcome from reth.

<div align="center">

![Receiving new state](images/receive_new_state.svg)

</div>


## Processing the state
Loom has its own database that implements the revm traits.