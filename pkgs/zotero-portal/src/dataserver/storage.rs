use super::*;

#[derive(Deserialize)]
struct StorageXml {
    quota: String,
    expiration: Option<i64>,
    usage: StorageUsageXml,
}

#[derive(Deserialize)]
struct StorageUsageXml {
    total: f64,
    library: f64,
    #[serde(default, rename = "group")]
    groups: Vec<StorageGroupXml>,
}

#[derive(Deserialize)]
struct StorageGroupXml {
    #[serde(rename = "@id")]
    id: std::num::NonZeroU64,
    #[serde(rename = "$text")]
    usage: f64,
}

impl Dataserver {
    pub async fn set_unlimited_storage(&self, user_id: UserId) -> anyhow::Result<()> {
        self.super_request(
            reqwest::Method::POST,
            &format!("/users/{user_id}/storageadmin"),
        )
        .form(&[("quota", "unlimited"), ("expiration", "0")])
        .send()
        .await
        .context("setting unlimited Zotero file storage")?
        .error_for_status()
        .context("Dataserver rejected unlimited file storage")?;
        Ok(())
    }

    pub async fn storage_usage(&self, user_id: UserId) -> anyhow::Result<StorageUsage> {
        let response = self
            .super_request(
                reqwest::Method::GET,
                &format!("/users/{user_id}/storageadmin"),
            )
            .send()
            .await
            .context("requesting Zotero storage usage")?
            .error_for_status()
            .context("Dataserver rejected storage usage request")?;
        let body = response
            .text()
            .await
            .context("reading Zotero storage usage response")?;
        decode_storage_usage(&body)
    }

    pub async fn purge_personal_storage(&self, user_id: UserId) -> anyhow::Result<()> {
        self.super_request(
            reqwest::Method::POST,
            &format!("/users/{user_id}/removestoragefiles"),
        )
        .send()
        .await
        .context("purging personal Zotero storage")?
        .error_for_status()
        .context("Dataserver rejected personal storage purge")?;
        Ok(())
    }
}

fn decode_storage_usage(body: &str) -> anyhow::Result<StorageUsage> {
    let storage: StorageXml =
        quick_xml::de::from_str(body).context("decoding Zotero storage usage XML")?;
    let quota = storage.quota.trim();
    let quota_mb = if quota == "unlimited" {
        None
    } else {
        let quota = quota
            .parse::<f64>()
            .context("Dataserver returned a nonnumeric storage quota")?;
        if !quota.is_finite() || quota < 0.0 {
            anyhow::bail!("Dataserver returned an invalid storage quota");
        }
        Some(quota)
    };
    let expiration = storage
        .expiration
        .filter(|expiration| *expiration != 0)
        .map(|expiration| {
            jiff::Timestamp::new(expiration, 0)
                .context("Dataserver returned an invalid storage expiration")
        })
        .transpose()?;
    Ok(StorageUsage {
        quota_mb,
        expiration,
        total_mb: storage.usage.total,
        library_mb: storage.usage.library,
        groups: storage
            .usage
            .groups
            .into_iter()
            .map(|group| StorageGroupUsage {
                group_id: group.id.get(),
                usage_mb: group.usage,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_personal_and_group_storage_usage() {
        let usage = decode_storage_usage(
            "<storage><quota>unlimited</quota><usage><total>1536.5</total><library>512</library><group id=\"42\">1024.5</group></usage></storage>",
        )
        .expect("valid storage response");

        assert_eq!(usage.total_mb, 1536.5);
        assert_eq!(usage.library_mb, 512.0);
        assert_eq!(usage.quota_mb, None);
        assert_eq!(usage.expiration, None);
        assert_eq!(usage.groups.len(), 1);
        assert_eq!(usage.groups[0].group_id, 42);
        assert_eq!(usage.groups[0].usage_mb, 1024.5);
        assert!(
            decode_storage_usage("<storage><quota>unlimited</quota><usage/></storage>").is_err()
        );
        assert!(decode_storage_usage("<storage><quota>unlimited</quota><usage><total>0</total><library>0</library><group id=\"0\">0</group></usage></storage>").is_err());

        let metered = decode_storage_usage(
            "<storage><quota>300</quota><expiration>1787011200</expiration><usage><total>0</total><library>0</library></usage></storage>",
        )
        .expect("metered storage response");
        assert_eq!(metered.quota_mb, Some(300.0));
        assert_eq!(metered.expiration.unwrap().as_second(), 1787011200);
    }
}
