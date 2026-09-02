SELECT
    U.userID AS `user_id: UserId`,
    U.username,
    U.email,
    UM.metaValue AS `real_name?`
FROM users AS U
    LEFT JOIN users_meta AS UM
        ON U.userID = UM.userID AND UM.metaKey = 'profile_realname'
    LEFT JOIN GDN_User AS GU ON U.userID = GU.UserID
WHERE
    (U.username = ? OR U.email = ?)
    AND U.role <> 'deleted'
    AND COALESCE(GU.Banned, 0) = 0
    AND EXISTS (
        SELECT 1
        FROM oidc_identities AS OI
        WHERE OI.userID = U.userID
    )
