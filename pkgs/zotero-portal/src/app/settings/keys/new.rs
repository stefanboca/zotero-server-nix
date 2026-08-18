use topcoat::{Result, context::Cx, router::page, view::view};

use super::{super::settings_user, key_editor};

#[page]
pub(in crate::app::settings) async fn new_key(cx: &Cx) -> Result {
    let user = settings_user(cx);
    view! {
        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"New Private Key"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Create an API key with only the permissions it needs."
            </p>
        </header>
        <section class="py-8">
            key_editor(user_id: user.user_id, key_value: None)
        </section>
    }
}
