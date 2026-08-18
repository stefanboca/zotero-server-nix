use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{href, page},
    runtime::shard,
    view::view,
};

use crate::{app::groups::group_id::GroupId, database::Database, dataserver::GroupRole};

use super::settings_context;

#[page]
pub(in crate::app::groups) async fn members(cx: &Cx) -> Result {
    let context = settings_context(cx);
    let group_id = context.group.id;
    let search_results_id = format!("member-search-results-{group_id}");
    view! {
        signal members_refresh = false;
        signal member_search_query = String::new();
        signal member_search_open = false;

        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"Group Members"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Add members and assign their roles."
            </p>
        </header>

        <input
            type="checkbox"
            hidden="hidden"
            data-members-refresh=""
            @change=$(|event: topcoat::runtime::Event| {
                members_refresh.set(event.target.checked)
            })
        >
        members_shard(refresh: $(members_refresh.get()))

        <section class="py-8">
            <h2 class="text-xl font-semibold">"Add a Member"</h2>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Search by name, username, or email. The account must have signed in to this server at least once."
            </p>
            <form
                class="mt-5 grid gap-3 sm:grid-cols-[minmax(0,1fr)_10rem_5.5rem] sm:items-start sm:gap-4"
                method="post"
                action=(href!(
                    super::super::members::create_member,
                    GroupId(group_id),
                ))
                data-member-picker=""
                data-member-create=""
            >
                <div class="relative">
                    <label class="sr-only" for="member-identifier">
                        "Name, username, or email"
                    </label>
                    <input
                        type="checkbox"
                        hidden="hidden"
                        data-member-search-open=""
                        @change=$(|event: topcoat::runtime::Event| {
                            member_search_open.set(event.target.checked)
                        })
                    >
                    <input
                        class="w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                        id="member-identifier"
                        type="search"
                        name="identifier"
                        placeholder="Name, username, or email"
                        required="required"
                        autocomplete="off"
                        role="combobox"
                        aria-autocomplete="list"
                        aria-expanded="false"
                        aria-controls=(search_results_id.as_str())
                        data-member-search=""
                        :value=$(member_search_query.get())
                        @input=$(|event: topcoat::runtime::Event| {
                            member_search_query.set(event.target.value)
                        })
                    >
                    <div
                        class="absolute z-10 mt-1 max-h-96 w-full overflow-y-auto border border-stone-300 bg-white shadow-lg dark:border-stone-700 dark:bg-stone-950"
                        id=(search_results_id.as_str())
                        role="listbox"
                        aria-live="polite"
                        data-member-search-results=""
                        :hidden=$(!member_search_open.get())
                    >
                        member_search_shard(query: $(member_search_query.get()))
                    </div>
                </div>
                <label>
                    <span class="sr-only">"Role"</span>
                    <select
                        class="w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                        name="role"
                    >
                        <option value="member">"Member"</option>
                        <option value="admin">"Administrator"</option>
                    </select>
                </label>
                <button
                    class="w-full bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
                    type="submit"
                >
                    "Add"
                </button>
            </form>
            <p
                class="mt-3 text-sm text-stone-600 dark:text-stone-400"
                hidden="hidden"
                role="status"
                data-member-feedback=""
            ></p>
        </section>

        <script
            type="module"
            src=(topcoat::asset::asset!(
                "src/app/groups/group_id/group_actions.js"
            ))
        ></script>
        <script
            type="module"
            src=(topcoat::asset::asset!(
                "src/app/groups/group_id/member_search.js"
            ))
        ></script>
    }
}

#[shard]
async fn members_shard(cx: &Cx, refresh: bool) -> Result {
    let _ = refresh;
    let context = settings_context(cx);
    let group = &context.group;
    let mut users = app_context::<Database>(cx)
        .users_by_ids(&group.user_ids())
        .await?;
    users.sort_by_key(|member| member.user_id != context.actor_id);
    let group_overview = href!(
        super::super::group_slug::group_overview,
        GroupId(group.id),
        super::super::group_slug::GroupSlug(group.slug()),
    )
    .resolve(cx);
    let groups = href!(crate::app::groups::list_groups).resolve(cx);

    view! {
        <section
            class="border-b border-stone-200 py-8 dark:border-stone-800"
            data-members-region=""
        >
            <h2 class="text-xl font-semibold">"Members"</h2>
            <form
                action=(href!(
                    super::super::members::create_member,
                    GroupId(group.id),
                ))
                data-member-roles=""
                data-redirect=(group_overview.as_str())
                data-removed-redirect=(groups.as_str())
            >
                <ul class="mt-5 divide-y divide-stone-200 dark:divide-stone-800">
                    for member in users {
                        let member_role = group
                            .role(member.user_id)
                            .unwrap_or(GroupRole::Member);
                        <li
                            class="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between"
                        >
                            <div class="min-w-0">
                                <p class="truncate font-medium">(member.display_name())</p>
                                <p
                                    class="truncate text-sm text-stone-600 dark:text-stone-400"
                                >
                                    <span class="font-mono font-semibold">
                                        (member.username.as_str())
                                    </span>
                                    " · "
                                    (member.email.as_str())
                                </p>
                            </div>
                            if member_role != GroupRole::Owner {
                                <label class="w-full sm:w-auto">
                                    <span class="sr-only">
                                        (format!("Role for {}", member.display_name()))
                                    </span>
                                    <select
                                        class="w-full border border-stone-300 bg-white px-3 py-2 text-sm dark:border-stone-700 dark:bg-stone-950 sm:w-auto"
                                        data-member-role=""
                                        data-user-id=(member.user_id.to_string())
                                        data-current-role=(member_role.to_string())
                                        data-self=(if member.user_id == context.actor_id {
                                            "true"
                                        } else {
                                            "false"
                                        })
                                    >
                                        <option
                                            value="admin"
                                            selected=(member_role == GroupRole::Admin)
                                        >
                                            "Administrator"
                                        </option>
                                        <option
                                            value="member"
                                            selected=(member_role == GroupRole::Member)
                                        >
                                            "Member"
                                        </option>
                                        <option value="remove">"Remove from group"</option>
                                    </select>
                                </label>
                            } else {
                                <span class="text-sm text-stone-600 dark:text-stone-400">
                                    (member_role.display_name())
                                </span>
                            }
                        </li>
                    }
                </ul>
                <div class="mt-6 flex flex-wrap items-center gap-3">
                    <button
                        class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
                        type="submit"
                    >
                        "Update Roles"
                    </button>
                    <p
                        class="text-sm text-stone-600 dark:text-stone-400"
                        hidden="hidden"
                        role="status"
                        data-role-feedback=""
                    ></p>
                </div>
            </form>
        </section>
    }
}

#[shard]
async fn member_search_shard(cx: &Cx, query: String) -> Result {
    let context = settings_context(cx);
    let query = query.trim();
    if query.chars().count() < 3 || query.chars().count() > 128 {
        return view! {};
    }
    let candidates = app_context::<Database>(cx)
        .search_users(query, 10)
        .await?
        .into_iter()
        .filter(|candidate| context.group.role(candidate.user_id).is_none())
        .collect::<Vec<_>>();
    view! {
        <div class="p-3">
            if candidates.is_empty() {
                <p class="px-2 py-4 text-stone-600 dark:text-stone-400">
                    "No matching users found. You can still enter an exact username or email."
                </p>
            } else {
                <ul class="divide-y divide-stone-200 dark:divide-stone-800">
                    for candidate in candidates {
                        <li>
                            <button
                                class="w-full px-2 py-3 text-left hover:bg-stone-100 focus:bg-stone-100 aria-selected:bg-stone-100 dark:hover:bg-stone-900 dark:focus:bg-stone-900 dark:aria-selected:bg-stone-900"
                                id=(format!("member-option-{}", candidate.username))
                                type="button"
                                role="option"
                                aria-selected="false"
                                data-member-option=""
                                data-username=(candidate.username.as_str())
                            >
                                <p class="font-medium">(candidate.display_name())</p>
                                <p
                                    class="break-words text-sm text-stone-600 dark:text-stone-400"
                                >
                                    <span class="font-mono font-semibold">
                                        (candidate.username.as_str())
                                    </span>
                                    " · "
                                    (candidate.email.as_str())
                                </p>
                            </button>
                        </li>
                    }
                </ul>
            }
        </div>
    }
}
