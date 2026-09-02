SELECT userID
FROM browser_sessions
WHERE userID = ? AND tokenHash = UNHEX(?)
FOR UPDATE
