SELECT userID AS `user_id: UserId`
FROM browser_sessions
WHERE tokenHash = ?
FOR UPDATE
