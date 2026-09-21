use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::{href, page, path_param};
use topcoat::view::{View, view};

use super::super::{ApiKeyValue, key_editor};
use crate::app::signed_in_user;

topcoat::router::path_param!(pub(in crate::app::settings) key_id: ApiKeyValue, error = not_found);

#[page]
pub(in crate::app::settings) async fn edit_key(cx: &Cx) -> Result<impl View> {
    let user = signed_in_user(cx).await?;
    let key_id = path_param::<KeyId>(cx)?;
    Ok(view! {
        <div class="portal-key-editor-page">
            <a
                class="portal-link"
                href=(href!(super::super::super::security::security).fragment(
                    "applications",
                ))
            >
                "Manage all access keys"
            </a>
            <header><h1 class="portal-key-title">"Edit Key"</h1></header>
            <div class="mt-4">
                key_editor(user_id: user.user_id, key_value: Some(key_id))
            </div>
        </div>
    })
}
