mod edit;
mod r#new;
mod revoke;

pub(super) use edit::{EditKeyId, edit_key as edit_key_route};
pub(super) use r#new::new_key as new_key_route;
pub(super) use revoke::revoke_key;

use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context, memoize},
    router::{error::not_found, href},
    view::{PromotedStr, component, view},
};

use crate::{
    app::confirm_button,
    dataserver::{
        ApiKey, ApiKeyAccess, ApiKeyGroupAccess, ApiKeyGroupTarget, ApiKeyLibraryAccess, Dataserver,
    },
    id::UserId,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(try_from = "String")]
pub(super) struct ApiKeyValue(String);

impl ApiKeyValue {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }

    fn bytes(&self) -> [u8; 24] {
        self.0
            .as_bytes()
            .try_into()
            .expect("ApiKeyValue validation enforces a 24-byte ASCII key")
    }
}

impl std::fmt::Display for ApiKeyValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<String> for ApiKeyValue {
    type Error = &'static str;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        if value.len() == 24 && value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            Ok(Self(value))
        } else {
            Err("API key must contain exactly 24 ASCII letters or digits")
        }
    }
}

impl std::str::FromStr for ApiKeyValue {
    type Err = &'static str;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Self::try_from(value.to_owned())
    }
}

#[memoize]
async fn memoized_access_key(cx: &Cx, user_id: UserId, key: [u8; 24]) -> Result<Option<ApiKey>> {
    let key = std::str::from_utf8(&key).expect("ApiKeyValue contains only ASCII");
    app_context::<Dataserver>(cx)
        .access_key(user_id, key)
        .await
        .map_err(Into::into)
}

async fn editor_key(
    cx: &Cx,
    user_id: UserId,
    key_value: Option<&ApiKeyValue>,
) -> Result<Option<ApiKey>> {
    let Some(key_value) = key_value else {
        return Ok(None);
    };
    match memoized_access_key(cx, user_id, key_value.bytes()).await {
        Ok(access_key) => Ok(Some(access_key.clone().ok_or_else(not_found)?)),
        Err(error) => {
            eprintln!("failed to load API key: {error:#}");
            Err(anyhow::anyhow!("failed to load API key").into())
        }
    }
}

fn default_access() -> ApiKeyAccess {
    ApiKeyAccess {
        user: ApiKeyLibraryAccess {
            library: true,
            notes: true,
            files: true,
            write: true,
        },
        groups: Default::default(),
    }
}

#[component]
pub(super) async fn key_editor(user_id: UserId, key_value: Option<&ApiKeyValue>) -> Result {
    view! {
        <form
            class="space-y-8"
            action=(href!(super::savekey::save_key))
            data-key-editor=""
            data-key=(key_value.map(ApiKeyValue::as_str).unwrap_or(""))
        >
            <div class="space-y-8" data-key-editor-fields="">
                key_editor_fields(user_id: user_id, key_value: key_value)
                specific_group_permissions(user_id: user_id, key_value: key_value)
                <div
                    class="flex flex-wrap items-center gap-3 border-t border-stone-200 pt-6 dark:border-stone-800"
                >
                    <button
                        class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700 disabled:opacity-50"
                        type="submit"
                    >
                        "Save Key"
                    </button>
                    <a
                        class="border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
                        href=(href!(super::security::security)
                            .fragment("applications"))
                    >
                        "Manage all access keys"
                    </a>
                </div>
            </div>

            <div
                class="border border-green-700 bg-green-50 p-4 text-green-950 dark:border-green-500 dark:bg-green-950 dark:text-green-100"
                hidden="hidden"
                role="status"
                data-key-created=""
            >
                <p>
                    "Your new API key has been created. Save it now; it will not be displayed again."
                </p>
                <code
                    class="mt-3 block break-all text-lg font-semibold"
                    data-key-value=""
                ></code>
            </div>
            <p
                class="border border-green-700 bg-green-50 p-4 text-green-950 dark:border-green-500 dark:bg-green-950 dark:text-green-100"
                hidden="hidden"
                role="status"
                data-key-saved=""
            >
                "Key Saved"
            </p>
            <p
                class="text-sm text-red-800 dark:text-red-400"
                hidden="hidden"
                role="alert"
                data-key-error=""
            ></p>
        </form>
        if let Some(key_value) = key_value {
            <form
                class="mt-8 border-t border-stone-200 pt-6 dark:border-stone-800"
                method="post"
                action=(href!(revoke_key))
            >
                <input type="hidden" name="key" value=(key_value.as_str())>
                confirm_button(
                    label: "Revoke Key",
                    confirmation: "Permanently revoke this API key? Applications using it will immediately lose access.",
                    class: "border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950"
                )
            </form>
        }
        <script
            type="module"
            src=(topcoat::asset::asset!("src/app/settings/keys/key_editor.js"))
        ></script>
    }
}

#[component]
async fn key_editor_fields(cx: &Cx, user_id: UserId, key_value: Option<&ApiKeyValue>) -> Result {
    let access_key = editor_key(cx, user_id, key_value).await?;
    let access = access_key
        .as_ref()
        .map(|access_key| access_key.access.clone())
        .unwrap_or_else(default_access);
    let all_groups = permission_value(access.groups.get(&ApiKeyGroupTarget::All));
    view! {
        <div>
            <label class="block font-medium" for="key-name">"Key name"</label>
            <input
                class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                id="key-name"
                name="name"
                value=(access_key
                    .as_ref()
                    .map(|access_key| access_key.name.as_str())
                    .unwrap_or(""))
                required="required"
                maxlength="255"
                autofocus="autofocus"
            >
        </div>

        <fieldset class="border-t border-stone-200 pt-6 dark:border-stone-800">
            <legend class="text-xl font-semibold">"Personal Library"</legend>
            <div class="mt-4 grid gap-3 sm:grid-cols-2">
                permission_checkbox(
                    id: PromotedStr(&"user-library"),
                    label: PromotedStr(&"Library access"),
                    name: PromotedStr(&"user-library"),
                    checked: access.user.library
                )
                permission_checkbox(
                    id: PromotedStr(&"user-notes"),
                    label: PromotedStr(&"Notes access"),
                    name: PromotedStr(&"user-notes"),
                    checked: access.user.notes
                )
                permission_checkbox(
                    id: PromotedStr(&"user-write"),
                    label: PromotedStr(&"Write access"),
                    name: PromotedStr(&"user-write"),
                    checked: access.user.write
                )
            </div>
        </fieldset>

        <fieldset class="border-t border-stone-200 pt-6 dark:border-stone-800">
            <legend class="text-xl font-semibold">"Group Permissions"</legend>
            <p class="mt-2 text-sm text-stone-600 dark:text-stone-400">
                "Allow access to all current and future groups."
            </p>
            permission_radios(
                name: "all-groups",
                group_id: None,
                selected: all_groups
            )
        </fieldset>
    }
}

#[component]
async fn specific_group_permissions(
    cx: &Cx,
    user_id: UserId,
    key_value: Option<&ApiKeyValue>,
) -> Result {
    // The sibling fields component starts the memoized key lookup while this
    // component loads the user's groups. Awaiting the shared key afterward
    // preserves parallel I/O without a page-level join.
    let groups = app_context::<Dataserver>(cx)
        .groups_for_user(user_id)
        .await?;
    let access_key = editor_key(cx, user_id, key_value).await?;
    let access = access_key
        .as_ref()
        .map(|access_key| access_key.access.clone())
        .unwrap_or_else(default_access);
    let per_group_enabled = access
        .groups
        .keys()
        .any(|target| matches!(target, ApiKeyGroupTarget::Group(_)));
    let groups = groups
        .into_iter()
        .map(|group| {
            let target = ApiKeyGroupTarget::Group(group.id);
            let permission = permission_value(access.groups.get(&target));
            (group, permission)
        })
        .collect::<Vec<_>>();
    view! {
        <fieldset class="border-t border-stone-200 pt-6 dark:border-stone-800">
            <legend class="text-xl font-semibold">"Specific Groups"</legend>
            <label class="mt-4 flex items-center gap-3">
                <input
                    type="checkbox"
                    checked=(per_group_enabled)
                    data-per-group-toggle=""
                >
                <span class="font-medium">"Per Group Permissions"</span>
            </label>
            <div
                class="mt-5 space-y-5"
                hidden=(!per_group_enabled)
                data-per-group-permissions=""
            >
                if groups.is_empty() {
                    <p class="text-sm text-stone-600 dark:text-stone-400">
                        "You do not currently belong to any groups."
                    </p>
                }
                for (group, permission) in groups {
                    let radio_name = format!("group-{}", group.id);
                    <div class="border border-stone-200 p-4 dark:border-stone-800">
                        <p class="font-medium">(group.name.as_str())</p>
                        permission_radios(
                            key: group.id,
                            name: radio_name.as_str(),
                            group_id: Some(group.id),
                            selected: permission
                        )
                    </div>
                }
            </div>
        </fieldset>
    }
}

#[component]
async fn permission_checkbox(
    id: PromotedStr,
    label: PromotedStr,
    name: PromotedStr,
    checked: bool,
) -> Result {
    view! {
        <label class="flex items-center gap-3" for=(id)>
            <input id=(id) type="checkbox" name=(name) checked=(checked)>
            <span>(label)</span>
        </label>
    }
}

#[component]
async fn permission_radios(name: &str, group_id: Option<u64>, selected: &str) -> Result {
    view! {
        <div
            class="mt-3 flex flex-wrap gap-x-6 gap-y-2"
            data-group-permission=(group_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "all".to_owned()))
        >
            for (value, label) in [
                (PromotedStr(&"none"), PromotedStr(&"None")),
                (PromotedStr(&"read"), PromotedStr(&"Read Only")),
                (PromotedStr(&"write"), PromotedStr(&"Read/Write")),
            ] {
                <label class="flex items-center gap-2">
                    <input
                        type="radio"
                        name=(name)
                        value=(value)
                        checked=(selected == *value)
                    >
                    <span>(label)</span>
                </label>
            }
        </div>
    }
}

fn permission_value(access: Option<&ApiKeyGroupAccess>) -> &'static str {
    match access {
        Some(access) if access.library && access.write => "write",
        Some(access) if access.library => "read",
        _ => "none",
    }
}
