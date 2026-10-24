use super::*;
use crate::sim::command::Command;
fn fixture(elite: bool) -> (Simulation, &'static RuleSet, u64, u64) {
    static RULES: std::sync::OnceLock<RuleSet> = std::sync::OnceLock::new();
    let rules = RULES.get_or_init(|| {
        let (ini, art) = crate::rules::retail_ini_fixture::retail_rules_and_art()
            .expect("Boris tests require retail INIs");
        let mut rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
        rules.install_art_data(crate::rules::art_data::ArtRegistry::from_ini(&art));
        rules.bind_animation_sequences(
            &crate::rules::infantry_sequence::parse_infantry_sequence_registry(&art),
        );
        rules
    });
    let (sim, source, target) = arena(rules, elite);
    (sim, rules, source, target)
}
fn arena(rules: &RuleSet, elite: bool) -> (Simulation, u64, u64) {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 41_3860);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    sim.playfield_bounds =
        Some(crate::map::playfield::PlayfieldBounds::from_normalized_local_size(20, 2, 2, 16, 12));
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    for house in sim.houses.values_mut() {
        house.is_human = true;
    }
    sim.spawn_object("GAPOWR", "Americans", 16, 16, 0, rules)
        .unwrap();
    sim.spawn_object("GAPOWR", "Russians", 20, 18, 0, rules)
        .unwrap();
    let source = sim
        .spawn_object("BORIS", "Americans", 10, 12, 64, rules)
        .unwrap();
    let target = sim
        .spawn_object("NAREFN", "Russians", 17, 12, 0, rules)
        .unwrap();
    if elite {
        crate::sim::combat::veterancy::set_elite(sim.substrate.entities.get_mut(source).unwrap());
    }
    (sim, source, target)
}
fn tick(sim: &mut Simulation, rules: &RuleSet) {
    let path = sim.path_grid_snapshot().unwrap();
    sim.advance_tick(&[], Some(rules), Some(&path), None, 33);
}
fn shot(sim: &mut Simulation, rules: &RuleSet, source: u64, target: u64) -> Vec<u64> {
    assert!(sim.apply_command(
        "Americans",
        &Command::Attack {
            attacker_id: source,
            target_id: target
        },
        Some(rules)
    ));
    for _ in 0..500 {
        tick(sim, rules);
        let m = sim
            .entities()
            .get(source)
            .unwrap()
            .airstrike
            .manager
            .as_ref()
            .unwrap();
        if m.target == Some(target) && !m.planes.is_empty() {
            return m.planes.clone();
        }
    }
    panic!(
        "no bombers: state={:?}, mission={:?}",
        sim.entities().get(source).unwrap().airstrike,
        sim.entities().get(source).unwrap().mission
    );
}
#[test]
fn original_admission_and_tint_controls_match() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/spatial_oracle/airstrike.json")).unwrap();
    for row in corpus["admission"].as_array().unwrap() {
        let m = AirstrikeManager {
            active: row["active"].as_bool().unwrap(),
            target: row["same"].as_bool().unwrap().then_some(123),
            last_finished: 50,
            ..Default::default()
        };
        assert_eq!(
            m.can_fire(
                123,
                if row["elite"].as_bool().unwrap() {
                    50
                } else {
                    100
                },
                row["frame"].as_i64().unwrap() as i32
            ),
            row["admitted"].as_bool().unwrap(),
            "{row}"
        );
    }
    for row in corpus["tint"].as_array().unwrap() {
        let mut tint = AirstrikeTint {
            stage: row["stage"].as_u64().unwrap() as u8,
            timer: CdTimer::started(1000, row["lifetime"].as_i64().unwrap() as i32),
            stage_timer: CdTimer::started(1000, row["left"].as_i64().unwrap() as i32),
        };
        let mut rng = SimRng::new(7);
        tint.update(1000, &mut rng);
        assert_eq!(
            (
                tint.stage as i64,
                tint.stage_timer.duration() as i64,
                tint.intensity(1000, 1000) as i64
            ),
            (
                row["new_stage"].as_i64().unwrap(),
                row["duration"].as_i64().unwrap(),
                row["intensity"].as_i64().unwrap()
            ),
            "{row}"
        );
        let mut expected = SimRng::new(7);
        for _ in row["draws"].as_array().unwrap() {
            expected.next_range_i32_inclusive(-5, 5);
        }
        assert_eq!(format!("{rng:?}"), format!("{expected:?}"));
    }
}
#[test]
fn retail_normal_and_elite_airstrikes_deploy_and_damage_the_target() {
    for elite in [false, true] {
        let (mut sim, rules, source, target) = fixture(elite);
        let health = sim.entities().get(target).unwrap().health.current;
        let planes = shot(&mut sim, rules, source, target);
        assert_eq!(planes.len(), if elite { 4 } else { 2 });
        for plane in &planes {
            let e = sim.entities().get(*plane).unwrap();
            assert_eq!(sim.interner.resolve(e.type_ref()), "BPLN");
            assert_eq!(e.airstrike.controller, Some(source));
            assert!(
                e.mission_leaf
                    .as_aircraft()
                    .unwrap()
                    .airstrike_manager_present()
            );
        }
        assert_eq!(
            sim.entities().get(target).unwrap().airstrike.controller,
            Some(source)
        );
        for _ in 0..2000 {
            tick(&mut sim, rules);
            if sim
                .entities()
                .get(target)
                .is_none_or(|e| e.health.current < health)
            {
                break;
            }
        }
        assert!(
            sim.entities()
                .get(target)
                .is_none_or(|e| e.health.current < health),
            "bombers never damaged target; planes={:?}",
            planes
                .iter()
                .map(|id| sim.entities().get(*id).map(|e| (
                    &e.position,
                    &e.mission,
                    &e.aircraft_ammo,
                    &e.attack_target
                )))
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn stop_cancels_designation_and_planes_retreat() {
    let (mut sim, rules, source, target) = fixture(false);
    let planes = shot(&mut sim, rules, source, target);
    assert!(sim.apply_command(
        "Americans",
        &Command::Stop { entity_id: source },
        Some(rules)
    ));
    assert_eq!(
        sim.entities()
            .get(source)
            .unwrap()
            .airstrike
            .manager
            .as_ref()
            .unwrap()
            .target,
        None
    );
    assert_eq!(
        sim.entities().get(target).unwrap().airstrike.controller,
        None
    );
    for _ in 0..2000 {
        tick(&mut sim, rules);
        if planes.iter().all(|id| !sim.entities().contains(*id)) {
            break;
        }
    }
    assert!(
        planes.iter().all(|id| !sim.entities().contains(*id)),
        "canceled bombers remained on map"
    );
    let m = sim
        .entities()
        .get(source)
        .unwrap()
        .airstrike
        .manager
        .as_ref()
        .unwrap();
    assert!(!m.active);
    assert!(m.last_finished > 0);
}
#[test]
fn saved_airstrike_restores_and_cleans_up_after_boris_dies() {
    let (mut sim, rules, source, target) = fixture(false);
    let planes = shot(&mut sim, rules, source, target);
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "airstrike", 0);
    let mut loaded = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    loaded.restore_after_snapshot_load().unwrap();
    loaded.resolve_type_handles(rules);
    loaded.install_resolved_terrain_for_new_map(sim.resolved_terrain.as_ref().unwrap().clone());
    assert!(loaded.rebuild_dynamic_navigation(rules));
    assert_eq!(
        loaded.entities().get(source).unwrap().airstrike,
        sim.entities().get(source).unwrap().airstrike
    );
    let hash = loaded.state_hash();
    loaded
        .substrate
        .entities
        .get_mut(source)
        .unwrap()
        .airstrike
        .manager
        .as_mut()
        .unwrap()
        .last_finished += 1;
    assert_ne!(hash, loaded.state_hash());
    loaded.uninit_with_context(source, crate::sim::world::UninitContext::with_rules(rules));
    loaded.flush_pending_delete();
    assert_eq!(
        loaded.entities().get(target).unwrap().airstrike.controller,
        None
    );
    for id in &planes {
        let e = loaded.entities().get(*id).unwrap();
        assert_eq!(e.airstrike.controller, None);
        assert!(
            e.attack_target.is_none(),
            "owner deletion must abort an armed plane"
        );
    }
    for _ in 0..2000 {
        tick(&mut loaded, rules);
        if planes.iter().all(|id| !loaded.entities().contains(*id)) {
            break;
        }
    }
    assert!(planes.iter().all(|id| !loaded.entities().contains(*id)));
    let mut invalid = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    invalid
        .substrate
        .entities
        .get_mut(source)
        .unwrap()
        .airstrike
        .manager
        .as_mut()
        .unwrap()
        .planes
        .push(u64::MAX - 1);
    assert!(invalid.restore_after_snapshot_load().is_err());
}

#[test]
fn last_destroyed_bomber_starts_recharge_and_target_death_aborts_the_next_run() {
    let (mut sim, rules, source, target) = fixture(false);
    let planes = shot(&mut sim, rules, source, target);
    for (index, id) in planes.iter().enumerate() {
        sim.uninit_with_context(*id, crate::sim::world::UninitContext::with_rules(rules));
        sim.flush_pending_delete();
        let m = sim
            .entities()
            .get(source)
            .unwrap()
            .airstrike
            .manager
            .as_ref()
            .unwrap();
        assert_eq!(m.active, index + 1 < planes.len());
    }
    sim.airstrike_detonation(source, Some(target), rules, None, FrameEffects::default());
    assert!(
        sim.entities()
            .get(source)
            .unwrap()
            .airstrike
            .manager
            .as_ref()
            .unwrap()
            .planes
            .is_empty()
    );
    sim.session.binary_frame += 100;
    sim.airstrike_detonation(source, Some(target), rules, None, FrameEffects::default());
    let m = sim
        .entities()
        .get(source)
        .unwrap()
        .airstrike
        .manager
        .as_ref()
        .unwrap();
    assert_eq!(m.planes.len(), 2);
    let planes = m.planes.clone();
    sim.uninit_with_context(target, crate::sim::world::UninitContext::with_rules(rules));
    sim.flush_pending_delete();
    assert_eq!(
        sim.entities()
            .get(source)
            .unwrap()
            .airstrike
            .manager
            .as_ref()
            .unwrap()
            .target,
        None
    );
    for id in planes {
        assert!(sim.entities().get(id).unwrap().attack_target.is_none());
    }
}

#[test]
fn retargeting_reuses_surviving_armed_planes_and_moves_the_designation() {
    let (mut sim, rules, source, target) = fixture(false);
    let planes = shot(&mut sim, rules, source, target);
    let other = sim
        .spawn_object("GAPOWR", "Russians", 19, 8, 0, rules)
        .unwrap();
    let retargeted = shot(&mut sim, rules, source, other);
    // Changing Boris's target first cancels the flight. A plane still at
    // the edge can leave before the next flare; only survivors can retarget.
    assert!(!retargeted.is_empty());
    assert!(retargeted.iter().all(|id| planes.contains(id)));
    assert!(planes.iter().filter(|id| !retargeted.contains(id)).all(|id| !sim.entities().contains(*id)));
    assert_eq!(
        sim.entities().get(target).unwrap().airstrike.controller,
        None
    );
    assert_eq!(
        sim.entities().get(other).unwrap().airstrike.controller,
        Some(source)
    );
    for id in retargeted {
        assert_eq!(
            sim.entities()
                .get(id)
                .unwrap()
                .attack_target
                .as_ref()
                .unwrap()
                .target,
            TargetKind::Entity(other)
        );
    }
}
#[test]
fn a20_layered_rules_boris_calls_a_real_bombing_run() {
    let map = std::env::var("VERA20K_WOJTEK_A20").expect("required A20 map path");
    let retail = crate::rules::retail_ini_fixture::retail_battle_rules_for_map(&map)
        .expect("required A20 assets");
    let rules = retail.rules;
    let (mut sim, source, target) = arena(&rules, false);
    let health = sim.entities().get(target).unwrap().health.current;
    assert_eq!(shot(&mut sim, &rules, source, target).len(), 2);
    for _ in 0..1000 {
        tick(&mut sim, &rules);
        if sim
            .entities()
            .get(target)
            .is_none_or(|e| e.health.current < health)
        {
            break;
        }
    }
    assert!(
        sim.entities()
            .get(target)
            .is_none_or(|e| e.health.current < health)
    );
}
