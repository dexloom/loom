# Custom messages
If you like to add new messages without modifying Loom you can easily add a custom struct like `Blockchain` to keep the references for states and channels.

## Custom Blockchain
```rust,ignore
pub struct CustomBlockchain {
    custom_channel: Broadcaster<CustomMessage>,
}

impl CustomBlockchain {
    pub fn new() -> Self {
        Self {
            custom_channel: Broadcaster::new(10),
        }
    }
    pub fn custom_channel(&self) -> Broadcaster<CustomMessage> {
        self.custom_channel.clone()
    }
}
```

## Custom Actor
Allow to set custom struct in your `Actor`:

```rust,ignore
#[derive(Consumer)]
pub struct ExampleActor {
    #[consumer]
    custom_channel_rx: Option<Broadcaster<CustomMessage>>,
}

impl Actor for ExampleActor {
    pub fn on_custom_bc(self, custom_bc: &CustomBlockchain) -> Self {
        Self { custom_channel_tx: Some(custom_bc.custom_channel()), ..self }
    }
}
```

## Start custom actor
When loading your custom actor, you can set the custom blockchain:

```rust,ignore
let custom_bc = CustomBlockchain::new();
let mut bc_actors = BlockchainActors::new(provider.clone(), bc.clone(), relays);
bc_actors.start(ExampleActor::new().on_custom_bc(&custom_bc))?;
```