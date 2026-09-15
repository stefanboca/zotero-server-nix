use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Form, error::see_other, href, page, route},
    view::{View, view},
};

use crate::{
    app::{
        authenticated_user,
        groups::group_id::{GroupId, library_settings},
        portal_page, section,
    },
    dataserver::Dataserver,
};

use super::GroupName;

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
async fn create_group(cx: &Cx, Form(input): Form<CreateGroup>) -> Result<()> {
    let user_id = authenticated_user(cx).await?;
    let group_id = app_context::<Dataserver>(cx)
        .create_private_group(user_id, input.name.as_str())
        .await?;
    tracing::info!(user_id = %user_id, group_id, "group created");
    Err(see_other(href!(library_settings, GroupId(group_id)).resolve(cx)).into())
}
