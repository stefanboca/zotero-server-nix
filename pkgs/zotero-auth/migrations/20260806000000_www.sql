-- Compatibility schema for the website database queried by Zotero Dataserver.
-- This is not a copy of Zotero's website schema. It contains only the tables,
-- columns, and indexes used by the pinned Dataserver source.

CREATE TABLE `users` (
  `userID` int unsigned NOT NULL AUTO_INCREMENT,
  `username` varchar(255) NOT NULL,
  `password` varchar(255) NOT NULL,
  `email` varchar(255) NOT NULL,
  `role` varchar(32) NOT NULL DEFAULT 'member',
  PRIMARY KEY (`userID`),
  UNIQUE KEY `username` (`username`),
  UNIQUE KEY `email` (`email`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `users_email` (
  `userID` int unsigned NOT NULL,
  `email` varchar(255) NOT NULL,
  `validated` tinyint unsigned NOT NULL DEFAULT 0,
  `dateAdded` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (`userID`, `email`),
  UNIQUE KEY `email` (`email`),
  CONSTRAINT `users_email_ibfk_1`
    FOREIGN KEY (`userID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `users_meta` (
  `userID` int unsigned NOT NULL,
  `metaKey` varchar(255) NOT NULL,
  `metaValue` text NOT NULL,
  PRIMARY KEY (`userID`, `metaKey`),
  CONSTRAINT `users_meta_ibfk_1`
    FOREIGN KEY (`userID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `sessions` (
  `id` varchar(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  `userID` int unsigned NOT NULL,
  `modified` bigint unsigned NOT NULL,
  `lifetime` int unsigned NOT NULL,
  PRIMARY KEY (`id`),
  KEY `userID` (`userID`),
  CONSTRAINT `sessions_ibfk_1`
    FOREIGN KEY (`userID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `GDN_User` (
  `UserID` int unsigned NOT NULL,
  `Banned` tinyint unsigned NOT NULL DEFAULT 0,
  PRIMARY KEY (`UserID`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `storage_institutions` (
  `institutionID` int unsigned NOT NULL AUTO_INCREMENT,
  `storageQuota` bigint unsigned NOT NULL,
  `domain` varchar(255) NOT NULL DEFAULT '',
  `domainBlacklist` varchar(255) NOT NULL DEFAULT '',
  PRIMARY KEY (`institutionID`),
  KEY `domain` (`domain`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `storage_institution_email` (
  `institutionID` int unsigned NOT NULL,
  `email` varchar(255) NOT NULL,
  PRIMARY KEY (`institutionID`, `email`),
  KEY `email` (`email`),
  CONSTRAINT `storage_institution_email_ibfk_1`
    FOREIGN KEY (`institutionID`)
    REFERENCES `storage_institutions` (`institutionID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
