use topcoat::{
    Result,
    context::Cx,
    router::{HeaderValue, StatusCode, error::not_found, header, page, path_param},
    view::view,
};

use crate::{app::authenticated_user, id::UserId};

use super::UserIdPath;

#[page]
async fn items(cx: &Cx) -> Result {
    let requested_user = UserId::new(*path_param::<UserIdPath>(cx)?);
    if authenticated_user(cx).await? != requested_user {
        return Err(not_found().into());
    }
    view! {
        (StatusCode::TEMPORARY_REDIRECT)
        ((header::LOCATION, HeaderValue::from_static("/")))
    }
}
