# Zotero server for NixOS

This flake provides a NixOS module for a private Zotero server. The module
starts Zotero Dataserver and its required local services. It also starts an S3
file store and an optional OIDC login portal.

Upstream does not support private Dataserver installations. Test client and
server updates before you deploy them.

## Services

The module starts these services:

- Zotero Dataserver
- MariaDB, Redis, and Memcached
- VersityGW for S3 file storage
- attachment-proxy
- stream-server for desktop notifications
- translation-server
- citeproc-js-server
- tinymce-clean-server
- Caddy

The module creates the required databases, S3 buckets, and internal secrets.
It stores each generated secret in a separate file in the systemd state
directory. You do not need to set internal service secrets.

The module does not start the optional Zotero production search system. That
system needs Elasticsearch, DynamoDB, SQS, and an external reindexer. Desktop
full-text search stays local and does not need this system. The module also
does not start the optional global-items and text-to-speech services. The
text-to-speech service generates audio for Read Aloud in the Zotero web
reader. Desktop clients do not use it.

## Basic configuration

Add the module to a NixOS configuration:

```nix
{
  inputs.zotero-server.url = "github:your-name/zotero-server-nix";

  outputs = {nixpkgs, zotero-server, ...}: {
    nixosConfigurations.server = nixpkgs.lib.nixosSystem {
      modules = [
        zotero-server.nixosModules.default
        {
          services.zotero-dataserver = {
            enable = true;
            virtualHost = "zotero.example.com";
          };

          services.caddy.openFirewall = true;
        }
      ];
    };
  };
}
```

Caddy gets a TLS certificate for the virtual host. The managed S3 service
listens only on the loopback interface. Clients use the public attachment
proxy and do not need S3 credentials.

`virtualHost` is also the public base URI for all server endpoints. A bare
host name uses HTTPS. Use an explicit `http://` URI only for a trusted local
deployment.

## OIDC login

Enable the OIDC portal if you do not have the Zotero website:

```nix
services.zotero-dataserver.auth = {
  enable = true;
  issuer = "https://auth.example.com";
  clientId = "zotero";
  clientSecretFile = config.sops.secrets.zotero-oidc-client-secret.path;
};
```

The callback URI is `https://zotero.example.com/auth/oidc/callback`. It must use
HTTPS because the portal uses a secure `__Host-session` cookie.

The portal uses OIDC discovery, Authorization Code flow, PKCE S256, state, and
nonce checks. It creates a Zotero user after the first successful login. It
does not link a new OIDC identity to an existing user by email. It also does
not sync OIDC groups or roles. Dataserver has no global administrator role.

### Authelia

First, configure an [Authelia OpenID Connect
provider](https://www.authelia.com/configuration/identity-providers/openid-connect/provider/).
Then generate a client secret:

```console
authelia crypto hash generate pbkdf2 --variant sha512 \
  --random --random.length 72 --random.charset rfc3986
```

Store the plain secret in the file for `clientSecretFile`. Store the hash in
the Authelia configuration:

```yaml
identity_providers:
  oidc:
    clients:
      - client_id: zotero
        client_name: Zotero
        client_secret: '$pbkdf2-sha512$...'
        public: false
        authorization_policy: two_factor
        require_pkce: true
        pkce_challenge_method: S256
        redirect_uris:
          - https://zotero.example.com/auth/oidc/callback
        scopes:
          - openid
          - profile
          - email
        response_types:
          - code
        grant_types:
          - authorization_code
        access_token_signed_response_alg: none
        userinfo_signed_response_alg: none
        token_endpoint_auth_method: client_secret_basic
```

Authelia must supply `email`, `email_verified`, `preferred_username`, and
`name`. The portal rejects an unverified email by default. Set
`auth.requireVerifiedEmail = false` only if the provider cannot supply the
verification claim.

See the [Authelia client
reference](https://www.authelia.com/configuration/identity-providers/openid-connect/clients/)
for the current client syntax. Role sync is not required and is not supported.

## Desktop client

Use this flake's overlay to build a client that uses the private server:

```nix
{pkgs, ...}: {
  nixpkgs.overlays = [zotero-server.overlays.default];

  environment.systemPackages = [
    (pkgs.zotero.withConfig {
      apiBaseUri = "https://zotero.example.com/";
    })
  ];
}
```

`apiBaseUri` is required. By default, the package uses it for the canonical
object URI, website URL, API URL, and domain name. It changes `https` to `wss`
and adds `/stream/` for the stream URL. You can override `baseUri`,
`wwwBaseUrl`, `domainName`, or `streamingUrl` if the deployment uses different
URLs.

For an unmodified Zotero package, open the Config Editor in the advanced
settings. Set these two hidden preferences:

```text
extensions.zotero.api.url = https://zotero.example.com/
extensions.zotero.streaming.url = wss://zotero.example.com/stream/
```

Restart Zotero. Keep `extensions.zotero.sync.storage.protocol` set to
`zotero`. The server sends file transfers through attachment-proxy, so the
client does not need a WebDAV URL or S3 settings.

The upstream client source reads the API and stream endpoints from these
preferences. See [syncRunner.js](https://github.com/zotero/zotero/blob/main/chrome/content/zotero/xpcom/sync/syncRunner.js)
and [streamer.js](https://github.com/zotero/zotero/blob/main/chrome/content/zotero/xpcom/streamer.js).

## File storage on a network share

Set the VersityGW data directory to the mounted share:

```nix
services.versitygw = {
  dataDir = "/mnt/zotero-objects";

  # Use a local sidecar if the share has no extended-attribute support.
  metadataSidecarDir = "/var/lib/versitygw-metadata";

  # Enable these options when the network filesystem needs them.
  disableOTmpfile = true;
  disableCopyFileRange = true;
};
```

Mount the share before the service starts. The module adds
`RequiresMountsFor` for both directories. The `versitygw` user must have write
access. Use `metadataSidecarDir` when the share cannot store extended
attributes.

## External S3 service

You can replace managed VersityGW with another S3-compatible service:

```nix
services.zotero-dataserver.storage = {
  manageBackend = false;
  endpoint = "https://s3.example.com";
  region = "us-east-1";
  forcePathStyle = true;
  accessKeyIdFile = config.sops.secrets.zotero-s3-access-key-id.path;
  secretAccessKeyFile = config.sops.secrets.zotero-s3-secret-access-key.path;
};
```

Create the configured buckets in the external service. The managed backend
creates them automatically.

## Auth UI development

Run the Topcoat development server from the auth package directory:

```console
cd pkgs/zotero-auth
nix develop ../.. -c topcoat dev
```

Open `http://127.0.0.1:3000`. Preview mode does not read the TOML file,
credentials, or database. It shows the login-complete page and reloads it after
source changes.

## Checks

Run all package, format, and VM checks:

```console
nix flake check -L
```

Run only the NixOS VM test:

```console
nix build -L .#checks.x86_64-linux.nixos-dataserver
```

The VM test checks schemas, OIDC login races, S3 file transfers, API sync,
streaming notifications, and all local microservices.
