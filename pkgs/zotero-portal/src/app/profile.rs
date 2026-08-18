use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::not_found, href, page, path_param},
    view::view,
};

use crate::{app::authenticated_user, database::Database};

topcoat::router::path_param!(pub(super) profile_username);

/// The official navbar's "My Profile" entry points to `/<username>`.
#[page("/{profile_username}")]
pub(super) async fn profile(cx: &Cx) -> Result {
    let actor_id = authenticated_user(cx).await?;
    let username = path_param::<ProfileUsername>(cx);
    let user = app_context::<Database>(cx)
        .user_by_identifier(username)
        .await?
        .filter(|user| user.username == username)
        .ok_or_else(not_found)?;
    let profile = app_context::<Database>(cx)
        .user_profile(user.user_id)
        .await?;

    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
                <h1 class="text-3xl font-semibold">(user.display_name())</h1>
                <p class="mt-2 text-stone-600 dark:text-stone-400">
                    <span class="font-mono font-semibold">
                        (user.username.as_str())
                    </span>
                </p>
            </header>
            if profile.location.is_some() || profile.disciplines.is_some()
                || profile.affiliation.is_some() || profile.about.is_some() {
                <section class="border-b border-stone-200 py-8 dark:border-stone-800">
                    <h2 class="text-xl font-semibold">"Profile"</h2>
                    <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
                        if let Some(location) = profile.location.as_deref() {
                            <dt class="text-stone-600 dark:text-stone-400">
                                "Location"
                            </dt>
                            <dd class="whitespace-pre-wrap">(location)</dd>
                        }
                        if let Some(disciplines) = profile
                            .disciplines
                            .as_deref() {
                            <dt class="text-stone-600 dark:text-stone-400">
                                "Disciplines"
                            </dt>
                            <dd class="whitespace-pre-wrap">(disciplines)</dd>
                        }
                        if let Some(affiliation) = profile
                            .affiliation
                            .as_deref() {
                            <dt class="text-stone-600 dark:text-stone-400">
                                "Affiliation"
                            </dt>
                            <dd class="whitespace-pre-wrap">(affiliation)</dd>
                        }
                        if let Some(about) = profile.about.as_deref() {
                            <dt class="text-stone-600 dark:text-stone-400">"About"</dt>
                            <dd class="whitespace-pre-wrap">(about)</dd>
                        }
                    </dl>
                </section>
            }
            if actor_id == user.user_id {
                <section class="py-8">
                    <h2 class="text-xl font-semibold">"Library"</h2>
                    <a
                        class="mt-5 inline-flex border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950"
                        href=(href!(
                            crate::app::_web_library::personal_library_root,
                            crate::app::_web_library::UserSlug(user.username.clone()),
                        ))
                    >
                        "Open My Library"
                    </a>
                </section>
            }
        </main>
    }
}
