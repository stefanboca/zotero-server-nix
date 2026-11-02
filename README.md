# Zotero server for NixOS

A self-hosted Zotero installation with desktop sync, shared libraries, file
storage, and a web interface.

The repository packages Zotero's server components and provides a NixOS module
to run them together. A Rust web portal supplies login, account settings, and
group management for the private installation, alongside the upstream Web
Library. Authentication uses your own OpenID Connect provider, and full-text
search runs locally.

It also includes desktop client configuration, a development VM, and
integration tests. Private Dataserver installations are unsupported upstream,
so compatibility with new Zotero releases needs to be checked.

## Use it

After importing the flake's NixOS module:

```nix
services.zotero-dataserver = {
  enable = true;
  virtualHost = "zotero.example.com";
};
```

See [setup and development](docs/setup.md) for authentication, desktop clients,
storage, and the current limitations of web-uploaded full-text search.
