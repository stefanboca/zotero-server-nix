use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        StatusCode,
        content::Form,
        error::{bad_request, forbidden},
        header, page,
    },
    view::view,
};

use crate::{
    app::request_confirmed,
    dataserver::{Dataserver, GroupRole},
};

use super::{super::groups_notice_location, current_group};

#[derive(Deserialize)]
struct DeleteGroup {
    confirmation: String,
}

#[page(POST)]
async fn delete_group(cx: &Cx, Form(input): Form<DeleteGroup>) -> Result {
    let (_, group, role) = current_group(cx).await?;
    if role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    if input.confirmation != group.name {
        return Err(bad_request("group name confirmation did not match").into());
    }
    let result = app_context::<Dataserver>(cx).delete_group(group.id).await;
    let notice = if let Err(error) = result {
        eprintln!("failed to delete group: {error:#}");
        if request_confirmed(cx) {
            return view! { (StatusCode::BAD_GATEWAY) };
        }
        "group-delete-failed"
    } else {
        "group-deleted"
    };
    let location = groups_notice_location(notice, &group.name)?;
    if request_confirmed(cx) {
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
