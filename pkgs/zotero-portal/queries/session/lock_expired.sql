SELECT
    userID AS `user_id: UserId`,
    LOWER(HEX(tokenHash)) AS `handle!`
FROM browser_sessions
WHERE expiresAt <= CURRENT_TIMESTAMP(6)
FOR UPDATE
