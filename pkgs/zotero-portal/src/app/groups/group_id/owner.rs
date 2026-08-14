use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden, not_found},
        header, page,
    },
    view::view,
};

use crate::{
    app::request_confirmed,
    database::Database,
    dataserver::{Dataserver, GroupRole},
};

use super::current_group;

#[derive(Deserialize)]
struct ConfirmTransfer {
    username: String,
    confirmation: String,
}

#[page(POST)]
async fn transfer_owner(cx: &Cx, Form(input): Form<ConfirmTransfer>) -> Result {
    let (_, group, actor_role) = current_group(cx).await?;
    if actor_role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    let database = app_context::<Database>(cx);
    let target = database
        .user_by_identifier(&input.username)
        .await?
        .ok_or_else(not_found)?;
    match group.role(target.user_id) {
        Some(GroupRole::Admin | GroupRole::Member) => {}
        Some(GroupRole::Owner) => {
            return Err(bad_request("that user already owns the group").into());
        }
        None => return Err(bad_request("the new owner must already be a group member").into()),
    }
    if input.confirmation != group.name {
        return Err(bad_request("group name confirmation did not match").into());
    }
    let result = app_context::<Dataserver>(cx)
        .set_group_user(group.id, target.user_id, GroupRole::Owner)
        .await;
    let notice = if let Err(error) = result {
        eprintln!("failed to transfer group ownership: {error:#}");
        if request_confirmed(cx) {
            return view! { (StatusCode::BAD_GATEWAY) };
        }
        "ownership-transfer-failed"
    } else {
        "ownership-transferred"
    };
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("notice", notice)
        .finish();
    let location = HeaderValue::try_from(format!("/groups/{}?{query}", group.id))?;
    if request_confirmed(cx) {
        return view! {
            (StatusCode::NO_CONTENT)
            ((header::LOCATION, location))
        };
    }
    view! {
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, location))
    }
}
