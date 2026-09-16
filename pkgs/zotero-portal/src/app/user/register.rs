use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::error::redirect;
use topcoat::router::{href, route};

#[route(*)]
async fn register(cx: &Cx) -> Result<()> {
    Err(redirect(href!(super::login::login).resolve(cx)).into())
}
