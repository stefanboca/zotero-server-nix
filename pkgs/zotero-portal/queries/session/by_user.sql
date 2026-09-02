SELECT
    userAgent AS `user_agent?`,
    LOWER(HEX(tokenHash)) AS `handle!`,
    DATE_FORMAT(lastSeenAt, '%Y-%m-%dT%H:%i:%s.%fZ') AS `last_seen_at!`,
    DATE_FORMAT(expiresAt, '%Y-%m-%dT%H:%i:%s.%fZ') AS `expires_at!`
FROM browser_sessions
WHERE userID = ?
ORDER BY lastSeenAt DESC
