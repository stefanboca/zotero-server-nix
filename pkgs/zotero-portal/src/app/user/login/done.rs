use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::page;
use topcoat::view::{View, view};

#[page]
pub(crate) async fn done(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-2xl font-semibold text-red-800">"Login complete"</h1>
            <p class="mt-4 leading-7 text-stone-700">
                "Your Zotero desktop session is ready. You can close this page and return to Zotero."
            </p>
        </main>
    })
}
