use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden},
        header, page,
    },
    view::view,
};

use crate::{
    app::{AppState, is_topcoat_dev},
    dataserver::GroupRole,
};

use super::current_group;

#[derive(Deserialize)]
struct DeleteGroup {
    confirmation: String,
}

#[page]
async fn delete_group_confirmation(cx: &Cx) -> Result {
    let (_, group, role) = current_group(cx).await?;
    if role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    let action = format!("/groups/{}/delete", group.id);
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-2xl font-semibold text-red-900 dark:text-red-300">"Delete group"</h1>
            <p class="mt-4 leading-7">
                "This permanently deletes the group library “" (group.name.as_str()) "”, including its synced items and files. This cannot be undone."
            </p>
            <form class="mt-6 space-y-4" method="post" action=(action)>
                <label class="block font-medium" for="confirmation">
                    "Type the group name to confirm: " <strong>(group.name.as_str())</strong>
                </label>
                <input class="w-full rounded-lg border border-red-300 bg-white px-3 py-2 dark:border-red-900 dark:bg-stone-950" id="confirmation" name="confirmation" required="required" autocomplete="off">
                <div class="flex flex-wrap gap-3">
                    <button class="rounded-lg bg-red-800 px-4 py-3 font-semibold text-white hover:bg-red-700" type="submit">
                        "Permanently delete group"
                    </button>
                    <a class="rounded-lg border border-stone-300 px-4 py-3 dark:border-stone-700" href=(format!("/groups/{}", group.id))>
                        "Cancel"
                    </a>
                </div>
            </form>
        </main>
    }
}

#[page(POST)]
async fn delete_group(cx: &Cx, Form(input): Form<DeleteGroup>) -> Result {
    let (_, group, role) = current_group(cx).await?;
    if role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    if input.confirmation != group.name {
        return Err(bad_request("group name confirmation did not match").into());
    }
    if is_topcoat_dev() {
        return view! {
            (StatusCode::SEE_OTHER)
            ((header::LOCATION, HeaderValue::from_static("/groups")))
        };
    }

    let app = app_context::<AppState>(cx);
    app.dataserver.delete_group(group.id).await?;
    view! {
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, HeaderValue::from_static("/groups")))
    }
}
