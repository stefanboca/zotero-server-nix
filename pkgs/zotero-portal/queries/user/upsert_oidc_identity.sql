INSERT INTO oidc_identities (issuer, subject, userID)
VALUES (?, ?, NULL)
ON DUPLICATE KEY UPDATE subject = VALUES(subject)
