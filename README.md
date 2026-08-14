# Zotero server for NixOS

This flake provides a NixOS module for a private Zotero server. The module
starts Zotero Dataserver and its required local services. It also starts an S3
file store and an OIDC-authenticated local web portal.

Upstream does not support private Dataserver installations. Test client and
server updates before you deploy them.

## Services

The module starts these services:

- Zotero Dataserver
- MariaDB, Redis, and Memcached
- OpenSearch for server-side full-text search
- VersityGW for S3 file storage
- zotero-portal for OIDC login, accounts, groups, and settings
- attachment-proxy
- stream-server for desktop notifications
- translation-server
- citeproc-js-server
- tinymce-clean-server
- Caddy

The module creates the required databases, S3 buckets, and internal secrets.
It stores each generated secret in a separate file in the systemd state
directory. You do not need to set internal service secrets.

The module replaces Zotero's AWS full-text indexing pipeline with synchronous
indexing into a local OpenSearch service. Existing full-text objects in
S3 are imported automatically the first time the index is initialized. This
provides server-side full-text search without DynamoDB, SQS, or Lambda.
The managed OpenSearch build drops unrelated bundled plugins and starts with a
128 MiB heap that may grow to 512 MiB. Override
`services.opensearch.extraJavaOptions` if different JVM settings are needed.

The module does not start the optional global-items and text-to-speech
services. The text-to-speech service generates audio for Read Aloud in the
Zotero web reader. Desktop clients do not use it.

## TODO

- Add a full-text indexer that extracts text from files uploaded by the Web
  Library and submits it to Dataserver. The existing OpenSearch integration
  indexes full-text content received through the API but does not extract text
  from stored PDFs itself.

## Development backend VM

Run the backend dependencies and a mock OIDC provider with QEMU:

```console
nix run
```

The VM persists its state in `nixos.qcow2` in the current directory. It does
not build, migrate, install, or run `zotero-portal`; run the portal on the host
on port `3000`. Caddy in the VM reaches it through QEMU's host gateway and
proxies it as part of the unified development origin. The development VM sets
`portal.enable = false`, while the module default remains `true` for normal
deployments.

| Service | Host endpoint |
| --- | --- |
| Portal and web library | `http://localhost:8080/` |
| Dataserver API | `http://localhost:8080/api/` |
| Attachment proxy | `http://localhost:8080/files/` |
| Proxied stream server | `ws://localhost:8080/stream/` |
| Direct stream server | `ws://localhost:8089` |
| Translation server | `http://localhost:1969` |
| MariaDB | `127.0.0.1:3306` |
| Mock OIDC issuer | `http://localhost:9091` |

Configure the host portal to listen on `127.0.0.1:3000`, with
`http://localhost:8080` as its public origin. Caddy serves the packaged web
library assets itself and proxies all remaining portal requests to
`http://10.0.2.2:3000`. The mock provider accepts the resulting callback URI,
`http://localhost:8080/auth/oidc/callback`. Run the portal's database migration
from the host; the VM intentionally does not run it.

The Dataserver superuser is `zotero-portal` with password `development`. The
host portal uses this account when calling the Dataserver API.

The mock provider contains three development identities: `alice`, `bob`, and
`carol`. Its authorization page has a one-click button for each identity;
there are no passwords. Each identity gets its own Zotero account on first
login. Use OIDC client ID `zotero` and client secret `development`; the mock
accepts any development client credentials and redirect URI.

MariaDB has a TCP user named `development` with password `development` and
access to the `zotero_ids`, `zotero_master`, `zotero_shard`, and `zotero_www`
databases. For example:

```console
mariadb --host=127.0.0.1 --port=3306 --user=development --password=development zotero_www
```

The Dataserver, OIDC issuer, and OIDC callback intentionally use plain HTTP.
No development certificate or custom trust configuration is required.

Shut down the guest with `sudo poweroff` and wait for QEMU to exit. Do not use
QEMU's `Ctrl-a x` shortcut for routine shutdown: it terminates the VM immediately
and can corrupt Dataserver's MyISAM ID-allocation tables.

The VM console automatically logs in as `nixos`. Its password is also `nixos`,
and it can run `sudo` without a password. These development credentials are
only for local use and must not be copied into a real deployment.

Dataserver PHP error summaries are sent to the systemd journal. Follow them
from the VM console while reproducing a request with:

```console
sudo journalctl --follow --identifier=zotero-dataserver
```

Full error reports may additionally be archived in the managed `zotero-errors`
S3 bucket.

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

All managed services are grouped under `zotero.target`, so they can be started
together:

```console
systemctl start zotero.target
```

## OIDC login

Configure the always-enabled local portal:

```nix
services.zotero-dataserver.portal = {
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

After a successful login, the portal retains a durable browser authentication
session for 30 days by default. A returning authenticated browser skips OIDC;
it goes directly to the requested portal page, or directly completes a pending
Zotero desktop login. Set
`portal.browserSessionLifetimeSeconds` to override the lifetime. The NixOS option
is `null` by default, so the generated configuration leaves the service's
30-day application default in effect.

The database stores only a hash of the browser session token, its Zotero user
binding, and its expiry. The transient OIDC state, nonce, PKCE verifier, and
return path live in an encrypted, short-lived browser cookie instead of the
database. Provider access and refresh tokens are never persisted; the access
token is used only for the callback's optional UserInfo request.

The first Web Library request in each browser session creates a dedicated
full-access Dataserver API key. Dataserver marks these as Web UI keys, so they
do not appear in the Connected apps list and cannot be edited through the key
update endpoint. Signing out or revoking a browser session also revokes its
Web UI key, including when one session revokes another.

Every successful OIDC login also assigns the user Dataserver's upstream
`unlimited` file-storage quota. This deployment stores unlimited as a zero
quota sentinel and bypasses the upload quota check for that value. The
assignment is refreshed on every login.

The same origin serves an OIDC-authenticated web portal. `/` is the account
home, `/groups` lists groups, and `/account` shows the OIDC identity and
unlimited storage policy. Unauthenticated requests are redirected to `/user/login` and
return to their original path after OIDC completes. The shared navigation shows
the current user and provides `/user/logout`.

Group owners and administrators can add a provisioned OIDC account by username
or email. The person must have signed in at least once. Owners can change roles,
remove members with confirmation, and transfer ownership to an existing member
after typing the group name to confirm. Dataserver makes the former owner an
administrator as part of the transfer. Owners can also permanently delete a
group after typing its name. Group deletion deletes that group's synced library
items and files; it never affects a personal library.

The account page gets current storage usage from Dataserver's storage
administration API and displays it alongside the unlimited quota policy. The
total includes the personal library and any group libraries the user owns.

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
`portal.requireVerifiedEmail = false` only if the provider cannot supply the
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
      baseUri = "https://zotero.example.com/";
    })
  ];
}
```

`baseUri` is required. By default, the package uses it for canonical object and
website URLs, adds `/api/` for the API URL, and changes `https` to `wss` while
adding `/stream/` for the stream URL. You can override `apiBaseUri`,
`wwwBaseUrl`, `domainName`, or `streamingUrl` if the deployment uses different
URLs. Dataserver is exposed only below `/api/`; other paths go to the web
portal when authentication is enabled.

The configured package also makes Zotero load the configured application
archive instead of the upstream archive reached through Nix store symlinks.
This makes website actions honor `wwwBaseUrl`: **New Group** opens the
self-hosted group-creation page, and storage's **Open Account Settings** opens
the self-hosted unlimited-storage page.

For an unmodified Zotero package, open the Config Editor in the advanced
settings. Set these two hidden preferences:

```text
extensions.zotero.api.url = https://zotero.example.com/api/
extensions.zotero.streaming.url = wss://zotero.example.com/stream/
```

Restart Zotero. Keep `extensions.zotero.sync.storage.protocol` set to
`zotero`. The server sends file transfers through attachment-proxy, so the
client does not need a WebDAV URL or S3 settings.

The upstream client source reads the API and stream endpoints from these
preferences. See [syncRunner.js](https://github.com/zotero/zotero/blob/main/chrome/content/zotero/xpcom/sync/syncRunner.js)
and [streamer.js](https://github.com/zotero/zotero/blob/main/chrome/content/zotero/xpcom/streamer.js).
The hidden preferences do not redirect website actions; use `withConfig` for
the self-hosted group and account pages.

### Patching Zotero without Nix

The same client patch can be applied to a conventional Linux, macOS, or Windows
installation. Quit Zotero first and make a backup of its application archive.
This changes the installed application, not the Zotero profile or data
directory, so it does not affect the local library database.

Find the `omni.ja` that contains both `resource/config.mjs` and
`chrome/content/zotero/`. Typical locations are:

- Linux: `<Zotero installation>/app/omni.ja`
- macOS: `/Applications/Zotero.app/Contents/Resources/app/omni.ja`
- Windows: `C:\Program Files\Zotero\app\omni.ja`

An installation can contain another `omni.ja` for the Mozilla runtime. Check
the archive contents and patch only the one containing Zotero's
`resource/config.mjs`. Copy that archive somewhere safe before modifying it.

Extract these four entries into an otherwise empty working directory, preserving
their directory names:

```text
resource/config.mjs
chrome/content/zotero/preferences/preferences_sync.xhtml
chrome/content/zotero/xpcom/uri.js
chrome/content/zotero/xpcom/storage/zfs.js
```

In `resource/config.mjs`, change these properties of `ZOTERO_CONFIG`, replacing
the example host with the deployment's public origin and keeping every trailing
slash:

```javascript
DOMAIN_NAME: 'zotero.example.com',
BASE_URI: 'https://zotero.example.com/',
WWW_BASE_URL: 'https://zotero.example.com/',
API_URL: 'https://zotero.example.com/api/',
STREAMING_URL: 'wss://zotero.example.com/stream/',
```

In `chrome/content/zotero/xpcom/storage/zfs.js`, replace the hard-coded storage
settings URL:

```javascript
let url = "https://www.zotero.org/settings/storage";
```

with:

```javascript
let url = ZOTERO_CONFIG.WWW_BASE_URL + "settings/storage";
```

In `chrome/content/zotero/preferences/preferences_sync.xhtml`, replace the
upstream account-registration target:

```html
href="http://zotero.org/user/register"
```

with the private compatibility route:

```html
href="https://zotero.example.com/user/register"
```

The portal temporarily redirects that route to `/user/login`; OIDC provisions
the account on first sign-in, so there is no separate registration form.

In `chrome/content/zotero/xpcom/uri.js`, make object URIs use the configured
canonical base by replacing:

```javascript
value: 'http://zotero.org/'
```

with:

```javascript
value: ZOTERO_CONFIG.BASE_URI
```

Also fix `getGroupURI()` so it keeps the converted website URL:

```javascript
uri = this.toWebURL(uri);
```

instead of calling `this.toWebURL(uri)` and discarding its return value. These
changes make double-clicking a personal library or group open the private web
origin. The portal maps a personal-library URL to its index and a group URL to
that group's management page.

Update the original archive with the four edited files. With Info-ZIP tools,
run this from the working directory:

```console
zip -u /path/to/Zotero/app/omni.ja \
  resource/config.mjs \
  chrome/content/zotero/preferences/preferences_sync.xhtml \
  chrome/content/zotero/xpcom/uri.js \
  chrome/content/zotero/xpcom/storage/zfs.js
```

Writing inside `Program Files` or `/Applications` may require administrator
access. Modifying the macOS application invalidates its existing signature; an
ad-hoc re-sign after patching is usually required:

```console
codesign --force --deep --sign - /Applications/Zotero.app
```

Start Zotero and also check the two hidden preferences shown above. An existing
user-set preference can override a newly patched application default. **New
Group** should open `/groups/new`, and **Open Account Settings** should open
`/settings/storage`, both on the private server. Clearing Zotero's cache does
not change these URLs.

Zotero updates replace `omni.ja`, so repeat the patch after every client update.
The substitutions deliberately depend on Zotero's current source layout;
inspect an updated archive rather than assuming an old patch still applied.

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

## Portal UI development

Run the Topcoat development server from the portal package directory:

```console
cd pkgs/zotero-portal
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
