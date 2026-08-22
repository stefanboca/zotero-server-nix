{
  oidc-provider-mock,
  port,
}: {
  lib,
  pkgs,
  ...
}: let
  inherit (lib.meta) getExe;

  users = pkgs.writeText "zotero-development-oidc-users.yaml" ''
    - sub: alice
      email: alice@example.com
      email_verified: true
      preferred_username: alice
      name: Alice Developer
    - sub: bob
      email: bob@example.com
      email_verified: true
      preferred_username: bob
      name: Bob Editor
    - sub: carol
      email: carol@example.com
      email_verified: true
      preferred_username: carol
      name: Carol Administrator
  '';
in {
  systemd.services.oidc-provider-mock = {
    description = "Mock OpenID Connect provider";
    wantedBy = ["multi-user.target"];
    after = ["network.target"];
    serviceConfig = {
      DynamicUser = true;
      ExecStart = "${getExe oidc-provider-mock} --host 0.0.0.0 --port ${toString port} --require-nonce true --user-claims-file ${users}";
      Restart = "on-failure";
      RestartSec = 1;
    };
  };
}
