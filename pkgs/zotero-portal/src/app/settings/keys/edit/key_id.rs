use topcoat::{
    Result,
    context::Cx,
    router::{page, path_param},
    view::view,
};

use crate::app::settings::settings_user;

use super::super::{ApiKeyValue, key_editor};

topcoat::router::path_param!(pub(in crate::app::settings) key_id: ApiKeyValue, error = not_found);

#[page]
pub(in crate::app::settings) async fn edit_key(cx: &Cx) -> Result {
    let user = settings_user(cx);
    let key_id = path_param::<KeyId>(cx)?;
    view! {
        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"Edit Key"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Change this API key’s name and library permissions."
            </p>
        </header>
        <section class="py-8">
            key_editor(user_id: user.user_id, key_value: Some(key_id))
        </section>
    }
}
