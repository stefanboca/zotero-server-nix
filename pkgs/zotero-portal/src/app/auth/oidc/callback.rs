use topcoat::context::{Cx, context};
use topcoat::cookie::{Cookies as _, SameSite, cookie_store, private_cookies};
use topcoat::router::error::{SeeOther, bad_request, forbidden, see_other};
use topcoat::router::request::headers;
use topcoat::router::{header, href, query_params, route};
use topcoat::{Error, Result, session};

use crate::app::user::complete_desktop_login;
use crate::database::Database;
use crate::dataserver::Dataserver;
use crate::oidc::{Oidc, OidcFlow};

#[query_params(error = bad_request)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[route(GET)]
async fn callback(cx: &Cx) -> Result<SeeOther> {
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

    let database = context::<Database>(cx);
    let dataserver = context::<Dataserver>(cx);
    let flow = cookie_store::<OidcFlow, _>(
        private_cookies(cx)
            .default_http_only(true)
            .default_same_site(SameSite::Lax)
            .default_path("/"),
        "oidc-flow",
    )
    .parse()?
    .ok_or_else(|| bad_request("missing or invalid OIDC flow cookie"))?;
    if flow.read().state != state {
        return Err(bad_request("unknown OIDC state").into());
    }
    if flow.read().expires_at < jiff::Timestamp::now() {
        return Err(bad_request("OIDC login flow expired").into());
    }
    let flow_value = flow.get();
    flow.remove();
    let flow = flow_value;

    let profile = context::<Oidc>(cx)
        .exchange_code(code, &flow)
        .await
        .map_err(Error::from_anyhow)?
        .ok_or_else(forbidden)?;
    let user_id = database
        .provision_user(&profile)
        .await
        .map_err(Error::from_anyhow)?
        .ok_or_else(forbidden)?;
    if let Some(session_token) = flow.session_token.as_deref() {
        complete_desktop_login(database, dataserver, session_token, user_id).await?;
    } else {
        dataserver
            .set_unlimited_storage(user_id)
            .await
            .map_err(Error::from_anyhow)?;
    }
    let browser_session = session::start(cx).await?;
    let user_agent = headers(cx)
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok());
    database
        .create_browser_session(
            dataserver,
            &*browser_session.token_hash,
            user_id,
            browser_session.expires_at.try_into()?,
            user_agent,
        )
        .await
        .map_err(Error::from_anyhow)?;
    tracing::info!(
        user_id = %user_id,
        desktop_login = flow.session_token.is_some(),
        "OIDC login completed",
    );

    if flow.session_token.is_some() {
        Ok(see_other(
            href!(crate::app::user::login::done::done).resolve(cx),
        ))
    } else {
        Ok(see_other(flow.return_path))
    }
}
