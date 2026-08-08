use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Form, error::bad_request, page},
    view::{component, view},
};

use crate::app::{AppState, authenticated_user, is_topcoat_dev};

#[derive(Deserialize)]
struct CreateGroup {
    name: String,
}

#[page]
async fn new_group(cx: &Cx) -> Result {
    authenticated_user(cx).await?;
    view! { group_form(created: None) }
}

#[page(POST)]
async fn create_group(cx: &Cx, Form(input): Form<CreateGroup>) -> Result {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(bad_request("group name cannot be empty").into());
    }
    if name.chars().count() > 255 {
        return Err(bad_request("group name cannot exceed 255 characters").into());
    }

    if is_topcoat_dev() {
        return view! { group_form(created: Some((name, 1))) };
    }

    let user_id = authenticated_user(cx).await?;
    let app = app_context::<AppState>(cx);
    let group_id = app.dataserver.create_private_group(user_id, name).await?;
    view! { group_form(created: Some((name, group_id))) }
}

#[component]
async fn group_form(created: Option<(&str, u64)>) -> Result {
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-2xl font-semibold">"Create a Zotero group"</h1>
            if let Some((name, group_id)) = created {
                <p class="mt-4 rounded-lg bg-green-100 p-4 text-green-900 dark:bg-green-950 dark:text-green-100">
                    "Created “" (name) "”. Sync Zotero to see the new group."
                </p>
                <a class="mt-5 inline-block rounded-lg bg-red-800 px-4 py-3 font-semibold text-white hover:bg-red-700 dark:bg-red-700" href=(format!("/groups/{group_id}"))>
                    "Manage members"
                </a>
            } else {
                <p class="mt-3 leading-7 text-stone-700 dark:text-stone-300">
                    "The new private group will allow all members to edit its library and files."
                </p>
                <form class="mt-6 space-y-4" method="post" action="/groups/new">
                    <label class="block font-medium" for="group-name">"Group name"</label>
                    <input
                        class="w-full rounded-lg border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                        id="group-name"
                        name="name"
                        required="required"
                        maxlength="255"
                    >
                    <button
                        class="w-full cursor-pointer rounded-lg bg-red-800 px-4 py-3 font-semibold text-white hover:bg-red-700 dark:bg-red-700 dark:hover:bg-red-600"
                        type="submit"
                    >
                        "Create group"
                    </button>
                </form>
            }
        </main>
    }
}
