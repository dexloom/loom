use alloy_json_rpc::RpcError;
use alloy_network::Ethereum;
use alloy_provider::Provider;
use alloy_signer_local::PrivateKeySigner;
use alloy_transport::TransportErrorKind;
use eyre::Result;
use thiserror::Error;
use url::Url;

use crate::client::SendBundleResponseType;
use crate::{
    client::bundle::{BundleRequest, SimulatedBundle},
    client::relay::{Relay, RelayError},
};

/// Errors for the Flashbots middleware.
#[derive(Debug, Error)]
pub enum FlashbotsMiddlewareError {
    /// Some parameters were missing.
    ///
    /// For bundle simulation, check that the following are set:
    /// - `simulation_block`
    /// - `simulation_timestamp`
    /// - `block`
    ///
    /// For bundle submission, check that the following are set:
    /// - `block`
    ///
    /// Additionally, `min_timestamp` and `max_timestamp` must
    /// both be set or unset.
    #[error("Some parameters were missing")]
    MissingParameters,
    /// The relay responded with an error.
    #[error(transparent)]
    RelayError(#[from] RelayError),
    /// An error occured in one of the middlewares.
    #[error(transparent)]
    MiddlewareError(#[from] RpcError<TransportErrorKind>),
}

#[derive(Clone)]
pub struct FlashbotsMiddleware<P> {
    provider: P,
    relay: Relay,
    simulation_relay: Option<Relay>,
}

impl<P> FlashbotsMiddleware<P>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
{
    /// Initialize a new Flashbots middleware.
    ///
    /// The signer is used to sign requests to the relay.
    pub fn new(relay_url: impl Into<Url>, provider: P) -> Self {
        Self { provider, relay: Relay::new(relay_url, Some(PrivateKeySigner::random())), simulation_relay: None }
    }

    pub fn new_no_signer(relay_url: impl Into<Url>, provider: P) -> Self {
        Self { provider, relay: Relay::new(relay_url, None), simulation_relay: None }
    }

    /// Get the relay client used by the middleware.
    pub fn relay(&self) -> &Relay {
        &self.relay
    }

    /// Get the relay client used by the middleware to simulate
    /// bundles if set.
    pub fn simulation_relay(&self) -> Option<&Relay> {
        self.simulation_relay.as_ref()
    }

    /// Set a separate relay to use for simulating bundles.
    ///
    /// This can either be a full Flashbots relay or a node that implements
    /// the `eth_callBundle` remote procedure call.
    pub fn set_simulation_relay(&mut self, relay_url: impl Into<Url>) {
        self.simulation_relay = Some(Relay::new(relay_url, None));
    }

    /// Simulate a bundle.
    ///
    /// See [`eth_callBundle`][fb_callBundle] for more information.
    ///
    /// [fb_callBundle]: https://docs.flashbots.net/flashbots-auction/searchers/advanced/rpc-endpoint#eth_callbundle
    pub async fn simulate_bundle(&self, bundle: &BundleRequest) -> Result<SimulatedBundle, FlashbotsMiddlewareError> {
        bundle
            .target_block()
            .and(bundle.simulation_block())
            .and(bundle.simulation_timestamp())
            .ok_or(FlashbotsMiddlewareError::MissingParameters)?;

        self.simulation_relay
            .as_ref()
            .unwrap_or(&self.relay)
            .request("eth_callBundle", [bundle])
            .await
            .map_err(FlashbotsMiddlewareError::RelayError)
    }

    pub async fn simulate_local_bundle(&self, bundle: &BundleRequest) -> Result<SimulatedBundle, FlashbotsMiddlewareError> {
        match self.provider.client().request("eth_callBundle", [bundle]).await {
            Ok(result) => Ok(result),
            Err(e) => Err(FlashbotsMiddlewareError::MiddlewareError(e)),
        }
    }

    /// Send a bundle to the relayer.
    ///
    /// See [`eth_sendBundle`][fb_sendBundle] for more information.
    ///
    /// [fb_sendBundle]: https://docs.flashbots.net/flashbots-auction/searchers/advanced/rpc-endpoint#eth_sendbundle
    pub async fn send_bundle(&self, bundle: &BundleRequest) -> Result<(), FlashbotsMiddlewareError> {
        // The target block must be set
        bundle.target_block().ok_or(FlashbotsMiddlewareError::MissingParameters)?;

        // `min_timestamp` and `max_timestamp` must both either be unset or set.
        if bundle.min_timestamp().xor(bundle.max_timestamp()).is_some() {
            return Err(FlashbotsMiddlewareError::MissingParameters);
        }

        let _response: SendBundleResponseType =
            self.relay.request("eth_sendBundle", [bundle]).await.map_err(FlashbotsMiddlewareError::RelayError)?;

        Ok(())
    }
}
