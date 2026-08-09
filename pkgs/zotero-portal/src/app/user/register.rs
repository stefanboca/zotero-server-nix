use topcoat::{
    Result,
    router::{HeaderValue, StatusCode, header, page},
    view::view,
};

#[page]
async fn register() -> Result {
    view! {
        (StatusCode::TEMPORARY_REDIRECT)
        ((header::LOCATION, HeaderValue::from_static("/user/login")))
    }
}
