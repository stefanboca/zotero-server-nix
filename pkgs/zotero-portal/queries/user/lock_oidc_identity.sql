SELECT
    OI.userID AS `user_id?: UserId`,
    U.role AS `role?`,
    GU.Banned AS `banned?`
FROM oidc_identities AS OI
    LEFT JOIN users AS U ON OI.userID = U.userID
    LEFT JOIN GDN_User AS GU ON OI.userID = GU.UserID
WHERE OI.issuer = ? AND OI.subject = ?
FOR UPDATE
