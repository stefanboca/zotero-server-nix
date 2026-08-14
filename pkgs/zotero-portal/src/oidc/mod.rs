mod authorization;

use anyhow::Context as _;
use std::sync::Arc;

use jiff::Timestamp;
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse as _, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse as _,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata, CoreUserInfoClaims},
};

use crate::config::Config;

pub type OidcClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub struct Oidc {
    config: Arc<Config>,
    client: OidcClient,
    http_client: reqwest::Client,
}

pub struct OidcAuthorization {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct OidcFlow {
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub session_token: Option<String>,
    pub return_path: String,
    pub expires_at: Timestamp,
}

#[derive(Clone)]
pub struct OidcProfile {
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub username: Option<String>,
    pub real_name: Option<String>,
    pub email_verified: bool,
}

impl Oidc {
    pub async fn new(config: Arc<Config>, client_secret: String) -> anyhow::Result<Self> {
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("building OIDC HTTP client")?;
        let provider = CoreProviderMetadata::discover_async(
            IssuerUrl::new(config.oidc.issuer.clone())?,
            &http_client,
        )
        .await
        .context("discovering OIDC provider")?;
        let client = CoreClient::from_provider_metadata(
            provider,
            ClientId::new(config.oidc.client_id.clone()),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.oidc.redirect_url.to_string())
                .context("building OIDC redirect URL")?,
        );

        Ok(Self {
            config,
            client,
            http_client,
        })
    }
}
