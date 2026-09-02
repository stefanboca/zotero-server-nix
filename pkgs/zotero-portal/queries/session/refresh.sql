UPDATE browser_sessions
SET
    expiresAt = FROM_UNIXTIME(?),
    lastSeenAt = CURRENT_TIMESTAMP(6),
    userAgent = COALESCE(userAgent, ?)
WHERE tokenHash = ?
