ALTER TABLE `oidc_browser_sessions`
  ADD COLUMN `createdAt` timestamp(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) AFTER `expiresAt`,
  ADD COLUMN `lastSeenAt` timestamp(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) AFTER `createdAt`;
