DELETE FROM browser_sessions
WHERE expiresAt <= CURRENT_TIMESTAMP(6)
