use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden, not_found},
        header, headers, page,
    },
    view::view,
};

use crate::{
    app::AppState,
    dataserver::{Group, GroupRole},
    id::UserId,
    store::UserSummary,
};

use super::current_group;

#[derive(Deserialize)]
struct ConfirmTransfer {
    username: String,
    confirmation: String,
}

fn validate_target(group: &Group, user_id: UserId) -> Result<()> {
    match group.role(user_id) {
        Some(GroupRole::Admin | GroupRole::Member) => Ok(()),
        Some(GroupRole::Owner) => Err(bad_request("that user already owns the group").into()),
        None => Err(bad_request("the new owner must already be a group member").into()),
    }
}

async fn target_user(cx: &Cx, group: &Group, username: &str) -> Result<UserSummary> {
    let target = app_context::<AppState>(cx)
        .backend
        .user_by_identifier(username)
        .await?
        .ok_or_else(not_found)?;
    validate_target(group, target.user_id)?;
    Ok(target)
}

fn confirmed(cx: &Cx) -> bool {
    headers(cx)
        .get("x-zotero-confirmed")
        .is_some_and(|value| value == "true")
}

#[page(POST)]
async fn transfer_owner(cx: &Cx, Form(input): Form<ConfirmTransfer>) -> Result {
    let (_, group, actor_role) = current_group(cx).await?;
    if actor_role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    let target = target_user(cx, &group, &input.username).await?;
    if input.confirmation != group.name {
        return Err(bad_request("group name confirmation did not match").into());
    }
    let result = app_context::<AppState>(cx)
        .backend
        .set_group_user(group.id, target.user_id, GroupRole::Owner)
        .await;
    let notice = if let Err(error) = result {
        eprintln!("failed to transfer group ownership: {error:#}");
        if confirmed(cx) {
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
    if confirmed(cx) {
        return view! { (StatusCode::NO_CONTENT) ((header::LOCATION, location)) };
    }
    view! {
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, location))
    }
}
