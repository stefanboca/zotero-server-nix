SELECT
    U.userID AS `user_id: UserId`,
    U.username,
    U.email,
    UM.metaValue AS `real_name?`
FROM users AS U
    LEFT JOIN users_meta AS UM
        ON U.userID = UM.userID AND UM.metaKey = 'profile_realname'
WHERE U.userID = ?
