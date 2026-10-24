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
