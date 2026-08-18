use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::see_other,
        href, page,
        response::{IntoResponse, Response},
        route,
    },
    view::view,
};

use crate::{
    app::{
        authenticated_user,
        groups::group_id::{GroupId, library_settings},
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
pub(super) async fn new_group(cx: &Cx) -> Result {
    authenticated_user(cx).await?;
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-3xl font-semibold">"Create a Group"</h1>
            <p class="mt-3 max-w-2xl text-stone-600 dark:text-stone-400">
                "Groups on this private server are invitation-free and visible only to their members."
            </p>
            <form
                class="mt-8 max-w-xl space-y-6"
                method="post"
                action=(href!(new_group))
            >
                <div>
                    <label class="block font-medium" for="group-name">
                        "Group name"
                    </label>
                    <input
                        class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                        id="group-name"
                        name="name"
                        required="required"
                        maxlength="255"
                        autofocus="autofocus"
                    >
                </div>
                <fieldset>
                    <legend class="font-medium">"Group type"</legend>
                    <label class="mt-3 flex items-start gap-3">
                        <input
                            class="mt-1"
                            type="radio"
                            checked="checked"
                            disabled="disabled"
                        >
                        <span>
                            <strong class="block">"Private Membership"</strong>
                            <span
                                class="mt-1 block text-sm text-stone-600 dark:text-stone-400"
                            >
                                "Only members can view the group and its library."
                            </span>
                        </span>
                    </label>
                </fieldset>
                <div class="flex flex-wrap gap-3">
                    <button
                        class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700"
                        type="submit"
                    >
                        "Create Group"
                    </button>
                    <a
                        class="border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
                        href=(href!(super::list_groups))
                    >
                        "Cancel"
                    </a>
                </div>
            </form>
        </main>
    }
}

#[route(POST)]
async fn create_group(cx: &Cx, Form(input): Form<CreateGroup>) -> Result<Response> {
    let user_id = authenticated_user(cx).await?;
    let group_id = app_context::<Dataserver>(cx)
        .create_private_group(user_id, input.name.as_str())
        .await?;
    see_other(href!(library_settings, GroupId(group_id)).resolve(cx)).into_response(cx)
}
