use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderName, HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden},
        header, headers, page, path_param,
    },
    view::view,
};

use crate::{app::AppState, dataserver::GroupRole};

use super::{GroupId, current_group};

#[derive(Deserialize)]
struct MemberAction {
    operation: String,
    identifier: Option<String>,
}

fn identifier(input: &MemberAction) -> Result<&str> {
    input
        .identifier
        .as_deref()
        .map(str::trim)
        .map(|identifier| identifier.strip_prefix('@').unwrap_or(identifier))
        .filter(|identifier| !identifier.is_empty())
        .ok_or_else(|| bad_request("member username or email is required").into())
}

fn action_location(
    group_id: u64,
    notice: &str,
    member: Option<&str>,
    role: Option<&str>,
    manage: Option<&str>,
) -> Result<HeaderValue> {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.append_pair("notice", notice);
    if let Some(member) = member {
        query.append_pair("member", member);
    }
    if let Some(role) = role {
        query.append_pair("role", role);
    }
    if let Some(username) = manage {
        query.append_pair("manage", username);
    }
    HeaderValue::try_from(format!("/groups/{group_id}?{}", query.finish())).map_err(Into::into)
}

fn left_group_location(group_name: &str) -> Result<HeaderValue> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("notice", "left-group")
        .append_pair("name", group_name)
        .finish();
    HeaderValue::try_from(format!("/groups?{query}")).map_err(Into::into)
}

async fn action_response(
    cx: &Cx,
    status: StatusCode,
    group_id: u64,
    notice: &str,
    member: Option<&str>,
    role: Option<&str>,
    manage: Option<&str>,
) -> Result {
    if headers(cx).get("x-zotero-in-place").is_some()
        || headers(cx)
            .get("x-zotero-confirmed")
            .is_some_and(|value| value == "true")
    {
        let outcome = HeaderValue::try_from(notice)?;
        return view! { cx =>
            (status)
            ((HeaderName::from_static("x-zotero-outcome"), outcome))
            if !status.is_success() {
                (notice)
            }
        };
    }
    let location = action_location(group_id, notice, member, role, manage)?;
    view! { cx =>
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, location))
    }
}

#[page]
async fn members(cx: &Cx) -> Result {
    current_group(cx).await?;
    let location = HeaderValue::try_from(format!("/groups/{}", path_param::<GroupId>(cx)?))?;
    view! {
        (StatusCode::TEMPORARY_REDIRECT)
        ((header::LOCATION, location))
    }
}

#[page(POST)]
async fn update_members(cx: &Cx, Form(input): Form<MemberAction>) -> Result {
    let (actor_id, group, actor_role) = current_group(cx).await?;
    let app = app_context::<AppState>(cx);

    match input.operation.as_str() {
        "leave" => {
            if actor_role == GroupRole::Owner {
                return Err(forbidden().into());
            }
            if let Err(error) = app.backend.remove_group_user(group.id, actor_id).await {
                eprintln!("failed to leave group: {error:#}");
                return action_response(
                    cx,
                    StatusCode::BAD_GATEWAY,
                    group.id,
                    "member-remove-failed",
                    None,
                    None,
                    None,
                )
                .await;
            }
            let location = left_group_location(&group.name)?;
            if headers(cx)
                .get("x-zotero-confirmed")
                .is_some_and(|value| value == "true")
            {
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
        "demote-self" => {
            if actor_role != GroupRole::Admin {
                return Err(forbidden().into());
            }
            let actor = app
                .backend
                .user_by_id(actor_id)
                .await?
                .ok_or_else(|| bad_request("authenticated user does not exist"))?;
            if let Err(error) = app
                .backend
                .set_group_user(group.id, actor_id, GroupRole::Member)
                .await
            {
                eprintln!("failed to demote group administrator: {error:#}");
                return action_response(
                    cx,
                    StatusCode::BAD_GATEWAY,
                    group.id,
                    "role-update-failed",
                    Some(&actor.username),
                    Some(GroupRole::Member.display_name()),
                    None,
                )
                .await;
            }
            let location = action_location(
                group.id,
                "role-updated",
                Some(&actor.username),
                Some(GroupRole::Member.display_name()),
                None,
            )?;
            if headers(cx)
                .get("x-zotero-confirmed")
                .is_some_and(|value| value == "true")
            {
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
        "remove" => {
            let target = app
                .backend
                .user_by_identifier(identifier(&input)?)
                .await?
                .ok_or_else(|| bad_request("user is not a group member"))?;
            match group.role(target.user_id) {
                Some(GroupRole::Owner) => return Err(forbidden().into()),
                Some(_) => {}
                None => return Err(bad_request("user is not a group member").into()),
            }
            if !actor_role.can_manage() && target.user_id != actor_id {
                return Err(forbidden().into());
            }
            if let Err(error) = app
                .backend
                .remove_group_user(group.id, target.user_id)
                .await
            {
                eprintln!("failed to remove group member: {error:#}");
                return action_response(
                    cx,
                    StatusCode::BAD_GATEWAY,
                    group.id,
                    "member-remove-failed",
                    Some(&target.username),
                    None,
                    None,
                )
                .await;
            }
            action_response(
                cx,
                StatusCode::NO_CONTENT,
                group.id,
                "member-removed",
                Some(&target.username),
                None,
                None,
            )
            .await
        }
        _ => return Err(bad_request("unknown member operation").into()),
    }
}
