{
  buildGoModule,
  fetchFromGitHub,
  lib,
  ...
}:
buildGoModule (finalAttrs: {
  pname = "versitygw";
  version = "1.4.1";

  src = fetchFromGitHub {
    owner = "versity";
    repo = "versitygw";
    tag = "v${finalAttrs.version}";
    hash = "sha256-UW6/OfGqX3Y1ddWVQ6w+pdFeOIuE4j5p9JkuZ05HN1o=";
  };

  vendorHash = "sha256-0AgO5A6kdcQluw8NVHsKVuhuOinwGURHUGbKcpbaIPw=";

  subPackages = ["cmd/versitygw"];

  doCheck = false;
  # TODO: more granular testing disables
  # see https://github.com/NixOS/nixpkgs/blob/master/doc/languages-frameworks/go.section.md#skipping-tests-ssec-skip-go-tests

  meta = {
    description = "versity s3 gateway";
    homepage = "https://github.com/versity/versitygw";
    mainProgram = "versitygw";
    license = lib.licenses.asl20;
  };
})
