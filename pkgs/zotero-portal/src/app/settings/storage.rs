mod purge;

use topcoat::{
    Result,
    context::{Cx, app_context, memoize},
    router::{href, page},
    view::{View, component, view},
};

use crate::{
    app::{confirm_button, section, signed_in_user},
    dataserver::{Dataserver, StorageUsage},
    id::UserId,
};

use super::{format_storage_size, format_timestamp};

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
            tracing::error!(
                user_id = %user_id,
                error = %format_args!("{error:#}"),
                "failed to load storage usage",
            );
            Err(anyhow::anyhow!("failed to load storage usage").into())
        }
    }
}

#[page]
pub(in crate::app) async fn storage(cx: &Cx) -> Result<impl View> {
    let user_id = signed_in_user(cx).await?.user_id;
    Ok(view! {
        <div class="space-y-6">
            section(
                <div>
                    current_plan(user_id: user_id)
                    current_usage(user_id: user_id)
                </div>
            )
            section(
                <form method="post" action=(href!(purge::purge_personal_storage))>
                    confirm_button(
                        label: "Purge Storage in My Library",
                        confirmation: "This will permanently delete all files you have synced to your personal library (My Library) on this Zotero server. This does not delete items, notes, local files, or files in group libraries. Continue?",
                        class: "portal-button portal-button-danger"
                    )
                </form>
                <p class="mt-2 text-sm text-stone-600">
                    "Use this to free up space for group libraries you own after disabling file syncing for My Library or switching to WebDAV storage."
                </p>
            )
        </div>
    })
}

#[component]
async fn current_plan(cx: &Cx, user_id: UserId) -> Result<impl View> {
    let usage = storage_usage(cx, user_id).await?;
    let quota = usage
        .quota_mb
        .map(format_storage_size)
        .unwrap_or_else(|| "Unlimited".to_owned());
    let expiration = usage
        .expiration
        .map(format_timestamp)
        .unwrap_or_else(|| "Never".to_owned());
    Ok(view! {
        <div>
            <h2 class="portal-storage-heading">"Current Plan"</h2>
            <dl class="portal-storage-details">
                <dt>"Quota"</dt>
                <dd>(quota)</dd>
                <dt>"Expiration"</dt>
                <dd>(expiration)</dd>
            </dl>
        </div>
    })
}

#[component]
async fn current_usage(cx: &Cx, user_id: UserId) -> Result<impl View> {
    let usage = storage_usage(cx, user_id).await?;
    let mut group_usage = usage.groups.clone();
    group_usage.sort_by_key(|usage| usage.group_id);
    Ok(view! {
        <dl class="portal-storage-details portal-storage-usage">
            <dt>"Current Usage"</dt>
            <dd>
                "My Library - "
                (format_storage_size(usage.library_mb))
            </dd>
            for usage in group_usage {
                group_storage_usage(
                    key: usage.group_id,
                    group_id: usage.group_id,
                    usage_mb: usage.usage_mb
                )
            }
            <dt aria-hidden="true"></dt>
            <dd>
                "Total - "
                (format_storage_size(usage.total_mb))
            </dd>
        </dl>
    })
}

#[component]
async fn group_storage_usage(cx: &Cx, group_id: u64, usage_mb: f64) -> Result<impl View> {
    let group_name = app_context::<Dataserver>(cx)
        .group(group_id)
        .await?
        .map(|group| group.name)
        .unwrap_or_else(|| format!("Group {group_id}"));
    Ok(view! {
        <dt aria-hidden="true"></dt>
        <dd>
            (group_name)
            " - "
            (format_storage_size(usage_mb))
        </dd>
    })
}
