use std::time::UNIX_EPOCH;

use openidconnect::{CsrfToken, Nonce, PkceCodeChallenge, Scope, core::CoreAuthenticationFlow};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        IntoResponse, Response,
        error::{bad_request, see_other},
        query_params, route,
    },
    session,
};

use crate::{
    app::{AppState, epoch_seconds, is_topcoat_dev, valid_session_token},
    store,
};

#[query_params(error = bad_request)]
struct StartQuery {
    #[serde(rename = "session")]
    dataserver_session: String,
}

#[route(POST)]
async fn start(cx: &Cx) -> Result<Response> {
    if is_topcoat_dev() {
        return see_other("/auth/oidc/callback").into_response(cx);
    }

    let query = query_params::<StartQuery>(cx)?;
    if !valid_session_token(&query.dataserver_session) {
        return Err(bad_request("invalid Zotero login-session token").into());
    }

    let app = app_context::<AppState>(cx);
    let info = app
        .dataserver
        .login_session_info(&query.dataserver_session)
        .await?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    if let Some(token_hash) = session::token_hash(cx).await?
        && !store::clear_oidc_flow(&app.database, &*token_hash).await?
    {
        return Err(bad_request("the current OIDC login is already being completed").into());
    }

    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut authorization = app.oidc.client.authorize_url(
        CoreAuthenticationFlow::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    );
    for scope in &app.config.oidc.scopes {
        if scope != "openid" {
            authorization = authorization.add_scope(Scope::new(scope.clone()));
        }
    }
    let (url, csrf, nonce) = authorization.set_pkce_challenge(challenge).url();
    store::delete_expired_oidc_flows(&app.database, epoch_seconds()?).await?;
    let browser_session = session::start(cx).await?;
    let expires = browser_session
        .expires_at
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let inserted = store::create_oidc_flow(
        &app.database,
        &*browser_session.token_hash,
        csrf.secret(),
        nonce.secret(),
        verifier.secret(),
        &query.dataserver_session,
        expires,
    )
    .await;
    if let Err(error) = inserted {
        session::stop(cx).await?;
        if store::is_duplicate_key(&error) {
            return Err(bad_request(
                "an OIDC login is already being completed for this Zotero session",
            )
            .into());
        }
        return Err(error.into());
    }

    // A Topcoat redirect error bypasses the cookie layer's response finalizer.
    // Return a successful redirect so session::start can emit its cookie.
    see_other(url.as_str()).into_response(cx)
}
