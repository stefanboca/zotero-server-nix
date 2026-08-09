use topcoat::{
    Result,
    context::Cx,
    router::page,
    view::view,
};

use crate::app::{signed_in_user, username_label};

#[page]
async fn account(cx: &Cx) -> Result {
    let user = signed_in_user(cx).await?;
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-2xl font-semibold">"Account"</h1>
            <section class="py-8">
                <dl class="grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
                    <dt class="text-stone-600 dark:text-stone-400">"Name"</dt>
                    <dd>(user.display_name())</dd>
                    <dt class="text-stone-600 dark:text-stone-400">"Username"</dt>
                    <dd class="break-all">username_label(username: user.username.as_str())</dd>
                    <dt class="text-stone-600 dark:text-stone-400">"Email"</dt>
                    <dd class="break-all">(user.email.as_str())</dd>
                </dl>
                <p class="mt-5 text-sm text-stone-600 dark:text-stone-400">
                    "Your identity is managed by the OpenID Connect provider."
                </p>
            </section>
        </main>
    }
}
