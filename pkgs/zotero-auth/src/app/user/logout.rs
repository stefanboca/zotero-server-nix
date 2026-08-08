use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{IntoResponse, Response, error::see_other, route},
    session,
};

use crate::{
    app::{AppState, is_topcoat_dev},
    store,
};

#[route(POST)]
async fn logout(cx: &Cx) -> Result<Response> {
    if is_topcoat_dev() {
        return see_other("/").into_response(cx);
    }
    if let Some(token_hash) = session::stop(cx).await? {
        let app = app_context::<AppState>(cx);
        store::delete_browser_session(&app.database, &*token_hash).await?;
    }
    see_other("/").into_response(cx)
}
