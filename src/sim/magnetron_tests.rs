use super::*;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::command::Command;
fn fixture() -> (Simulation, &'static RuleSet, u64, u64) {
    static RULES: std::sync::OnceLock<RuleSet> = std::sync::OnceLock::new();
    let rules = RULES.get_or_init(|| {
        let (ini, art) = crate::rules::retail_ini_fixture::retail_rules_and_art()
            .expect("Magnetron tests require retail INIs");
        let mut rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
        rules.install_art_data(crate::rules::art_data::ArtRegistry::from_ini(&art));
        rules
    });
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 710000);
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
        .spawn_object("TELE", "Americans", 10, 12, 64, rules)
        .unwrap();
    let target = sim
        .spawn_object("MTNK", "Russians", 17, 12, 0, rules)
        .unwrap();
    let victim = sim.substrate.entities.get_mut(target).unwrap();
    victim.mission.write_dispatch_epilogue(0, 10000);
    victim.passive_scan_timer.arm(0, 10000);
    (sim, rules, source, target)
}
fn tick(sim: &mut Simulation, rules: &RuleSet) {
    let path = sim.path_grid_snapshot().unwrap();
    sim.advance_tick(&[], Some(rules), Some(&path), None, 33);
}
fn shot(sim: &mut Simulation, rules: &RuleSet, source: u64, target: u64) {
    assert!(sim.apply_command(
        "Americans",
        &Command::Attack {
            attacker_id: source,
            target_id: target
        },
        Some(rules)
    ));
    for _ in 0..150 {
        tick(sim, rules);
        if sim.entities().get(source).unwrap().magnetron.target == Some(target) {
            return;
        }
    }
    panic!(
        "no Magnetron lift: source={:?}, target={:?}",
        sim.entities().get(source),
        sim.entities().get(target)
    );
}
#[test]
fn retail_shot_lifts_and_stop_drops_restores_and_allows_movement() {
    let (mut sim, rules, source, target) = fixture();
    let old = sim
        .entities()
        .get(target)
        .unwrap()
        .locomotor
        .as_ref()
        .unwrap()
        .kind;
    shot(&mut sim, rules, source, target);
    assert_eq!(
        sim.entities()
            .get(target)
            .unwrap()
            .locomotor
            .as_ref()
            .unwrap()
            .kind,
        LocomotorKind::Jumpjet
    );
    assert!(
        sim.entities()
            .get(target)
            .unwrap()
            .foot_locomotor_swap_active
    );
    let start = sim.entities().get(target).unwrap().position.clone();
    for _ in 0..45 {
        tick(&mut sim, rules);
    }
    let e = sim.entities().get(target).unwrap();
    assert!(
        current_fly_height(e, sim.resolved_terrain.as_ref()) > 104,
        "lift did not rise: {e:?}"
    );
    assert_ne!(
        (
            e.position.rx,
            e.position.ry,
            e.position.sub_x,
            e.position.sub_y,
            e.position.exact_z_leptons
        ),
        (
            start.rx,
            start.ry,
            start.sub_x,
            start.sub_y,
            start.exact_z_leptons
        )
    );
    assert!(sim.apply_command(
        "Americans",
        &Command::Stop { entity_id: source },
        Some(rules)
    ));
    assert_eq!(sim.entities().get(source).unwrap().magnetron.target, None);
    assert!(sim.entities().get(target).unwrap().magnetron.dropped);
    for _ in 0..300 {
        tick(&mut sim, rules);
        if !sim
            .entities()
            .get(target)
            .unwrap()
            .foot_locomotor_swap_active
        {
            break;
        }
    }
    let e = sim.entities().get(target).unwrap();
    assert!(!e.foot_locomotor_swap_active, "fall stuck: {e:?}");
    assert!(!e.crashing);
    assert_eq!(e.locomotor.as_ref().unwrap().kind, old);
    assert_eq!(e.health.current, rules.object("MTNK").unwrap().strength);
    let start = (e.position.rx, e.position.ry);
    assert!(sim.apply_command(
        "Russians",
        &Command::Move {
            entity_id: target,
            target_rx: 18,
            target_ry: 14,
            queue: false
        },
        Some(rules)
    ));
    for _ in 0..120 {
        tick(&mut sim, rules);
    }
    let e = sim.entities().get(target).unwrap();
    assert_ne!(
        (e.position.rx, e.position.ry),
        start,
        "landed unit cannot move"
    );
}
#[test]
fn airborne_save_restores_links_hashes_and_continues_drop_after_source_destruction() {
    let (mut sim, rules, source, target) = fixture();
    shot(&mut sim, rules, source, target);
    for _ in 0..35 {
        tick(&mut sim, rules);
    }
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "lift", 0);
    let mut loaded = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    loaded.restore_after_snapshot_load().unwrap();
    loaded.resolve_type_handles(rules);
    loaded.install_resolved_terrain_for_new_map(sim.resolved_terrain.as_ref().unwrap().clone());
    assert!(loaded.rebuild_dynamic_navigation(rules));
    assert_eq!(
        loaded.entities().get(target).unwrap().magnetron.source,
        Some(source)
    );
    assert_eq!(
        loaded.entities().get(target).unwrap().locomotor,
        sim.entities().get(target).unwrap().locomotor
    );
    let hash = loaded.state_hash();
    loaded
        .substrate
        .entities
        .get_mut(target)
        .unwrap()
        .magnetron
        .source = None;
    assert_ne!(loaded.state_hash(), hash);
    loaded
        .substrate
        .entities
        .get_mut(target)
        .unwrap()
        .magnetron
        .source = Some(source);
    loaded.uninit_with_context(source, UninitContext::with_rules(rules));
    loaded.flush_pending_delete();
    assert_eq!(
        loaded.entities().get(target).unwrap().magnetron.source,
        None
    );
    for _ in 0..300 {
        tick(&mut loaded, rules);
        if !loaded
            .entities()
            .get(target)
            .unwrap()
            .foot_locomotor_swap_active
        {
            break;
        }
    }
    assert_eq!(
        loaded
            .entities()
            .get(target)
            .unwrap()
            .locomotor
            .as_ref()
            .unwrap()
            .kind,
        LocomotorKind::Drive
    );
    let mut invalid = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    invalid
        .substrate
        .entities
        .get_mut(target)
        .unwrap()
        .magnetron
        .source = Some(u64::MAX - 1);
    assert!(invalid.restore_after_snapshot_load().is_err());
}
#[test]
fn impact_crushes_infantry() {
    let (mut sim, rules, source, target) = fixture();
    shot(&mut sim, rules, source, target);
    for _ in 0..60 {
        tick(&mut sim, rules);
    }
    let (x, y) = {
        let e = sim.entities().get(target).unwrap();
        (e.position.rx, e.position.ry)
    };
    let infantry = sim.spawn_object("E1", "Russians", x, y, 0, rules).unwrap();
    sim.substrate
        .entities
        .get_mut(target)
        .unwrap()
        .magnetron
        .dropped = true;
    sim.substrate
        .entities
        .get_mut(target)
        .unwrap()
        .magnetron
        .dropper = Some(source);
    sim.magnetron_release(source, false, Some(rules), None, FrameEffects::default());
    sim.set_object_height(target, 0, Some(rules), None, FrameEffects::default());
    sim.magnetron_impact(target, rules, None, FrameEffects::default());
    assert!(
        sim.entities()
            .get(infantry)
            .is_none_or(|e| e.health.current <= 0 || e.dying)
    );
    assert!(sim.entities().get(target).unwrap().health.current > 0);
}

#[test]
fn admission_and_fall_match_original_executable_controls() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/spatial_oracle/magnetron.json")).unwrap();
    for row in data["admission"].as_array().unwrap() {
        let i = &row["input"];
        let b = |k: &str| i[k].as_bool().unwrap();
        let target = b("target").then_some(2);
        let held = if b("same") {
            Some(2)
        } else {
            b("previous").then_some(3)
        };
        let mut events = Vec::new();
        if source_admits(
            b("source"),
            target,
            held,
            b("source_held"),
            b("source_swapped"),
        ) {
            if held.is_some() {
                events.push("release");
            }
            if target.is_some()
                && receiver_admits(
                    b("foot"),
                    i["kind"].as_i64().unwrap() as i32,
                    i["layer"] == 2,
                    b("protected"),
                    b("swapped"),
                    b("factory"),
                    i["damage"].as_i64().unwrap() as i32,
                    i["weight"].as_f64().unwrap(),
                )
            {
                events.push("lift");
            }
        }
        assert_eq!(serde_json::json!(events), row["events"], "{i}");
    }
    for row in data["fall"].as_array().unwrap() {
        let (z, speed) = falling_step(
            row["z"].as_i64().unwrap() as i32,
            (row["speed"].as_f64().unwrap() as f32).to_bits(),
        );
        assert_eq!(z, row["new_z"].as_i64().unwrap() as i32, "{row}");
        assert_eq!(speed, row["next_crash"].as_u64().unwrap() as u32, "{row}");
    }
}

#[test]
fn water_drop_sinks_and_ground_drop_release_flags_match_original() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/spatial_oracle/magnetron.json")).unwrap();
    for row in data["release"].as_array().unwrap() {
        let (mut sim, rules, source, target) = fixture();
        let height = row["height"].as_i64().unwrap() as i32;
        sim.substrate
            .entities
            .get_mut(source)
            .unwrap()
            .magnetron
            .target = Some(target);
        let frame = sim.session.binary_frame;
        let e = sim.substrate.entities.get_mut(target).unwrap();
        e.magnetron.source = Some(source);
        e.foot_locomotor_swap_active = true;
        e.locomotor
            .as_mut()
            .unwrap()
            .begin_piggyback(LocomotorKind::Jumpjet, frame);
        sim.set_object_height(target, height, Some(rules), None, FrameEffects::default());
        sim.magnetron_release(
            source,
            row["clear"].as_bool().unwrap(),
            Some(rules),
            None,
            FrameEffects::default(),
        );
        let e = sim.entities().get(target).unwrap();
        assert_eq!(e.crashing, row["flags"][0] == 1, "{row}");
        assert_eq!(e.magnetron.dropped, row["flags"][2] == 1, "{row}");
        assert_eq!(e.magnetron.source, None);
        assert_eq!(sim.entities().get(source).unwrap().magnetron.target, None);
        assert_eq!(
            e.magnetron.dropper == Some(source),
            row["credit"][0].as_bool().unwrap()
        );
        if height <= 0 {
            assert!(e.magnetron.end_requested);
        }
    }
    let (mut sim, rules, source, target) = fixture();
    shot(&mut sim, rules, source, target);
    let (x, y) = {
        let e = sim.entities().get(target).unwrap();
        (e.position.rx, e.position.ry)
    };
    let cell = sim
        .resolved_terrain
        .as_mut()
        .unwrap()
        .cell_mut(x, y)
        .unwrap();
    cell.is_water = true;
    cell.yr_cell_land_type = 2;
    cell.speed_costs.track = None;
    sim.substrate
        .entities
        .get_mut(target)
        .unwrap()
        .magnetron
        .dropped = true;
    sim.substrate
        .entities
        .get_mut(target)
        .unwrap()
        .magnetron
        .dropper = Some(source);
    sim.magnetron_release(source, false, Some(rules), None, FrameEffects::default());
    sim.set_object_height(target, 0, Some(rules), None, FrameEffects::default());
    sim.magnetron_impact(target, rules, None, FrameEffects::default());
    assert!(sim.entities().get(target).unwrap().sinking.is_active());
}

#[test]
fn automatic_pull_moves_toward_firer_and_releases_on_arrival() {
    let (mut sim, rules, source, target) = fixture();
    shot(&mut sim, rules, source, target);
    let start = crate::sim::movement::ground_pose::position_world_coord(
        &sim.entities().get(target).unwrap().position,
    );
    let mut moved = false;
    let mut released = false;
    for _ in 0..900 {
        tick(&mut sim, rules);
        let Some(e) = sim.entities().get(target) else {
            break;
        };
        let now = crate::sim::movement::ground_pose::position_world_coord(&e.position);
        moved |= now.x < start.x - 256;
        released |= e.magnetron.source.is_none();
        if released && !e.foot_locomotor_swap_active {
            break;
        }
    }
    assert!(moved, "lift never translated toward the Magnetron");
    assert!(
        released,
        "arrival never released the victim: {:?}",
        sim.entities().get(target)
    );
    assert_eq!(sim.entities().get(source).unwrap().magnetron.target, None);
}
#[test]
fn moving_firer_and_retargeting_release_the_previous_lift() {
    for moving in [true, false] {
        let (mut sim, rules, source, target) = fixture();
        shot(&mut sim, rules, source, target);
        for _ in 0..35 {
            tick(&mut sim, rules);
        }
        let other = sim
            .spawn_object("MTNK", "Russians", 17, 15, 0, rules)
            .unwrap();
        let command = if moving {
            Command::Move {
                entity_id: source,
                target_rx: 12,
                target_ry: 14,
                queue: false,
            }
        } else {
            Command::Attack {
                attacker_id: source,
                target_id: other,
            }
        };
        assert!(sim.apply_command("Americans", &command, Some(rules)));
        assert_eq!(sim.entities().get(target).unwrap().magnetron.source, None);
        assert!(sim.entities().get(target).unwrap().magnetron.dropped);
    }
}

#[test]
fn dropped_vehicle_lands_on_bridge_deck() {
    let (mut sim, rules, source, target) = fixture();
    shot(&mut sim, rules, source, target);
    for _ in 0..20 {
        tick(&mut sim, rules);
    }
    let (x, y) = {
        let e = sim.entities().get(target).unwrap();
        (e.position.rx, e.position.ry)
    };
    let c = sim
        .resolved_terrain
        .as_mut()
        .unwrap()
        .cell_mut(x, y)
        .unwrap();
    c.has_bridge_deck = true;
    c.bridge_walkable = true;
    c.bridge_deck_level = c.level + 4;
    c.bridge_facts.raw_flags |= crate::map::bridge_facts::BRIDGE_FLAG_STRUCTURAL;
    sim.set_object_height(target, 750, Some(rules), None, FrameEffects::default());
    sim.magnetron_release(source, false, Some(rules), None, FrameEffects::default());
    for _ in 0..200 {
        tick(&mut sim, rules);
        if sim
            .entities()
            .get(target)
            .is_none_or(|e| !e.foot_locomotor_swap_active)
        {
            break;
        }
    }
    let e = sim
        .entities()
        .get(target)
        .expect("vehicle survives deck landing");
    assert!(e.on_bridge);
    assert_eq!(e.position.exact_z_leptons, Some(416));
    assert_eq!(e.locomotor.as_ref().unwrap().kind, LocomotorKind::Drive);
}

#[test]
fn jumpjet_end_gate_and_restoration_match_original() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/spatial_oracle/magnetron.json")).unwrap();
    for row in data["end"].as_array().unwrap() {
        let (mut sim, _rules, _source, target) = fixture();
        let e = sim.substrate.entities.get_mut(target).unwrap();
        let b = |k: &str| row[k].as_bool().unwrap();
        e.foot_locomotor_swap_active = b("swapped");
        e.magnetron.end_requested = b("end");
        let l = e.locomotor.as_mut().unwrap();
        l.begin_piggyback(LocomotorKind::Jumpjet, 0);
        if !b("stash") {
            l.piggyback = None;
        }
        let r = l.jumpjet_runtime_mut().unwrap();
        *r = r.clone().with_moving_for_test(b("moving"));
        let physical = crate::sim::movement::ground_pose::position_world_coord(&e.position);
        assert_eq!(
            crate::sim::movement::tick_locomotor_piggyback_restore_one(
                &mut sim.substrate.entities,
                target
            ),
            b("admitted"),
            "{row}"
        );
        let e = sim.entities().get(target).unwrap();
        assert_eq!(
            crate::sim::movement::ground_pose::position_world_coord(&e.position),
            physical
        );
        if b("admitted") {
            assert_eq!(e.locomotor.as_ref().unwrap().kind, LocomotorKind::Drive);
            assert!(!e.foot_locomotor_swap_active);
            assert_eq!(e.magnetron, Default::default());
        }
    }
}

#[test]
fn victim_deletion_clears_the_firer_link() {
    let (mut sim, rules, source, target) = fixture();
    shot(&mut sim, rules, source, target);
    sim.uninit_with_context(target, UninitContext::with_rules(rules));
    sim.flush_pending_delete();
    assert_eq!(sim.entities().get(source).unwrap().magnetron.target, None);
}
