//! Production damage/pickup regressions for the native Unit death-crate tail.
//! Expected gates and search order come from the installed 73838A..738457
//! instructions, not from executing a substituted Windows death routine.
use crate::map::basic::parse_basic_section;
use crate::rules::ini_parser::IniFile;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::{EntityDamageEvent, RAD_NO_ATTACKER, ReceiverCallFlags};
use crate::sim::house_state::HouseState;
use crate::sim::scenario_session::ScenarioDescriptor;
use crate::sim::world::{FrameEffects, Simulation};

fn fixture(truck_crate: bool, train_crate: bool) -> (Simulation, RuleSet, OverlayTypeRegistry) {
    let rules_text = "\
[VehicleTypes]
0=TRUCKB
1=TRUCKA
2=MODDED
3=TRAIN
[TRUCKB]
Strength=200
Speed=4
Locomotor={4A582741-9839-11d1-B709-00A024DDAFD1}
MovementZone=Normal
CarriesCrate=yes
[TRUCKA]
Strength=200
Speed=4
Locomotor={4A582741-9839-11d1-B709-00A024DDAFD1}
MovementZone=Normal
[MODDED]
Strength=200
Speed=4
Locomotor={4A582741-9839-11d1-B709-00A024DDAFD1}
MovementZone=Normal
CarriesCrate=yes
[TRAIN]
Strength=200
Speed=4
Locomotor={4A582741-9839-11d1-B709-00A024DDAFD1}
MovementZone=Normal
CarriesCrate=yes
IsTrain=yes
[Warheads]
0=KILLWH
[KILLWH]
Verses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%
[CrateRules]
CrateImg=SILVER
WoodCrateImg=WOOD
WaterCrateImg=WATER
[OverlayTypes]
0=SILVER
1=WOOD
2=WATER
[SILVER]
Crate=yes
[WOOD]
Crate=yes
[WATER]
Crate=yes
";
    let (mut sim, rules, registry) =
        crate::sim::world::entry_test_fixture::fixture_with_rules(rules_text);
    sim.playfield_size_height = Some(16);
    sim.session.truck_crate = truck_crate;
    sim.session.train_crate = train_crate;
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.crates = true;
    let owner = sim.interner.intern("Americans");
    let mut house = HouseState::new(owner, 0, None, true, 0, 10);
    house.player_control = true;
    sim.houses.insert(owner, house);
    sim.session.house_order.push(owner);
    (sim, rules, registry)
}

fn spawn(sim: &mut Simulation, rules: &RuleSet, kind: &str) -> u64 {
    let id = sim
        .spawn_object_at_height(kind, "Americans", 10, 12, 0, 0, rules)
        .unwrap();
    assert!(sim.rebuild_dynamic_navigation(rules));
    id
}

fn damage(
    sim: &mut Simulation,
    rules: &RuleSet,
    registry: &OverlayTypeRegistry,
    id: u64,
    amount: i32,
) {
    let warhead = sim.interner.intern("KILLWH");
    let event = EntityDamageEvent::direct_receiver(
        id,
        amount,
        0,
        RAD_NO_ATTACKER,
        None,
        warhead,
        ReceiverCallFlags {
            ignore_defenses: false,
            arg6: false,
        },
    );
    sim.commit_noncombat_aoe_hits(rules, Some(registry), &[event], FrameEffects::default());
}

fn cells(sim: &Simulation) -> Vec<(i16, i16)> {
    sim.crate_authority
        .slots()
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| (s.cell_x, s.cell_y))
        .collect()
}

#[test]
fn truck_crate_loaded_vehicle_leaves_one_random_collectible_crate() {
    let (mut sim, mut rules, registry) = fixture(true, false);
    let id = spawn(&mut sim, &rules, "TRUCKB");
    damage(&mut sim, &rules, &registry, id, 10);
    assert!(
        cells(&sim).is_empty(),
        "nonfatal damage cannot drop the load"
    );
    damage(&mut sim, &rules, &registry, id, 100_000);
    assert_eq!(cells(&sim), [(10, 12)]);
    let cell = sim.overlay_grid.as_ref().unwrap().cell(10, 12);
    assert_eq!(cell.overlay_id, registry.id_for_name("WOOD"));
    assert_eq!(
        cell.overlay_data,
        u8::MAX,
        "random powerup selected on pickup"
    );
    damage(&mut sim, &rules, &registry, id, 100_000);
    assert_eq!(
        cells(&sim),
        [(10, 12)],
        "repeated damage cannot duplicate the load"
    );

    // Exercise the regular collector with deterministic Money-only rewards.
    // Disable regeneration after the drop to isolate removal of this one crate.
    sim.session.game_options.crates = false;
    sim.session.game_options.bases = false;
    rules.powerups.weights.fill(0);
    rules.powerups.weights[crate::rules::powerups::POWERUP_MONEY] = 1;
    let collector = sim
        .spawn_object_at_height("TRUCKA", "Americans", 11, 12, 0, 0, &rules)
        .expect("collector on the adjacent cell");
    let owner = sim.interner.get("Americans").unwrap();
    let credits = sim.houses[&owner].economy.credits();
    assert!(super::pickup_crate(
        &mut sim,
        &rules,
        &registry,
        (10, 12),
        collector,
        FrameEffects::default()
    ));
    assert!(cells(&sim).is_empty());
    assert_eq!(
        sim.overlay_grid.as_ref().unwrap().cell(10, 12).overlay_id,
        None
    );
    assert!(sim.houses[&owner].economy.credits() > credits);
}

#[test]
fn truck_crate_respects_type_map_and_match_flags() {
    for (kind, trucks, trains, crates, expected) in [
        ("TRUCKB", true, false, true, 1),
        ("TRUCKA", true, true, true, 0),
        ("TRUCKB", false, true, true, 0),
        ("TRUCKB", true, false, false, 0),
        ("MODDED", true, false, true, 1),
        ("TRAIN", true, false, true, 0),
        ("TRAIN", false, true, true, 1),
        ("TRAIN", false, true, false, 0),
    ] {
        let (mut sim, rules, registry) = fixture(trucks, trains);
        sim.session.game_options.crates = crates;
        let id = spawn(&mut sim, &rules, kind);
        damage(&mut sim, &rules, &registry, id, 100_000);
        assert_eq!(
            cells(&sim).len(),
            expected,
            "{kind}: truck={trucks}, train={trains}, crates={crates}"
        );
    }
}

#[test]
fn truck_crate_finds_empty_ground_when_an_overlay_is_under_the_truck() {
    let (mut sim, rules, registry) = fixture(true, false);
    let id = spawn(&mut sim, &rules, "TRUCKB");
    // Install an authored overlay literally: the regular crate placer would
    // already snap away from the living truck's occupied cell.
    let wood = registry.id_for_name("WOOD").unwrap();
    assert!(super::write_crate_mark_fields(
        &mut sim,
        &registry,
        (10, 12),
        wood,
        0
    ));
    damage(&mut sim, &rules, &registry, id, 100_000);
    let occupied = cells(&sim);
    assert_eq!(
        occupied.len(),
        1,
        "first search must reject existing overlays"
    );
    assert_ne!(occupied[0], (10, 12));
    assert!(
        occupied
            .iter()
            .all(|&(x, y)| (x - 10).abs() <= 1 && (y - 12).abs() <= 1)
    );
    assert_eq!(
        sim.overlay_grid.as_ref().unwrap().cell(10, 12).overlay_data,
        0
    );
}

#[test]
fn truck_crate_map_flags_are_parsed_saved_and_hashed() {
    let basic = parse_basic_section(&IniFile::from_str(
        "[Basic]\nTruckCrate=yes\nTrainCrate=no\n",
    ));
    assert_eq!(
        (basic.truck_crate, basic.train_crate),
        (Some(true), Some(false))
    );
    let absent = parse_basic_section(&IniFile::from_str("[Basic]\n"));
    assert_eq!((absent.truck_crate, absent.train_crate), (None, None));
    let ordinary = Simulation::from_descriptor(&ScenarioDescriptor::default());
    for (truck_crate, train_crate) in [(true, false), (false, true), (true, true)] {
        let sim = Simulation::from_descriptor(&ScenarioDescriptor {
            truck_crate,
            train_crate,
            ..Default::default()
        });
        assert_ne!(sim.state_hash(), ordinary.state_hash());
        let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 1, 2, "trucks.map", 0);
        let restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
            .unwrap()
            .sim;
        assert_eq!(
            (restored.session.truck_crate, restored.session.train_crate),
            (truck_crate, train_crate)
        );
        assert_eq!(restored.state_hash(), sim.state_hash());
    }
}

/// Optional local integration: exercises stock rules + authored map overrides,
/// map staging, real Neutral TRUCKB instances and the production damage path.
#[test]
#[ignore = "requires VERA_TRUCK_TEST_ASSETS and VERA_TRUCK_TEST_MAP"]
fn truck_crate_retail_map_loaded_trucks_drop_crates() {
    let assets = std::env::var("VERA_TRUCK_TEST_ASSETS").expect("retail asset directory");
    let map = std::env::var("VERA_TRUCK_TEST_MAP").expect("authored map filename");
    let mut scenario = crate::headless_scenario::load(std::path::Path::new(&assets), &map, 7)
        .expect("load the actual authored skirmish");
    let runtime = &mut scenario.runtime;
    let sim = &mut runtime.simulation;
    let rules = &runtime.resources.rules;
    let registry = &runtime.resources.overlay_registry;
    assert!(
        sim.session.truck_crate,
        "map Basic.TruckCrate reached the live session"
    );
    assert!(
        sim.session.game_options.crates,
        "match permits death crates"
    );
    assert!(rules.object("TRUCKB").unwrap().carries_crate);
    assert!(!rules.object("TRUCKA").unwrap().carries_crate);
    let trucks: Vec<_> = sim
        .substrate
        .entities
        .values_sorted()
        .filter(|e| sim.interner.resolve(e.type_ref()) == "TRUCKB")
        .map(|e| e.stable_id)
        .collect();
    assert!(trucks.len() >= 3, "authored loaded trucks are present");
    let warhead = sim.interner.intern("SA");
    for id in [trucks[0], trucks[trucks.len() / 2], *trucks.last().unwrap()] {
        let before = cells(sim);
        let origin = sim
            .substrate
            .entities
            .get(id)
            .map(|e| (e.position.rx, e.position.ry))
            .unwrap();
        let event = EntityDamageEvent::direct_receiver(
            id,
            100_000,
            0,
            RAD_NO_ATTACKER,
            None,
            warhead,
            ReceiverCallFlags {
                ignore_defenses: false,
                arg6: false,
            },
        );
        sim.commit_noncombat_aoe_hits(rules, Some(registry), &[event], FrameEffects::default());
        let after = cells(sim);
        assert_eq!(after.len(), before.len() + 1, "truck {id} at {origin:?}");
        let dropped = after.iter().find(|cell| !before.contains(cell)).unwrap();
        let overlay = sim
            .overlay_grid
            .as_ref()
            .unwrap()
            .cell(dropped.0 as u16, dropped.1 as u16);
        assert!(
            registry
                .flags(overlay.overlay_id.unwrap())
                .unwrap()
                .crate_type
        );
        assert_eq!(overlay.overlay_data, u8::MAX);
        eprintln!("PASS {map}: truck {id} at {origin:?} dropped a random crate at {dropped:?}");
    }
}

#[test]
fn truck_crate_old_snapshot_is_rejected_before_session_decode() {
    use crate::sim::snapshot::{GameSnapshot, SnapshotError};
    let sim = Simulation::new();
    let mut bytes = GameSnapshot::save(&sim, 1, 2, "trucks.map", 0);
    bytes[12..16].copy_from_slice(&328u32.to_le_bytes());
    assert!(matches!(
        GameSnapshot::load(&bytes),
        Err(SnapshotError::VersionMismatch {
            expected: 334,
            found: 328
        })
    ));
}
