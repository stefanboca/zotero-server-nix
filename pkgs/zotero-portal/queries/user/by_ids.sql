SELECT
    U.userID AS `user_id: UserId`,
    U.username,
    U.email,
    UM.metaValue AS `real_name?`
FROM JSON_TABLE(
    ?, '$[*]' COLUMNS (userID INT UNSIGNED PATH '$')
) AS IDs
    INNER JOIN users AS U ON IDs.userID = U.userID
    LEFT JOIN users_meta AS UM
        ON U.userID = UM.userID AND UM.metaKey = 'profile_realname'
ORDER BY U.username
