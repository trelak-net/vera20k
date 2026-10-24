use super::*;
use crate::sim::command::Command;

#[test]
fn stages_roll_effect_positions_and_draw_order_match_1550_original_frames() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/squid_grapple.json"
    ))
    .unwrap();
    let mut count = 0;
    for timeline in data["timelines"].as_array().unwrap() {
        let facing = timeline["facing"].as_u64().unwrap() as u16;
        let last = timeline["policy"] == "last";
        let mut state = GrappleState::default();
        let mut anim_frame = 0;
        for row in timeline["steps"].as_array().unwrap() {
            let mut draws = Vec::new();
            let mut draw = |lo, hi| {
                let v = if last { hi } else { lo };
                draws.push(vec![lo, hi, v]);
                v
            };
            let mut effects = Vec::new();
            let mut hit = Vec::new();
            let mut wakes = false;
            let coord = DriveCoord {
                x: 5248,
                y: 5760,
                z: 0,
            };
            match state.phase {
                0 => {
                    state.enter(1, facing);
                    effects.push(serde_json::json!([0, [5248, 5760, 0], 0, 1, 1536, 0, 0]));
                }
                1 => {
                    let (done, frame) = state.step(facing);
                    if let Some(f) = frame {
                        anim_frame = f;
                    }
                    if done {
                        state.restart_rock(facing, draw(0, 1));
                        wakes = true;
                    }
                }
                2 | 3 => {
                    let phase = state.phase;
                    let (done, frame) = state.step(facing);
                    if let Some(f) = frame {
                        anim_frame = f;
                    }
                    if done {
                        state.enter(4, facing);
                    }
                    state.rock(phase == 2);
                }
                4 => {
                    state.roll = 0;
                    for i in 0..3 {
                        let y = if draw(0, 1) == 0 { 64 } else { -64 };
                        let p = displaced(coord, facing, i * 128 - 128, y);
                        let typ = (draw(0, 1) + 1) * 4096;
                        effects.push(serde_json::json!([
                            typ,
                            [p.x, p.y, p.z],
                            2,
                            1,
                            1536,
                            -10,
                            0
                        ]));
                    }
                    hit.push(15);
                    state.restart_rock(facing, draw(0, 1));
                    wakes = true;
                }
                _ => unreachable!(),
            }
            if wakes {
                for _ in 0..3 {
                    let p = displaced(coord, facing, draw(-180, 180), draw(-64, 64));
                    effects.push(serde_json::json!([
                        12288,
                        [p.x, p.y, p.z],
                        0,
                        1,
                        1536,
                        0,
                        0
                    ]));
                }
            }
            let frame = row["frame"].as_u64().unwrap();
            let actual = serde_json::json!({"frame":frame,"state":[state.phase,state.stage,state.accumulator],"rock":i64::from(state.roll as i32),"anim_frame":anim_frame,"paralysis":[frame,32767],"draws":draws,"fx":effects,"hits":hit});
            assert_eq!(actual, *row, "facing={facing} last={last}");
            count += 1;
        }
    }
    assert_eq!(count, 1550);
}
fn fixture() -> (Simulation, &'static RuleSet, u64, u64) {
    static RULES: std::sync::OnceLock<RuleSet> = std::sync::OnceLock::new();
    let rules = RULES.get_or_init(|| {
        let (ini, art) = crate::rules::retail_ini_fixture::retail_rules_and_art()
            .expect("Squid regressions require retail INIs");
        let mut rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
        rules.install_art_data(crate::rules::art_data::ArtRegistry::from_ini(&art));
        let (_, assets) = crate::rules::retail_ini_fixture::retail_assets()
            .expect("Squid regressions require retail assets");
        let roots = crate::rules::effect_asset_catalog::anim_class_roots(&rules);
        assert!(roots.iter().any(|r| r == "SQDG"));
        rules.bind_anim_class_assets(&roots, &assets, "TEM", "TEMPERATE");
        assert!(
            rules
                .art()
                .anim_runtime_config("SQDG")
                .unwrap()
                .raw_shp_frame_count
                .is_some()
        );
        rules
    });
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 6297);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    sim.session.current_house = sim.interner.get("Americans");
    for house in sim.houses.values_mut() {
        house.is_human = true;
    }
    use crate::rules::terrain_rules::{LandType, SpeedCostProfile, TerrainClass};
    let costs = SpeedCostProfile {
        float: Some(100),
        hover: Some(100),
        ..Default::default()
    };
    let water = LandType::Water.as_index();
    let cells = (0..32)
        .flat_map(|y| {
            (0..32).map(move |x| crate::map::resolved_terrain::ResolvedTerrainCell {
                land_type: water,
                yr_cell_land_type: water,
                base_land_type: water,
                base_yr_cell_land_type: water,
                terrain_class: TerrainClass::Water,
                base_terrain_class: TerrainClass::Water,
                speed_costs: costs,
                base_speed_costs: costs,
                is_water: true,
                zone_type: 4,
                ..crate::map::resolved_terrain::test_flat_cell(x, y)
            })
        })
        .collect();
    let mut grid = crate::map::resolved_terrain::ResolvedTerrainGrid::from_cells(32, 32, cells);
    grid.set_projectile_water_set_base(0);
    sim.install_resolved_terrain_for_new_map(grid);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    assert!(sim.rebuild_dynamic_navigation(&rules));
    sim.spawn_object("GAPOWR", "Americans", 16, 16, 0, &rules)
        .unwrap();
    sim.spawn_object("GAPOWR", "Russians", 20, 18, 0, &rules)
        .unwrap();
    let squid = sim
        .spawn_object("SQD", "Americans", 10, 12, 0, &rules)
        .unwrap();
    let ship = sim
        .spawn_object("AEGIS", "Russians", 11, 12, 0, &rules)
        .unwrap();
    (sim, rules, squid, ship)
}
fn tick(sim: &mut Simulation, rules: &RuleSet) {
    let grid = sim.path_grid_snapshot().unwrap();
    sim.advance_tick(&[], Some(rules), Some(&grid), None, 33);
}
fn attach(sim: &mut Simulation, rules: &RuleSet, squid: u64, ship: u64) {
    assert!(sim.apply_command(
        "Americans",
        &Command::Attack {
            attacker_id: squid,
            target_id: ship
        },
        Some(rules)
    ));
    for _ in 0..400 {
        tick(sim, rules);
        if sim.entities().get(ship).unwrap().parasite_eating_me == Some(squid) {
            return;
        }
    }
    panic!("Squid never attached: {:?}", sim.entities().get(squid));
}
#[test]
fn retail_shot_hides_squid_paralyzes_rocks_and_damages_ship_every_forty_frames() {
    let (mut sim, rules, squid, ship) = fixture();
    attach(&mut sim, &rules, squid, ship);
    assert!(sim.entities().get(squid).unwrap().lifecycle.in_limbo);
    let mut last = sim.entities().get(ship).unwrap().health.current;
    let mut hits = Vec::new();
    let mut rocked = false;
    for _ in 0..170 {
        tick(&mut sim, &rules);
        let victim = sim.entities().get(ship).unwrap();
        assert!(victim.is_paralyzed(sim.session.binary_frame));
        rocked |= victim.rocking.is_some_and(|r| {
            r.angle_sideways.abs() > crate::util::fixed_math::SimFixed::lit("0.2")
        });
        if victim.health.current != last {
            hits.push(sim.session.binary_frame);
            last = victim.health.current;
        }
    }
    assert!(rocked);
    assert_eq!(hits.len(), 3, "{hits:?}");
    assert!(hits.windows(2).all(|w| w[1] - w[0] == 40), "{hits:?}");
    let before = ground_pose::position_world_coord(&sim.entities().get(ship).unwrap().position);
    assert!(sim.apply_command(
        "Russians",
        &Command::Move {
            entity_id: ship,
            target_rx: 18,
            target_ry: 12,
            queue: false
        },
        Some(&rules)
    ));
    for _ in 0..15 {
        tick(&mut sim, &rules);
    }
    assert_eq!(
        ground_pose::position_world_coord(&sim.entities().get(ship).unwrap().position),
        before
    );
}
#[test]
fn sonic_release_and_culling_clear_the_entire_grapple() {
    for cull in [false, true] {
        let (mut sim, rules, squid, ship) = fixture();
        attach(&mut sim, &rules, squid, ship);
        for _ in 0..45 {
            tick(&mut sim, &rules);
        }
        let anim = sim
            .entities()
            .get(squid)
            .unwrap()
            .parasite
            .as_ref()
            .unwrap()
            .grapple
            .anim
            .unwrap();
        if cull {
            sim.substrate.entities.get_mut(ship).unwrap().health.current = 10;
            for _ in 0..50 {
                tick(&mut sim, &rules);
                if sim.entities().get(ship).unwrap().sinking.is_active() {
                    break;
                }
            }
            assert!(sim.entities().get(ship).unwrap().sinking.is_active());
        } else {
            let warhead = sim.interner.intern("SonicWarhead");
            sim.foot_receive_damage_parasite_prefix(
                ship,
                None,
                1,
                warhead,
                &rules,
                FrameEffects::default(),
            );
        }
        let eater = sim.entities().get(squid).unwrap();
        assert!(!eater.lifecycle.in_limbo);
        assert_eq!(eater.parasite.as_ref().unwrap().victim, None);
        assert_eq!(
            eater.parasite.as_ref().unwrap().grapple,
            GrappleState::default()
        );
        let victim = sim.entities().get(ship).unwrap();
        assert_eq!(victim.parasite_eating_me, None);
        assert!(!victim.is_paralyzed(sim.session.binary_frame));
        assert!(victim.rocking.is_none());
        sim.flush_pending_delete();
        assert!(sim.anim(anim).is_none());
    }
}
#[test]
fn grapple_snapshot_preserves_phase_and_references_and_resumes_damage() {
    let (mut sim, rules, squid, ship) = fixture();
    attach(&mut sim, &rules, squid, ship);
    for _ in 0..47 {
        tick(&mut sim, &rules);
    }
    let state = sim
        .entities()
        .get(squid)
        .unwrap()
        .parasite
        .as_ref()
        .unwrap()
        .grapple
        .clone();
    let hp = sim.entities().get(ship).unwrap().health.current;
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "squid", 0);
    let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    restored.restore_after_snapshot_load().unwrap();
    restored.resolve_type_handles(&rules);
    restored.install_resolved_terrain_for_new_map(sim.resolved_terrain.as_ref().unwrap().clone());
    assert!(restored.rebuild_dynamic_navigation(&rules));
    assert_eq!(
        restored
            .entities()
            .get(squid)
            .unwrap()
            .parasite
            .as_ref()
            .unwrap()
            .grapple,
        state
    );
    assert_eq!(
        restored.anim(state.anim.unwrap()).unwrap().owner_entity,
        Some(ship)
    );
    let hash = restored.state_hash();
    restored
        .substrate
        .entities
        .get_mut(squid)
        .unwrap()
        .parasite
        .as_mut()
        .unwrap()
        .grapple
        .stage += 1;
    assert_ne!(hash, restored.state_hash());
    restored
        .substrate
        .entities
        .get_mut(squid)
        .unwrap()
        .parasite
        .as_mut()
        .unwrap()
        .grapple = state.clone();
    for _ in 0..40 {
        tick(&mut restored, &rules);
    }
    assert!(restored.entities().get(ship).unwrap().health.current < hp);
    let mut invalid = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    invalid
        .substrate
        .entities
        .get_mut(squid)
        .unwrap()
        .parasite
        .as_mut()
        .unwrap()
        .grapple
        .anim = Some(u64::MAX - 1);
    assert!(invalid.restore_after_snapshot_load().is_err());
    // External animation expiry clears its identity before physical deletion.
    sim.destroy_anim(state.anim.unwrap(), &rules);
    sim.flush_pending_delete();
    assert_eq!(
        sim.entities()
            .get(squid)
            .unwrap()
            .parasite
            .as_ref()
            .unwrap()
            .grapple
            .anim,
        None
    );
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "squid", 0);
    crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim
        .restore_after_snapshot_load()
        .unwrap();
}

#[test]
fn host_death_and_eater_death_retire_grapple_effects_without_dangling_saves() {
    for kill_host in [false, true] {
        let (mut sim, rules, squid, ship) = fixture();
        attach(&mut sim, &rules, squid, ship);
        for _ in 0..40 {
            tick(&mut sim, &rules);
        }
        let anim = sim
            .entities()
            .get(squid)
            .unwrap()
            .parasite
            .as_ref()
            .unwrap()
            .grapple
            .anim
            .unwrap();
        let target = if kill_host { ship } else { squid };
        let event = crate::sim::combat::EntityDamageEvent::direct_receiver(
            target,
            10000,
            0,
            crate::sim::combat::RAD_NO_ATTACKER,
            None,
            sim.interner.intern("Super"),
            crate::sim::combat::ReceiverCallFlags {
                ignore_defenses: true,
                arg6: true,
            },
        );
        sim.commit_direct_damage_receiver(&rules, None, event, FrameEffects::default());
        sim.flush_pending_delete();
        assert!(sim.anim(anim).is_none());
        if let Some(eater) = sim.entities().get(squid) {
            assert!(!eater.lifecycle.in_limbo);
            assert_eq!(eater.parasite.as_ref().unwrap().victim, None);
        }
        if let Some(ship) = sim.entities().get(ship) {
            assert_eq!(ship.parasite_eating_me, None);
            assert!(!ship.is_paralyzed(sim.session.binary_frame));
            assert!(ship.rocking.is_none());
        }
        let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "squid-death", 0);
        crate::sim::snapshot::GameSnapshot::load(&bytes)
            .unwrap()
            .sim
            .restore_after_snapshot_load()
            .unwrap();
    }
}

#[test]
fn elite_squid_culls_yellow_ships_but_an_ordinary_squid_keeps_damaging_them() {
    for elite in [false, true] {
        let (mut sim, rules, squid, ship) = fixture();
        if elite {
            sim.substrate
                .entities
                .get_mut(squid)
                .unwrap()
                .set_veterancy_rank(200);
        }
        attach(&mut sim, &rules, squid, ship);
        let strength = rules.object("AEGIS").unwrap().strength;
        let hp = (strength as f64 * 0.4) as i32;
        sim.substrate.entities.get_mut(ship).unwrap().health.current = hp;
        for _ in 0..75 {
            tick(&mut sim, &rules);
        }
        let victim = sim.entities().get(ship).unwrap();
        assert_eq!(victim.sinking.is_active(), elite);
        if !elite {
            assert!(victim.health.current < hp);
            assert_eq!(victim.parasite_eating_me, Some(squid));
        }
    }
}

#[test]
fn normal_dolphin_attack_order_frees_a_friendly_ship() {
    let (mut sim, rules, squid, ship) = fixture();
    let dolphin = sim
        .spawn_object("DLPH", "Russians", 13, 12, 0, &rules)
        .unwrap();
    assert!(!sim.can_sonic_rescue(dolphin, ship, &rules));
    attach(&mut sim, &rules, squid, ship);
    assert!(sim.can_sonic_rescue(dolphin, ship, &rules));
    assert!(!sim.can_sonic_rescue(ship, ship, &rules));
    assert!(sim.apply_command(
        "Russians",
        &Command::Attack {
            attacker_id: dolphin,
            target_id: ship
        },
        Some(&rules)
    ));
    for _ in 0..180 {
        tick(&mut sim, &rules);
        if sim
            .entities()
            .get(ship)
            .unwrap()
            .parasite_eating_me
            .is_none()
        {
            break;
        }
    }
    assert_eq!(sim.entities().get(ship).unwrap().parasite_eating_me, None);
    assert!(!sim.entities().get(squid).unwrap().lifecycle.in_limbo);
    assert!(!sim.can_sonic_rescue(dolphin, ship, &rules));
}

#[test]
fn finishing_thresholds_match_42_original_culling_controls() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/squid_cull.json"
    ))
    .unwrap();
    let rows = data["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 42);
    for row in rows {
        let expected = row["output"]["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit == "release");
        assert_eq!(
            should_cull(
                row["culling"] == 1,
                row["health"].as_i64().unwrap() as i32,
                800,
                row["rank"] == 2,
                0.25,
                0.5
            ),
            expected,
            "{row}"
        );
    }
}
