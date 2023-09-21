{
  outputs = { fl, fl-rust, ... }: fl ./. {
    systems = [
      "aarch64-darwin"
      "x86_64-linux"
    ];
    imports = [
      fl-rust.flakelightModules.default
    ];
    devShell = {
      packages = pkgs: with pkgs; [
        bzip2
        curl
        nodejs_20
        openssl
        pkg-config
        zlib
        # TODO: these sys crates don't use system libraries so we can't pin them:
        # libgit2-sys
        # libssh2-sys
        # libnghttp2-sys
        # libsqlite3-sys
        # zstd-sys
      ] ++ lib.optionals stdenv.isDarwin ([
        libiconv
      ] ++ (with darwin.apple_sdk.frameworks; [
        CoreFoundation
        SystemConfiguration
      ]));
    };
  };

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    fl = {
      url = "github:accelbread/flakelight";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fl-rust = {
      url = "github:accelbread/flakelight-rust";
      inputs.flakelight.follows = "fl";
    };
  };
}
