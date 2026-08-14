use topcoat::{
    Result,
    router::{error::not_found, page},
};

topcoat::router::segment!(kind = CatchAll);

// fallback route to ensure non-existing routes get the nice error layout
#[page]
async fn fallback() -> Result {
    Err(not_found().into())
}
