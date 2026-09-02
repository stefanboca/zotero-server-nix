SELECT userID AS `user_id: UserId`
FROM users
WHERE userID = ?
FOR UPDATE
