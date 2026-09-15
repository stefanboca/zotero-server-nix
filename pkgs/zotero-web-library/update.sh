#!/usr/bin/env nix-shell
#! nix-shell -i bash -p nix-update

set -o errexit -o nounset -o pipefail

attr="${UPDATE_NIX_ATTR_PATH:-zotero-web-library}"
nix-update --flake --version=branch=master "$attr"
nix-update --flake --use-github-releases "$attr.release"
nix-update --flake --version=branch=master "$attr.citationStyles"
