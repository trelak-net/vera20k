//! Retail Engineer/FV cargo selection composed with normal friendly repair input.
//! Supplied flat-map placement and target damage; no mouse picking or window capture.

use super::*;
use crate::app::input::cursor::{capability_cursor_for_hover, cursor_id_for_feedback};
use crate::app::types::{CursorFeedbackKind, CursorId};
use crate::rules::ruleset::RuleSet;
use crate::sim::house_state::HouseState;
use crate::sim::passenger::{PassengerRole, tick_passenger_system};
use crate::sim::world::UnitRepairAction;
use crate::sim::world::{FrameEffects, Simulation};

const OWNER: &str = "Americans";

fn boarded_ifv(rules: &RuleSet, passenger_type: &str) -> (Simulation, u64, u64) {
    let mut sim = Simulation::new();
    for (name, side) in [(OWNER, 0), ("Russians", 1)] {
        let owner = sim.interner.intern(name);
        sim.houses
            .insert(owner, HouseState::new(owner, side, None, true, 0, 10));
        sim.session.house_order.push(owner);
    }
    sim.session.current_house = sim.interner.get(OWNER);
    crate::sim::arena_fixture::flat_arena(&mut sim, rules);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let ifv = sim.spawn_object("FV", OWNER, 16, 16, 64, rules).unwrap();
    let engineer = sim
        .spawn_object(passenger_type, OWNER, 16, 15, 64, rules)
        .unwrap();
    assert!(sim.apply_command(
        OWNER,
        &Command::EnterTransport {
            passenger_id: engineer,
            transport_id: ifv,
        },
        Some(rules)
    ));
    tick_passenger_system(&mut sim, rules, None, FrameEffects::default());
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..240 {
        if matches!(
            sim.entities().get(engineer).unwrap().passenger_role,
            PassengerRole::Inside { .. }
        ) {
            break;
        }
        sim.advance_tick(&[], Some(rules), Some(&grid), None, 67);
    }
    assert!(
        matches!(sim.entities().get(engineer).unwrap().passenger_role,
        PassengerRole::Inside { transport_id, .. } if transport_id == ifv)
    );
    let tank = sim.spawn_object("MTNK", OWNER, 17, 16, 64, rules).unwrap();
    sim.entities_mut().get_mut(tank).unwrap().health.current = 100;
    (sim, ifv, tank)
}

fn repair_ifv(rules: &RuleSet) -> (Simulation, u64, u64) {
    let (sim, ifv, tank) = boarded_ifv(rules, "ENGINEER");
    assert_eq!(sim.entities().get(ifv).unwrap().current_weapon_number(), 1);
    assert_eq!(
        crate::sim::combat::combat_weapon::weapon_damage_value(
            sim.entities().get(ifv).unwrap(),
            rules.object("FV").unwrap(),
            rules
        ),
        -50
    );
    (sim, ifv, tank)
}

fn repair_click(sim: &Simulation, rules: &RuleSet, ifv: u64, tank: u64) -> Command {
    let owner = sim.entities().get(ifv).unwrap().owner();
    let prior = sim.state_hash();
    let dispatch = unit_repair_branch_orders(
        sim,
        rules,
        &[ifv],
        &HoverTargetKindWithId {
            kind: HoverTargetKind::FriendlyUnit,
            stable_id: tank,
        },
        owner,
        OrderModifier::Normal,
        OrderMode::Move,
    );
    assert!(dispatch.consumed && !dispatch.select_fallback);
    assert_eq!(dispatch.receivers, [ifv].into_iter().collect());
    assert_eq!(dispatch.commands.len(), 1);
    assert!(
        matches!(dispatch.commands[0], Command::ForceAttack { attacker_id, target_id }
        if attacker_id == ifv && target_id == tank)
    );
    assert_eq!(
        sim.state_hash(),
        prior,
        "input must not mutate gameplay/RNG"
    );
    dispatch.commands.into_iter().next().unwrap()
}

#[test]
fn retail_engineer_ifv_offers_normal_repair_cursor_on_damaged_friend() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let (sim, ifv, tank) = repair_ifv(&retail.rules);
    let prior = sim.state_hash();
    let hover = HoverTargetKindWithId {
        kind: HoverTargetKind::FriendlyUnit,
        stable_id: tank,
    };
    let feedback =
        capability_cursor_for_hover(&sim, &[ifv], Some(ifv), &hover, Some(&retail.rules), None);
    assert_eq!(feedback, CursorFeedbackKind::RepairAction(true));
    assert_eq!(cursor_id_for_feedback(feedback), Some(CursorId::Repair));
    assert_eq!(
        sim.state_hash(),
        prior,
        "a hover must not mutate gameplay or RNG"
    );
}

#[test]
fn retail_berserk_engineer_ifv_keeps_cursor_but_refuses_click() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, ifv, tank) = repair_ifv(rules);
    let already_sent = repair_click(&sim, rules, ifv, tank);
    sim.entities_mut().get_mut(ifv).unwrap().berserk.active = true;
    let native: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/bunker_attack_action.json",
    ))
    .unwrap();
    let control = native["repair_rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["input"]["name"] == "berserk_friend")
        .unwrap();
    assert_eq!(control["action"], 29);
    assert_eq!(control["cursor"]["output"]["cursor_row"], 33);
    assert!(!control["trace"].to_string().contains("command_sink"));
    let hover = HoverTargetKindWithId {
        kind: HoverTargetKind::FriendlyUnit,
        stable_id: tank,
    };
    let prior = sim.state_hash();
    let feedback = capability_cursor_for_hover(&sim, &[ifv], Some(ifv), &hover, Some(rules), None);
    assert_eq!(feedback, CursorFeedbackKind::RepairAction(true));
    assert_eq!(cursor_id_for_feedback(feedback), Some(CursorId::Repair));
    let dispatch = unit_repair_branch_orders(
        &sim,
        rules,
        &[ifv],
        &hover,
        sim.entities().get(ifv).unwrap().owner(),
        OrderModifier::Normal,
        OrderMode::Move,
    );
    assert!(dispatch.consumed && !dispatch.select_fallback);
    assert_eq!(dispatch.receivers, [ifv].into_iter().collect());
    assert!(
        dispatch.commands.is_empty(),
        "native Unit738890 refuses berserk clicks"
    );
    assert_eq!(sim.state_hash(), prior);
    // The refusal belongs to ClickedAction, not Event::Execute. An event
    // already published before the actor became berserk is still received.
    assert!(sim.apply_command(OWNER, &already_sent, Some(rules)));
    assert_eq!(
        sim.entities()
            .get(ifv)
            .unwrap()
            .attack_target
            .as_ref()
            .map(|attack| attack.target),
        Some(TargetKind::Entity(tank))
    );
}

#[test]
fn retail_engineer_ifv_plain_repair_click_reaches_attack_mission_and_heals() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, ifv, tank) = repair_ifv(rules);
    // No Ctrl: the actual normal-click producer uses the existing friendly
    // Attack-event alias. It is one native Attack mission, not force-fire state.
    let command = repair_click(&sim, rules, ifv, tank);
    assert!(sim.apply_command(OWNER, &command, Some(rules)));
    assert_eq!(
        sim.entities()
            .get(ifv)
            .unwrap()
            .attack_target
            .as_ref()
            .map(|attack| attack.target),
        Some(TargetKind::Entity(tank))
    );
    let grid = sim.path_grid_snapshot().unwrap();
    let before = sim.entities().get(tank).unwrap().health.current;
    for _ in 0..120 {
        sim.advance_tick(&[], Some(rules), Some(&grid), None, 67);
        if sim.entities().get(tank).unwrap().health.current > before {
            break;
        }
    }
    let after = sim.entities().get(tank).unwrap().health.current;
    assert!(
        after > before && after <= rules.object("MTNK").unwrap().strength,
        "the ordinary selected repair weapon must raise target health"
    );
}

#[test]
fn retail_far_repair_click_drives_into_range_and_heals_without_ctrl() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, ifv, _) = repair_ifv(rules);
    let far = sim.spawn_object("MTNK", OWNER, 25, 16, 64, rules).unwrap();
    sim.entities_mut().get_mut(far).unwrap().health.current = 100;
    let start = sim.entities().get(ifv).unwrap().position;
    let command = repair_click(&sim, rules, ifv, far);
    assert!(sim.apply_command(OWNER, &command, Some(rules)));
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..900 {
        sim.advance_tick(&[], Some(rules), Some(&grid), None, 67);
        if sim.entities().get(far).unwrap().health.current > 100 {
            break;
        }
    }
    let current = sim.entities().get(ifv).unwrap().position;
    assert_ne!(
        (current.rx, current.ry),
        (start.rx, start.ry),
        "the ordered repair must approach a distant ally"
    );
    assert!(
        sim.entities().get(far).unwrap().health.current > 100,
        "a normal repair click must reach its damaged target and fire"
    );
}

#[test]
fn retail_repair_click_preserves_native_modifier_and_target_controls() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, ifv, tank) = repair_ifv(rules);
    let owner = sim.entities().get(ifv).unwrap().owner();
    let hover = HoverTargetKindWithId {
        kind: HoverTargetKind::FriendlyUnit,
        stable_id: tank,
    };
    let packet: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/bunker_attack_action.json",
    ))
    .unwrap();
    for (modifier, name) in [
        (OrderModifier::Normal, "damaged_friend"),
        (OrderModifier::ForceFire, "ctrl_friend"),
        (OrderModifier::ForceMove, "alt_friend"),
        (OrderModifier::GuardArea, "ctrl_alt_friend"),
        (OrderModifier::Queue, "shift_friend"),
    ] {
        let native = packet["repair_rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["input"]["name"] == name)
            .unwrap();
        let dispatch = unit_repair_branch_orders(
            &sim,
            rules,
            &[ifv],
            &hover,
            owner,
            modifier,
            OrderMode::Move,
        );
        let native_orders: Vec<_> = native["trace"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["kind"] == "command_sink")
            .collect();
        assert_eq!(dispatch.commands.len(), native_orders.len(), "{name}");
        if !native_orders.is_empty() {
            assert_eq!(native_orders[0]["mission"], 1);
            assert!(
                matches!(dispatch.commands[0], Command::ForceAttack { .. }),
                "{name}"
            );
        }
    }
    assert_eq!(
        sim.unit_repair_action(ifv, tank, owner, rules),
        Some(UnitRepairAction::Repair)
    );
    assert_eq!(
        sim.unit_repair_action(ifv, ifv, owner, rules),
        Some(UnitRepairAction::Select)
    );
    let wrong_owner = sim.interner.get("Russians").unwrap();
    assert_eq!(sim.unit_repair_action(ifv, tank, wrong_owner, rules), None);
    sim.entities_mut().get_mut(tank).unwrap().health.current =
        rules.object("MTNK").unwrap().strength;
    assert!(
        unit_repair_branch_orders(
            &sim,
            rules,
            &[ifv],
            &hover,
            owner,
            OrderModifier::Normal,
            OrderMode::Move
        )
        .commands
        .is_empty()
    );
    sim.entities_mut().get_mut(tank).unwrap().health.current = 100;
    let enemy = sim
        .spawn_object("MTNK", "Russians", 23, 16, 64, rules)
        .unwrap();
    sim.entities_mut().get_mut(enemy).unwrap().health.current = 100;
    assert_eq!(
        sim.unit_repair_action(ifv, enemy, owner, rules),
        Some(UnitRepairAction::Move)
    );
    let enemy_hover = HoverTargetKindWithId {
        kind: HoverTargetKind::EnemyUnit,
        stable_id: enemy,
    };
    let move_dispatch = unit_repair_branch_orders(
        &sim,
        rules,
        &[ifv],
        &enemy_hover,
        owner,
        OrderModifier::Normal,
        OrderMode::Move,
    );
    assert_eq!(move_dispatch.commands.len(), 1);
    assert!(
        matches!(move_dispatch.commands[0], Command::Move { entity_id, .. } if entity_id == ifv)
    );
    sim.house_alliances
        .entry(OWNER.to_ascii_uppercase())
        .or_default()
        .insert("RUSSIANS".to_string());
    assert_eq!(
        sim.unit_repair_action(ifv, enemy, owner, rules),
        Some(UnitRepairAction::Repair),
        "the actor's one-way alliance is the native query"
    );
    let empty = sim.spawn_object("FV", OWNER, 20, 20, 64, rules).unwrap();
    assert_eq!(sim.unit_repair_action(empty, tank, owner, rules), None);
    assert!(
        !sim.apply_command(
            OWNER,
            &Command::Attack {
                attacker_id: empty,
                target_id: tank
            },
            Some(rules)
        ),
        "ordinary weapons still cannot order a friendly Attack"
    );
    let (sim, gi_ifv, tank) = boarded_ifv(rules, "E1");
    assert_eq!(
        sim.unit_repair_action(
            gi_ifv,
            tank,
            sim.entities().get(gi_ifv).unwrap().owner(),
            rules
        ),
        None,
        "a different passenger must not inherit Engineer repair admission"
    );
}

#[test]
fn issued_repair_attack_is_not_readmitted_when_health_or_cargo_changes() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, ifv, tank) = repair_ifv(rules);
    let command = repair_click(&sim, rules, ifv, tank);
    sim.entities_mut().get_mut(tank).unwrap().health.current =
        rules.object("MTNK").unwrap().strength;
    assert!(sim.apply_command(OWNER, &command, Some(rules)));
    assert_eq!(
        sim.entities()
            .get(ifv)
            .unwrap()
            .attack_target
            .as_ref()
            .map(|a| a.target),
        Some(TargetKind::Entity(tank)),
        "the event still commits its original target"
    );
    let (mut sim, ifv, tank) = repair_ifv(rules);
    let command = repair_click(&sim, rules, ifv, tank);
    use crate::sim::passenger::{DepartureRoute, depart_cargo_head, reveal_unloaded_passenger};
    depart_cargo_head(
        &mut sim,
        rules,
        None,
        ifv,
        DepartureRoute::Vehicle,
        |sim, passenger| {
            reveal_unloaded_passenger(
                sim,
                rules,
                ifv,
                passenger,
                16,
                15,
                0,
                FrameEffects::default(),
            )
        },
        FrameEffects::default(),
    )
    .unwrap();
    assert_eq!(sim.entities().get(ifv).unwrap().current_weapon_number(), 0);
    assert!(sim.apply_command(OWNER, &command, Some(rules)));
    assert_eq!(
        sim.entities()
            .get(ifv)
            .unwrap()
            .attack_target
            .as_ref()
            .map(|a| a.target),
        Some(TargetKind::Entity(tank)),
        "cargo changes do not replace the issued event"
    );
}

#[test]
fn unit_repair_vehicle_class_matches_fire_error_including_undeploy_building() {
    let rules = RuleSet::from_ini_with_fixed_art_for_test(
        &crate::rules::ini_parser::IniFile::from_str(
            "[VehicleTypes]\n0=REPAIRER\n1=TANK\n2=ORGANIC\n\
             [BuildingTypes]\n0=DEPLOYED\n1=ORDINARY\n\
             [REPAIRER]\nStrength=100\nPrimary=Fix\nSpeed=5\n\
             [TANK]\nStrength=100\nArmor=heavy\nSpeed=5\n\
             [ORGANIC]\nStrength=100\nArmor=heavy\nNonVehicle=yes\nSpeed=5\n\
             [DEPLOYED]\nStrength=100\nArmor=heavy\nUndeploysInto=TANK\n\
             [ORDINARY]\nStrength=100\nArmor=heavy\n\
             [Fix]\nDamage=-50\nRange=5\nOmniFire=yes\nProjectile=Invisible\nWarhead=Mechanical\n\
             [Invisible]\nInviso=yes\n\
             [Mechanical]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n",
        ),
        &crate::rules::ini_parser::IniFile::from_str(
            "[DEPLOYED]\nFoundation=1x1\n[ORDINARY]\nFoundation=1x1\n",
        ),
    )
    .unwrap();
    let mut sim = Simulation::new();
    let owner = sim.interner.intern(OWNER);
    sim.session.current_house = Some(owner);
    sim.houses
        .insert(owner, HouseState::new(owner, 0, None, true, 0, 10));
    sim.session.house_order.push(owner);
    crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let actor = sim
        .spawn_object("REPAIRER", OWNER, 10, 10, 64, &rules)
        .unwrap();
    let mut targets = Vec::new();
    for (kind, cell) in [
        ("TANK", (11, 10)),
        ("ORGANIC", (11, 11)),
        ("DEPLOYED", (12, 10)),
        ("ORDINARY", (12, 11)),
    ] {
        let target = sim
            .spawn_object(kind, OWNER, cell.0, cell.1, 64, &rules)
            .unwrap();
        sim.entities_mut().get_mut(target).unwrap().health.current = 50;
        targets.push(target);
    }
    assert_eq!(
        sim.unit_repair_action(actor, targets[0], owner, &rules),
        Some(UnitRepairAction::Repair)
    );
    assert_eq!(
        sim.unit_repair_action(actor, targets[1], owner, &rules),
        Some(UnitRepairAction::Select)
    );
    assert_eq!(
        sim.unit_repair_action(actor, targets[2], owner, &rules),
        Some(UnitRepairAction::Repair)
    );
    assert_eq!(
        sim.unit_repair_action(actor, targets[3], owner, &rules),
        None
    );
    use crate::sim::combat::fire_error::FireError;
    assert_eq!(
        sim.selected_weapon_fire_error(&rules, actor, TargetKind::Entity(targets[2]), None),
        FireError::Ok,
        "the same native Building vt+80 fact must reach UnitFireError"
    );
    assert_eq!(
        sim.selected_weapon_fire_error(&rules, actor, TargetKind::Entity(targets[1]), None),
        FireError::Illegal
    );
}

#[test]
fn retail_engineer_ifv_automatically_acquires_and_repairs_damaged_allies() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = &retail.rules;
    let (mut sim, ifv, tank) = repair_ifv(rules);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    sim.spawn_object("MTNK", "Russians", 28, 28, 64, rules)
        .unwrap();
    // The usual Guard scan must take the damaged friendly vehicle, while a
    // healthy vehicle standing nearer is ineligible.
    sim.spawn_object("MTNK", OWNER, 16, 17, 64, rules).unwrap();
    let scan = |sim: &mut Simulation| {
        sim.greatest_threat_represented(
            rules,
            None,
            ifv,
            crate::sim::combat::ScanMission::Guard,
            None,
            crate::sim::combat::combat_targeting::greatest_threat_for_entity,
        )
    };
    assert_eq!(scan(&mut sim), Some(tank));
    assert!(sim.entities().get(ifv).unwrap().attack_target.is_none());
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..400 {
        sim.advance_tick(&[], Some(rules), Some(&grid), None, 33);
        if sim.entities().get(tank).unwrap().health.current > 100 {
            break;
        }
    }
    assert!(
        sim.entities().get(tank).unwrap().health.current > 100,
        "repair IFV must heal without any click"
    );
    sim.entities_mut().get_mut(tank).unwrap().health.current =
        rules.object("MTNK").unwrap().strength;
    assert_eq!(
        scan(&mut sim),
        None,
        "no automatic healing of full-health vehicles"
    );
}

#[test]
fn attack_friendlies_affects_guard_and_hunt_but_cursor_flag_alone_does_not() {
    let (ini, art) =
        crate::rules::retail_ini_fixture::retail_rules_and_art().expect("retail INIs required");
    for (flag, expected) in [
        ("AttackCursorOnFriendlies", false),
        ("AttackFriendlies", true),
    ] {
        let mut input = ini.clone();
        input.merge(&crate::rules::ini_parser::IniFile::from_str(&format!(
            "[E1]\n{flag}=yes\n"
        )));
        let rules = RuleSet::from_ini_with_fixed_art_for_test(&input, &art).unwrap();
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 9);
        sim.session.game_mode_nonzero = true;
        sim.intern_rule_type_ids(&rules);
        sim.resolve_type_handles(&rules);
        let actor = sim.spawn_object("E1", OWNER, 10, 10, 0, &rules).unwrap();
        let friend = sim.spawn_object("E1", OWNER, 11, 10, 0, &rules).unwrap();
        for mask in [
            crate::sim::combat::ScanMission::Guard,
            crate::sim::combat::ScanMission::Hunt,
        ] {
            let target = sim.greatest_threat_represented(
                &rules,
                None,
                actor,
                mask,
                None,
                crate::sim::combat::combat_targeting::greatest_threat_for_entity,
            );
            assert_eq!(target, expected.then_some(friend), "{flag}, {mask:?}");
        }
    }
}

#[test]
fn controlled_open_topped_passenger_uses_the_controllers_alliances() {
    let Some((mut ini, art)) = crate::rules::retail_ini_fixture::retail_rules_and_art() else {
        return;
    };
    ini.merge(&crate::rules::ini_parser::IniFile::from_str(
        "[FV]\nOpenTopped=yes\n",
    ));
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
    let (mut sim, carrier, _tank) = boarded_ifv(&rules, "E1");
    sim.session.game_mode_nonzero = true;
    let rider = sim
        .entities()
        .get(carrier)
        .unwrap()
        .passenger_role
        .cargo()
        .unwrap()
        .passengers[0];
    let enemy = sim
        .spawn_object("E1", "Russians", 18, 16, 64, &rules)
        .unwrap();
    let controller = sim
        .spawn_object("YURI", "Russians", 28, 28, 64, &rules)
        .unwrap();
    let scan = |sim: &mut Simulation| {
        sim.greatest_threat_represented(
            &rules,
            None,
            rider,
            crate::sim::combat::ScanMission::Hunt,
            None,
            crate::sim::combat::combat_targeting::greatest_threat_for_entity,
        )
    };
    assert_eq!(scan(&mut sim), Some(enemy));
    // Supply the represented carrier/controller link to isolate its scan
    // semantics; capture admission/release has its own integration suite.
    sim.entities_mut().get_mut(carrier).unwrap().mind_control =
        crate::sim::capture_manager::MindControlLink::controlled_by_for_test(controller);
    for berserk in [false, true] {
        sim.entities_mut().get_mut(rider).unwrap().berserk.active = berserk;
        let picked = scan(&mut sim).expect("a controller enemy remains available");
        assert_eq!(
            sim.interner
                .resolve(sim.entities().get(picked).unwrap().owner()),
            OWNER
        );
        assert_ne!(picked, enemy);
    }
}

#[test]
fn infantry_healer_automatically_heals_a_wounded_ally() {
    let Some((mut ini, art)) = crate::rules::retail_ini_fixture::retail_rules_and_art() else {
        return;
    };
    ini.merge(&crate::rules::ini_parser::IniFile::from_str(
        "[E1]\nPrimary=Heal\nSecondary=Heal\nDeployer=no\n[Heal]\nDamage=-50\nROF=5\nRange=5\nProjectile=Invisible\nSpeed=100\nWarhead=SA\n"));
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 6);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let medic = sim.spawn_object("E1", OWNER, 10, 10, 0, &rules).unwrap();
    let wounded = sim.spawn_object("E2", OWNER, 12, 10, 0, &rules).unwrap();
    sim.entities_mut().get_mut(wounded).unwrap().health.current = 25;
    sim.spawn_object("E2", OWNER, 11, 10, 0, &rules).unwrap();
    let target = sim.greatest_threat_represented(
        &rules,
        None,
        medic,
        crate::sim::combat::ScanMission::Guard,
        None,
        crate::sim::combat::combat_targeting::greatest_threat_for_entity,
    );
    assert_eq!(target, Some(wounded));
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..240 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
        if sim.entities().get(wounded).unwrap().health.current > 25 {
            break;
        }
    }
    assert!(sim.entities().get(wounded).unwrap().health.current > 25);
}
