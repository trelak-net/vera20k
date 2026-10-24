use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::{command::Command, snapshot::GameSnapshot, world::FrameEffects};

fn fixture() -> (Simulation, RuleSet, u64) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        r#"
[Countries]
0=Americans
1=Russians
[General]
SecretInfantry=REWARD,SECOND
SecretUnits=TANK
SecretBuildings=LAB
[InfantryTypes]
0=REWARD
1=SECOND
[VehicleTypes]
0=TANK
[BuildingTypes]
0=LAB
1=BARRACKS
[REWARD]
Strength=100
Speed=4
Cost=10
TechLevel=-1
Owner=Americans,Russians
RequiredHouses=Russians
RequiresStolenThirdTech=yes
BuildLimit=1
[SECOND]
Strength=100
[TANK]
Strength=100
[LAB]
Strength=1000
SecretLab=yes
Capturable=yes
SecretInfantry=REWARD
[BARRACKS]
Strength=1000
Factory=InfantryType
Owner=Americans,Russians
"#,
    ))
    .unwrap();
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 68_050);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    let lab = sim
        .spawn_object("LAB", "Russians", 15, 15, 0, &rules)
        .unwrap();
    sim.spawn_object("BARRACKS", "Americans", 10, 10, 0, &rules)
        .unwrap();
    (sim, rules, lab)
}
fn visible(sim: &Simulation, rules: &RuleSet, owner: &str, name: &str) -> bool {
    crate::sim::production::build_options_for_owner(sim, rules, owner)
        .iter()
        .any(|o| sim.interner.resolve(o.type_id).eq_ignore_ascii_case(name) && o.enabled)
}
#[test]
fn secret_lab_selection_matches_seventy_five_original_cases() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../tools/projectile_oracle/secret_labs.json"
    ))
    .unwrap();
    assert_eq!(data["selection"].as_array().unwrap().len(), 75);
    for row in data["selection"].as_array().unwrap() {
        let pool: Vec<i32> = serde_json::from_value(row["pool"].clone()).unwrap();
        let mut calls = Vec::new();
        let picked = select_rewards(&pool, row["labs"].as_u64().unwrap() as usize, |lo, hi| {
            let draw = match row["policy"].as_str().unwrap() {
                "first" => lo,
                "last" => hi,
                _ => (lo + hi) / 2,
            };
            calls.push([lo, hi, draw]);
            draw
        });
        let mut expected: Vec<i32> = serde_json::from_value(row["rewards"].clone()).unwrap();
        if picked.is_empty() {
            assert!(expected.iter().all(|v| *v == 0));
            expected.clear();
        }
        assert_eq!(picked, expected, "{row}");
        assert_eq!(serde_json::json!(calls), row["calls"]);
    }
}
#[test]
fn secret_lab_fixed_priority_matches_sixteen_original_cases() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../tools/projectile_oracle/secret_labs.json"
    ))
    .unwrap();
    let (mut sim, rules, lab) = fixture();
    let mut ty = rules.object("LAB").unwrap().clone();
    let ids = ["one", "two", "three", "four"].map(|id| sim.interner.intern(id));
    for row in data["getter"].as_array().unwrap() {
        let values: Vec<u32> = serde_json::from_value(row["values"].clone()).unwrap();
        ty.secret_lab_fixed = std::array::from_fn(|i| {
            (values[i] != 0).then(|| sim.interner.resolve(ids[i]).to_owned())
        });
        sim.substrate
            .entities
            .get_mut(lab)
            .unwrap()
            .secret_lab_reward = (values[3] != 0).then_some(SecretLabReward {
            category: ObjectCategory::Infantry,
            type_id: ids[3],
        });
        let picked = sim.secret_production(lab, &ty).map_or(0, |r| {
            ids.iter().position(|&id| id == r.type_id).unwrap() as u32 + 1
        });
        assert_eq!(serde_json::json!(picked), row["reward"]);
    }
}
#[test]
fn capture_loss_destruction_build_limits_and_save_keep_the_reward_consistent() {
    let (mut sim, rules, lab) = fixture();
    let owner = sim.interner.get("Americans").unwrap();
    let other = sim.interner.get("Russians").unwrap();
    let before = sim.state_hash();
    let mut expected_rng = sim.scenario_rng.clone();
    expected_rng.next_range_u32_inclusive(0, 3);
    sim.initialize_secret_labs(&rules);
    assert_eq!(sim.scenario_rng.state(), expected_rng.state());
    assert_ne!(sim.state_hash(), before);
    let fallback = sim.entities().get(lab).unwrap().secret_lab_reward;
    assert!(fallback.is_some());
    assert!(!visible(&sim, &rules, "Americans", "REWARD"));
    sim.change_owner_with_rules(lab, owner, &rules, None, FrameEffects::default());
    assert!(visible(&sim, &rules, "Americans", "REWARD"));
    let reward = sim
        .spawn_object("REWARD", "Americans", 12, 12, 0, &rules)
        .unwrap();
    assert!(
        !visible(&sim, &rules, "Americans", "REWARD"),
        "lab must not bypass BuildLimit"
    );
    sim.uninit_with_rules(reward, &rules);
    sim.flush_pending_delete();
    assert!(visible(&sim, &rules, "Americans", "REWARD"));
    sim.change_owner_with_rules(lab, other, &rules, None, FrameEffects::default());
    assert!(!visible(&sim, &rules, "Americans", "REWARD"));
    sim.change_owner_with_rules(lab, owner, &rules, None, FrameEffects::default());
    assert_eq!(sim.entities().get(lab).unwrap().secret_lab_reward, fallback);
    // Native Scenario load reseeds RNG to zero, but does not reselect labs.
    let rng = crate::sim::rng::SimRng::new(0).state();
    let bytes = GameSnapshot::save(&sim, 0, 0, "secret", 0);
    let mut restored = GameSnapshot::load(&bytes).unwrap().sim;
    restored.restore_after_snapshot_load().unwrap();
    restored.resolve_type_handles(&rules);
    assert_eq!(restored.scenario_rng.state(), rng);
    assert_eq!(
        restored.entities().get(lab).unwrap().secret_lab_reward,
        fallback
    );
    assert!(visible(&restored, &rules, "Americans", "REWARD"));
    restored.uninit_with_rules(lab, &rules);
    restored.flush_pending_delete();
    assert!(!visible(&restored, &rules, "Americans", "REWARD"));
}
#[test]
fn random_labs_use_all_three_typed_pools_and_do_not_draw_if_undersupplied() {
    let (mut sim, mut rules, lab) = fixture();
    // A rules fixture clone can remove its fixed pointer without changing the
    // derived set: every random reward remains in the original pools.
    let mut ty = rules.object("LAB").unwrap().clone();
    ty.secret_lab_fixed = Default::default();
    sim.initialize_secret_labs(&rules);
    assert_eq!(
        sim.secret_production(lab, &ty),
        sim.entities().get(lab).unwrap().secret_lab_reward
    );
    let rng = sim.scenario_rng.state();
    rules.general.secret_lab_pools = Default::default();
    sim.initialize_secret_labs(&rules);
    assert_eq!(sim.scenario_rng.state(), rng);
}
#[test]
fn retail_a20_engineer_capture_unlocks_desolator_production_and_loss_revokes_it() {
    let Ok(map) = std::env::var("VERA20K_WOJTEK_A20") else {
        assert!(
            std::env::var_os("VERA20K_REQUIRE_RETAIL_ASSETS").is_none(),
            "required A20 map path missing"
        );
        return;
    };
    let retail = crate::rules::retail_ini_fixture::retail_battle_rules_for_map(&map)
        .expect("required retail A20 rules");
    let rules = retail.rules;
    assert!(rules.object("NAPSYA").unwrap().secret_lab);
    assert_eq!(
        rules.object("NAPSYA").unwrap().secret_lab_fixed[0].as_deref(),
        Some("DESO")
    );
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 68_050);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    let owner = sim.interner.get("Americans").unwrap();
    let other = sim.interner.get("Russians").unwrap();
    sim.session.current_house = Some(owner);
    sim.houses
        .get_mut(&owner)
        .unwrap()
        .economy
        .set_credits_for_test(50000);
    // The lab bypasses country/tech/prerequisites, but original FindFactory
    // still requires a compatible factory. A captured Soviet barracks supplies it.
    sim.spawn_object("NAHAND", "Americans", 10, 12, 0, &rules)
        .unwrap();
    sim.spawn_object("GAPOWR", "Americans", 6, 17, 0, &rules)
        .unwrap();
    let lab = sim
        .spawn_object("NAPSYA", "Russians", 17, 17, 0, &rules)
        .unwrap();
    sim.spawn_object("NAPOWR", "Russians", 26, 26, 0, &rules)
        .unwrap();
    let engineer = sim
        .spawn_object("ENGINEER", "Americans", 16, 17, 0, &rules)
        .unwrap();
    sim.initialize_secret_labs(&rules);
    assert!(!visible(&sim, &rules, "Americans", "DESO"));
    sim.fog.reveal_all_for_owner(owner);
    assert!(sim.apply_command(
        "Americans",
        &Command::CaptureBuilding {
            engineer_id: engineer,
            target_building_id: lab
        },
        Some(&rules)
    ));
    sim.rebuild_dynamic_navigation(&rules);
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..900 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
        if sim.entities().get(lab).unwrap().owner() == owner {
            break;
        }
    }
    assert_eq!(
        sim.entities().get(lab).unwrap().owner(),
        owner,
        "engineer must actually capture"
    );
    assert!(
        visible(&sim, &rules, "Americans", "DESO"),
        "captured lab unlocks real sidebar"
    );
    let type_id = sim.interner.get("DESO").unwrap();
    assert!(sim.apply_command(
        "Americans",
        &Command::QueueProduction { type_id },
        Some(&rules)
    ));
    let mut delivered = false;
    eprintln!(
        "queued {:?}",
        crate::sim::production::queue_view_for_owner(&sim, &rules, "Americans")
    );
    for _ in 0..6000 {
        crate::sim::production::publish_production_changes(&mut sim, &rules);
        let due = sim.take_due_commands();
        sim.advance_tick(&due, Some(&rules), Some(&grid), None, 33);
        delivered = sim
            .entities()
            .values()
            .any(|e| e.owner() == owner && e.type_ref() == type_id && !e.lifecycle.in_limbo);
        if delivered {
            break;
        }
        if crate::sim::production::queue_view_for_owner(&sim, &rules, "Americans").is_empty() {
            eprintln!(
                "frame {} defeated={} grants={} labs={:?} entities={:?}",
                sim.session.binary_frame,
                sim.houses[&owner].is_defeated,
                visible(&sim, &rules, "Americans", "DESO"),
                sim.houses[&owner].base_projection.buildings(),
                sim.entities()
                    .values()
                    .map(|e| (
                        e.stable_id(),
                        sim.interner.resolve(e.type_ref()),
                        sim.interner.resolve(e.owner()),
                        e.lifecycle.in_limbo,
                        e.health.current
                    ))
                    .collect::<Vec<_>>()
            );
            break;
        }
    }
    assert!(
        delivered,
        "Desolator must leave production: queue={:?}, commands={:?}",
        crate::sim::production::queue_view_for_owner(&sim, &rules, "Americans"),
        sim.pending_command_snapshot()
    );
    sim.change_owner_with_rules(lab, other, &rules, None, FrameEffects::default());
    assert!(!visible(&sim, &rules, "Americans", "DESO"));
}

#[test]
fn secret_lab_rules_keep_order_duplicates_and_typed_none_across_passes() {
    use crate::rules::native_processing::{RulesLayerKind, RulesLayerStack};
    let mut stack = RulesLayerStack::new(IniFile::from_str(
        "[General]\nSecretInfantry=A,A,B\nSecretUnits=U\nSecretBuildings=T\n[BuildingTypes]\n0=LAB\n[LAB]\nSecretLab=yes\nSecretInfantry=A\nSecretUnit=U\n",
    ));
    stack.push(
        RulesLayerKind::GameMode,
        IniFile::from_str("[General]\nSecretInfantry=\nSecretUnits=V,U\n[LAB]\nSecretInfantry=\n"),
    );
    let processed = stack.process().unwrap();
    let rules = RuleSet::from_processed_rules(&processed).unwrap();
    assert_eq!(
        rules.general.secret_lab_pools,
        [vec!["A", "A", "B"], vec!["V", "U"], vec!["T"]]
    );
    assert_eq!(
        rules.object("LAB").unwrap().secret_lab_fixed[0].as_deref(),
        Some("A")
    );
    stack.push(
        RulesLayerKind::Scenario,
        IniFile::from_str("[General]\nSecretInfantry=<none>\n[LAB]\nSecretInfantry=none\n"),
    );
    let processed = stack.process().unwrap();
    let rules = RuleSet::from_processed_rules(&processed).unwrap();
    assert!(rules.general.secret_lab_pools[0].is_empty());
    assert_eq!(rules.object("LAB").unwrap().secret_lab_fixed[0], None);
    assert_eq!(
        rules.object("LAB").unwrap().secret_lab_fixed[1].as_deref(),
        Some("U")
    );
}
