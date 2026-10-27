{
  description = "Offline Zcash: Logos Basecamp QML app for zcash_offline_module";

  inputs = {
    # Pinned to the builder revision this app was built and checked with.
    logos-module-builder.url = "github:logos-co/logos-module-builder/8f56bf8";
    # The core module this app calls (its LIDL contract is bundled into the lgx;
    # `nix run .` / `.#ui-dev` also load it).
    zcash_offline_module.url = "path:../zcash_offline_module";
  };

  outputs = inputs@{ logos-module-builder, ... }:
    logos-module-builder.lib.mkLogosQmlModule {
      src = ./.;
      configFile = ./metadata.json;
      flakeInputs = inputs;
    };
}
