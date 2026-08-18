use topcoat::{
    Result,
    context::Cx,
    router::{error::redirect, href, route},
};

#[route(*)]
async fn register(cx: &Cx) -> Result<()> {
    Err(redirect(&href!(super::login::login).resolve(cx)).into())
}
