use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{bad_request, see_other},
        href, page,
        response::{IntoResponse, Response},
        route,
    },
    view::{PromotedStr, view},
};

use crate::database::{Database, UserProfile};

use super::settings_user;

#[page]
pub(super) async fn profile(cx: &Cx) -> Result {
    let user = settings_user(cx);
    let public_profile = app_context::<Database>(cx)
        .user_profile(user.user_id)
        .await?;
    view! {
        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"Profile"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Your account details on this Zotero server."
            </p>
        </header>
        <section class="border-b border-stone-200 py-8 dark:border-stone-800">
            <h2 class="text-xl font-semibold">"Account"</h2>
            <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
                <dt class="text-stone-600 dark:text-stone-400">"Name"</dt>
                <dd class="font-medium">(user.display_name())</dd>
                <dt class="text-stone-600 dark:text-stone-400">"Username"</dt>
                <dd class="break-all">
                    <span class="font-mono font-semibold">
                        (user.username.as_str())
                    </span>
                </dd>
                <dt class="text-stone-600 dark:text-stone-400">"Email"</dt>
                <dd class="break-all font-medium">(user.email.as_str())</dd>
            </dl>
            <p class="mt-4 text-sm text-stone-600 dark:text-stone-400">
                "Your username and email are managed by the OpenID Connect provider."
            </p>
        </section>
        <form class="space-y-6 py-8" method="post" action=(href!(profile))>
            <div>
                <h2 class="text-xl font-semibold">"Public Profile"</h2>
                <p class="mt-2 text-stone-600 dark:text-stone-400">
                    "These details appear on your profile within this private server."
                </p>
            </div>
            profile_input(
                id: PromotedStr(&"profile-real-name"),
                name: PromotedStr(&"real_name"),
                label: PromotedStr(&"Real name"),
                value: public_profile.real_name.as_deref().unwrap_or(""),
                maxlength: 255
            )
            profile_input(
                id: PromotedStr(&"profile-location"),
                name: PromotedStr(&"location"),
                label: PromotedStr(&"Location"),
                value: public_profile.location.as_deref().unwrap_or(""),
                maxlength: 255
            )
            profile_input(
                id: PromotedStr(&"profile-affiliation"),
                name: PromotedStr(&"affiliation"),
                label: PromotedStr(&"Affiliation"),
                value: public_profile.affiliation.as_deref().unwrap_or(""),
                maxlength: 255
            )
            <div>
                <label class="block font-medium" for="profile-disciplines">
                    "Disciplines"
                </label>
                <textarea
                    class="mt-2 min-h-24 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                    id="profile-disciplines"
                    name="disciplines"
                    maxlength="2000"
                    placeholder="One per line or separated by commas"
                >
                    (public_profile.disciplines.as_deref().unwrap_or(""))
                </textarea>
            </div>
            <div>
                <label class="block font-medium" for="profile-about">"About you"</label>
                <textarea
                    class="mt-2 min-h-36 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                    id="profile-about"
                    name="about"
                    maxlength="10000"
                >
                    (public_profile.about.as_deref().unwrap_or(""))
                </textarea>
            </div>
            <button
                class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700"
                type="submit"
            >
                "Save Profile"
            </button>
        </form>
    }
}

#[topcoat::view::component]
async fn profile_input(
    id: PromotedStr,
    name: PromotedStr,
    label: PromotedStr,
    value: &str,
    maxlength: usize,
) -> Result {
    view! {
        <div>
            <label class="block font-medium" for=(id)>(label)</label>
            <input
                class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                id=(id)
                name=(name)
                value=(value)
                maxlength=(maxlength)
            >
        </div>
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileSettings {
    real_name: String,
    location: String,
    disciplines: String,
    affiliation: String,
    about: String,
}

#[route(POST)]
async fn save_profile(cx: &Cx, Form(input): Form<ProfileSettings>) -> Result<Response> {
    let public_profile = UserProfile {
        real_name: profile_value(input.real_name, 255, "real name")?,
        location: profile_value(input.location, 255, "location")?,
        disciplines: profile_value(input.disciplines, 2000, "disciplines")?,
        affiliation: profile_value(input.affiliation, 255, "affiliation")?,
        about: profile_value(input.about, 10_000, "about")?,
    };
    let user = settings_user(cx);
    app_context::<Database>(cx)
        .update_user_profile(user.user_id, &public_profile)
        .await?;
    see_other(href!(profile).resolve(cx)).into_response(cx)
}

fn profile_value(value: String, maximum: usize, field: &str) -> Result<Option<String>> {
    let value = value.trim();
    if value.chars().count() > maximum {
        return Err(bad_request(&format!("{field} cannot exceed {maximum} characters")).into());
    }
    Ok((!value.is_empty()).then(|| value.to_owned()))
}
