use topcoat::{Result, router::page, view::view};

#[page]
async fn done() -> Result {
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <h1 class="text-2xl font-semibold text-red-800 dark:text-red-400">
                "Login complete"
            </h1>
            <p class="mt-4 leading-7 text-stone-700 dark:text-stone-300">
                "Your Zotero desktop session is ready. You can close this page and return to Zotero."
            </p>
        </main>
    }
}
