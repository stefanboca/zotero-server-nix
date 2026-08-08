mod group_id;
mod r#new;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{page, query_params},
    view::{component, view},
};

#[topcoat::router::query_params(error = topcoat::router::error::bad_request)]
struct GroupsQuery {
    notice: Option<String>,
    name: Option<String>,
}

use crate::{
    app::{AppState, is_topcoat_dev, signed_in_user},
    dataserver::{Group, GroupRole},
    store::UserSummary,
};

#[page]
async fn list_groups(cx: &Cx) -> Result {
    let parameters = query_params::<GroupsQuery>(cx)?;
    let user = signed_in_user(cx).await?;
    if is_topcoat_dev() {
        let groups = vec![Group {
            id: 1,
            name: "Research group".to_owned(),
            owner: 1,
            admins: Vec::new(),
            members: vec![2],
            group_type: "Private".to_owned(),
            library_editing: "members".to_owned(),
            library_reading: "members".to_owned(),
            file_editing: "members".to_owned(),
            description: String::new(),
            url: String::new(),
            has_image: false,
        }];
        return view! { groups_index(user: &user, groups: &groups, left_group: left_group(parameters)) };
    }

    let app = app_context::<AppState>(cx);
    let groups = app.dataserver.groups_for_user(user.user_id).await?;
    view! { groups_index(user: &user, groups: &groups, left_group: left_group(parameters)) }
}

fn left_group(parameters: &GroupsQuery) -> Option<&str> {
    (parameters.notice.as_deref() == Some("left-group"))
        .then(|| parameters.name.as_deref())
        .flatten()
}

#[component]
async fn groups_index(user: &UserSummary, groups: &[Group], left_group: Option<&str>) -> Result {
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <div class="pointer-events-none fixed inset-x-4 top-20 z-50 sm:left-auto sm:right-5 sm:w-96" aria-live="polite" aria-atomic="true">
                if let Some(group_name) = left_group {
                    <div class="portal-toast border px-4 py-3 shadow-lg" role="status" data-toast="" data-toast-kind="success">
                        "You left “" (group_name) "”."
                    </div>
                }
            </div>
            <section class="border-b border-stone-200 py-8 dark:border-stone-800">
                <div class="flex flex-wrap items-center justify-between gap-4">
                    <div>
                        <h1 class="text-2xl font-semibold">"Zotero groups"</h1>
                    </div>
                    <a class="rounded-lg bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700 dark:bg-red-700" href="/groups/new">
                        "New group"
                    </a>
                </div>
            </section>

            <section class="pt-8">
                <h2 class="text-xl font-semibold">"Your groups"</h2>
                if groups.is_empty() {
                    <p class="mt-4 text-stone-600 dark:text-stone-400">
                        "You do not belong to any groups yet."
                    </p>
                } else {
                    <ul class="mt-4 divide-y divide-stone-200 dark:divide-stone-800">
                        for group in groups {
                            let role = group.role(user.user_id).unwrap_or(GroupRole::Member);
                            let member_count = group.user_ids().len();
                            <li class="py-4">
                                <a class="flex flex-wrap items-center justify-between gap-3 p-3 hover:bg-stone-100 dark:hover:bg-stone-800" href=(format!("/groups/{}", group.id))>
                                    <span class="font-medium">(group.name.as_str())</span>
                                    <span class="text-sm text-stone-600 dark:text-stone-400">
                                        (role.user_summary()) " · " (member_count) " " if member_count == 1 { "person" } else { "people" } " total"
                                    </span>
                                </a>
                            </li>
                        }
                    </ul>
                }
            </section>
            <script type="module" src=(topcoat::asset::asset!("src/app/toast.js"))></script>
        </main>
    }
}
