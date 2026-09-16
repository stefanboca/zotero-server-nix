use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::{href, page};
use topcoat::view::{View, view};

use super::key_editor;
use crate::app::signed_in_user;

#[page]
pub(in crate::app::settings) async fn new_key(cx: &Cx) -> Result<impl View> {
    let user = signed_in_user(cx).await?;
    Ok(view! {
        <div class="portal-key-editor-page">
            <a
                class="portal-link"
                href=(href!(super::super::security::security)
                    .fragment("applications"))
            >
                "Manage all access keys"
            </a>
            <header><h1 class="portal-key-title">"New Key"</h1></header>
            <div class="mt-4">key_editor(user_id: user.user_id, key_value: None)</div>
        </div>
    })
}
