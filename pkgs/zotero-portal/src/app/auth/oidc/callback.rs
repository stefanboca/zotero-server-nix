use topcoat::{
    Result,
    context::{Cx, app_context},
    cookie::{Cookies as _, SameSite, cookie_store, private_cookies},
    router::{
        HeaderValue, StatusCode,
        error::{bad_request, forbidden},
        header, page, query_params,
    },
    session,
    view::view,
};

use crate::{
    app::user::complete_desktop_login,
    database::Database,
    dataserver::Dataserver,
    oidc::{Oidc, OidcFlow},
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

    let database = app_context::<Database>(cx);
    let dataserver = app_context::<Dataserver>(cx);
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

    let profile = app_context::<Oidc>(cx)
        .exchange_code(code, &flow)
        .await?
        .ok_or_else(forbidden)?;
    let user_id = database
        .provision_user(&profile)
        .await?
        .ok_or_else(forbidden)?;
    if let Some(session_token) = flow.session_token.as_deref() {
        complete_desktop_login(database, dataserver, session_token, user_id).await?;
    } else {
        dataserver.set_unlimited_storage(user_id).await?;
    }
    let browser_session = session::start(cx).await?;
    database
        .create_browser_session(
            dataserver,
            &*browser_session.token_hash,
            user_id,
            browser_session.expires_at.try_into()?,
        )
        .await?;

    if flow.session_token.is_some() {
        view! {
            cx =>
            (StatusCode::SEE_OTHER)
            ((
                header::LOCATION,
                HeaderValue::from_static("/user/login/done"),
            ))
        }
    } else {
        let location = HeaderValue::try_from(flow.return_path)?;
        view! {
            cx =>
            (StatusCode::SEE_OTHER)
            ((header::LOCATION, location))
        }
    }
}
