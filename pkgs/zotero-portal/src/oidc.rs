use anyhow::Context as _;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use openidconnect::{
    ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet, IssuerUrl, RedirectUrl,
    core::{CoreClient, CoreProviderMetadata},
};

use serde::Serialize;
use serde_json::Value;

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
    pub client: OidcClient,
    pub http_client: reqwest::Client,
}

impl Oidc {
    pub async fn discover(config: &Config, client_secret: String) -> anyhow::Result<Self> {
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
            client,
            http_client,
        })
    }
}

pub fn jwt_claims<T: Serialize>(token: &T) -> anyhow::Result<Value> {
    let serialized = serde_json::to_value(token)?;
    let compact = serialized
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("ID token did not serialize as a JWT"))?;
    let payload = compact
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("ID token was not a compact JWT"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .context("decoding ID-token claims")?;
    serde_json::from_slice(&decoded).context("decoding ID-token claims JSON")
}

pub fn merge_claims(base: &mut Value, overlay: Value) -> anyhow::Result<()> {
    let base = base
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("ID-token claims were not an object"))?;
    let Value::Object(overlay) = overlay else {
        anyhow::bail!("UserInfo claims were not an object");
    };
    base.extend(overlay);
    Ok(())
}

pub fn value_at_path<'a>(claims: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(claims, |value, part| value.get(part))
}

pub fn value_string<'a>(claims: &'a Value, path: &str) -> Option<&'a str> {
    value_at_path(claims, path).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nested_claim_values() {
        let claims = serde_json::json!({
            "email": "user@example.com",
            "profile": {"name": "Example User"}
        });

        assert_eq!(value_string(&claims, "email"), Some("user@example.com"));
        assert_eq!(value_string(&claims, "profile.name"), Some("Example User"));
        assert_eq!(value_string(&claims, "missing"), None);
    }
}
