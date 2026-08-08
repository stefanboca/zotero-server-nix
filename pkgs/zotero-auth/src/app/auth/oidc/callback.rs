use anyhow::{Context as _, anyhow};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, Nonce, OAuth2TokenResponse as _, PkceCodeVerifier,
    TokenResponse as _,
};
use serde_json::Value;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        error::{bad_request, forbidden},
        page, query_params,
    },
    session,
    view::view,
};

use crate::{
    app::{AppState, epoch_seconds, is_topcoat_dev},
    oidc::{jwt_claims, merge_claims, value_at_path, value_string},
    store::{self, OidcProfile},
};

#[query_params(error = bad_request)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[page]
async fn callback(cx: &Cx) -> Result {
    if is_topcoat_dev() {
        return success_page().await;
    }

    let result = callback_inner(cx).await;
    if let Err(error) = &result {
        eprintln!("OIDC callback failed: {error:#}");
    }
    result
}

async fn callback_inner(cx: &Cx) -> Result {
    let query = query_params::<CallbackQuery>(cx)?;
    if let Some(error) = &query.error {
        let description = query.error_description.as_deref().unwrap_or(error);
        return Err(bad_request(format!("OIDC provider rejected login: {description}")).into());
    }
    let code = query
        .code
        .as_deref()
        .ok_or_else(|| bad_request("missing authorization code"))?;
    let state = query
        .state
        .as_deref()
        .ok_or_else(|| bad_request("missing OIDC state"))?;

    let app = app_context::<AppState>(cx);
    let token_hash = session::token_hash(cx)
        .await?
        .ok_or_else(|| bad_request("missing or invalid OIDC flow session"))?;
    let flow = store::consume_oidc_flow(&app.database, &*token_hash, state)
        .await?
        .ok_or_else(|| bad_request("unknown or already-used OIDC state"))?;
    if flow.expires < epoch_seconds()? {
        return Err(bad_request("OIDC login flow expired").into());
    }

    let token_response = app
        .oidc
        .client
        .exchange_code(AuthorizationCode::new(code.to_owned()))?
        .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce_verifier))
        .request_async(&app.oidc.http_client)
        .await
        .context("exchanging OIDC authorization code")?;
    let id_token = token_response
        .id_token()
        .ok_or_else(|| anyhow!("OIDC provider omitted ID token"))?;
    let verifier = app.oidc.client.id_token_verifier();
    let claims = id_token
        .claims(&verifier, &Nonce::new(flow.nonce))
        .context("validating OIDC ID token")?;

    if let Some(expected_hash) = claims.access_token_hash() {
        let actual_hash = AccessTokenHash::from_token(
            token_response.access_token(),
            id_token.signing_alg()?,
            id_token.signing_key(&verifier)?,
        )?;
        if &actual_hash != expected_hash {
            return Err(anyhow!("OIDC access-token hash did not match ID token").into());
        }
    }

    let mut all_claims = jwt_claims(id_token)?;
    if let Some(endpoint) = app.oidc.client.user_info_url() {
        let user_info: Value = app
            .oidc
            .http_client
            .get(endpoint.as_str())
            .bearer_auth(token_response.access_token().secret())
            .send()
            .await
            .context("requesting OIDC UserInfo")?
            .error_for_status()
            .context("OIDC UserInfo returned an error")?
            .json()
            .await
            .context("decoding OIDC UserInfo")?;
        if value_string(&user_info, "sub") != Some(claims.subject().as_str()) {
            return Err(anyhow!("OIDC UserInfo subject did not match ID token").into());
        }
        merge_claims(&mut all_claims, user_info)?;
    }

    let email = value_string(&all_claims, "email")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| bad_request("OIDC provider did not supply an email address"))?;
    let email_verified = value_at_path(&all_claims, "email_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if app.config.identity.require_verified_email && !email_verified {
        return Err(forbidden().into());
    }
    let profile = OidcProfile {
        issuer: claims.issuer().as_str().to_owned(),
        subject: claims.subject().as_str().to_owned(),
        email: email.to_owned(),
        username: value_string(&all_claims, &app.config.identity.username_claim).map(str::to_owned),
        real_name: value_string(&all_claims, &app.config.identity.real_name_claim)
            .map(str::to_owned),
        email_verified,
    };
    let user_id = provision_and_complete_login(app, &flow.session_token, &profile).await?;
    let rotation = session::rotate(cx)
        .await?
        .ok_or_else(|| bad_request("missing or invalid OIDC flow session"))?;
    let expires = rotation
        .session
        .expires_at
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    store::replace_browser_session(
        &app.database,
        &*rotation.revoked,
        &*rotation.session.token_hash,
        user_id,
        expires,
    )
    .await?;
    store::delete_oidc_flow(&app.database, &*rotation.revoked).await?;

    success_page().await
}

async fn success_page() -> Result {
    view! {
        <main
            class="w-full max-w-md rounded-xl border border-stone-200 bg-white p-8 shadow-sm dark:border-stone-800 dark:bg-stone-900"
        >
            <h1 class="text-2xl font-semibold text-red-800 dark:text-red-400">
                "Login complete"
            </h1>
            <p class="mt-4 leading-7 text-stone-700 dark:text-stone-300">
                "Your Zotero desktop session is ready. You can close this page and return to Zotero."
            </p>
        </main>
    }
}

async fn provision_and_complete_login(
    app: &AppState,
    session_token: &str,
    profile: &OidcProfile,
) -> Result<u64> {
    let user_id = store::provision_user(&app.database, profile)
        .await?
        .ok_or_else(forbidden)?;

    // Hold the identity-row lock through Dataserver completion. Dataserver's
    // first-login shard setup is not atomic, so concurrent logins for one OIDC
    // identity must not complete at the same time. Cancellation rolls back the
    // transaction and releases this lock automatically.
    let transaction = store::lock_identity_for_login_completion(&app.database, profile, user_id)
        .await?
        .ok_or_else(|| anyhow!("OIDC identity changed during login completion"))?;

    let info = app.dataserver.login_session_info(session_token).await?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    if info.user_id.is_some_and(|existing| existing != user_id) {
        return Err(forbidden().into());
    }
    app.dataserver
        .complete_login_session(session_token, user_id, info.access)
        .await?;
    transaction.commit().await?;
    Ok(user_id)
}
