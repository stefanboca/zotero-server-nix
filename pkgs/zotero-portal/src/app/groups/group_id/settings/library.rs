use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::see_other,
        href, page,
        response::{IntoResponse, Response},
        route,
    },
    view::{PromotedStr, view},
};

use crate::{app::groups::group_id::GroupId, dataserver::Dataserver};

use super::settings_context;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LibraryEditing {
    Members,
    Admins,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FileEditing {
    Members,
    Admins,
    None,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LibrarySettings {
    library_editing: LibraryEditing,
    file_editing: FileEditing,
}

#[page]
pub(in crate::app::groups) async fn library(cx: &Cx) -> Result {
    let group = &settings_context(cx).group;
    view! {
        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"Library Settings"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Control who can view and change the group library."
            </p>
        </header>
        <form
            class="space-y-8 py-8"
            method="post"
            action=(href!(library, GroupId(group.id)))
        >
            <fieldset>
                <legend class="text-xl font-semibold">"Group Type"</legend>
                <label class="mt-4 flex items-start gap-3">
                    <input
                        class="mt-1"
                        type="radio"
                        checked="checked"
                        disabled="disabled"
                    >
                    <span>
                        <strong class="block">"Private Membership"</strong>
                        <span
                            class="mt-1 block text-sm text-stone-600 dark:text-stone-400"
                        >
                            "Only members can view this group."
                        </span>
                    </span>
                </label>
            </fieldset>
            <fieldset class="border-t border-stone-200 pt-6 dark:border-stone-800">
                <legend class="text-xl font-semibold">"Library Reading"</legend>
                <label class="mt-4 flex items-center gap-3">
                    <input type="radio" checked="checked" disabled="disabled">
                    <span>"Any group member"</span>
                </label>
            </fieldset>
            <fieldset class="border-t border-stone-200 pt-6 dark:border-stone-800">
                <legend class="text-xl font-semibold">"Library Editing"</legend>
                <div class="mt-4 space-y-3">
                    setting_radio(
                        id: PromotedStr(&"library-edit-members"),
                        name: PromotedStr(&"library_editing"),
                        value: PromotedStr(&"members"),
                        label: PromotedStr(&"Any group member"),
                        checked: group.library_editing == "members"
                    )
                    setting_radio(
                        id: PromotedStr(&"library-edit-admins"),
                        name: PromotedStr(&"library_editing"),
                        value: PromotedStr(&"admins"),
                        label: PromotedStr(&"Only group admins"),
                        checked: group.library_editing == "admins"
                    )
                </div>
            </fieldset>
            <fieldset class="border-t border-stone-200 pt-6 dark:border-stone-800">
                <legend class="text-xl font-semibold">"File Editing"</legend>
                <div class="mt-4 space-y-3">
                    setting_radio(
                        id: PromotedStr(&"file-edit-members"),
                        name: PromotedStr(&"file_editing"),
                        value: PromotedStr(&"members"),
                        label: PromotedStr(&"Any group member"),
                        checked: group.file_editing == "members"
                    )
                    setting_radio(
                        id: PromotedStr(&"file-edit-admins"),
                        name: PromotedStr(&"file_editing"),
                        value: PromotedStr(&"admins"),
                        label: PromotedStr(&"Only group admins"),
                        checked: group.file_editing == "admins"
                    )
                    setting_radio(
                        id: PromotedStr(&"file-edit-none"),
                        name: PromotedStr(&"file_editing"),
                        value: PromotedStr(&"none"),
                        label: PromotedStr(&"No group file storage"),
                        checked: group.file_editing == "none"
                    )
                </div>
            </fieldset>
            <button
                class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700"
                type="submit"
            >
                "Save Settings"
            </button>
        </form>
    }
}

#[topcoat::view::component]
async fn setting_radio(
    id: PromotedStr,
    name: PromotedStr,
    value: PromotedStr,
    label: PromotedStr,
    checked: bool,
) -> Result {
    view! {
        <label class="flex items-center gap-3" for=(id)>
            <input
                id=(id)
                type="radio"
                name=(name)
                value=(value)
                checked=(checked)
                required="required"
            >
            <span>(label)</span>
        </label>
    }
}

#[route(POST)]
async fn save_library(cx: &Cx, Form(input): Form<LibrarySettings>) -> Result<Response> {
    let mut group = settings_context(cx).group.clone();
    group.group_type = "Private".to_owned();
    group.library_reading = "members".to_owned();
    group.library_editing = match input.library_editing {
        LibraryEditing::Members => "members",
        LibraryEditing::Admins => "admins",
    }
    .to_owned();
    group.file_editing = match input.file_editing {
        FileEditing::Members => "members",
        FileEditing::Admins => "admins",
        FileEditing::None => "none",
    }
    .to_owned();
    app_context::<Dataserver>(cx).update_group(&group).await?;
    see_other(href!(library, GroupId(group.id)).resolve(cx)).into_response(cx)
}
