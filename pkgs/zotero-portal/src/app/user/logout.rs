use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{IntoResponse, Response, error::see_other, route},
    session,
};

use crate::{database::Database, dataserver::Dataserver};

#[route(POST)]
async fn logout(cx: &Cx) -> Result<Response> {
    if let Some(token_hash) = session::stop(cx).await? {
        app_context::<Database>(cx)
            .delete_browser_session(app_context::<Dataserver>(cx), &*token_hash)
            .await?;
    }
    see_other("/").into_response(cx)
}
