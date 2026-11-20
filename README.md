# OptiFine Capes for Sunrise

A native [Sunrise](https://github.com/sunrise-org/sunrise) mod that shows the capes players got from
[OptiFine](https://optifine.net), as OptiFine and the cape mods for Java do, without OptiFine.

- **Everyone's capes.** A player with an OptiFine cape wears it on their back, and on their elytra when the
  cape's picture has an elytra part. Your own cape shows too, in the world and in your inventory.
- **Over a Mojang cape.** A player's OptiFine cape is drawn instead of their Minecraft cape.
- **HD capes.** A 92x44 cape keeps its detail on a 128x64 texture.

## Privacy

To find a player's cape, the mod asks OptiFine's cape server for `http://s.optifine.net/capes/<name>.png`,
by the player's name. **OptiFine's server so learns the name of every player you see, over plain HTTP**
(it has no HTTPS). Turn the mod's switch off, or remove the mod, and nothing is asked.

Each name is asked once a game. Answers stay in memory only: nothing is written to disk.

## Settings

**Options > Mods > OptiFine Capes > Config**

| Setting | Default | |
|---|---|---|
| OptiFine Capes | On | Off asks the cape server nothing and shows no OptiFine capes. |

## How it works

The game asks the mod, each time it draws a player's cape or wings, whether to use another texture
(`Hooks::resolve_player_layer`). Once a tick the mod reads the player list, asks the cape server about names
it hasn't asked yet (four at a time, on threads of their own), and uploads the capes of the players drawn
lately as dynamic textures: yours first, then the latest drawn, sixteen at most (a mod's dynamic textures).

A cape's picture is laid on the smallest 64·2^n by 32·2^n texture that holds it, as cape makers draw them:
a 46x22 picture on 64x32, a 92x44 one on 128x64. Only an answer that is a PNG of at most 256 KiB and
2048x1024 counts as a cape.

```
src/lib.rs        the mod's hooks
src/capes.rs      the session's capes: who is asked, the textures, the draw's answer
src/downloads.rs  asking the cape server
src/settings.rs   the switch
src/core/         names, pictures, answers and which capes are shown: plain Rust, tested
```

## Building

The mod needs a Sunrise build with `Hooks::resolve_player_layer`
([sunrise-org/sunrise#3624](https://github.com/sunrise-org/sunrise/pull/3624)). Once one is released:

```sh
cargo sunrise-mod update      # moves the pin in sunrise-mod.toml to it
cargo sunrise-mod build
cargo test --manifest-path src/core/Cargo.toml
```

`SUNRISE_OPTIFINE_CAPES` points the mod at another cape server (a test's own).

No OptiFine code was read to write this mod, and it isn't made by or with OptiFine's author.

## Licence

[MIT](LICENSE).
