use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::app::{section, signed_in_user};

#[page]
pub(super) async fn profile(cx: &Cx) -> Result<impl View> {
    let user = signed_in_user(cx).await?;
    Ok(view! {
        <div>
            section(
                <h2 class="portal-section-heading">"Account"</h2>
                <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
                    <dt class="text-stone-600">"Name"</dt>
                    <dd class="font-medium">(user.display_name())</dd>
                    <dt class="text-stone-600">"Username"</dt>
                    <dd class="break-all">
                        <span class="font-mono font-semibold">
                            (user.username.as_str())
                        </span>
                    </dd>
                    <dt class="text-stone-600">"Email"</dt>
                    <dd class="break-all font-medium">(user.email.as_str())</dd>
                </dl>
                <p class="mt-4 text-sm text-stone-600">
                    "Your username and email are managed by the OpenID Connect provider."
                </p>
            )
        </div>
    })
}
