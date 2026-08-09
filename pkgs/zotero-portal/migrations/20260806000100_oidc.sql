CREATE TABLE `oidc_identities` (
  `issuer` varchar(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  `subject` varchar(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  `userID` int unsigned DEFAULT NULL,
  PRIMARY KEY (`issuer`, `subject`),
  UNIQUE KEY `userID` (`userID`),
  CONSTRAINT `oidc_identities_ibfk_1`
    FOREIGN KEY (`userID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `oidc_flows` (
  `tokenHash` binary(32) NOT NULL,
  `state` varchar(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  `nonce` varchar(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  `pkceVerifier` varchar(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  `sessionToken` char(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  `expires` bigint unsigned NOT NULL,
  `consumed` tinyint unsigned NOT NULL DEFAULT 0,
  PRIMARY KEY (`tokenHash`),
  UNIQUE KEY `state` (`state`),
  UNIQUE KEY `sessionToken` (`sessionToken`),
  KEY `expires` (`expires`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
