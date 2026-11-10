use topcoat::context::{Cx, context};
use topcoat::router::error::{SeeOther, see_other};
use topcoat::router::{href, route};
use topcoat::{Error, Result, session};

use crate::database::Database;
use crate::dataserver::Dataserver;

// The built-in Web Library navigation renders menu entries as links, matching
// zotero.org's GET /user/logout endpoint. Keep POST for the portal's form.
#[route([GET, POST])]
pub(crate) async fn logout(cx: &Cx) -> Result<SeeOther> {
    if let Some(token_hash) = session::stop(cx).await? {
        context::<Database>(cx)
            .delete_browser_session(context::<Dataserver>(cx), &*token_hash)
            .await
            .map_err(Error::from_anyhow)?;
    }
    Ok(see_other(
        href!(crate::app::_web_library::landing).resolve(cx),
    ))
}
