use serde::Deserialize;
use topcoat::context::{Cx, context};
use topcoat::router::content::Form;
use topcoat::router::error::see_other;
use topcoat::router::response::{IntoResponse as _, Response};
use topcoat::router::{href, page, route};
use topcoat::view::{View, view};
use topcoat::{Error, Result};

use super::GroupName;
use crate::app::groups::group_id::{group_id, library_settings};
use crate::app::{authenticated_user, portal_page, section};
use crate::dataserver::Dataserver;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateGroup {
    name: GroupName,
}

#[page]
pub(super) async fn new_group(cx: &Cx) -> Result<impl View> {
    authenticated_user(cx).await?;
    Ok(view! {
        portal_page(
            section(
                <h1 class="text-3xl font-semibold">"Create a Group"</h1>
                <p class="mt-3 max-w-2xl text-stone-600">
                    "Groups are visible only to their members."
                </p>
                <form
                    class="mt-8 max-w-xl space-y-6"
                    method="post"
                    action=(href!(new_group))
                >
                    <div>
                        <label class="portal-form-label" for="group-name">
                            "Group name"
                        </label>
                        <input
                            class="portal-form-control portal-form-control-block"
                            id="group-name"
                            name="name"
                            required="required"
                            maxlength="255"
                            autofocus="autofocus"
                        >
                    </div>
                    <div class="flex flex-wrap gap-3">
                        <button
                            class="portal-button portal-button-primary"
                            type="submit"
                        >
                            "Create Group"
                        </button>
                        <a
                            class="portal-button portal-button-secondary"
                            href=(href!(super::list_groups))
                        >
                            "Cancel"
                        </a>
                    </div>
                </form>
            )
        )
    })
}

#[route(POST)]
async fn create_group(cx: &Cx, Form(input): Form<CreateGroup>) -> Result<Response> {
    let user_id = authenticated_user(cx).await?;
    let group_id = context::<Dataserver>(cx)
        .create_private_group(user_id, input.name.as_str())
        .await
        .map_err(Error::from_anyhow)?;
    tracing::info!(user_id = %user_id, group_id, "group created");
    see_other(href!(library_settings, group_id: group_id).resolve(cx)).into_response(cx)
}
