use topcoat::{
    Result,
    context::Cx,
    router::{href, page, path_param},
    view::view,
};

use crate::app::signed_in_user;

use super::super::{ApiKeyValue, key_editor};

topcoat::router::path_param!(pub(in crate::app::settings) key_id: ApiKeyValue, error = not_found);

#[page]
pub(in crate::app::settings) async fn edit_key(cx: &Cx) -> Result {
    let user = signed_in_user(cx).await?;
    let key_id = path_param::<KeyId>(cx)?;
    view! {
        <div class="portal-key-editor-page">
            <a
                class="text-blue-700 hover:underline"
                href=(href!(super::super::super::security::security)
                    .fragment("applications"))
            >
                "Manage all access keys"
            </a>
            <header>
                <h1 class="text-[2.5rem] font-normal leading-tight">"Edit Key"</h1>
            </header>
            <div class="mt-6">
                key_editor(user_id: user.user_id, key_value: Some(key_id))
            </div>
        </div>
    }
}
