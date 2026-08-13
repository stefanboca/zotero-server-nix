{
  address,
  clientId,
  redirectUri,
  session,
  signingKey,
  startBeforePortal ? true,
  tls ? {},
}: {lib, ...}: let
  inherit (lib.modules) mkAfter mkIf;

  instance = "development";
  configDirectory = "/etc/authelia-${instance}";
  stateDirectory = "/var/lib/authelia-${instance}";
in {
  services.authelia.instances.${instance} = {
    enable = true;
    secrets = {
      jwtSecretFile = "${configDirectory}/jwt-secret";
      oidcHmacSecretFile = "${configDirectory}/oidc-hmac-secret";
      oidcIssuerPrivateKeyFile = "${configDirectory}/oidc-private-key.pem";
      storageEncryptionKeyFile = "${configDirectory}/storage-encryption-key";
    };
    settings = {
      server =
        {
          inherit address;
        }
        // tls;
      log = {
        format = "text";
        level = "info";
      };
      authentication_backend.file.path = "${configDirectory}/users.yml";
      access_control.default_policy = "one_factor";
      inherit session;
      storage.local.path = "${stateDirectory}/db.sqlite3";
      notifier.filesystem.filename = "${stateDirectory}/notifications.txt";
      identity_providers.oidc.clients = [
        {
          client_id = clientId;
          client_name = "Zotero development backend";
          client_secret = "$pbkdf2-sha512$100000$hRtAJlEuAmMxGbnC3NNE4g$ManSf9Uy.QZ9frwetiXLGzA3Vkt.H1snR6TASUedaP0/g8kkp4Pyir7NrEH.Kjlocm4J1cvqSC2iGdWkK28BZQ";
          public = false;
          authorization_policy = "one_factor";
          consent_mode = "implicit";
          require_pkce = true;
          pkce_challenge_method = "S256";
          redirect_uris = [redirectUri];
          scopes = [
            "openid"
            "profile"
            "email"
          ];
          response_types = ["code"];
          grant_types = ["authorization_code"];
          access_token_signed_response_alg = "none";
          userinfo_signed_response_alg = "none";
          token_endpoint_auth_method = "client_secret_basic";
        }
      ];
    };
  };

  environment.etc = {
    "authelia-development/jwt-secret" = {
      mode = "0400";
      text = "development-jwt-secret";
    };
    "authelia-development/oidc-hmac-secret" = {
      mode = "0400";
      text = "development-oidc-hmac-secret-development-oidc-hmac-secret";
    };
    "authelia-development/oidc-private-key.pem" = {
      mode = "0400";
      source = signingKey;
    };
    "authelia-development/storage-encryption-key" = {
      mode = "0400";
      text = "development-storage-encryption-key";
    };
    "authelia-development/users.yml" = {
      mode = "0400";
      user = "authelia-development";
      text = ''
        users:
          development:
            disabled: false
            displayname: Development User
            password: $argon2id$v=19$m=65536,t=3,p=4$tUe1D6Enlpn+OUrCRi1OuA$QzSaAimGjf16BmbirSqHJlSotUmibE3gnVFmAfDneyc
            email: development@example.test
            groups:
              - developers
          alice:
            disabled: false
            displayname: Alice Developer
            password: $argon2id$v=19$m=65536,t=3,p=4$tUe1D6Enlpn+OUrCRi1OuA$QzSaAimGjf16BmbirSqHJlSotUmibE3gnVFmAfDneyc
            email: alice@example.test
            groups:
              - developers
          bob:
            disabled: false
            displayname: Bob Editor
            password: $argon2id$v=19$m=65536,t=3,p=4$tUe1D6Enlpn+OUrCRi1OuA$QzSaAimGjf16BmbirSqHJlSotUmibE3gnVFmAfDneyc
            email: bob@example.test
            groups:
              - editors
          carol:
            disabled: false
            displayname: Carol Administrator
            password: $argon2id$v=19$m=65536,t=3,p=4$tUe1D6Enlpn+OUrCRi1OuA$QzSaAimGjf16BmbirSqHJlSotUmibE3gnVFmAfDneyc
            email: carol@example.test
            groups:
              - admins
      '';
    };
  };

  systemd.services.zotero-portal = mkIf startBeforePortal {
    wants = mkAfter ["authelia-development.service"];
    after = mkAfter ["authelia-development.service"];
  };
}
