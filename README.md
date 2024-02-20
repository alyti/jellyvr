<div align=center>

# ❄️🎥 jellyvr 🕶️🦀

Jellyfin proxy for VR Media Players 
(just HereSphere for now)

</div>

## Usage
This project uses Nix for development, [direnv](https://direnv.net/) support is included for convenience.

- `nix develop`: opens up a `bash` shell with useful toolset
- `nix build` : builds the Rust project. Outputs the binary to `./result/bin/<name>`
- `nix run`: runs the Rust program.
- `nix run .#watch`: launches a watch-rebuild server behind systemfd on `http://0.0.0.0:3000`.

## Reference

1. [wiki/Flakes](https://nixos.wiki/wiki/Flakes)
2. [Jellyfin openapi docs](./jellyfin-openapi-stable.json)
