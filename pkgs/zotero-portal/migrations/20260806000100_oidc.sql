CREATE TABLE `oidc_identities` (
  `issuer` varchar(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  `subject` varchar(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  `userID` int unsigned DEFAULT NULL,
  PRIMARY KEY (`issuer`, `subject`),
  UNIQUE KEY `userID` (`userID`),
  CONSTRAINT `oidc_identities_ibfk_1`
    FOREIGN KEY (`userID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `browser_sessions` (
  `tokenHash` binary(32) NOT NULL,
  `userID` int unsigned NOT NULL,
  `expiresAt` timestamp(6) NOT NULL,
  `createdAt` timestamp(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `lastSeenAt` timestamp(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`tokenHash`),
  KEY `expiresAt` (`expiresAt`),
  CONSTRAINT `browser_sessions_ibfk_1`
    FOREIGN KEY (`userID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
