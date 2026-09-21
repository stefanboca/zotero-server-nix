use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::{href, page};
use topcoat::runtime::{shard, signal};
use topcoat::view::{View, component, view};

use super::managed_group_settings;
use crate::app::groups::group_id::GroupId;
use crate::app::section;
use crate::database::Database;
use crate::dataserver::{Dataserver, GroupRole};

#[page]
pub(in crate::app::groups) async fn members(cx: &Cx) -> Result<impl View> {
    let context = managed_group_settings(cx).await?;
    let group_id = context.group.id;

    let members_refresh = signal(cx, || false);

    Ok(view! {
        <div class="space-y-6">
            <input
                type="checkbox"
                hidden=""
                data-members-refresh=""
                @change=$(|event: topcoat::runtime::Event| {
                    members_refresh.set(event.target.checked)
                })
            >
            section(members_shard(refresh: $(members_refresh.get())))
            member_invitations(group_id: group_id)
        </div>

        <script
            type="module"
            src=(topcoat::asset::asset!("src/app/groups/group_id/group_actions.js"))
        ></script>
    })
}

#[shard]
async fn members_shard(cx: &Cx, refresh: bool) -> Result<impl View> {
    let _ = refresh;
    let context = managed_group_settings(cx).await?;
    let group = context.group.clone();
    let mut users = app_context::<Database>(cx)
        .users_by_ids(&group.user_ids())
        .await?;
    let joined_by_user = app_context::<Dataserver>(cx)
        .group_memberships(group.id)
        .await?
        .into_iter()
        .map(|membership| (membership.user_id, membership.joined))
        .collect::<std::collections::BTreeMap<_, _>>();
    users.sort_by_key(|member| member.user_id != context.actor_id);
    let group_overview = href!(
        super::super::group_slug::group_overview,
        GroupId(group.id),
        super::super::group_slug::GroupSlug(group.slug()),
    )
    .resolve(cx);
    let groups = href!(crate::app::groups::list_groups).resolve(cx);

    Ok(view! {
        <div data-members-region="">
            <h2 class="portal-section-heading">"Current Members"</h2>
            <form
                action=(href!(super::super::members::create_member, GroupId(group.id)))
                data-member-roles=""
                data-redirect=(group_overview.as_str())
                data-removed-redirect=(groups.as_str())
            >
                <div class="portal-table-scroll">
                    <table class="portal-compact-table">
                        <thead>
                            <tr>
                                <th>"Username"</th>
                                <th>"Full Name"</th>
                                <th>"Member Since"</th>
                                <th>"Role"</th>
                            </tr>
                        </thead>
                        <tbody>
                            for member in users {
                                let member_role = group
                                    .role(member.user_id)
                                    .unwrap_or(GroupRole::Member);
                                <tr>
                                    <td class="font-mono font-semibold">
                                        <a
                                            class="portal-link"
                                            href=(href!(
                                                crate::app::profile::profile,
                                                crate::app::profile::ProfileUsername(
                                                    member.username.clone(),
                                                ),
                                            ))
                                        >
                                            (member.username.as_str())
                                        </a>
                                    </td>
                                    <td>(member.display_name())</td>
                                    <td class="whitespace-nowrap">
                                        if let Some(joined) = joined_by_user.get(&member.user_id) {
                                            <time datetime=(joined.to_string())>
                                                (joined.strftime("%Y-%m-%d %H:%M:%S").to_string())
                                            </time>
                                        } else {
                                            "--"
                                        }
                                    </td>
                                    <td>
                                        if member_role != GroupRole::Owner {
                                            <label>
                                                <span class="sr-only">
                                                    (format!("Role for {}", member.display_name()))
                                                </span>
                                                <select
                                                    class="portal-form-control w-full text-sm sm:w-auto"
                                                    data-member-role=""
                                                    data-user-id=(member.user_id.to_string())
                                                    data-current-role=(member_role.to_string())
                                                    data-self=(if member.user_id == context.actor_id {
                                                        "true"
                                                    } else {
                                                        "false"
                                                    })
                                                >
                                                    <option
                                                        value="admin"
                                                        selected=(member_role == GroupRole::Admin)
                                                    >
                                                        "Admin"
                                                    </option>
                                                    <option
                                                        value="member"
                                                        selected=(member_role == GroupRole::Member)
                                                    >
                                                        "Member"
                                                    </option>
                                                    <option value="remove">"Remove"</option>
                                                </select>
                                            </label>
                                        } else {
                                            (member_role.display_name())
                                        }
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
                <div class="mt-6 flex flex-wrap items-center gap-3">
                    <button
                        class="portal-button portal-button-primary disabled:cursor-not-allowed"
                        type="submit"
                    >
                        "Update Roles"
                    </button>
                    <p
                        class="text-sm text-stone-600"
                        hidden="hidden"
                        role="status"
                        data-role-feedback=""
                    ></p>
                </div>
            </form>
        </div>
    })
}

#[component]
async fn member_invitations(cx: &Cx, group_id: u64) -> Result<impl View> {
    let invitations = app_context::<Database>(cx)
        .group_invitations(group_id)
        .await?;
    Ok(view! {
        section(
            <h2 class="portal-section-heading">"Member Invitations"</h2>
            if !invitations.is_empty() {
                <div class="portal-table-scroll">
                    <table class="portal-compact-table">
                        <thead>
                            <tr>
                                <th>"Username"</th>
                                <th>"Full Name"</th>
                                <th>"Invited On"</th>
                                <th>"Cancel Invitation"</th>
                            </tr>
                        </thead>
                        <tbody>
                            for invitation in invitations {
                                let date = invitation
                                    .invited_at
                                    .strftime("%Y-%m-%d %H:%M:%S")
                                    .to_string();
                                <tr>
                                    <td class="font-mono font-semibold">
                                        <a
                                            class="portal-link"
                                            href=(href!(
                                                crate::app::profile::profile,
                                                crate::app::profile::ProfileUsername(
                                                    invitation.invitee.username.clone(),
                                                ),
                                            ))
                                        >
                                            (invitation.invitee.username.as_str())
                                        </a>
                                    </td>
                                    <td>(invitation.invitee.display_name())</td>
                                    <td class="whitespace-nowrap">
                                        <time datetime=(invitation.invited_at.to_string())>
                                            (date.as_str())
                                        </time>
                                    </td>
                                    <td>
                                        <form
                                            method="post"
                                            action=(href!(
                                                super::super::cancel_invitation,
                                                GroupId(group_id),
                                                super::super::InviteeUserId(invitation.invitee.user_id),
                                            ))
                                        >
                                            <button class="portal-link-danger" type="submit">
                                                "Cancel"
                                            </button>
                                        </form>
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
            } else {
                <p class="mt-2 text-stone-600">"No pending invitations."</p>
            }
            <a
                class="portal-link mt-4 inline-block"
                href=(href!(super::super::invite_members, GroupId(group_id)))
            >
                "Send More Invitations"
            </a>
        )
    })
}
