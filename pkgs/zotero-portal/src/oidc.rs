use anyhow::Context as _;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use openidconnect::{
    ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet, IssuerUrl, RedirectUrl,
    core::{CoreClient, CoreProviderMetadata},
};
use serde::Serialize;
use serde_json::Value;
use url::Url;

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
        .set_redirect_uri(redirect_url(&config.server.base_uri)?);

        Ok(Self {
            client,
            http_client,
        })
    }
}

fn redirect_url(base_uri: &str) -> anyhow::Result<RedirectUrl> {
    let mut url = match Url::parse(base_uri) {
        Ok(url) if url.has_host() => url,
        Ok(_) | Err(url::ParseError::RelativeUrlWithoutBase) => {
            Url::parse(&format!("https://{base_uri}")).context("parsing server base URI")?
        }
        Err(error) => return Err(error).context("parsing server base URI"),
    };
    if url.host_str().is_none() {
        anyhow::bail!("server base URI must include a host");
    }
    url.set_scheme("https")
        .map_err(|()| anyhow::anyhow!("server base URI cannot use HTTPS"))?;
    url.set_path("/auth/oidc/callback");
    url.set_query(None);
    url.set_fragment(None);

    RedirectUrl::new(url.to_string()).context("building OIDC redirect URL")
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

    #[test]
    fn builds_https_callback_from_a_hostname() {
        let redirect = redirect_url("zotero.example.com").expect("valid redirect URL");

        assert_eq!(
            redirect.as_str(),
            "https://zotero.example.com/auth/oidc/callback"
        );
    }

    #[test]
    fn forces_https_and_replaces_the_path() {
        let redirect = redirect_url("http://zotero.example.com:8443/old?query=1#fragment")
            .expect("valid redirect URL");

        assert_eq!(
            redirect.as_str(),
            "https://zotero.example.com:8443/auth/oidc/callback"
        );
    }
}
