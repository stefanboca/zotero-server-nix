use std::collections::HashSet;

use serde::Deserialize;
use topcoat::context::{Cx, context};
use topcoat::router::content::Form;
use topcoat::router::error::see_other;
use topcoat::router::{href, page};
use topcoat::view::{View, component, view};
use topcoat::{Error, Result};

use super::group_id;
use super::members::managed_group;
use crate::app::{authenticated_user, portal_page, section};
use crate::database::Database;
use crate::dataserver::Group;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invitations {
    identifiers: String,
}

#[component]
async fn invitation_form(
    cx: &Cx,
    group: &Group,
    identifiers: &str,
    error: Option<&str>,
) -> Result<impl View> {
    Ok(view! {
        portal_page(
            section(
                <header class="border-b border-stone-200 pb-6">
                    <h1 class="text-3xl font-semibold">"Invite Members"</h1>
                    <p class="mt-2 text-stone-600">
                        "Group: "
                        (group.name.as_str())
                    </p>
                </header>
                if let Some(error) = error {
                    <p class="portal-alert portal-alert-danger mt-6" role="alert">
                        (error)
                    </p>
                }
                <form
                    class="space-y-6 pt-8"
                    method="post"
                    action=(href!(invite_members, group_id: group.id))
                >
                    <div>
                        <label class="portal-form-label" for="invite-identifiers">
                            "Invite Members"
                        </label>
                        <textarea
                            class="portal-form-control portal-form-control-block min-h-40"
                            id="invite-identifiers"
                            name="identifiers"
                            required="required"
                            maxlength="8192"
                            aria-invalid=(error.map(|_| "true"))
                        >
                            (identifiers)
                        </textarea>
                        <p class="mt-2 text-sm text-stone-600">
                            "Separate email addresses or zotero usernames with commas or lines."
                        </p>
                    </div>
                    <div class="flex flex-wrap gap-3">
                        <button
                            class="portal-button portal-button-primary"
                            type="submit"
                        >
                            "Invite Members"
                        </button>
                        <a
                            class="portal-button portal-button-secondary"
                            href=(href!(super::members_settings, group_id: group.id))
                        >
                            "Cancel"
                        </a>
                    </div>
                </form>
            )
        )
    })
}

#[page]
pub(in crate::app::groups) async fn invite_members(cx: &Cx) -> Result<impl View> {
    let group = managed_group(cx).await?;
    Ok(view! { invitation_form(group: &group, identifiers: "", error: None) })
}

#[page(POST)]
async fn send_invitations(cx: &Cx, Form(input): Form<Invitations>) -> Result<impl View> {
    let group = managed_group(cx).await?;
    let actor_id = authenticated_user(cx).await?;
    let mut seen = HashSet::new();
    let identifiers = input
        .identifiers
        .split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.strip_prefix('@').unwrap_or(value))
        .filter(|value| seen.insert(*value))
        .collect::<Vec<_>>();
    let mut validation_error = if identifiers.is_empty() {
        Some("At least one username or email address is required.".to_owned())
    } else if identifiers.len() > 100 {
        Some("At most 100 people can be invited at once.".to_owned())
    } else {
        None
    };

    let mut invitee_ids = HashSet::with_capacity(identifiers.len());
    if validation_error.is_none() {
        let database = context::<Database>(cx);
        for identifier in identifiers {
            let Some(invitee) = database
                .user_by_identifier(identifier)
                .await
                .map_err(Error::from_anyhow)?
            else {
                validation_error = Some(format!("Error sending invitation to \"{identifier}\""));
                break;
            };
            if invitee.user_id == group.owner || group.role(invitee.user_id).is_some() {
                validation_error = Some(format!(
                    "{} is already a member of this group.",
                    invitee.username,
                ));
                break;
            }
            invitee_ids.insert(invitee.user_id);
        }
    }

    if let Some(error) = validation_error {
        return Ok(view! {
            invitation_form(
                group: &group,
                identifiers: input.identifiers.as_str(),
                error: Some(error.as_str())
            )
        });
    }

    let database = context::<Database>(cx);
    for invitee_id in invitee_ids {
        database
            .create_group_invitation(group.id, actor_id, invitee_id)
            .await
            .map_err(Error::from_anyhow)?;
    }

    Err(see_other(href!(super::members_settings, group_id: group.id).resolve(cx)).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_input_is_typed_and_bounded() {
        let input: Invitations = serde_json::from_value(serde_json::json!({
            "identifiers": "alice, bob@example.test"
        }))
        .expect("typed invitation input");
        assert!(input.identifiers.contains("alice"));
        assert!(
            serde_json::from_value::<Invitations>(serde_json::json!({
                "identifiers": 42.5
            }))
            .is_err()
        );
    }
}
