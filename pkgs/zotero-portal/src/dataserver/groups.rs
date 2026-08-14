use super::*;
impl GroupRole {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    pub fn can_manage(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Owner => "Group owner",
            Self::Admin => "Administrator",
            Self::Member => "Member",
        }
    }

    pub fn user_summary(self) -> &'static str {
        match self {
            Self::Owner => "You are the owner",
            Self::Admin => "You are an administrator",
            Self::Member => "You are a member",
        }
    }

    pub fn eq(&self, other: &Self) -> BoolSurrogate {
        (*self == *other).into_surrogate()
    }

    pub fn ne(&self, other: &Self) -> BoolSurrogate {
        (*self != *other).into_surrogate()
    }
}

impl std::fmt::Display for GroupRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.wire_name())
    }
}

impl std::str::FromStr for GroupRole {
    type Err = &'static str;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            _ => Err("unknown group role"),
        }
    }
}

impl TryFrom<String> for GroupRole {
    type Error = &'static str;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl Serialize for GroupRole {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TaggedRole<'a> {
            t: &'static str,
            v: &'a str,
        }

        TaggedRole {
            t: "str",
            v: self.wire_name(),
        }
        .serialize(serializer)
    }
}

impl Surrogated for GroupRole {
    type Surrogate = Self;

    fn into_surrogate(self) -> Self::Surrogate {
        self
    }
}

impl Surrogate for GroupRole {
    type Real = GroupRole;

    fn into_real(self) -> Self::Real {
        self
    }
}

impl Surrogated for &GroupRole {
    type Surrogate = Self;

    fn into_surrogate(self) -> Self::Surrogate {
        self
    }
}

impl Surrogate for &GroupRole {
    type Real = Self;

    fn into_real(self) -> Self::Real {
        self
    }
}
impl Group {
    pub fn role(&self, user_id: UserId) -> Option<GroupRole> {
        if self.owner == user_id {
            Some(GroupRole::Owner)
        } else if self.admins.contains(&user_id) {
            Some(GroupRole::Admin)
        } else if self.members.contains(&user_id) {
            Some(GroupRole::Member)
        } else {
            None
        }
    }

    pub fn user_ids(&self) -> Vec<UserId> {
        let mut ids = Vec::with_capacity(1 + self.admins.len() + self.members.len());
        for user_id in std::iter::once(&self.owner)
            .chain(&self.admins)
            .chain(&self.members)
        {
            if !ids.contains(user_id) {
                ids.push(*user_id);
            }
        }
        ids
    }
}
impl From<GroupEnvelope> for Group {
    fn from(value: GroupEnvelope) -> Self {
        Self {
            id: value.id,
            name: value.data.name,
            owner: value.data.owner,
            admins: value.data.admins,
            members: value.data.members,
            group_type: value.data.group_type,
            library_editing: value.data.library_editing,
            library_reading: value.data.library_reading,
            file_editing: value.data.file_editing,
            description: value.data.description,
            url: value.data.url,
            has_image: value.data.has_image != 0,
        }
    }
}

impl Dataserver {
    pub async fn groups_for_user(&self, user_id: UserId) -> anyhow::Result<Vec<Group>> {
        let response = self
            .super_request(reqwest::Method::GET, &format!("/users/{user_id}/groups"))
            .query(&[("limit", "100")])
            .send()
            .await
            .context("listing Zotero groups")?
            .error_for_status()
            .context("Dataserver rejected group listing")?;
        let groups: Vec<GroupEnvelope> = response
            .json()
            .await
            .context("decoding Zotero group listing")?;
        Ok(groups.into_iter().map(Into::into).collect())
    }

    pub async fn group(&self, group_id: u64) -> anyhow::Result<Option<Group>> {
        let response = self
            .super_request(reqwest::Method::GET, &format!("/groups/{group_id}"))
            .send()
            .await
            .context("loading Zotero group")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response
            .error_for_status()
            .context("Dataserver rejected group lookup")?;
        let group: GroupEnvelope = response.json().await.context("decoding Zotero group")?;
        Ok(Some(group.into()))
    }

    pub async fn create_private_group(&self, user_id: UserId, name: &str) -> anyhow::Result<u64> {
        let name = xml_escape(name);
        let body = format!(
            r#"<group owner="{user_id}" name="{name}" type="Private" libraryEditing="members" libraryReading="members" fileEditing="members" hasImage="0"><description></description><url></url></group>"#
        );
        let response = self
            .super_request(reqwest::Method::POST, "/groups")
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .context("creating Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group creation")?;
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .context("Dataserver omitted created group location")?
            .to_str()
            .context("Dataserver returned an invalid group location")?;
        location
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .context("Dataserver returned an empty group location")?
            .parse()
            .context("Dataserver returned a group location without a numeric ID")
    }

    pub async fn set_group_user(
        &self,
        group_id: u64,
        user_id: UserId,
        role: GroupRole,
    ) -> anyhow::Result<()> {
        let body = format!(r#"<user id="{user_id}" role="{role}"/>"#);
        self.super_request(
            reqwest::Method::PUT,
            &format!("/groups/{group_id}/users/{user_id}"),
        )
        .header("Content-Type", "text/xml")
        .body(body)
        .send()
        .await
        .context("updating Zotero group member")?
        .error_for_status()
        .context("Dataserver rejected group member update")?;
        Ok(())
    }

    pub async fn rename_group(&self, group: &Group, name: &str) -> anyhow::Result<()> {
        let body = format!(
            r#"<group id="{}" owner="{}" name="{}" type="{}" libraryEditing="{}" libraryReading="{}" fileEditing="{}" hasImage="{}"><description>{}</description><url>{}</url></group>"#,
            group.id,
            group.owner,
            xml_escape(name),
            xml_escape(&group.group_type),
            xml_escape(&group.library_editing),
            xml_escape(&group.library_reading),
            xml_escape(&group.file_editing),
            u8::from(group.has_image),
            xml_escape(&group.description),
            xml_escape(&group.url),
        );
        self.super_request(reqwest::Method::PUT, &format!("/groups/{}", group.id))
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .context("renaming Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group rename")?;
        Ok(())
    }

    pub async fn remove_group_user(&self, group_id: u64, user_id: UserId) -> anyhow::Result<()> {
        self.super_request(
            reqwest::Method::DELETE,
            &format!("/groups/{group_id}/users/{user_id}"),
        )
        .send()
        .await
        .context("removing Zotero group member")?
        .error_for_status()
        .context("Dataserver rejected group member removal")?;
        Ok(())
    }

    pub async fn delete_group(&self, group_id: u64) -> anyhow::Result<()> {
        self.super_request(reqwest::Method::DELETE, &format!("/groups/{group_id}"))
            .send()
            .await
            .context("deleting Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group deletion")?;
        Ok(())
    }
}
pub(super) fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
