use super::*;

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
        Ok(StorageUsage {
            total_mb: xml_number(&body, "total")
                .context("Dataserver storage response omitted total usage")?,
            library_mb: xml_number(&body, "library")
                .context("Dataserver storage response omitted library usage")?,
        })
    }
}

pub(super) fn xml_number(value: &str, element: &str) -> Option<f64> {
    let opening = format!("<{element}>");
    let closing = format!("</{element}>");
    let start = value.find(&opening)? + opening.len();
    let end = value[start..].find(&closing)? + start;
    value[start..end].trim().parse().ok()
}
