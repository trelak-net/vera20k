# VERA20k — Wojtek’s native Linux fork

This is [Wojciech Trelak’s (`trelak-net`)](https://github.com/trelak-net) personal
fork of [YuriPlanet/VERA20k](https://github.com/YuriPlanet/vera20k), a Rust
reimplementation of the Command & Conquer: Red Alert 2 — Yuri’s Revenge engine.
I use this fork to play skirmishes on Linux and develop it with **OpenAI Codex**.

The goal is to preserve the original game’s gameplay, appearance and controls,
including on my large custom Wojtek maps, while improving native Linux
performance. The client runs without Wine or Proton and reads the original
retail assets. You need your own copy of Red Alert 2: Yuri’s Revenge.

This repository is maintained independently of the upstream VERA20k project.
Its source history, GPLv3 license and upstream attribution are preserved.

## Current status

**Native Linux skirmishes are playable; reconstruction is still in progress.**
The fork includes 24 patches developed in the `cncyr` integration project, each
imported as a separate commit. That work includes:

- Resolution, fullscreen and zoom controls, plus large-map performance improvements.
- Original control-group shortcuts and visible group numbers; Scatter and unit readouts.
- Loaded-truck crate drops, Z-key waypoint planning and corrected large-map radar orders.
- Cloning Vats that each produce an additional free infantry clone.
- Spy actions, Secret Lab rewards, map trigger actions and offline factory handling.
- Tesla charging, flak scatter, friendly targeting, Aegis distributed fire,
  Giant Squid grappling, Magnetron lifting and Boris airstrikes.
- AI economy recovery and offline skirmish diplomacy.

There are still visible and behavioral differences from the original game.
**Z-selection/waypoint lines remain a visual-fidelity issue:** the earlier change
to whitish, unnumbered lines did not make them look sufficiently like the original.
Lighting, searchlights, mind-control effects, shadows and drawing order also need
further comparison. Some AI, movement and map-script behavior remains incomplete.

The [roadmap](ROADMAP.md) records the priorities and remaining work.
The [downstream notes](downstream/README.md) document controls, testing and the
[24-patch history](downstream/patches.json).

Native saves currently use format **340**. Older native formats and original
Windows saves are incompatible. Complete campaigns, playable network multiplayer
and original save/replay compatibility are outside the current skirmish milestone.
Further general performance optimization is paused pending a dedicated pass.
The fork’s current validation focuses on Linux; inherited support for other
platforms is not a claim that these downstream changes have been tested there.

## Build and run

You need:

- Your own Red Alert 2: Yuri’s Revenge installation.
- Rust installed with [rustup](https://rustup.rs/) and Git.
- Linux build tools. On Debian/Ubuntu:
  `sudo apt install build-essential libasound2-dev pkg-config`.
- A Vulkan-capable graphics driver.

```sh
git clone https://github.com/trelak-net/vera20k.git
cd vera20k
cp config.toml.example config.toml
# Set paths.ra2_dir in config.toml to your game directory.
cargo build --release --locked --bin vera20k
./target/release/vera20k
```

Use a release build for play. Resolution, fullscreen and zoom are available in
the in-game options. Local configuration is stored in the ignored `config.toml`;
the engine log is in `logs/ra2.log`. Retail assets and the private Wojtek maps
used for some regression checks are not supplied by this fork.

## Development with Codex

I set priorities and provide feedback from playing the game. **Codex is the coding
agent used to develop this fork**, inspect the original executable, implement
changes in Rust, and run focused checks. The original game and its retail data
are the reference for reconstruction; a visual approximation or a passing unit
test alone does not establish full original-game equivalence.

The [original-executable comparison tools](tools/native_oracle.md) and controlled
scene comparisons support this work. Linux testing stays muted; see the
[silent-test instructions](downstream/README.md#tests). Each completed gameplay
or rendering change should have a descriptive commit and appropriate validation.
New fork commits use `wojtek@trelak.net` as author and committer.

## Contributing

Report reproducible differences or propose changes through
[this fork’s issues](https://github.com/trelak-net/vera20k/issues) and
[pull requests](https://github.com/trelak-net/vera20k/pulls). Include the map,
settings, steps and expected original-game behavior when possible. The
[roadmap](ROADMAP.md) describes this fork’s scope and priorities.

Gameplay is in `src/sim/`, rendering in `src/render/` and `src/app/presentation/`,
and input and interface code in `src/app/` and `src/ui/`.
Contributions use the project’s GPLv3 license.

## Upstream and credits

This fork started from upstream commit
[`64bac4056a7ac81b04ec2ae9a3ca96271e0dc6c5`](https://github.com/YuriPlanet/vera20k/commit/64bac4056a7ac81b04ec2ae9a3ca96271e0dc6c5),
also tagged here as `cncyr-base`. Credit for the original VERA20k engine and its
history belongs to the upstream authors and contributors. The inherited
`README.*.md` translations describe that upstream snapshot; this English README
and the roadmap describe the current fork.

The upstream project also credits OpenRA, XCC Mixer, the ModEnc wiki, Project
Perfect Mod, EA’s GPL source releases of Command & Conquer and Red Alert,
World-Altering Editor, Final Alert, YRpp, Ares, Phobos and others.

Licensed under the [GPLv3](LICENSE-GPL). Command & Conquer and Red Alert are
trademarks of Electronic Arts Inc.; original game artwork belongs to its
respective owners. This fork is not affiliated with or endorsed by Electronic Arts.
