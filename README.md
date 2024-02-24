<div align=center>

# ❄️🎥 jellyvr 🕶️🦀

Jellyfin proxy for VR Media Players 
(just HereSphere for now)

</div>

## Features
- [x] HereSphere JSON API v1 support
- [x] JellyFin QuickConnect as auth
- [x] JellyFin playback tracking
- [ ] Configuration through
  - [x] Environment 
    - `JELLYFIN_HOST` (Required) Jellyfin server host
    - `JELLYFIN_REMOTE_HOST` Override urls pointing to Jellyfin instance (media & images), defaults to `JELLYFIN_HOST`.
    - `RUST_LOG` Logging configuration, see [tracing_subscriber::filter::EnvFilter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) for details.
  - [ ] YAML
  - [x] ~~Code~~ (Sorry)

## Usage

### Install & Config
Find built container in [Packages](https://github.com/alyti/jellyvr/packages).
Configure by setting `JELLYFIN_HOST` env and mounting `/data` to some persistent location.

### Login
In HereSphere, navigate to root page (ex. `https://jellyvr.tld/`), you should see a code, on another device go to your jellyfin server and in QuickConnect page enter the code from jellyvr.
After a few seconds jellyvr will reload itself and show a dashboard (TODO, it's just the credentials for now), in there you can find a username and password.
The username is your jellyfin username.
The password is a short random one, used for logging into HereSphere, try to remember it or write it down.
Now you can either click the HereSphere link on the page or navigate to it manually by just appending `/heresphere` to the root page from earlier (ex. `https://jellyvr.tld/heresphere`)
You will be prompted to login, enter your new credentials now.
This session should persist for however long jellyfin decides to keep it, there's no built in expiration logic.

### Browsing
After login you should see your entire jellyfin library dumped in front of you.
On the right side you can find tags, at the top right corner you will see a tag category selection box.
For series one of the key tag categories is `Series` (and/or `Studio` which is just an alias which HereSphere has special treatment for)
If you think there are tag categories missing feel free to open a PR or an Issue about it.

## Limitations
HereSphere has it's own codec limitations, notably it can't play Dolby audio, if you have media with ac3, eac3 or dts codecs you will have to either transcode them yourself or find alternatives.

## Development
This project uses Nix for development, [direnv](https://direnv.net/) support is included for convenience.

- `nix develop`: opens up a `bash` shell with useful toolset
- `nix build` : builds the Rust project. Outputs the binary to `./result/bin/<name>`
- `nix run`: runs the Rust program.
- `nix run .#watch`: launches a watch-rebuild server behind systemfd on `http://0.0.0.0:3000`.

## Reference

1. [wiki/Flakes](https://nixos.wiki/wiki/Flakes)
2. [Jellyfin openapi docs](./jellyfin-openapi-stable.json)
