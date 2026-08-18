use super::*;

#[derive(Serialize)]
#[serde(rename = "group")]
struct GroupXml<'a> {
    #[serde(rename = "@id", skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    #[serde(rename = "@owner")]
    owner: UserId,
    #[serde(rename = "@name")]
    name: &'a str,
    #[serde(rename = "@type")]
    group_type: &'a str,
    #[serde(rename = "@libraryEditing")]
    library_editing: &'a str,
    #[serde(rename = "@libraryReading")]
    library_reading: &'a str,
    #[serde(rename = "@fileEditing")]
    file_editing: &'a str,
    #[serde(rename = "@hasImage")]
    has_image: u8,
    description: &'a str,
    url: &'a str,
}

#[derive(Serialize)]
#[serde(rename = "user")]
struct GroupUserXml {
    #[serde(rename = "@id")]
    id: UserId,
    #[serde(rename = "@role")]
    role: GroupRole,
}

#[derive(Deserialize)]
struct GroupUsersFeed {
    #[serde(default, rename = "entry")]
    entries: Vec<GroupUserEntry>,
}

#[derive(Deserialize)]
struct GroupUserEntry {
    published: String,
    content: GroupUserContent,
}

#[derive(Deserialize)]
struct GroupUserContent {
    user: GroupUserData,
}

#[derive(Deserialize)]
struct GroupUserData {
    #[serde(rename = "@id")]
    id: UserId,
}

#[derive(Deserialize)]
struct GroupItemEnvelope {
    key: String,
    data: GroupItemData,
    meta: GroupItemMeta,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupItemData {
    item_type: String,
    #[serde(default)]
    content_type: String,
    #[serde(default)]
    link_mode: String,
    #[serde(default)]
    title: String,
    date_modified: jiff::Timestamp,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupItemMeta {
    #[serde(default)]
    created_by_user: Option<GroupItemUser>,
}

#[derive(Deserialize)]
struct GroupItemUser {
    #[serde(default)]
    name: String,
    username: String,
}

// Mirrors Web Library's item icon selection so the portal uses the same
// symbols as the library instead of maintaining a second visual vocabulary.
fn group_item_icon(item: &GroupItemData) -> &'static str {
    if item.item_type == "attachment" {
        let is_file = matches!(
            item.link_mode.as_str(),
            "linked_file" | "imported_file" | "imported_url"
        );
        if item.content_type == "application/pdf" && is_file {
            return if item.link_mode == "linked_file" {
                "attachment-pdf-link"
            } else {
                "attachment-pdf"
            };
        }
        if item.content_type == "application/epub+zip" && is_file {
            return if item.link_mode == "linked_file" {
                "attachment-epub-link"
            } else {
                "attachment-epub"
            };
        }
        if item.content_type.starts_with("image/") && is_file {
            return if item.link_mode == "linked_file" {
                "attachment-image-link"
            } else {
                "attachment-image"
            };
        }
        if item.content_type.starts_with("video/") && is_file {
            return if item.link_mode == "linked_file" {
                "attachment-video-link"
            } else {
                "attachment-video"
            };
        }
        return match item.link_mode.as_str() {
            "linked_url" => "attachment-web-link",
            "imported_url" => "attachment-snapshot",
            "linked_file" => "attachment-link",
            _ => "document",
        };
    }
    match item.item_type.as_str() {
        "artwork" => "artwork",
        "audioRecording" => "audio-recording",
        "bill" => "bill",
        "blogPost" => "blog-post",
        "book" => "book",
        "bookSection" => "book-section",
        "case" => "case",
        "computerProgram" => "computer-program",
        "conferencePaper" => "conference-paper",
        "dataset" => "dataset",
        "dictionaryEntry" => "dictionary-entry",
        "document" => "document",
        "email" => "email",
        "encyclopediaArticle" => "encyclopedia-article",
        "film" => "film",
        "forumPost" => "forum-post",
        "hearing" => "hearing",
        "instantMessage" => "instant-message",
        "interview" => "interview",
        "journalArticle" => "journal-article",
        "letter" => "letter",
        "magazineArticle" => "magazine-article",
        "manuscript" => "manuscript",
        "map" => "map",
        "newspaperArticle" => "newspaper-article",
        "note" => "note",
        "patent" => "patent",
        "podcast" => "podcast",
        "preprint" => "preprint",
        "presentation" => "presentation",
        "radioBroadcast" => "radio-broadcast",
        "report" => "report",
        "standard" => "standard",
        "statute" => "statute",
        "thesis" => "thesis",
        "tvBroadcast" => "tv-broadcast",
        "videoRecording" => "video-recording",
        "webpage" => "webpage",
        _ => "document",
    }
}

fn total_results(response: &reqwest::Response) -> anyhow::Result<u64> {
    response
        .headers()
        .get("Total-Results")
        .context("Dataserver omitted Total-Results from a multi-object response")?
        .to_str()
        .context("Dataserver returned an invalid Total-Results header")?
        .parse::<u64>()
        .context("Dataserver returned a nonnumeric Total-Results header")
}

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
            Self::Owner => "Owner",
            Self::Admin => "Administrator",
            Self::Member => "Member",
        }
    }
}

impl std::fmt::Display for GroupRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.wire_name())
    }
}

impl Group {
    pub fn slug(&self) -> String {
        group_slug(&self.name)
    }

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

pub(crate) fn group_slug(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| {
            character.is_alphanumeric()
                || character.is_whitespace()
                || matches!(character, '.' | '_' | '-')
        })
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_whitespace() {
                '_'
            } else {
                character
            }
        })
        .collect()
}
impl From<GroupEnvelope> for Group {
    fn from(value: GroupEnvelope) -> Self {
        Self {
            id: value.id,
            name: value.data.name,
            created: value.meta.created,
            owner: value.data.owner,
            admins: value.data.admins,
            members: value.data.members,
            library_editing: value.data.library_editing,
            file_editing: value.data.file_editing,
            description: value.data.description,
            url: value.data.url,
            has_image: value.data.has_image != 0,
        }
    }
}

impl Dataserver {
    pub async fn group_memberships(&self, group_id: u64) -> anyhow::Result<Vec<GroupMembership>> {
        let response = self
            .super_request(reqwest::Method::GET, &format!("/groups/{group_id}/users"))
            .send()
            .await
            .context("listing Zotero group members")?
            .error_for_status()
            .context("Dataserver rejected group member listing")?;
        let body = response
            .text()
            .await
            .context("reading Zotero group member listing")?;
        decode_group_memberships(&body)
    }

    pub async fn groups_for_user(&self, user_id: UserId) -> anyhow::Result<Vec<Group>> {
        let mut groups = Vec::new();
        loop {
            let start = groups.len().to_string();
            let response = self
                .super_request(reqwest::Method::GET, &format!("/users/{user_id}/groups"))
                .query(&[("limit", "100"), ("start", start.as_str())])
                .send()
                .await
                .context("listing Zotero groups")?
                .error_for_status()
                .context("Dataserver rejected group listing")?;
            let total = total_results(&response)?;
            let page = response
                .json::<Vec<GroupEnvelope>>()
                .await
                .context("decoding Zotero group listing")?;
            if page.is_empty() {
                if u64::try_from(groups.len())? < total {
                    anyhow::bail!("Dataserver ended the group listing before Total-Results");
                }
                break;
            }
            groups.extend(page.into_iter().map(Into::into));
            if u64::try_from(groups.len())? >= total {
                break;
            }
        }
        Ok(groups)
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

    pub async fn recent_group_items(&self, group_id: u64) -> anyhow::Result<RecentGroupItems> {
        let response = self
            .super_request(
                reqwest::Method::GET,
                &format!("/groups/{group_id}/items/top"),
            )
            .query(&[
                ("format", "json"),
                ("limit", "10"),
                ("sort", "dateModified"),
                ("direction", "desc"),
            ])
            .send()
            .await
            .context("listing recently added Zotero group items")?
            .error_for_status()
            .context("Dataserver rejected recent group item listing")?;
        let total = total_results(&response)?;
        let items = response
            .json::<Vec<GroupItemEnvelope>>()
            .await
            .context("decoding recently added Zotero group items")?;
        Ok(RecentGroupItems {
            items: items
                .into_iter()
                .map(|item| {
                    let icon_name = group_item_icon(&item.data);
                    GroupItemSummary {
                        key: item.key,
                        icon_name,
                        title: item.data.title,
                        added_by: item.meta.created_by_user.map(|user| {
                            if user.name.trim().is_empty() {
                                user.username
                            } else {
                                user.name
                            }
                        }),
                        date_modified: item.data.date_modified,
                    }
                })
                .collect(),
            total,
        })
    }

    pub async fn create_private_group(&self, user_id: UserId, name: &str) -> anyhow::Result<u64> {
        let body = quick_xml::se::to_string(&GroupXml {
            id: None,
            owner: user_id,
            name,
            group_type: "Private",
            library_editing: "members",
            library_reading: "members",
            file_editing: "members",
            has_image: 0,
            description: "",
            url: "",
        })
        .context("encoding new Zotero group XML")?;
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
        let body = quick_xml::se::to_string(&GroupUserXml { id: user_id, role })
            .context("encoding Zotero group member XML")?;
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

    pub async fn update_group(&self, group: &Group) -> anyhow::Result<()> {
        let body = quick_xml::se::to_string(&GroupXml {
            id: Some(group.id),
            owner: group.owner,
            name: &group.name,
            group_type: "Private",
            library_editing: &group.library_editing,
            library_reading: "members",
            file_editing: &group.file_editing,
            has_image: u8::from(group.has_image),
            description: &group.description,
            url: &group.url,
        })
        .context("encoding updated Zotero group XML")?;
        self.super_request(reqwest::Method::PUT, &format!("/groups/{}", group.id))
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .context("updating Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group update")?;
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

fn decode_group_memberships(body: &str) -> anyhow::Result<Vec<GroupMembership>> {
    let feed: GroupUsersFeed =
        quick_xml::de::from_str(body).context("decoding Zotero group member listing")?;
    feed.entries
        .into_iter()
        .map(|entry| {
            Ok(GroupMembership {
                user_id: entry.content.user.id,
                joined: entry
                    .published
                    .parse()
                    .context("Dataserver returned an invalid group join date")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct DecodedGroup {
        #[serde(rename = "@owner")]
        owner: UserId,
        #[serde(rename = "@name")]
        name: String,
        description: String,
        url: String,
    }

    #[test]
    fn group_slugs_match_the_web_library() {
        assert_eq!(group_slug(" Research & Notes "), "research__notes");
        assert_eq!(group_slug("Crème brûlée"), "crème_brûlée");
        assert_eq!(group_slug("Test 2"), "test_2");
        assert_eq!(group_slug("test2"), "test2");
    }

    #[test]
    fn group_user_ids_include_each_person_once() {
        let group = Group {
            id: 1,
            name: "Research".to_owned(),
            created: jiff::Timestamp::MIN,
            owner: UserId::new(1),
            admins: vec![UserId::new(1), UserId::new(2)],
            members: vec![UserId::new(1), UserId::new(2), UserId::new(3)],
            library_editing: "members".to_owned(),
            file_editing: "members".to_owned(),
            description: String::new(),
            url: String::new(),
            has_image: false,
        };

        assert_eq!(
            group.user_ids(),
            [UserId::new(1), UserId::new(2), UserId::new(3)]
        );
    }

    #[test]
    fn group_role_uses_standard_json_strings() {
        let serialized =
            serde_json::to_string(&GroupRole::Admin).expect("group role must serialize");
        assert_eq!(serialized, r#""admin""#);

        let deserialized: GroupRole =
            serde_json::from_str(r#""member""#).expect("browser role must deserialize");
        assert_eq!(deserialized, GroupRole::Member);
    }

    #[test]
    fn serializes_group_attributes_and_text_as_xml() {
        let xml = quick_xml::se::to_string(&GroupXml {
            id: None,
            owner: UserId::new(42),
            name: "Research & <notes> \"team\"'s",
            group_type: "Private",
            library_editing: "members",
            library_reading: "members",
            file_editing: "members",
            has_image: 0,
            description: "One < two & three",
            url: "https://example.test/?a=1&b=2",
        })
        .expect("serializable group");
        let decoded: DecodedGroup =
            quick_xml::de::from_str(&xml).expect("serialized group must be valid XML");

        assert_eq!(decoded.owner, UserId::new(42));
        assert_eq!(decoded.name, "Research & <notes> \"team\"'s");
        assert_eq!(decoded.description, "One < two & three");
        assert_eq!(decoded.url, "https://example.test/?a=1&b=2");
    }

    #[test]
    fn decodes_group_membership_join_dates_from_atom() {
        let memberships = decode_group_memberships(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <feed xmlns="http://www.w3.org/2005/Atom"
                      xmlns:xfer="http://zotero.org/ns/transfer">
                    <entry>
                        <published>2026-08-18T20:15:16Z</published>
                        <content type="application/xml">
                            <xfer:user id="42" role="member"/>
                        </content>
                    </entry>
                </feed>"#,
        )
        .expect("valid group member feed");

        assert_eq!(memberships.len(), 1);
        assert_eq!(memberships[0].user_id, UserId::new(42));
        assert_eq!(memberships[0].joined.to_string(), "2026-08-18T20:15:16Z");
    }
}
