use super::*;
use crate::rules::{overlay_types::OverlayTypeRegistry, ruleset::RuleSet};
use crate::sim::{
    command::Command,
    house_state::HouseState,
    world::{FrameEffects, Simulation},
};

fn fixture() -> (Simulation, RuleSet, OverlayTypeRegistry, u64, u64) {
    let (mut sim, rules, registry) = crate::sim::world::entry_test_fixture::fixture_with_rules(
        "[VehicleTypes]\n0=TESTTANK\n[TESTTANK]\nStrength=400\nSpeed=8\nSpeedType=Track\nMovementZone=Normal\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\n",
    );
    sim.playfield_size_height = Some(16);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.bases = false;
    sim.session.game_options.short_game = false;
    for name in ["Americans", "Russians"] {
        let owner = sim.interner.intern(name);
        let mut house = HouseState::new(owner, 0, None, true, 0, 10);
        house.player_control = true;
        sim.houses.insert(owner, house);
        sim.session.house_order.push(owner);
    }
    let a = sim
        .spawn_object_at_height("TESTTANK", "Americans", 15, 15, 0, 0, &rules)
        .unwrap();
    let b = sim
        .spawn_object_at_height("TESTTANK", "Americans", 15, 17, 0, 0, &rules)
        .unwrap();
    sim.spawn_object_at_height("TESTTANK", "Russians", 20, 20, 0, 0, &rules)
        .unwrap();
    assert!(sim.rebuild_dynamic_navigation(&rules));
    (sim, rules, registry, a, b)
}
fn apply(sim: &mut Simulation, rules: &RuleSet, reg: &OverlayTypeRegistry, c: Command) -> bool {
    sim.apply_command_with_overlays(
        "Americans",
        &c,
        Some(rules),
        Some(reg),
        FrameEffects::default(),
    )
}
fn order(id: u64, cell: (u16, u16)) -> PlannedOrder {
    PlannedOrder {
        actor: id,
        cell,
        action: PlannedAction::Move,
    }
}
fn add(
    sim: &mut Simulation,
    rules: &RuleSet,
    reg: &OverlayTypeRegistry,
    id: u64,
    cell: (u16, u16),
) -> bool {
    apply(
        sim,
        rules,
        reg,
        Command::PlanConnect {
            orders: vec![order(id, cell)],
        },
    )
}
fn tick(sim: &mut Simulation, rules: &RuleSet, reg: &OverlayTypeRegistry) {
    let grid = sim.path_grid_snapshot().unwrap();
    sim.advance_tick(&[], Some(rules), Some(&grid), Some(reg), 100);
}
fn paths(sim: &Simulation) -> &[WaypointPath] {
    sim.waypoint_plans
        .houses
        .get(&sim.interner.get("Americans").unwrap())
        .map_or(&[], Vec::as_slice)
}
#[test]
fn waypoint_draft_waits_then_runs_multiple_legs_through_real_movement() {
    let (mut sim, rules, reg, a, _) = fixture();
    assert!(add(&mut sim, &rules, &reg, a, (17, 15)));
    assert!(add(&mut sim, &rules, &reg, a, (17, 18)));
    for _ in 0..25 {
        tick(&mut sim, &rules, &reg);
    }
    let e = sim.entities().get(a).unwrap();
    assert_eq!((e.position.rx, e.position.ry), (15, 15));
    assert!(!paths(&sim)[0].committed);
    assert!(apply(&mut sim, &rules, &reg, Command::PlanCommit));
    let mut saw_first = false;
    for _ in 0..500 {
        tick(&mut sim, &rules, &reg);
        let e = sim.entities().get(a).unwrap();
        saw_first |= (e.position.rx, e.position.ry) == (17, 15);
        if paths(&sim).is_empty() {
            break;
        }
    }
    assert!(saw_first, "route must visit its first node");
    assert!(
        paths(&sim).is_empty(),
        "route should finish: {:?}",
        paths(&sim)
    );
    let e = sim.entities().get(a).unwrap();
    assert_eq!((e.position.rx, e.position.ry), (17, 18));
}
#[test]
fn waypoint_all_draft_groups_start_on_one_commit() {
    let (mut sim, rules, reg, a, b) = fixture();
    assert!(add(&mut sim, &rules, &reg, a, (17, 15)));
    assert!(add(&mut sim, &rules, &reg, b, (17, 17)));
    assert_eq!(paths(&sim).len(), 2);
    assert!(paths(&sim).iter().all(|p| !p.committed));
    apply(&mut sim, &rules, &reg, Command::PlanCommit);
    assert!(
        paths(&sim)
            .iter()
            .all(|p| p.committed && p.members.values().all(|m| m.issued))
    );
    assert!(paths(&sim).iter().all(|p| {
        p.members
            .values()
            .all(|m| m.issued_frame == sim.session.binary_frame)
    }));
}
#[test]
fn waypoint_group_plan_keeps_each_members_destination() {
    let (mut sim, rules, reg, a, b) = fixture();
    for x in [17, 19] {
        assert!(apply(
            &mut sim,
            &rules,
            &reg,
            Command::PlanConnect {
                orders: vec![order(a, (x, 15)), order(b, (x, 17))]
            }
        ));
    }
    assert_eq!(paths(&sim).len(), 1);
    assert_eq!(paths(&sim)[0].nodes.len(), 2);
    apply(&mut sim, &rules, &reg, Command::PlanCommit);
    for _ in 0..600 {
        tick(&mut sim, &rules, &reg);
        if paths(&sim).is_empty() {
            break;
        }
    }
    for (id, cell) in [(a, (19, 15)), (b, (19, 17))] {
        let e = sim.entities().get(id).unwrap();
        assert_eq!((e.position.rx, e.position.ry), cell);
    }
    assert!(paths(&sim).is_empty());
}
#[test]
fn waypoint_move_and_stop_cancel_only_the_ordered_actor() {
    let (mut sim, rules, reg, a, b) = fixture();
    apply(
        &mut sim,
        &rules,
        &reg,
        Command::PlanConnect {
            orders: vec![order(a, (18, 15)), order(b, (18, 17))],
        },
    );
    apply(&mut sim, &rules, &reg, Command::PlanCommit);
    apply(
        &mut sim,
        &rules,
        &reg,
        Command::Move {
            entity_id: a,
            target_rx: 16,
            target_ry: 14,
            queue: false,
        },
    );
    assert_eq!(
        paths(&sim)[0].members.keys().copied().collect::<Vec<_>>(),
        [b]
    );
    apply(&mut sim, &rules, &reg, Command::Stop { entity_id: b });
    assert!(paths(&sim).is_empty());
}
#[test]
fn waypoint_foreign_commands_cannot_edit_or_cancel_a_plan() {
    let (mut sim, rules, reg, a, _) = fixture();
    add(&mut sim, &rules, &reg, a, (17, 15));
    let id = paths(&sim)[0].id;
    for c in [
        Command::PlanDelete {
            path_id: id,
            node: 0,
        },
        Command::Stop { entity_id: a },
        Command::PlanConnect {
            orders: vec![order(a, (20, 15))],
        },
    ] {
        assert!(!sim.apply_command_with_overlays(
            "Russians",
            &c,
            Some(&rules),
            Some(&reg),
            FrameEffects::default()
        ));
        assert_eq!(paths(&sim).len(), 1);
    }
    sim.apply_command_with_overlays(
        "Russians",
        &Command::PlanCommit,
        Some(&rules),
        Some(&reg),
        FrameEffects::default(),
    );
    assert!(!paths(&sim)[0].committed);
}
#[test]
fn waypoint_node_limit_and_deleting_middle_or_current_nodes() {
    let (mut sim, mut rules, reg, a, _) = fixture();
    rules.general.max_waypoint_path_length = 3;
    for cell in [(17, 15), (19, 15), (19, 18)] {
        assert!(add(&mut sim, &rules, &reg, a, cell));
    }
    assert!(!add(&mut sim, &rules, &reg, a, (20, 18)));
    let id = paths(&sim)[0].id;
    assert!(apply(
        &mut sim,
        &rules,
        &reg,
        Command::PlanDelete {
            path_id: id,
            node: 1
        }
    ));
    assert_eq!(
        paths(&sim)[0]
            .nodes
            .iter()
            .map(|n| n[0].cell)
            .collect::<Vec<_>>(),
        [(17, 15), (19, 18)]
    );
    apply(&mut sim, &rules, &reg, Command::PlanCommit);
    assert!(apply(
        &mut sim,
        &rules,
        &reg,
        Command::PlanDelete {
            path_id: id,
            node: 0
        }
    ));
    tick(&mut sim, &rules, &reg);
    assert_eq!(paths(&sim)[0].nodes[0][0].cell, (19, 18));
    assert!(apply(
        &mut sim,
        &rules,
        &reg,
        Command::PlanDelete {
            path_id: id,
            node: 0
        }
    ));
    assert!(paths(&sim).is_empty());
    assert!(sim.entities().get(a).unwrap().movement_target.is_none());
}
#[test]
fn waypoint_patrol_loops_until_new_order() {
    let (mut sim, rules, reg, a, _) = fixture();
    add(&mut sim, &rules, &reg, a, (17, 15));
    add(&mut sim, &rules, &reg, a, (17, 18));
    let id = paths(&sim)[0].id;
    assert!(apply(
        &mut sim,
        &rules,
        &reg,
        Command::PlanLoop {
            path_id: id,
            node: 0
        }
    ));
    apply(&mut sim, &rules, &reg, Command::PlanCommit);
    let mut visited_second = false;
    let mut returned = false;
    for _ in 0..600 {
        tick(&mut sim, &rules, &reg);
        assert!(!paths(&sim).is_empty());
        let e = sim.entities().get(a).unwrap();
        let cell = (e.position.rx, e.position.ry);
        visited_second |= cell == (17, 18);
        returned |= visited_second && cell == (17, 15);
        if returned {
            break;
        }
    }
    assert!(returned, "patrol must traverse and return");
    assert!(apply(
        &mut sim,
        &rules,
        &reg,
        Command::PlanDelete {
            path_id: id,
            node: 0
        }
    ));
    assert_eq!(
        paths(&sim)[0].loop_to,
        None,
        "a single surviving node cannot remain a patrol loop"
    );
    apply(&mut sim, &rules, &reg, Command::Stop { entity_id: a });
    assert!(paths(&sim).is_empty());
}
#[test]
fn waypoint_save_load_preserves_plans_progress_and_hash() {
    let (mut sim, rules, reg, a, _) = fixture();
    add(&mut sim, &rules, &reg, a, (17, 15));
    add(&mut sim, &rules, &reg, a, (17, 18));
    apply(&mut sim, &rules, &reg, Command::PlanCommit);
    for _ in 0..10 {
        tick(&mut sim, &rules, &reg);
    }
    // Native load restarts the scenario RNG from the saved seed; compare on that cursor.
    sim.scenario_rng = crate::sim::rng::SimRng::new(0);
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 1, 2, "waypoints.map", 0);
    let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    restored.restore_after_snapshot_load().unwrap();
    restored.rebuild_caches_after_load(
        sim.resolved_terrain.as_ref().unwrap().clone(),
        crate::sim::pathfinding::terrain_speed::TerrainSpeedConfig::default(),
        &rules,
    );
    restored
        .restore_map_authority_after_snapshot_load(&rules, &reg)
        .unwrap();
    assert_eq!(restored.waypoint_plans, sim.waypoint_plans);
    // Load deliberately rebuilds other world authorities; isolate the new
    // serialized component, then prove it still executes after restoration.
    let plan_hash = |plans: &WaypointPlans| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        plans.hash_state(&mut h);
        h.finish()
    };
    assert_eq!(
        plan_hash(&restored.waypoint_plans),
        plan_hash(&sim.waypoint_plans)
    );
    let before = restored.state_hash();
    let saved_plans = std::mem::take(&mut restored.waypoint_plans);
    assert_ne!(restored.state_hash(), before);
    restored.waypoint_plans = saved_plans;
    for _ in 0..500 {
        tick(&mut restored, &rules, &reg);
        if paths(&restored).is_empty() {
            break;
        }
    }
    assert!(paths(&restored).is_empty());
    let e = restored.entities().get(a).unwrap();
    assert_eq!((e.position.rx, e.position.ry), (17, 18));
    let command = Command::PlanConnect {
        orders: vec![order(a, (17, 15))],
    };
    let encoded = serde_json::to_string(&command).unwrap();
    let decoded: Command = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, command);
}
#[test]
fn waypoint_force_attack_friend_does_not_complete_immediately() {
    let (sim, _, _, a, b) = fixture();
    assert!(!sim.planned_order_finished(&PlannedOrder {
        actor: a,
        cell: (15, 17),
        action: PlannedAction::Attack {
            target: b,
            force: true
        }
    }));
    assert!(sim.planned_order_finished(&PlannedOrder {
        actor: a,
        cell: (15, 17),
        action: PlannedAction::Attack {
            target: u64::MAX,
            force: false
        }
    }));
}
#[test]
fn waypoint_special_order_roundtrip_preserves_infantry_deploy_and_bunker() {
    let (sim, _, _, a, b) = fixture();
    for c in [
        Command::ToggleInfantryDeploy { entity_id: a },
        Command::EnterBunker {
            unit_id: a,
            bunker_id: b,
        },
    ] {
        assert_eq!(PlannedOrder::from_command(&c, &sim).unwrap().command(), c);
    }
}
#[test]
fn waypoint_owner_change_clears_members_before_issuing_further_orders() {
    let (mut sim, rules, reg, a, _) = fixture();
    add(&mut sim, &rules, &reg, a, (17, 15));
    sim.change_owner(a, sim.interner.get("Russians").unwrap());
    tick(&mut sim, &rules, &reg);
    assert!(paths(&sim).is_empty());
}
#[test]
fn waypoint_twelve_path_limit_allows_replacing_an_existing_route() {
    let (mut sim, rules, reg, a, b) = fixture();
    let mut ids = vec![a, b];
    for i in 0..11 {
        ids.push(
            sim.spawn_object_at_height(
                "ENGINEER",
                "Americans",
                10 + i % 6,
                12 + i / 6,
                0,
                0,
                &rules,
            )
            .unwrap(),
        );
    }
    for &id in &ids[..12] {
        assert!(add(&mut sim, &rules, &reg, id, (20, 15)));
    }
    assert_eq!(paths(&sim).len(), 12);
    assert!(!add(&mut sim, &rules, &reg, ids[12], (20, 15)));
    apply(&mut sim, &rules, &reg, Command::PlanCommit);
    assert!(add(&mut sim, &rules, &reg, a, (18, 15)));
    assert_eq!(paths(&sim).len(), 12);
    assert!(!paths(&sim).last().unwrap().committed);
}
#[test]
fn waypoint_special_orders_wait_for_their_queued_mission() {
    let (mut sim, _, _, a, b) = fixture();
    for (mission, action) in [
        (
            crate::sim::mission::MissionType::Unload,
            PlannedAction::InfantryDeploy,
        ),
        (
            crate::sim::mission::MissionType::Enter,
            PlannedAction::Repair { target: b },
        ),
    ] {
        sim.queue_order_mission(a, mission);
        assert!(!sim.planned_order_finished(&PlannedOrder {
            actor: a,
            cell: (15, 15),
            action
        }));
    }
}

#[test]
fn waypoint_a20_far_edge_clicks_and_routes_keep_full_coordinates() {
    use crate::app::{
        input::planning::node_position,
        match_runtime::sim_tick::{screen_point_to_world_with_camera, world_point_to_cell},
    };
    let Ok(path) = std::env::var("VERA20K_WOJTEK_A20") else {
        assert!(
            std::env::var_os("VERA20K_REQUIRE_RETAIL_ASSETS").is_none(),
            "A20 path required"
        );
        return;
    };
    let dir = std::env::var("RA2_DIR").expect("retail assets required");
    let scenario = crate::headless_scenario::load(std::path::Path::new(&dir), &path, 6379).unwrap();
    let source = &scenario.runtime.simulation;
    let terrain = source.resolved_terrain.as_ref().unwrap();
    assert!(terrain.width() >= 400 && terrain.height() >= 400);
    let heights = terrain
        .cells()
        .iter()
        .map(|c| ((c.rx, c.ry), c.level))
        .collect();
    // This actual A20 cell is obscured in the tactical projection: the
    // previous radar->screen->cell roundtrip changed (143,182) into(145,184).
    let radar_cell = (143, 182);
    let point = node_position(source, radar_cell);
    assert_ne!(
        world_point_to_cell(point.0, point.1, &heights, Some(terrain)),
        radar_cell
    );
    assert_eq!(
        crate::app::input::context_order::context_order_cell(
            point,
            &heights,
            Some(terrain),
            Some(radar_cell)
        ),
        radar_cell
    );
    let mut checked = 0;
    let mut maximum = (0, 0);
    for c in terrain
        .cells()
        .iter()
        .filter(|c| c.rx % 13 == 0 && c.ry % 13 == 0)
    {
        if !crate::sim::cell_rect::cell_is_in_playfield_height_aware(
            (c.rx as i32, c.ry as i32),
            source.playfield_bounds,
            Some(terrain),
        ) {
            continue;
        }
        // A projected cell hidden behind a cliff/bridge is not the visible
        // click receiver. Sample unobscured surfaces for inverse roundtrips.
        if c.rx < 6
            || c.ry < 6
            || !(-6i32..=6).all(|dy| {
                (-6i32..=6).all(|dx| {
                    terrain
                        .cell((c.rx as i32 + dx) as u16, (c.ry as i32 + dy) as u16)
                        .is_some_and(|t| t.level == c.level && !t.is_elevated_bridge_cell())
                })
            })
        {
            continue;
        }
        let cell = (c.rx, c.ry);
        let world = node_position(source, cell);
        for zoom in [1.0, 1.25, 1.5, 2.0, 3.0] {
            // Fractional camera offsets and negative world X are intentional.
            let camera = (world.0 - 310.25, world.1 - 220.5);
            let screen = ((world.0 - camera.0) * zoom, (world.1 - camera.1) * zoom);
            let click = screen_point_to_world_with_camera(screen, camera, zoom);
            assert_eq!(
                world_point_to_cell(click.0, click.1, &heights, Some(terrain)),
                cell,
                "cell={cell:?} zoom={zoom}"
            );
            checked += 1;
        }
        maximum = (maximum.0.max(c.rx), maximum.1.max(c.ry));
    }
    assert!(
        checked > 1000 && maximum.0 >= 377 && maximum.1 >= 377,
        "coverage {checked} {maximum:?}"
    );
    // Execute short routes at the extreme X/Y ends and above255 on both axes,
    // on actual A20 terrain. Bound the actors so unrelated authored armies/AI
    // cannot obscure movement or make this a performance benchmark.
    let (mut sim, rules, _, _, _) = fixture();
    let ids = sim
        .entities()
        .values()
        .map(|e| e.stable_id())
        .collect::<Vec<_>>();
    for id in ids {
        sim.uninit_with_rules(id, &rules);
    }
    sim.flush_pending_delete();
    sim.session.map_width = terrain.width();
    sim.session.map_height = terrain.height();
    sim.playfield_bounds = source.playfield_bounds;
    sim.playfield_size_height = source.playfield_size_height;
    sim.overlay_grid = source.overlay_grid.clone();
    sim.bridge_state = source.bridge_state.clone();
    sim.install_resolved_terrain_for_new_map(terrain.clone());
    assert!(sim.rebuild_dynamic_navigation(&rules));
    let reg = &scenario.runtime.resources.overlay_registry;
    let oid = sim.interner.get("Americans").unwrap();
    sim.session.current_house = Some(oid);
    sim.fog.reveal_all_for_owner(oid);
    let grid = sim.path_grid_snapshot().unwrap();
    let candidates: Vec<_> = terrain
        .cells()
        .iter()
        .filter(|c| {
            let (x, y) = (c.rx, c.ry);
            x > 10
                && y > 10
                && x + 7 < terrain.width()
                && y + 7 < terrain.height()
                && (0..7).all(|dy| {
                    (0..7).all(|dx| {
                        crate::sim::cell_rect::cell_is_in_playfield_height_aware(
                            ((x + dx) as i32, (y + dy) as i32),
                            sim.playfield_bounds,
                            Some(terrain),
                        ) && terrain.cell(x + dx, y + dy).is_some_and(|t| {
                            !t.is_elevated_bridge_cell()
                                && t.level == c.level
                                && t.speed_costs.track.is_some_and(|v| v > 0)
                        })
                    })
                })
        })
        .map(|c| (c.rx, c.ry))
        .collect();
    // Keep a live opponent: removing the original arena actors must not
    // finish the skirmish halfway through the first route.
    let &(enemy_x, enemy_y) = candidates.first().unwrap();
    sim.spawn_object("TESTTANK", "Russians", enemy_x, enemy_y, 0, &rules)
        .unwrap();
    for axis in 0..3 {
        let &(x, y) = candidates
            .iter()
            .max_by_key(|&&(x, y)| match axis {
                0 => x,
                1 => y,
                _ => x.min(y),
            })
            .expect("clear far-edge patch");
        assert!(
            match axis {
                0 => x > 330,
                1 => y > 330,
                _ => x > 255 && y > 255,
            },
            "far-edge coverage {axis} at {x},{y}"
        );
        let actor = sim
            .spawn_object("TESTTANK", "Americans", x, y, 0, &rules)
            .unwrap();
        let cells = [(x + 3, y), (x + 3, y + 3), (x, y + 3)];
        for cell in cells {
            let goal = crate::app::input::commands::ordinary_cell_move_goal(
                &sim, &rules, oid, actor, cell, true, false,
            )
            .unwrap_or_else(|| {
                panic!(
                    "click {cell:?} rejected for actor at {x},{y}, revealed={} native={:?}",
                    sim.fog.is_cell_revealed(oid, cell.0, cell.1),
                    sim.ordinary_ground_foot_cell_input(oid, actor, cell, &rules)
                )
            });
            assert_eq!(goal.cell, cell, "high cell must not wrap or clamp");
            let command = Command::Move {
                entity_id: actor,
                target_rx: goal.cell.0,
                target_ry: goal.cell.1,
                queue: false,
            };
            let order = PlannedOrder::from_command(&command, &sim).unwrap();
            let payload = Command::PlanConnect {
                orders: vec![order],
            };
            let bytes = bincode::serialize(&payload).unwrap();
            let restored: Command = bincode::deserialize(&bytes).unwrap();
            assert!(apply(&mut sim, &rules, reg, restored));
        }
        assert_eq!(
            paths(&sim)[0]
                .nodes
                .iter()
                .map(|n| n[0].cell)
                .collect::<Vec<_>>(),
            cells
        );
        apply(&mut sim, &rules, reg, Command::PlanCommit);
        let mut visited = [false; 3];
        for _ in 0..900 {
            sim.advance_tick(&[], Some(&rules), Some(&grid), Some(reg), 100);
            let e = sim.entities().get(actor).unwrap();
            for (i, cell) in cells.iter().enumerate() {
                visited[i] |= (e.position.rx, e.position.ry) == *cell;
            }
            if paths(&sim).is_empty() {
                break;
            }
        }
        assert!(
            visited.into_iter().all(|v| v) && paths(&sim).is_empty(),
            "route at {x},{y}, visits={visited:?}, position={:?}, outcome={:?}, paths={:?}",
            sim.entities().get(actor).unwrap().position,
            sim.houses[&oid].outcome_state,
            paths(&sim)
        );
        sim.uninit_with_rules(actor, &rules);
        sim.flush_pending_delete();
        eprintln!("A20 far-edge route: {cells:?}");
    }
    eprintln!("A20 click projections: {checked}, maximum sampled cells {maximum:?}");
}
