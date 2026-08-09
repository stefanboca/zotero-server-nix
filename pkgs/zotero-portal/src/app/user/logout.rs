use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{IntoResponse, Response, error::see_other, route},
    session,
};

use crate::app::AppState;

#[route(POST)]
async fn logout(cx: &Cx) -> Result<Response> {
    if let Some(token_hash) = session::stop(cx).await? {
        let app = app_context::<AppState>(cx);
        app.backend.delete_browser_session(&*token_hash).await?;
    }
    see_other("/").into_response(cx)
}
