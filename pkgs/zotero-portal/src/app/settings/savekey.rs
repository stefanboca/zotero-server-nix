use serde::{Deserialize, Serialize};
use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::content::Json;
use topcoat::router::error::{bad_request, not_found};
use topcoat::router::{query_params, route};

use super::keys::ApiKeyValue;
use crate::app::signed_in_user;
use crate::dataserver::{ApiKeyAccess, ApiKeyGroupTarget, Dataserver};

#[topcoat::router::query_params(error = bad_request)]
#[serde(deny_unknown_fields)]
struct SaveKeyQuery {
    key: Option<ApiKeyValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveKey {
    key: Option<ApiKeyValue>,
    name: String,
    access: ApiKeyAccess,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedKey {
    key: String,
    name: String,
    access: ApiKeyAccess,
    created: bool,
}

#[route(POST)]
pub(in crate::app::settings) async fn save_key(
    cx: &Cx,
    Json(input): Json<SaveKey>,
) -> Result<Json<SavedKey>> {
    let user = signed_in_user(cx).await?;
    let query = query_params::<SaveKeyQuery>(cx)?;
    let query_key = query.key.as_ref();
    let name = input.name.trim();
    if name.is_empty() {
        return Err(bad_request("key name is required").into());
    }
    if name.chars().count() > 255 {
        return Err(bad_request("key name cannot exceed 255 characters").into());
    }
    if input.access.user.library == false
        && (input.access.user.notes || input.access.user.files || input.access.user.write)
    {
        return Err(bad_request("personal-library permissions require library access").into());
    }
    if input
        .access
        .groups
        .values()
        .any(|access| access.write && !access.library)
    {
        return Err(bad_request("group write permission requires library access").into());
    }

    let dataserver = app_context::<Dataserver>(cx);
    let groups = dataserver.groups_for_user(user.user_id).await?;
    for target in input.access.groups.keys() {
        if let ApiKeyGroupTarget::Group(group_id) = target
            && !groups.iter().any(|group| group.id == *group_id)
        {
            return Err(
                bad_request("permissions include a group the user does not belong to").into(),
            );
        }
    }

    let (key, created) = match (query_key, input.key.as_ref()) {
        (None, None) => (
            dataserver
                .create_access_key(user.user_id, name, &input.access)
                .await?,
            true,
        ),
        (Some(query_key), Some(body_key)) if query_key == body_key => (
            dataserver
                .update_access_key(user.user_id, query_key.as_str(), name, &input.access)
                .await?
                .ok_or_else(not_found)?,
            false,
        ),
        (Some(_), Some(_)) => {
            return Err(bad_request("query and request-body API keys do not match").into());
        }
        _ => return Err(bad_request("an API key is required only when editing a key").into()),
    };
    tracing::info!(
        user_id = %user.user_id,
        created,
        key_name = %key.name,
        "API key saved",
    );

    Ok(Json(SavedKey {
        key: key.key,
        name: key.name,
        access: key.access,
        created,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataserver::ApiKeyGroupAccess;

    #[test]
    fn official_save_key_payload_deserializes_without_numeric_coercion() {
        let input: SaveKey = serde_json::from_value(serde_json::json!({
            "key": "Abcdefghijklmnopqrstuv12",
            "name": "Citation exporter",
            "access": {
                "user": {
                    "library": true,
                    "notes": true,
                    "files": true,
                    "write": true
                },
                "groups": {
                    "all": {"library": true, "write": false},
                    "42": {"library": true, "write": true}
                }
            }
        }))
        .expect("official save-key payload must deserialize");

        assert_eq!(
            input.key.as_ref().map(ApiKeyValue::as_str),
            Some("Abcdefghijklmnopqrstuv12")
        );
        assert!(input.access.user.files);
        assert_eq!(
            input.access.groups.get(&ApiKeyGroupTarget::All).copied(),
            Some(ApiKeyGroupAccess {
                library: true,
                write: false,
            })
        );
        assert_eq!(
            input
                .access
                .groups
                .get(&ApiKeyGroupTarget::Group(42))
                .copied(),
            Some(ApiKeyGroupAccess {
                library: true,
                write: true,
            })
        );
        assert!(
            serde_json::from_value::<SaveKey>(serde_json::json!({
                "key": 42.5,
                "name": "Invalid",
                "access": {"user": {}, "groups": {}}
            }))
            .is_err()
        );
    }
}
