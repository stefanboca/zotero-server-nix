use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{IntoResponse, Response, error::see_other, route},
    session,
};

use crate::backend::Backend;

#[route(POST)]
async fn logout(cx: &Cx) -> Result<Response> {
    if let Some(token_hash) = session::stop(cx).await? {
        app_context::<Backend>(cx)
            .delete_browser_session(&*token_hash)
            .await?;
    }
    see_other("/").into_response(cx)
}
