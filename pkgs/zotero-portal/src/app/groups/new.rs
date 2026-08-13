use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{HeaderValue, StatusCode, content::Form, error::bad_request, header, page},
    view::view,
};

use crate::{
    app::{authenticated_user, request_confirmed},
    backend::Backend,
};

use super::groups_notice_location;

#[derive(Deserialize)]
struct CreateGroup {
    name: String,
}

#[page]
async fn new_group(cx: &Cx) -> Result {
    authenticated_user(cx).await?;
    view! {
        (StatusCode::TEMPORARY_REDIRECT)
        ((header::LOCATION, HeaderValue::from_static("/groups?dialog=new")))
    }
}

#[page(POST)]
async fn create_group(cx: &Cx, Form(input): Form<CreateGroup>) -> Result {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(bad_request("group name cannot be empty").into());
    }
    if name.chars().count() > 255 {
        return Err(bad_request("group name cannot exceed 255 characters").into());
    }

    let user_id = authenticated_user(cx).await?;
    let succeeded = if let Err(error) = app_context::<Backend>(cx)
        .create_private_group(user_id, name)
        .await
    {
        eprintln!("failed to create group: {error:#}");
        false
    } else {
        true
    };
    let notice = if succeeded {
        "group-created"
    } else {
        "group-create-failed"
    };
    let location = groups_notice_location(notice, name)?;
    if request_confirmed(cx) {
        if !succeeded {
            return view! { (StatusCode::BAD_GATEWAY) };
        }
        return view! {
            (StatusCode::NO_CONTENT)
            ((header::LOCATION, location))
        };
    }
    view! {
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, location))
    }
}
