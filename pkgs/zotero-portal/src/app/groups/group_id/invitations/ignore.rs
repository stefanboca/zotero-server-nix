use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        error::not_found,
        href, path_param,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::{app::authenticated_user, database::Database};

use super::super::GroupId;

#[route(POST)]
pub(in crate::app::groups) async fn ignore(cx: &Cx) -> Result<Response> {
    let user_id = authenticated_user(cx).await?;
    let group_id = *path_param::<GroupId>(cx)?;
    if !app_context::<Database>(cx)
        .delete_group_invitation(group_id, user_id)
        .await?
    {
        return Err(not_found().into());
    }
    topcoat::router::error::see_other(href!(crate::app::groups::list_groups).resolve(cx))
        .into_response(cx)
}
