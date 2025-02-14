use crate::client::{
    make_signed_body, BundleRequest, BundleTransaction, FlashbotsMiddleware, FlashbotsMiddlewareError, RelayConfig, SendBundleResponseType,
    SimulatedBundle,
};
use alloy_network::Ethereum;
use alloy_primitives::{TxHash, U64};
use alloy_provider::Provider;
use alloy_signer_local::PrivateKeySigner;
use eyre::{eyre, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info};
use url::Url;

#[derive(Clone)]
pub struct FlashbotsClient<T> {
    pub flashbots_middleware: FlashbotsMiddleware<T>,
    pub name: String,
}

impl<P> FlashbotsClient<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(provider: P, url: &str) -> Self {
        let flashbots_middleware = Self::create_flashbots_middleware(provider, url);

        let name = url.to_string();

        FlashbotsClient { flashbots_middleware, name }
    }

    pub fn new_no_sign(provider: P, url: &str) -> Self {
        let flashbots_client = FlashbotsClient::create_flashbots_no_signer_middleware(provider, url);

        let name = url.to_string();

        FlashbotsClient { flashbots_middleware: flashbots_client, name }
    }

    fn create_flashbots_middleware(provider: P, url: &str) -> FlashbotsMiddleware<P> {
        let flashbots: FlashbotsMiddleware<P> = FlashbotsMiddleware::new(Url::parse(url).unwrap(), provider);

        flashbots
    }

    fn create_flashbots_no_signer_middleware(provider: P, url: &str) -> FlashbotsMiddleware<P> {
        let flashbots: FlashbotsMiddleware<P> = FlashbotsMiddleware::new_no_signer(Url::parse(url).unwrap(), provider);
        flashbots
    }

    pub async fn call_bundle(&self, request: &BundleRequest) -> Result<SimulatedBundle> {
        match self.flashbots_middleware.simulate_local_bundle(request).await {
            Ok(x) => Ok(x),
            Err(e) => {
                error!("{}", e);
                Err(eyre!("FLASHBOTS LOCAL ERROR"))
            }
        }
    }

    #[allow(dead_code)]
    pub async fn send_bundle(&self, request: &BundleRequest) -> Result<()> {
        match self.flashbots_middleware.send_bundle(request).await {
            Ok(_resp) => {
                info!("Bundle sent to : {}", self.name);
                Ok(())
            }
            Err(error) => match error {
                FlashbotsMiddlewareError::MissingParameters => {
                    error!("{} : Missing paramter", self.name);
                    Err(eyre!("FLASHBOTS_MISSING_PARAMETER"))
                }
                FlashbotsMiddlewareError::RelayError(x) => {
                    error!("{} {}", self.name, x.to_string());
                    Err(eyre!("FLASHBOTS_RELAY_ERROR"))
                }
                FlashbotsMiddlewareError::MiddlewareError(x) => {
                    error!("{} {}", self.name, x.to_string());
                    Err(eyre!("FLASHBOTS_MIDDLEWARE_ERROR"))
                }
            },
        }
    }

    pub async fn send_signed_body(&self, body: String, signature: String) -> Result<()> {
        match self.flashbots_middleware.relay().serialized_request::<SendBundleResponseType>(body, Some(signature)).await {
            Ok(_resp) => {
                debug!("Bundle sent to : {}", self.name);
                Ok(())
            }
            Err(error) => {
                error!("{} {}", self.name, error.to_string());
                Err(eyre!("FLASHBOTS_RELAY_ERROR"))
            }
        }
    }
}

pub struct Flashbots<P> {
    req_id: AtomicU64,
    signer: PrivateKeySigner,
    provider: P,
    simulation_client: FlashbotsClient<P>,
    clients: Vec<Arc<FlashbotsClient<P>>>,
}

impl<P> Flashbots<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(provider: P, simulation_endpoint: &str, signer: Option<PrivateKeySigner>) -> Self {
        let signer = signer.unwrap_or(PrivateKeySigner::random());
        let simulation_client = FlashbotsClient::new(provider.clone(), simulation_endpoint);

        Flashbots { req_id: AtomicU64::new(0), signer, provider, clients: vec![], simulation_client }
    }

    pub fn with_default_relays(self) -> Self {
        let provider = self.provider.clone();

        let flashbots = FlashbotsClient::new(provider.clone(), "https://relay.flashbots.net");
        let beaverbuild = FlashbotsClient::new(provider.clone(), "https://rpc.beaverbuild.org/");
        let titan = FlashbotsClient::new(provider.clone(), "https://rpc.titanbuilder.xyz");
        let rsync = FlashbotsClient::new(provider.clone(), "https://rsync-builder.xyz");
        //let builder0x69 = FlashbotsClient::new_no_sign(provider.clone(), "https://builder0x69.io");
        let eden = FlashbotsClient::new(provider.clone(), "https://api.edennetwork.io/v1/bundle");
        let eth_builder = FlashbotsClient::new_no_sign(provider.clone(), "https://eth-builder.com");
        let secureapi = FlashbotsClient::new_no_sign(provider.clone(), "https://api.securerpc.com/v1");
        //let blocknative = FlashbotsClient::new(provider.clone(), "https://api.blocknative.com/v1/auction");
        let buildai = FlashbotsClient::new_no_sign(provider.clone(), "https://BuildAI.net");
        let payloadde = FlashbotsClient::new_no_sign(provider.clone(), "https://rpc.payload.de");
        let fibio = FlashbotsClient::new(provider.clone(), "https://rpc.f1b.io");
        let loki = FlashbotsClient::new(provider.clone(), "https://rpc.lokibuilder.xyz");
        let ibuilder = FlashbotsClient::new(provider.clone(), "https://rpc.ibuilder.xyz");
        let jetbuilder = FlashbotsClient::new(provider.clone(), "https://rpc.jetbldr.xyz");
        let penguinbuilder = FlashbotsClient::new(provider.clone(), "https://rpc.penguinbuild.org");
        let gambitbuilder = FlashbotsClient::new(provider.clone(), "https://builder.gmbit.co/rpc");

        let clients_vec = vec![
            flashbots,
            /* builder0x69,*/ titan,
            fibio,
            eden,
            eth_builder,
            beaverbuild,
            secureapi,
            rsync,
            /*blocknative,*/ buildai,
            payloadde,
            loki,
            ibuilder,
            jetbuilder,
            penguinbuilder,
            gambitbuilder,
        ];

        let clients = clients_vec.into_iter().map(Arc::new).collect();

        Self { clients, ..self }
    }

    pub fn with_relay(self, url: &str) -> Self {
        let mut clients = self.clients;
        clients.push(Arc::new(FlashbotsClient::new(self.provider.clone(), url)));
        Self { clients, ..self }
    }

    pub fn with_relays(self, relays: Vec<RelayConfig>) -> Self {
        let clients: Vec<Arc<FlashbotsClient<P>>> = relays
            .into_iter()
            .map(|relay| {
                if relay.no_sign.unwrap_or(false) {
                    Arc::new(FlashbotsClient::new_no_sign(self.provider.clone(), relay.url.as_str()))
                } else {
                    Arc::new(FlashbotsClient::new(self.provider.clone(), relay.url.as_str()))
                }
            })
            .collect();
        Self { clients, ..self }
    }

    pub async fn simulate_txes<TX>(
        &self,
        txs: Vec<TX>,
        block_number: u64,
        access_list_request: Option<Vec<TxHash>>,
    ) -> Result<SimulatedBundle>
    where
        BundleTransaction: From<TX>,
    {
        let mut bundle = BundleRequest::new()
            .set_target_block(U64::from(block_number + 1))
            .set_simulation_block(U64::from(block_number))
            .set_access_list_hashes(access_list_request);

        for t in txs.into_iter() {
            bundle = bundle.push_transaction(t);
        }

        self.simulation_client.call_bundle(&bundle).await
    }

    pub async fn broadcast_txes<TX>(&self, txs: Vec<TX>, target_block: u64) -> Result<()>
    where
        BundleTransaction: From<TX>,
    {
        let mut bundle = BundleRequest::new().set_target_block(U64::from(target_block));

        for t in txs.into_iter() {
            bundle = bundle.push_transaction(t);
        }

        let next_req_id = self.req_id.load(Ordering::SeqCst) + 1;
        self.req_id.store(next_req_id, Ordering::SeqCst);

        let (body, signature) = make_signed_body(next_req_id, "eth_sendBundle", bundle, &self.signer)?;

        for client in self.clients.iter() {
            let client_clone = client.clone();
            let body_clone = body.clone();
            let signature_clone = signature.clone();

            tokio::task::spawn(async move {
                debug!("Sending bundle to {}", client_clone.name);
                let bundle_result = client_clone.send_signed_body(body_clone, signature_clone).await;
                match bundle_result {
                    Ok(_) => {
                        debug!("Flashbots bundle broadcast successfully {}", client_clone.name);
                    }
                    Err(x) => {
                        error!("Broadcasting error to {} : {}", client_clone.name, x.to_string());
                    }
                }
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::Bytes;
    use alloy_provider::ProviderBuilder;
    use std::env;

    use super::*;

    #[tokio::test]
    async fn test_client_send_bundle() -> Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or("debug,flashbots=off"));
        let node_url = Url::try_from(env::var("MAINNET_HTTP")?.as_str())?;

        let provider = ProviderBuilder::new().disable_recommended_fillers().on_http(node_url);
        let block = provider.get_block_number().await?;

        let flashbots_client = FlashbotsClient::new(provider.clone(), "https://relay.flashbots.net");

        let tx = Bytes::from(vec![1, 1, 1, 1]);

        let bundle_request = BundleRequest::new().set_target_block(U64::from(block)).push_transaction(tx);

        match flashbots_client.send_bundle(&bundle_request).await {
            Ok(resp) => {
                debug!("{:?}", resp);
                panic!("SHOULD_FAIL");
            }
            Err(e) => {
                debug!("{}", e);
                assert_eq!(e.to_string(), "FLASHBOTS_RELAY_ERROR".to_string());
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_send_bundle() -> Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or("trace"));
        let node_url = Url::try_from(env::var("MAINNET_HTTP")?.as_str())?;

        let provider = ProviderBuilder::new().disable_recommended_fillers().on_http(node_url);
        let block = provider.get_block_number().await?;

        let flashbots_client =
            Flashbots::new(provider.clone(), "https://relay.flashbots.net", None).with_relay("https://relay.flashbots.net");

        let tx = Bytes::from(vec![1, 1, 1, 1]);

        match flashbots_client.broadcast_txes(vec![tx], block).await {
            Ok(_resp) => {}
            Err(e) => {
                error!("{}", e);
                panic!("SHOULD_NOT_FAIL");
            }
        }

        Ok(())
    }
}
