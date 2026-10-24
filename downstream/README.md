# Native Linux skirmish changes

This repository contains the VERA20k engine plus the changes developed in the
`cncyr` integration project for native Linux Yuri's Revenge skirmishes. It is
based on [YuriPlanet/vera20k](https://github.com/YuriPlanet/vera20k) at commit
`64bac4056a7ac81b04ec2ae9a3ca96271e0dc6c5`, also tagged here as `cncyr-base`.
The original upstream history, credits and [GPLv3 license](../LICENSE-GPL) are
retained. Each of the 24 downstream patches is a separate commit on `main`.

The client runs without Wine or Proton and needs your own Red Alert 2: Yuri's
Revenge installation. The engine is still under development. This branch focuses
on offline skirmishes and large custom maps; it does not establish complete
campaign, multiplayer, or original Windows save/replay compatibility.

## Build and run

Use the prerequisites in the [upstream README](../README.md#build-and-run):
Rust installed with rustup, Git, platform build tools, and supported graphics.
On Debian/Ubuntu the listed build packages are `build-essential`,
`libasound2-dev` and `pkg-config`.

```sh
git clone https://github.com/trelak-net/vera20k.git
cd vera20k
cp config.toml.example config.toml
# Set paths.ra2_dir in config.toml to your own game directory.
cargo build --release --locked --bin vera20k
./target/release/vera20k
```

Use a release build for play. Resolution, fullscreen and zoom controls are in
the in-game options. Local configuration, retail assets, saves and build outputs
are not part of the downstream commits. The private Wojtek maps used for some
regressions are also not included.

## Gameplay and interface changes

The changes include display and performance improvements, original control-group
shortcuts and number badges, loaded-truck crates, waypoint planning, Scatter,
unit readouts, and AI economy recovery. The feature commits also add Spy actions,
flak scatter, offline factory handling, Tesla charging, friendly targeting,
Aegis distributed fire, Secret Labs, map trigger actions, Giant Squid grappling,
Magnetron lifting, Boris airstrikes, offline diplomacy, and Cloning Vats.

- **Groups:** Ctrl+0–9 assigns, 0–9 selects, Ctrl+Shift+0–9 adds selected units.
- **Waypoints:** hold Z and click destinations; release Z to execute. Click an
  earlier node for a patrol loop. Delete removes a node while planning. Routes
  use whitish lines without numbers, and radar commands retain the selected cell
  on large maps.
- **Cloning Vats:** every owned vat supplies one free infantry clone; multiple
  vats stack. A blocked vat can miss its clone.
- **Diplomacy:** Tab opens the player panel; AI alliances are set before the match.

Native saves use snapshot format **340**. Earlier native formats and original
Windows saves are incompatible. Further performance work, some AI and movement
behavior, additional map-script coverage, and visual/audio fidelity remain open.

## Tests

Keep Linux tests silent by disabling ALSA output:

```sh
ALSA_CONFIG_PATH="$PWD/downstream/no-audio.conf" cargo test --locked --lib display_
```

The inherited test suite includes checks that need retail assets or other local
fixtures. `RA2_DIR` identifies a retail asset directory; tests for the large
Wojtek A20 map use `VERA20K_WOJTEK_A20` to locate a separately supplied map.
`VERA20K_REQUIRE_RETAIL_ASSETS=1` and `VERA20K_REQUIRE_RETAIL_INI=1` can make
relevant missing fixtures fail instead of skip. Do not interpret skipped checks
as validation. GUI tests must also use `-NOAUDIO` with the ALSA override above.

## Patch provenance

[patches.json](patches.json) records the application order, original patch SHA256,
source-project commit, and corresponding commit in this repository. The `cncyr`
source-project hashes refer to the integration repository; the links below refer
to commits available here. The patches are already applied; normal development
can proceed directly against this source tree.

| # | Change | Commit |
| --- | --- | --- |
| 1 | Fix Linux IME lifecycle notifications during hidden captures | [72b40b1f3](https://github.com/trelak-net/vera20k/commit/72b40b1f378686a6224b0c859db4507d6efd5f46) |
| 2 | Add native resolution, fullscreen and interface scaling controls | [12c51bb02](https://github.com/trelak-net/vera20k/commit/12c51bb0274cc82ce7ef37c61103ca5e10ee0ae0) |
| 3 | Improve large-map simulation and rendering performance | [be14c6787](https://github.com/trelak-net/vera20k/commit/be14c67871adc285635a96f90cb6eb22590eb9c4) |
| 4 | Restore original control-group keyboard shortcuts | [dff76a502](https://github.com/trelak-net/vera20k/commit/dff76a502e42119e24bfbecb54eee00bb1d09da1) |
| 5 | Display control-group numbers on assigned units | [90fb3e5d5](https://github.com/trelak-net/vera20k/commit/90fb3e5d5ffbbe0208be348eb2a43b198476feda) |
| 6 | Drop crates when loaded trucks are destroyed | [b6af1ec40](https://github.com/trelak-net/vera20k/commit/b6af1ec40e2bd239802bd335d43f803fdfad14fc) |
| 7 | Implement Z-key waypoint planning and patrol routes | [9cef07743](https://github.com/trelak-net/vera20k/commit/9cef0774312c008b956344d846a2fbb4d080bab4) |
| 8 | fix: wire Scatter hotkey to native unit movement receivers | [18e121a2c](https://github.com/trelak-net/vera20k/commit/18e121a2caa3e53f3d78cb19cdc9a52722c593f7) |
| 9 | feat: render passenger ammo spawn and mind-control pips | [76971b3dd](https://github.com/trelak-net/vera20k/commit/76971b3dd74039ce082e17d4f4bc24a2c8d4d173) |
| 10 | fix: restore AI economic recovery after lost income | [b4db470b6](https://github.com/trelak-net/vera20k/commit/b4db470b6a5e56e3de11c60a7ce8367bc8057777) |
| 11 | feat: restore Spy disguise infiltration and stolen technology | [ae134f2c1](https://github.com/trelak-net/vera20k/commit/ae134f2c19eac5f50c9db58ea24b709861b654dd) |
| 12 | fix: restore original anti-aircraft flak scatter | [5ad7abffe](https://github.com/trelak-net/vera20k/commit/5ad7abffea85923c15d88cc81cdd8e144661f714) |
| 13 | fix: suspend production while all eligible factories are offline | [3d5bdba2a](https://github.com/trelak-net/vera20k/commit/3d5bdba2a45612f3d60e4023814bb804416a2d39) |
| 14 | Implement Tesla Trooper charging and coil overpower state | [597fffbc2](https://github.com/trelak-net/vera20k/commit/597fffbc2fa54566658543f210fa1efa8261307d) |
| 15 | Restore berserk and friendly target acquisition and automatic healing | [a039f8a22](https://github.com/trelak-net/vera20k/commit/a039f8a22d0e4fb3498ff735c59beb6cf2f8878b) |
| 16 | Implement Aegis distributed fire with persistent target history | [b20529393](https://github.com/trelak-net/vera20k/commit/b2052939300b2f4533fb3a1881471592e1a3178c) |
| 17 | Implement Secret Lab rewards and captured production unlocks | [b8e9c33c3](https://github.com/trelak-net/vera20k/commit/b8e9c33c33e4c16ba8bf0416b78dde06f53da8a2) |
| 18 | Restore Wojtek map triggers and one-use superweapon rewards | [2592ee5b7](https://github.com/trelak-net/vera20k/commit/2592ee5b7a29cbc368d3ce53c073e7add4028876) |
| 19 | Implement Giant Squid grappling and Dolphin rescue with original sonic wave sampling | [bc9514c58](https://github.com/trelak-net/vera20k/commit/bc9514c587c178b811af09f905274b020f311174) |
| 20 | Implement Magnetron lifting, pulling, drops and native locomotor restoration | [e03629302](https://github.com/trelak-net/vera20k/commit/e0362930210b2ce236e8752ab074228b617a7ad5) |
| 21 | Implement Boris bomber airstrikes, designation tint and native cancellation | [59fd1e996](https://github.com/trelak-net/vera20k/commit/59fd1e99674aa63af53b76b0f6d58f74808126fb) |
| 22 | Implement diplomacy controls and reject stale Free For All teams | [6f915e1c7](https://github.com/trelak-net/vera20k/commit/6f915e1c7176caba279318477d5d3ccb1a8868e0) |
| 23 | Implement free infantry clones from every owned Cloning Vat | [bd138396f](https://github.com/trelak-net/vera20k/commit/bd138396ff961741914e2b70fdcb58b1741b5ac6) |
| 24 | Restore unnumbered whitish waypoint routes and preserve radar target cells | [a3186f0fc](https://github.com/trelak-net/vera20k/commit/a3186f0fce6121f47b555f5933ef2c4ab2e623d5) |
