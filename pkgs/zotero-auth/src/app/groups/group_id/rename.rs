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
};

use super::current_group;

#[derive(Deserialize)]
struct RenameGroup {
    name: String,
}

#[page]
async fn rename_group_form(cx: &Cx) -> Result {
    let (_, group, role) = current_group(cx).await?;
    if !role.can_manage() {
        return Err(forbidden().into());
    }
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-2xl font-semibold">"Rename group"</h1>
            <form class="mt-6 space-y-4" method="post" action=(format!("/groups/{}/rename", group.id))>
                <label class="block font-medium" for="group-name">"Group name"</label>
                <input class="w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950" id="group-name" name="name" value=(group.name.as_str()) required="required" autocomplete="off">
                <div class="flex flex-wrap gap-3">
                    <button class="bg-red-800 px-4 py-3 font-semibold text-white hover:bg-red-700" type="submit">"Save name"</button>
                    <a class="border border-stone-300 px-4 py-3 dark:border-stone-700" href=(format!("/groups/{}", group.id))>"Cancel"</a>
                </div>
            </form>
        </main>
    }
}

#[page(POST)]
async fn rename_group(cx: &Cx, Form(input): Form<RenameGroup>) -> Result {
    let (_, group, role) = current_group(cx).await?;
    if !role.can_manage() {
        return Err(forbidden().into());
    }
    let name = input.name.trim();
    if name.is_empty() {
        return Err(bad_request("group name is required").into());
    }

    let notice = if is_topcoat_dev() {
        "group-renamed"
    } else if let Err(error) = app_context::<AppState>(cx)
        .dataserver
        .rename_group(&group, name)
        .await
    {
        eprintln!("failed to rename group: {error:#}");
        "group-rename-failed"
    } else {
        "group-renamed"
    };
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("notice", notice)
        .append_pair("name", name)
        .finish();
    let location = HeaderValue::try_from(format!("/groups/{}?{query}", group.id))?;
    view! {
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, location))
    }
}
