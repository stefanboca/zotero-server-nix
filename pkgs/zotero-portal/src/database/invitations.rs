use super::*;

struct GroupInvitationRow {
    group_id: u64,
    invitee_user_id: UserId,
    username: String,
    email: String,
    real_name: Option<String>,
    invited_at: String,
}

impl Database {
    pub async fn create_group_invitation(
        &self,
        group_id: u64,
        inviter_id: UserId,
        invitee_id: UserId,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query_file!(
            "queries/invitation/insert.sql",
            group_id,
            inviter_id,
            invitee_id,
        )
        .execute(&self.db)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn group_invitation(
        &self,
        group_id: u64,
        invitee_id: UserId,
    ) -> anyhow::Result<Option<GroupInvitation>> {
        let row = sqlx::query_file_as!(
            GroupInvitationRow,
            "queries/invitation/by_group_and_user.sql",
            group_id,
            invitee_id,
        )
        .fetch_optional(&self.db)
        .await?;
        row.map(decode_group_invitation).transpose()
    }

    pub async fn group_invitations(&self, group_id: u64) -> anyhow::Result<Vec<GroupInvitation>> {
        let rows = sqlx::query_file_as!(
            GroupInvitationRow,
            "queries/invitation/by_group.sql",
            group_id,
        )
        .fetch_all(&self.db)
        .await?;
        rows.into_iter().map(decode_group_invitation).collect()
    }

    pub async fn group_invitations_for_user(
        &self,
        invitee_id: UserId,
    ) -> anyhow::Result<Vec<GroupInvitation>> {
        let rows = sqlx::query_file_as!(
            GroupInvitationRow,
            "queries/invitation/by_user.sql",
            invitee_id,
        )
        .fetch_all(&self.db)
        .await?;
        rows.into_iter().map(decode_group_invitation).collect()
    }

    pub async fn delete_group_invitation(
        &self,
        group_id: u64,
        invitee_id: UserId,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query_file!("queries/invitation/delete.sql", group_id, invitee_id,)
            .execute(&self.db)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn delete_group_invitations(&self, group_id: u64) -> anyhow::Result<()> {
        sqlx::query_file!("queries/invitation/delete_for_group.sql", group_id,)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}

fn decode_group_invitation(row: GroupInvitationRow) -> anyhow::Result<GroupInvitation> {
    Ok(GroupInvitation {
        group_id: row.group_id,
        invitee: UserSummary {
            user_id: row.invitee_user_id,
            username: row.username,
            email: row.email,
            real_name: row.real_name,
        },
        invited_at: row.invited_at.parse()?,
    })
}
