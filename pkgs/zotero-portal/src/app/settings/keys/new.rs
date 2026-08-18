use topcoat::{
    Result,
    context::Cx,
    router::{href, page},
    view::view,
};

use crate::app::signed_in_user;

use super::key_editor;

#[page]
pub(in crate::app::settings) async fn new_key(cx: &Cx) -> Result {
    let user = signed_in_user(cx).await?;
    view! {
        <div class="portal-key-editor-page">
            <a
                class="text-blue-700 hover:underline"
                href=(href!(super::super::security::security)
                    .fragment("applications"))
            >
                "Manage all access keys"
            </a>
            <header>
                <h1 class="text-[2.5rem] font-normal leading-tight">"New Key"</h1>
            </header>
            <div class="mt-6">key_editor(user_id: user.user_id, key_value: None)</div>
        </div>
    }
}
