use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{bad_request, unauthorized},
        page,
    },
    session,
    view::view,
};

use crate::{
    app::{AppState, is_topcoat_dev},
    store,
};

#[derive(Deserialize)]
struct CreateGroup {
    name: String,
}

#[page("/groups/new")]
async fn new_group(cx: &Cx) -> Result {
    if !is_topcoat_dev() {
        authenticated_user(cx).await?;
    }
    group_form(None).await
}

#[page(POST "/groups/new")]
async fn create_group(cx: &Cx, Form(input): Form<CreateGroup>) -> Result {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(bad_request("group name cannot be empty").into());
    }
    if name.chars().count() > 255 {
        return Err(bad_request("group name cannot exceed 255 characters").into());
    }

    if is_topcoat_dev() {
        return group_form(Some(name)).await;
    }

    let user_id = authenticated_user(cx).await?;
    let app = app_context::<AppState>(cx);
    app.dataserver.create_private_group(user_id, name).await?;
    group_form(Some(name)).await
}

async fn authenticated_user(cx: &Cx) -> Result<u64> {
    let Some(browser_session) = session::refresh(cx).await? else {
        return Err(unauthorized().into());
    };
    let expires = browser_session
        .expires_at
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let app = app_context::<AppState>(cx);
    let Some(user_id) =
        store::refresh_browser_session(&app.database, &*browser_session.token_hash, expires)
            .await?
    else {
        return Err(unauthorized().into());
    };
    Ok(user_id)
}

async fn group_form(created: Option<&str>) -> Result {
    view! {
        <main
            class="w-full max-w-md rounded-xl border border-stone-200 bg-white p-8 shadow-sm dark:border-stone-800 dark:bg-stone-900"
        >
            <h1 class="text-2xl font-semibold">"Create a Zotero group"</h1>
            if let Some(name) = created {
                <p class="mt-4 rounded-lg bg-green-100 p-4 text-green-900 dark:bg-green-950 dark:text-green-100">
                    "Created “" (name) "”. Sync Zotero to see the new group."
                </p>
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
                        required=true
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

#[page("/settings/storage")]
async fn storage_settings(cx: &Cx) -> Result {
    if !is_topcoat_dev() {
        authenticated_user(cx).await?;
    }

    view! {
        <main
            class="w-full max-w-md rounded-xl border border-stone-200 bg-white p-8 shadow-sm dark:border-stone-800 dark:bg-stone-900"
        >
            <h1 class="text-2xl font-semibold">"Zotero storage"</h1>
            <p class="mt-4 leading-7 text-stone-700 dark:text-stone-300">
                "File storage quota: Unlimited"
            </p>
        </main>
    }
}
