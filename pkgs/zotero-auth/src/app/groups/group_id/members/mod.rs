use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden},
        header, headers, page, path_param,
    },
    view::view,
};

use crate::{
    app::{AppState, is_topcoat_dev},
    dataserver::GroupRole,
    store,
};

use super::{GroupId, current_group};

#[derive(Deserialize)]
struct MemberAction {
    operation: String,
    identifier: Option<String>,
    role: Option<String>,
}

fn role(value: Option<&str>) -> Result<GroupRole> {
    match value {
        Some("member") => Ok(GroupRole::Member),
        Some("admin") => Ok(GroupRole::Admin),
        _ => Err(bad_request("role must be member or admin").into()),
    }
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
    if headers(cx)
        .get("x-zotero-in-place")
        .is_some_and(|value| value == "role")
    {
        return view! { cx => (status) };
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
    if is_topcoat_dev() {
        let username = identifier(&input)?;
        let (notice, role_name, manage) = match input.operation.as_str() {
            "add" => ("member-added", Some(role(input.role.as_deref())?.display_name()), None),
            "role" => (
                "role-updated",
                Some(role(input.role.as_deref())?.display_name()),
                Some(username),
            ),
            "remove" => ("member-removed", None, None),
            _ => return Err(bad_request("unknown member operation").into()),
        };
        return action_response(
            cx,
            StatusCode::NO_CONTENT,
            group.id,
            notice,
            Some(username),
            role_name,
            manage,
        )
        .await;
    }
    let app = app_context::<AppState>(cx);

    match input.operation.as_str() {
        "add" => {
            if !actor_role.can_manage() {
                return Err(forbidden().into());
            }
            let identifier = identifier(&input)?;
            let new_role = role(input.role.as_deref())?;
            let Some(target) = store::oidc_user_by_identifier(&app.database, identifier).await?
            else {
                return action_response(
                    cx,
                    StatusCode::BAD_REQUEST,
                    group.id,
                    "member-not-found",
                    Some(identifier),
                    Some(new_role.display_name()),
                    None,
                )
                .await;
            };
            if group.role(target.user_id).is_some() {
                return action_response(
                    cx,
                    StatusCode::BAD_REQUEST,
                    group.id,
                    "already-member",
                    Some(&target.username),
                    None,
                    None,
                )
                .await;
            }
            if let Err(error) = app
                .dataserver
                .set_group_user(group.id, target.user_id, new_role)
                .await
            {
                eprintln!("failed to add group member: {error:#}");
                return action_response(
                    cx,
                    StatusCode::BAD_GATEWAY,
                    group.id,
                    "member-add-failed",
                    Some(&target.username),
                    Some(new_role.display_name()),
                    None,
                )
                .await;
            }
            action_response(
                cx,
                StatusCode::NO_CONTENT,
                group.id,
                "member-added",
                Some(&target.username),
                Some(new_role.display_name()),
                None,
            )
            .await
        }
        "role" => {
            if !actor_role.can_manage() {
                return Err(forbidden().into());
            }
            let new_role = role(input.role.as_deref())?;
            let target = store::oidc_user_by_identifier(&app.database, identifier(&input)?)
                .await?
                .ok_or_else(|| bad_request("user is not a group member"))?;
            match group.role(target.user_id) {
                Some(GroupRole::Owner) => return Err(forbidden().into()),
                Some(_) => {}
                None => return Err(bad_request("user is not a group member").into()),
            }
            if let Err(error) = app
                .dataserver
                .set_group_user(group.id, target.user_id, new_role)
                .await
            {
                eprintln!("failed to update group member role: {error:#}");
                return action_response(
                    cx,
                    StatusCode::BAD_GATEWAY,
                    group.id,
                    "role-update-failed",
                    Some(&target.username),
                    Some(new_role.display_name()),
                    Some(&target.username),
                )
                .await;
            }
            action_response(
                cx,
                StatusCode::NO_CONTENT,
                group.id,
                "role-updated",
                Some(&target.username),
                Some(new_role.display_name()),
                Some(&target.username),
            )
            .await
        }
        "remove" => {
            let target = store::oidc_user_by_identifier(&app.database, identifier(&input)?)
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
                .dataserver
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
            if target.user_id == actor_id {
                let location = left_group_location(&group.name)?;
                return view! {
                    (StatusCode::SEE_OTHER)
                    ((header::LOCATION, location))
                };
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
