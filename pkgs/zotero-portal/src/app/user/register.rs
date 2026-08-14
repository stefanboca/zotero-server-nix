use topcoat::{
    Result,
    router::{error::redirect, route},
};

#[route(*)]
async fn register() -> Result<()> {
    Err(redirect("/user/login").into())
}
