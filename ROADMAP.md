# Roadmap

Updated 25 October 2026. This is the roadmap for
[trelak-net/vera20k](https://github.com/trelak-net/vera20k), Wojciech Trelak’s
personal VERA20k fork developed with OpenAI Codex.

## Scope

Prioritize native Linux offline skirmishes, fidelity to the original Yuri’s
Revenge gameplay and appearance, and large custom Wojtek maps. Use the installed
original executable and retail assets to establish behavior. Preserve responsive
play while correcting differences. All automated and GUI testing stays muted.

## Delivered baseline

The original upstream history plus [24 separately committed patches](downstream/patches.json)
provide the current native client. Implemented work includes display controls,
performance improvements, control groups and number badges, truck crates,
waypoint planning, unit readouts, Scatter, and the 15 major skirmish gaps from
the initial review. The subsequent Cloning Vats and large-map waypoint fixes are
also included. This is a working baseline, with remaining fidelity gaps below.

Native snapshots use format 340. The imported source was checked against the
existing patched engine, and the separate repository passed its compile check.
Prior feature checks include muted large-map GUI runs at 1× and 2× zoom;
these do not establish exhaustive visual equivalence or long-match performance.

## Next priority: original-game visual fidelity

These items are open. The order starts with the most recently reported problem.

1. **Z-selection / waypoint lines and markers.** The current lines still look
   wrong to the player. Planning, patrols and node removal work, and the earlier
   fix removed numbers and changed lines to whitish, but visual fidelity remains
   unfinished. Compare the original line color, thickness, pattern, endpoints,
   node and selection markers, layering, visibility and any animation before
   changing the drawing behavior. Check held-Z and latched planning, multiple
   selected groups, patrol loops and node deletion. Preserve correct tactical
   and radar destinations on large maps: cover X and Y above 255, the tested
   A20 edges near 387, terrain height/bridges, viewport clipping and zoom.
2. **Lighting and palettes.** Reproduce vehicle brightness on bridge decks and
   other documented cell-light cases. Compare terrain, buildings, units and
   animations under matching map lighting, including storm/effect transitions.
3. **Building searchlights and mind-control feedback.** Complete searchlight
   motion and rendering, mind-control connection lines, and Mastermind overload
   flashing and rocking using the original producers, timing and drawing rules.
4. **Shadows, effect colors and object layering.** Investigate remaining bridge
   railing/shadow bindings, effect tinting, animation ordering, and overlapping
   units/buildings. Compare concrete scenes before choosing renderer changes.
5. **Interface and selection presentation.** Compare selection indicators,
   sidebar text and controls, menus and scaling against matching original
   scenes. Treat differences here as investigation targets until reproduced.

For each visual item, capture a controlled scene with matching map, resolution,
camera, state and animation frame. Establish the original calculation or draw
path, implement it in the native renderer, and retain an appropriate regression
check. Use pixel comparisons where the scene and presentation format permit
meaningful comparison. Test zoom separately from the baseline original scale.
A matching primitive or single screenshot does not establish whole-game parity.

## Remaining gameplay and interface work

- Construction Yard repack-and-move commands.
- AI retreat positions, Grinder use for captured units, wall targeting, and
  remaining specialist target-selection behavior.
- Further custom team actions and map events beyond the implemented Wojtek-map
  trigger coverage.
- Vehicle tunnel entry; focused validation of congestion, blocked exits, mixed
  groups, transport boarding/unloading, and aircraft return/rearming behavior.
- Aircraft-waiting sidebar alerts and lower-priority beacon, cheer, paging and
  taunt controls, according to their relevance to offline skirmishes.
- Audio timing and playlist behavior. Automated checks remain silent; audible
  comparison requires a change to the current testing preference.
- Longer-match native save/load validation. Format migration is separate work.

## Performance — paused

Earlier improvements made large-map play better, but further optimization is
paused at the player’s request. The next performance pass should profile long
A20 matches on the normal GPU at the player’s usual resolution and zoom.
Measure simulation, visibility, render preparation and GPU/frame presentation
before selecting changes. Fullscreen or lower resolution alone is not an
established solution. Visual fixes should avoid introducing new performance
regressions.

## Outside the current milestone

- Complete campaign compatibility.
- Playable network transport, lobbies and multiplayer-specific behavior.
- Original Windows saves, replays and network wire compatibility.
- Demonstrating upstream’s 30-player / 20,000-unit scale targets.

These are inherited ambitions or compatibility areas, not completed features
or commitments for this skirmish-focused fork.

## Completing a roadmap item

Record the reproduced problem and the scope of the original-game evidence.
Verify the changed behavior with focused tests and, where relevant, muted GUI
comparisons. Preserve existing gameplay and large-map behavior. Commit each
completed feature or fix separately with a descriptive message, update this
roadmap, and distinguish implemented behavior from remaining limitations.
