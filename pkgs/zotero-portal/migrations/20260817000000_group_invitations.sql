CREATE TABLE `group_invitations` (
  `groupID` bigint unsigned NOT NULL,
  `inviterUserID` int unsigned NOT NULL,
  `inviteeUserID` int unsigned NOT NULL,
  `invitedAt` timestamp(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`groupID`, `inviteeUserID`),
  KEY `inviteeUserID` (`inviteeUserID`),
  CONSTRAINT `group_invitations_inviter_ibfk`
    FOREIGN KEY (`inviterUserID`) REFERENCES `users` (`userID`) ON DELETE CASCADE,
  CONSTRAINT `group_invitations_invitee_ibfk`
    FOREIGN KEY (`inviteeUserID`) REFERENCES `users` (`userID`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
