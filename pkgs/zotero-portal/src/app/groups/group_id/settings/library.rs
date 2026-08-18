use serde::{Deserialize, Serialize};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::see_other,
        href, page, query_params,
        response::{IntoResponse, Response},
        route,
    },
    view::{PromotedStr, view},
};

use crate::{
    app::{groups::group_id::GroupId, section, success_message},
    dataserver::Dataserver,
};

use super::managed_group_settings;

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

#[topcoat::router::query_params(error = bad_request)]
#[derive(Serialize)]
struct LibraryQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    saved: Option<bool>,
}

#[page]
pub(in crate::app::groups) async fn library(cx: &Cx) -> Result {
    let context = managed_group_settings(cx).await?;
    let group = &context.group;
    let query = query_params::<LibraryQuery>(cx)?;
    view! {
        if query.saved == Some(true) {
            success_message(message: "Settings saved")
        }
        section(
            <form
                class="space-y-8"
                method="post"
                action=(href!(library, GroupId(group.id)))
            >
                <fieldset>
                    <legend class="portal-section-heading">"Library Editing"</legend>
                    <div class="mt-4 space-y-3">
                        setting_radio(
                            id: PromotedStr(&"library-edit-members"),
                            name: PromotedStr(&"library_editing"),
                            value: PromotedStr(&"members"),
                            label: PromotedStr(&"Any group members"),
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
                    <p class="mt-3 text-sm text-stone-600">
                        "Who can add, edit, and remove items from this group's library?"
                    </p>
                </fieldset>
                <fieldset>
                    <legend class="portal-section-heading">"File Editing"</legend>
                    <div class="mt-4 space-y-3">
                        setting_radio(
                            id: PromotedStr(&"file-edit-members"),
                            name: PromotedStr(&"file_editing"),
                            value: PromotedStr(&"members"),
                            label: PromotedStr(&"Any group members"),
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
                    <p class="mt-3 text-sm text-stone-600">
                        "Who can work with files stored in the group?"
                    </p>
                </fieldset>
                <button class="portal-button portal-button-primary" type="submit">
                    "Save Settings"
                </button>
            </form>
        )
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
    let mut group = managed_group_settings(cx).await?.group;
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
    see_other(
        href!(library, GroupId(group.id))
            .query(LibraryQuery { saved: Some(true) })
            .resolve(cx),
    )
    .into_response(cx)
}
