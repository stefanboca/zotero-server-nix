mod edit;
mod r#new;
mod revoke;

pub(super) use edit::{EditKeyId, edit_key as edit_key_route};
pub(super) use r#new::new_key as new_key_route;
pub(super) use revoke::revoke_key;
use serde::Deserialize;
use topcoat::Result;
use topcoat::context::{Cx, app_context, memoize};
use topcoat::router::error::not_found;
use topcoat::router::href;
use topcoat::view::{PromotedStr, View, component, view};

use crate::app::confirm_button;
use crate::dataserver::{
    ApiKey, ApiKeyAccess, ApiKeyGroupAccess, ApiKeyGroupTarget, ApiKeyLibraryAccess, Dataserver,
};
use crate::id::UserId;

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
    type Error = anyhow::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        if value.len() == 24 && value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            Ok(Self(value))
        } else {
            anyhow::bail!("API key must contain exactly 24 ASCII letters or digits")
        }
    }
}

impl std::str::FromStr for ApiKeyValue {
    type Err = anyhow::Error;

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
            tracing::error!(
                user_id = %user_id,
                error = %format_args!("{error:#}"),
                "failed to load API key",
            );
            Err(anyhow::anyhow!("failed to load API key").into())
        }
    }
}

fn default_access() -> ApiKeyAccess {
    ApiKeyAccess {
        user: ApiKeyLibraryAccess {
            library: true,
            notes: false,
            files: true,
            write: false,
        },
        groups: [(
            ApiKeyGroupTarget::All,
            ApiKeyGroupAccess {
                library: true,
                write: false,
            },
        )]
        .into_iter()
        .collect(),
    }
}

#[component]
pub(super) async fn key_editor(
    user_id: UserId,
    key_value: Option<&ApiKeyValue>,
) -> Result<impl View> {
    Ok(view! {
        <form
            class="space-y-8"
            id="key-editor-form"
            action=(href!(super::savekey::save_key))
            data-key-editor=""
            data-key=(key_value.map(ApiKeyValue::as_str).unwrap_or(""))
        >
            <div data-key-editor-fields="">
                key_name(user_id: user_id, key_value: key_value)
                <div class="portal-card mt-3">
                    <h2 class="text-xl font-normal">"Permissions"</h2>
                    <ul
                        class="mt-4 list-disc space-y-1 pl-12"
                        data-permissions-summary=""
                    >
                        <li>"No additional permissions selected"</li>
                    </ul>
                </div>
                <div class="portal-card mt-5 space-y-6">
                    library_permissions(user_id: user_id, key_value: key_value)
                    specific_group_permissions(user_id: user_id, key_value: key_value)
                </div>
            </div>

            <div class="space-y-4" hidden="hidden" role="status" data-key-created="">
                <p>
                    "Your new API key has been created and is displayed below. Please save it now as it will not be accessible again after this point."
                </p>
                <div class="portal-alert portal-alert-success text-center">
                    <code
                        class="block break-all text-lg font-semibold"
                        data-key-value=""
                    ></code>
                </div>
            </div>
            <p
                class="portal-alert portal-alert-success"
                hidden="hidden"
                role="status"
                data-key-saved=""
            >
                "Key Saved"
            </p>
            <p
                class="portal-alert portal-alert-danger text-sm"
                hidden="hidden"
                role="alert"
                data-key-error=""
            ></p>
        </form>

        <div class="mt-8 flex flex-wrap items-center gap-3" data-key-editor-actions="">
            <button
                class="portal-button portal-button-primary portal-button-large"
                type="submit"
                form="key-editor-form"
                data-save-key=""
            >
                "Save Key"
            </button>
            if let Some(key_value) = key_value {
                <form method="post" action=(href!(revoke_key))>
                    <input type="hidden" name="key" value=(key_value.as_str())>
                    confirm_button(
                        label: "Revoke Key",
                        confirmation: "Permanently revoke this API key? Applications using it will immediately lose access.",
                        class: "portal-button portal-button-primary portal-button-large"
                    )
                </form>
            }
        </div>
        <a
            class="portal-link mt-4 inline-block"
            href=(href!(super::security::security).fragment("applications"))
        >
            "Manage all access keys"
        </a>
        <script
            type="module"
            src=(topcoat::asset::asset!("src/app/settings/keys/key_editor.js"))
        ></script>
    })
}

#[component]
async fn key_name(cx: &Cx, user_id: UserId, key_value: Option<&ApiKeyValue>) -> Result<impl View> {
    let access_key = editor_key(cx, user_id, key_value).await?;
    Ok(view! {
        <div>
            <label class="portal-form-label" for="key-name">"Key Name"</label>
            <input
                class="portal-form-control portal-form-control-block max-w-[17.5rem]"
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
    })
}

#[component]
async fn library_permissions(
    cx: &Cx,
    user_id: UserId,
    key_value: Option<&ApiKeyValue>,
) -> Result<impl View> {
    let access_key = editor_key(cx, user_id, key_value).await?;
    let access = access_key
        .as_ref()
        .map(|access_key| access_key.access.clone())
        .unwrap_or_else(default_access);
    let all_groups = permission_value(access.groups.get(&ApiKeyGroupTarget::All));
    Ok(view! {
        <section class="portal-key-permission-box">
            <h2 class="portal-key-heading">"Personal Library"</h2>
            <div class="mt-4 space-y-1">
                permission_checkbox(
                    id: PromotedStr(&"user-library"),
                    label: PromotedStr(&"Allow library access"),
                    description: PromotedStr(
                        &"Allow third party to access your library.",
                    ),
                    name: PromotedStr(&"user-library"),
                    checked: access.user.library
                )
                permission_checkbox(
                    id: PromotedStr(&"user-notes"),
                    label: PromotedStr(&"Allow notes access"),
                    description: PromotedStr(&"Allow third party to access your notes."),
                    name: PromotedStr(&"user-notes"),
                    checked: access.user.notes
                )
                permission_checkbox(
                    id: PromotedStr(&"user-write"),
                    label: PromotedStr(&"Allow write access"),
                    description: PromotedStr(
                        &"Allow third party to make changes to your library.",
                    ),
                    name: PromotedStr(&"user-write"),
                    checked: access.user.write
                )
            </div>
        </section>

        <section class="portal-key-permission-box">
            <h2 class="portal-key-heading">"Default Group Permissions"</h2>
            <p class="mt-4">"All Groups"</p>
            permission_radios(name: "all-groups", group_id: None, selected: all_groups)
            <p class="mt-2 text-sm text-stone-600">
                "Allow access to all current and future groups."
            </p>
        </section>
    })
}

#[component]
async fn specific_group_permissions(
    cx: &Cx,
    user_id: UserId,
    key_value: Option<&ApiKeyValue>,
) -> Result<impl View> {
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
    Ok(view! {
        <section>
            <h2 class="portal-key-heading">"Specific Groups"</h2>
            <label class="mt-4 flex items-center gap-2">
                <input
                    type="checkbox"
                    checked=(per_group_enabled)
                    data-per-group-toggle=""
                >
                <span>"Per Group Permissions"</span>
            </label>
            <p class="ml-6 mt-1 text-sm text-stone-600">
                "Set group by group permissions for this key."
            </p>
            <div
                class="mt-2 space-y-6"
                hidden=(!per_group_enabled)
                data-per-group-permissions=""
            >
                if groups.is_empty() {
                    <p class="text-sm text-stone-600">
                        "You do not currently belong to any groups."
                    </p>
                }
                for (group, permission) in groups {
                    let radio_name = format!("group-{}", group.id);
                    <div data-key-group=(group.name.as_str())>
                        <p>(group.name.as_str())</p>
                        permission_radios(
                            key: group.id,
                            name: radio_name.as_str(),
                            group_id: Some(group.id),
                            selected: permission
                        )
                        <p class="mt-3 text-sm text-stone-600">
                            "Access level to this group."
                        </p>
                    </div>
                }
            </div>
        </section>
    })
}

#[component]
async fn permission_checkbox(
    id: PromotedStr,
    label: PromotedStr,
    description: PromotedStr,
    name: PromotedStr,
    checked: bool,
) -> Result<impl View> {
    Ok(view! {
        <label class="flex items-start gap-3" for=(id)>
            <input class="mt-1" id=(id) type="checkbox" name=(name) checked=(checked)>
            <span>
                <span class="portal-form-label">(label)</span>
                <span class="mt-1 block text-sm text-stone-600">(description)</span>
            </span>
        </label>
    })
}

#[component]
async fn permission_radios(name: &str, group_id: Option<u64>, selected: &str) -> Result<impl View> {
    Ok(view! {
        <div
            class="mt-3 space-y-2"
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
    })
}

fn permission_value(access: Option<&ApiKeyGroupAccess>) -> &'static str {
    match access {
        Some(access) if access.library && access.write => "write",
        Some(access) if access.library => "read",
        _ => "none",
    }
}
