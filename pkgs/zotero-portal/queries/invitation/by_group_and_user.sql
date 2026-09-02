SELECT
    GI.groupID AS `group_id!: u64`,
    GI.inviteeUserID AS `invitee_user_id: UserId`,
    U.username,
    U.email,
    UM.metaValue AS `real_name?`,
    DATE_FORMAT(GI.invitedAt, '%Y-%m-%dT%H:%i:%s.%fZ') AS `invited_at!`
FROM group_invitations AS GI
    INNER JOIN users AS U ON GI.inviteeUserID = U.userID
    LEFT JOIN users_meta AS UM
        ON U.userID = UM.userID AND UM.metaKey = 'profile_realname'
WHERE GI.groupID = ? AND GI.inviteeUserID = ?
