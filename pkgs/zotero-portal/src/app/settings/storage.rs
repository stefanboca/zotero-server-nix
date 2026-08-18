mod purge;

use topcoat::{
    Result,
    context::{Cx, app_context, memoize},
    router::{href, page},
    view::{component, view},
};

use crate::{
    app::confirm_button,
    dataserver::{Dataserver, StorageUsage},
    id::UserId,
};

use super::{display_timestamp, format_storage_size, settings_user};

#[memoize]
async fn memoized_storage_usage(cx: &Cx, user_id: UserId) -> Result<StorageUsage> {
    app_context::<Dataserver>(cx)
        .storage_usage(user_id)
        .await
        .map_err(Into::into)
}

async fn storage_usage(cx: &Cx, user_id: UserId) -> Result<StorageUsage> {
    match memoized_storage_usage(cx, user_id).await {
        Ok(usage) => Ok(usage.clone()),
        Err(error) => {
            eprintln!("failed to load storage usage: {error:#}");
            Err(anyhow::anyhow!("failed to load storage usage").into())
        }
    }
}

#[page]
pub(in crate::app) async fn storage(cx: &Cx) -> Result {
    let user_id = settings_user(cx).user_id;
    view! {
        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"Storage"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Review file-storage usage for your libraries."
            </p>
        </header>
        current_plan(user_id: user_id)
        current_usage(user_id: user_id)
        <section class="border-t border-stone-200 py-8 dark:border-stone-800">
            <h2 class="text-xl font-semibold">"Purge Storage in My Library"</h2>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Use this to free space after disabling file syncing for My Library or switching it to WebDAV storage. Synced clients may upload the files again if file syncing remains enabled."
            </p>
            <form
                class="mt-5"
                method="post"
                action=(href!(purge::purge_personal_storage))
            >
                confirm_button(
                    label: "Purge My Library Storage",
                    confirmation: "Permanently remove all stored files for My Library?",
                    class: "border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950"
                )
            </form>
        </section>
    }
}

#[component]
async fn current_plan(cx: &Cx, user_id: UserId) -> Result {
    let usage = storage_usage(cx, user_id).await?;
    let quota = usage
        .quota_mb
        .map(format_storage_size)
        .unwrap_or_else(|| "Unlimited".to_owned());
    let expiration = usage
        .expiration
        .map(|expiration| display_timestamp(jiff::Timestamp::now(), expiration).absolute)
        .unwrap_or_else(|| "Never".to_owned());
    view! {
        <section class="border-b border-stone-200 py-8 dark:border-stone-800">
            <h2 class="text-xl font-semibold">"Current Plan"</h2>
            <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
                <dt class="text-stone-600 dark:text-stone-400">"Quota"</dt>
                <dd class="font-medium">(quota)</dd>
                <dt class="text-stone-600 dark:text-stone-400">"Expiration"</dt>
                <dd class="font-medium">(expiration)</dd>
            </dl>
        </section>
    }
}

#[component]
async fn current_usage(cx: &Cx, user_id: UserId) -> Result {
    let usage = storage_usage(cx, user_id).await?;
    let mut group_usage = usage.groups.clone();
    group_usage.sort_by_key(|usage| usage.group_id);
    view! {
        <section class="py-8">
            <h2 class="text-xl font-semibold">"Current Usage"</h2>
            <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
                <dt class="text-stone-600 dark:text-stone-400">"My Library"</dt>
                <dd class="font-medium">(format_storage_size(usage.library_mb))</dd>
                for usage in group_usage {
                    group_storage_usage(
                        key: usage.group_id,
                        group_id: usage.group_id,
                        usage_mb: usage.usage_mb
                    )
                }
                <dt class="text-stone-600 dark:text-stone-400">"Total"</dt>
                <dd class="font-medium">(format_storage_size(usage.total_mb))</dd>
            </dl>
            <p class="mt-4 text-sm text-stone-600 dark:text-stone-400">
                "Usage includes your personal library and group libraries you own."
            </p>
        </section>
    }
}

#[component]
async fn group_storage_usage(cx: &Cx, group_id: u64, usage_mb: f64) -> Result {
    let group_name = app_context::<Dataserver>(cx)
        .group(group_id)
        .await?
        .map(|group| group.name)
        .unwrap_or_else(|| format!("Group {group_id}"));
    view! {
        <dt class="text-stone-600 dark:text-stone-400">(group_name)</dt>
        <dd class="font-medium">(format_storage_size(usage_mb))</dd>
    }
}
