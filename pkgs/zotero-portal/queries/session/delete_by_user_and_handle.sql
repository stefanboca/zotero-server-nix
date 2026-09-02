DELETE FROM browser_sessions
WHERE userID = ? AND tokenHash = UNHEX(?)
