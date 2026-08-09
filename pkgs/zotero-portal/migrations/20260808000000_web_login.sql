ALTER TABLE `oidc_flows`
  MODIFY `sessionToken` char(32) CHARACTER SET ascii COLLATE ascii_bin NULL,
  ADD COLUMN `returnPath` varchar(2048) NOT NULL DEFAULT '/' AFTER `sessionToken`;
