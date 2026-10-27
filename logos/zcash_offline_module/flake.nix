{
  description = "Zcash Offline: the ozm-node device as a Logos core module";

  nixConfig = {
    extra-substituters = [ "https://cache.nix.logos.co/public" ];
    extra-trusted-public-keys = [ "public:l4HrXgL4nw246+LBh2SOJyhz64BoGegOYLheT/iIAPU=" ];
  };

  inputs = {
    logos-module-builder.url = "github:logos-co/logos-module-builder/8f56bf8";
  };

  outputs = inputs@{ logos-module-builder, ... }:
    let
      nixpkgs = logos-module-builder.inputs.nixpkgs;
      lib = nixpkgs.lib;
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      noBuildOutputs = src: lib.cleanSourceWith {
        inherit src;
        filter = path: _type:
          !(builtins.elem (baseNameOf path) [ "target" "result" "generated" "workspace-crates" "logos-rust-sdk-src" "lib" ]);
      };

      # The builder copies only `<src>/rust-lib` (plus the SDK beside it), so
      # the repository crates the module path-depends on are staged inside it.
      stagedSrc = system:
        nixpkgs.legacyPackages.${system}.runCommand "zcash_offline_module-src" {} ''
          cp -r ${noBuildOutputs ./.} $out
          chmod -R u+w $out
          mkdir -p $out/rust-lib/workspace-crates
          for c in zcash-offline-core ozm-mesh ozm-ble ozm-node; do
            cp -r ${noBuildOutputs ../../crates}/$c $out/rust-lib/workspace-crates/$c
          done
        '';

      module = system: logos-module-builder.lib.mkLogosModule {
        src = stagedSrc system;
        configFile = ./metadata.json;
        flakeInputs = inputs;
      };
    in
    {
      packages = lib.genAttrs systems (system:
        (module system).packages.${system} // {
          # For dev-stage.sh: the generator and SDK source the build uses.
          lidl-gen = logos-module-builder.inputs.logos-rust-sdk.packages.${system}.lidl-gen;
          rust-sdk-src = logos-module-builder.packages.${system}.rust-sdk-src;
        });
    };
}
