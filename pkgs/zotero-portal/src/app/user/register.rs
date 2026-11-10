use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::error::{RedirectError, redirect};
use topcoat::router::{href, route};

#[route(*)]
async fn register(cx: &Cx) -> Result<RedirectError> {
    Ok(redirect(href!(super::login::login).resolve(cx)))
}
