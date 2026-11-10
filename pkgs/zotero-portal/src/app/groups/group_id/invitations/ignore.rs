use topcoat::context::{Cx, context};
use topcoat::router::error::{SeeOther, not_found};
use topcoat::router::{href, route};
use topcoat::{Error, Result};

use super::super::group_id;
use crate::app::authenticated_user;
use crate::database::Database;

#[route(POST)]
pub(in crate::app::groups) async fn ignore(cx: &Cx) -> Result<SeeOther> {
    let user_id = authenticated_user(cx).await?;
    let group_id = group_id(cx)?;
    if !context::<Database>(cx)
        .delete_group_invitation(*group_id, user_id)
        .await
        .map_err(Error::from_anyhow)?
    {
        return Err(not_found().into());
    }
    Ok(topcoat::router::error::see_other(
        href!(crate::app::groups::list_groups).resolve(cx),
    ))
}
