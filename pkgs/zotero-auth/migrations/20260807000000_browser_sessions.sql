CREATE TABLE `oidc_browser_sessions` (
  `tokenHash` binary(32) NOT NULL,
  `userID` int unsigned NOT NULL,
  `expiresAt` timestamp(6) NOT NULL,
  PRIMARY KEY (`tokenHash`),
  KEY `expiresAt` (`expiresAt`),
  CONSTRAINT `oidc_browser_sessions_ibfk_1`
    FOREIGN KEY (`userID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
