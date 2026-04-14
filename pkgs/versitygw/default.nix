{
  buildGoModule,
  fetchFromGitHub,
  lib,
  ...
}:
buildGoModule (finalAttrs: {
  pname = "versitygw";
  version = "1.4.0";

  src = fetchFromGitHub {
    owner = "versity";
    repo = "versitygw";
    tag = "v${finalAttrs.version}";
    hash = "sha256-sJ5gq6o31o8c9wVJXT6Ip9OlgnrO5MyqYeW2EFqcDTc=";
  };

  vendorHash = "sha256-7LcJsgJ6loNFH9xMkx+CSZsr1JJhn6C+kg4cy7/ZO/Y=";

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
