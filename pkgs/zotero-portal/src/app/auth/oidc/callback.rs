use anyhow::anyhow;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        error::{bad_request, forbidden},
        header, page, query_params,
    },
    session,
    view::view,
};

use crate::{
    app::AppState,
    id::UserId,
    store::OidcProfile,
    epoch_seconds,
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
    let flow = app
        .backend
        .consume_oidc_flow(&*token_hash, state)
        .await?
        .ok_or_else(|| bad_request("unknown or already-used OIDC state"))?;
    if flow.expires < epoch_seconds()? {
        return Err(bad_request("OIDC login flow expired").into());
    }

    let profile = app
        .backend
        .exchange_oidc_code(code, &flow)
        .await?
        .ok_or_else(forbidden)?;
    let user_id = if let Some(session_token) = flow.session_token.as_deref() {
        provision_and_complete_login(app, session_token, &profile).await?
    } else {
        let user_id = app
            .backend
            .provision_user(&profile)
            .await?
            .ok_or_else(forbidden)?;
        app.backend.set_unlimited_storage(user_id).await?;
        user_id
    };
    let rotation = session::rotate(cx)
        .await?
        .ok_or_else(|| bad_request("missing or invalid OIDC flow session"))?;
    let expires = rotation
        .session
        .expires_at
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    app.backend
        .replace_browser_session(
            &*rotation.revoked,
            &*rotation.session.token_hash,
            user_id,
            expires,
        )
        .await?;
    app.backend.delete_oidc_flow(&*rotation.revoked).await?;

    if flow.session_token.is_some() {
        success_page().await
    } else {
        let location = HeaderValue::try_from(flow.return_path)?;
        view! { cx =>
            (StatusCode::SEE_OTHER)
            ((header::LOCATION, location))
        }
    }
}

async fn success_page() -> Result {
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
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
) -> Result<UserId> {
    let user_id = app
        .backend
        .provision_user(profile)
        .await?
        .ok_or_else(forbidden)?;

    // Hold the identity-row lock through Dataserver completion. Dataserver's
    // first-login shard setup is not atomic, so concurrent logins for one OIDC
    // identity must not complete at the same time. Cancellation rolls back the
    // transaction and releases this lock automatically.
    let transaction = app
        .backend
        .lock_identity_for_login_completion(profile, user_id)
        .await?
        .ok_or_else(|| anyhow!("OIDC identity changed during login completion"))?;

    let info = app.backend.login_session_info(session_token).await?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    if info.user_id.is_some_and(|existing| existing != user_id) {
        return Err(forbidden().into());
    }
    app.backend
        .complete_login_session(session_token, user_id, info.access)
        .await?;
    app.backend.set_unlimited_storage(user_id).await?;
    transaction.commit().await?;
    Ok(user_id)
}
