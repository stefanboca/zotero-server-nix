use topcoat::context::{Cx, app_context};
use topcoat::router::error::see_other;
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};
use topcoat::{Result, session};

use crate::database::Database;
use crate::dataserver::Dataserver;

// The built-in Web Library navigation renders menu entries as links, matching
// zotero.org's GET /user/logout endpoint. Keep POST for the portal's form.
#[route([GET, POST])]
pub(crate) async fn logout(cx: &Cx) -> Result<Response> {
    if let Some(token_hash) = session::stop(cx).await? {
        app_context::<Database>(cx)
            .delete_browser_session(app_context::<Dataserver>(cx), &*token_hash)
            .await?;
    }
    see_other(href!(crate::app::_web_library::landing).resolve(cx)).into_response(cx)
}
