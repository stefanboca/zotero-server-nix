use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden},
        header, page,
    },
    view::view,
};

use crate::{app::request_confirmed, backend::Backend};

use super::current_group;

#[derive(Deserialize)]
struct RenameGroup {
    name: String,
}

#[page(POST)]
async fn rename_group(cx: &Cx, Form(input): Form<RenameGroup>) -> Result {
    let (_, group, role) = current_group(cx).await?;
    if !role.can_manage() {
        return Err(forbidden().into());
    }
    let name = input.name.trim();
    if name.is_empty() {
        return Err(bad_request("group name is required").into());
    }
    if name.chars().count() > 255 {
        return Err(bad_request("group name cannot exceed 255 characters").into());
    }

    let succeeded = if let Err(error) = app_context::<Backend>(cx).rename_group(&group, name).await
    {
        eprintln!("failed to rename group: {error:#}");
        false
    } else {
        true
    };
    let notice = if succeeded {
        "group-renamed"
    } else {
        "group-rename-failed"
    };
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("notice", notice)
        .append_pair("name", name)
        .finish();
    let location = HeaderValue::try_from(format!("/groups/{}?{query}", group.id))?;
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
