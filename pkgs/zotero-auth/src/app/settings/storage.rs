use topcoat::{
    Result,
    context::Cx,
    router::{error::redirect_permanent, page},
};

use crate::app::authenticated_user;

#[page]
async fn storage(cx: &Cx) -> Result {
    authenticated_user(cx).await?;
    Err(redirect_permanent("/settings#storage").into())
}
