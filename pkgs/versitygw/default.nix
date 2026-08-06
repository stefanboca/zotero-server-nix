{
  buildGoModule,
  fetchFromGitHub,
  lib,
  ...
}:
buildGoModule (finalAttrs: {
  pname = "versitygw";
  version = "1.7.0";

  src = fetchFromGitHub {
    owner = "versity";
    repo = "versitygw";
    tag = "v${finalAttrs.version}";
    hash = "sha256-O3rXqg0wSAb4YXxgXqf42oo9sJinZhZ1U6e5WCnvo9I=";
  };

  vendorHash = "sha256-8WrGFLIoXmHQmyFGhOjBAFkaYZ1xhx0aldpyZULfAL4=";

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
