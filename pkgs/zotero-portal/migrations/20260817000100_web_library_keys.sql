ALTER TABLE `browser_sessions`
  ADD COLUMN `webLibraryKey` char(24) CHARACTER SET ascii COLLATE ascii_bin DEFAULT NULL,
  ADD UNIQUE KEY `webLibraryKey` (`webLibraryKey`);
