use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden, not_found},
        header, page, query_params,
    },
    view::view,
};

use crate::{
    app::{AppState, is_topcoat_dev, username_label},
    dataserver::{Group, GroupRole},
    store::{self, UserSummary},
};

use super::current_group;

#[query_params(error = bad_request)]
struct TransferTarget {
    username: String,
}

#[derive(Deserialize)]
struct ConfirmTransfer {
    username: String,
    confirmation: String,
}

fn validate_target(group: &Group, user_id: u64) -> Result<()> {
    match group.role(user_id) {
        Some(GroupRole::Admin | GroupRole::Member) => Ok(()),
        Some(GroupRole::Owner) => Err(bad_request("that user already owns the group").into()),
        None => Err(bad_request("the new owner must already be a group member").into()),
    }
}

async fn target_user(cx: &Cx, group: &Group, username: &str) -> Result<UserSummary> {
    if is_topcoat_dev() {
        let target = UserSummary {
            user_id: 2,
            username: "new-owner".to_owned(),
            email: "new-owner@example.com".to_owned(),
            real_name: Some("New Group Owner".to_owned()),
        };
        validate_target(group, target.user_id)?;
        return Ok(target);
    }
    let target = store::oidc_user_by_identifier(&app_context::<AppState>(cx).database, username)
        .await?
        .ok_or_else(|| not_found())?;
    validate_target(group, target.user_id)?;
    Ok(target)
}

#[page]
async fn transfer_owner_confirmation(cx: &Cx) -> Result {
    let parameters = query_params::<TransferTarget>(cx)?;
    let (_, group, actor_role) = current_group(cx).await?;
    if actor_role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    let target = target_user(cx, &group, &parameters.username).await?;
    let action = format!("/groups/{}/owner", group.id);
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-2xl font-semibold">"Transfer group ownership"</h1>
            <p class="mt-4 leading-7">
                "Transfer “" (group.name.as_str()) "” to " <strong>(target.display_name())</strong>
                " (" username_label(username: target.username.as_str()) ")?"
            </p>
            <p class="mt-4 leading-7 text-red-800 dark:text-red-300">
                "The new owner will control membership and can delete the group and its library. You will become an administrator. Only the new owner can transfer ownership again."
            </p>
            <form class="mt-6 space-y-4" method="post" action=(action)>
                <input type="hidden" name="username" value=(target.username.as_str())>
                <label class="block font-medium" for="confirmation">
                    "Type the group name to confirm: " <strong>(group.name.as_str())</strong>
                </label>
                <input class="w-full rounded border border-red-300 bg-white px-3 py-2 dark:border-red-900 dark:bg-stone-950" id="confirmation" name="confirmation" required="required" autocomplete="off">
                <div class="flex flex-wrap gap-3">
                    <button class="rounded bg-red-800 px-4 py-3 font-semibold text-white hover:bg-red-700" type="submit">
                        "Transfer ownership"
                    </button>
                    <a class="rounded border border-stone-300 px-4 py-3 dark:border-stone-700" href=(format!("/groups/{}", group.id))>
                        "Cancel"
                    </a>
                </div>
            </form>
        </main>
    }
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
    if !is_topcoat_dev() {
        app_context::<AppState>(cx)
            .dataserver
            .set_group_user(group.id, target.user_id, GroupRole::Owner)
            .await?;
    }
    let location = HeaderValue::try_from(format!(
        "/groups/{}?notice=ownership-transferred",
        group.id
    ))?;
    view! {
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, location))
    }
}
