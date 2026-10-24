use super::*;
use crate::rules::{object_type::FactoryType, ruleset::RuleSet};
use crate::sim::{
    command::Command,
    intern::InternedId,
    world::{FrameEffects, Simulation},
};

fn fixture(vats: usize) -> Option<(Simulation, RuleSet, u64, Vec<u64>)> {
    let Ok(map) = std::env::var("VERA20K_WOJTEK_A20") else {
        assert!(
            std::env::var_os("VERA20K_REQUIRE_RETAIL_ASSETS").is_none(),
            "A20 map required"
        );
        return None;
    };
    let retail = crate::rules::retail_ini_fixture::retail_battle_rules_for_map(&map)
        .expect("retail A20 rules");
    let rules = retail.rules;
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 444_998);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    let owner = sim.interner.get("Americans").unwrap();
    sim.session.current_house = Some(owner);
    sim.houses
        .get_mut(&owner)
        .unwrap()
        .economy
        .set_credits_for_test(50_000);
    sim.spawn_object("GACNST", "Americans", 6, 6, 0, &rules)
        .unwrap();
    sim.spawn_object("NACNST", "Russians", 26, 26, 0, &rules)
        .unwrap();
    let barracks = sim
        .spawn_object("GAPILE", "Americans", 10, 10, 0, &rules)
        .unwrap();
    let mut ids = vec![];
    for &(x, y) in [(16, 10), (10, 18), (20, 18)].iter().take(vats) {
        ids.push(
            sim.spawn_object("NACLON", "Americans", x, y, 0, &rules)
                .unwrap(),
        );
    }
    assert!(sim.rebuild_dynamic_navigation(&rules));
    Some((sim, rules, barracks, ids))
}
fn owner(sim: &Simulation) -> InternedId {
    sim.interner.get("Americans").unwrap()
}
fn products(sim: &Simulation, name: &str) -> Vec<u64> {
    sim.entities()
        .values()
        .filter(|e| {
            e.owner() == owner(sim)
                && sim.interner.resolve(e.type_ref()) == name
                && !e.lifecycle.in_limbo
        })
        .map(|e| e.stable_id())
        .collect()
}
fn place(sim: &mut Simulation, rules: &RuleSet, name: &str) -> u64 {
    let oid = owner(sim);
    tests::arm_build_via(
        sim,
        rules,
        "Americans",
        name,
        ProductionCategory::Infantry,
        1,
    );
    assert!(
        sim.production
            .factories
            .test_arm_ready(oid, ProductionCategory::Infantry)
    );
    let id = factory_lifecycle::active_entity_id(sim, oid, ProductionCategory::Infantry).unwrap();
    assert!(sim.apply_command(
        "Americans",
        &Command::PlaceProducedMobile {
            category: ProductionCategory::Infantry
        },
        Some(rules)
    ));
    assert!(!sim.entities().get(id).unwrap().lifecycle.in_limbo);
    id
}
#[test]
fn cloning_vats_each_deliver_one_free_infantry_including_heroes() {
    for count in [0, 1, 3] {
        for name in ["E1", "TANY", "BORIS", "YURIPR"] {
            let Some((mut sim, rules, _, vats)) = fixture(count) else {
                return;
            };
            let cash = credits_for_owner(&sim, "Americans");
            let original = place(&mut sim, &rules, name);
            assert_eq!(
                products(&sim, name).len(),
                count + 1,
                "{name}, {count} vats"
            );
            assert_eq!(
                credits_for_owner(&sim, "Americans"),
                cash,
                "clone must be free"
            );
            assert!(
                sim.production
                    .factories
                    .view(owner(&sim), ProductionCategory::Infantry)
                    .is_none()
            );
            for id in products(&sim, name)
                .into_iter()
                .filter(|id| *id != original)
            {
                let p = &sim.entities().get(id).unwrap().position;
                assert!(
                    vats.iter().any(|vat| {
                        let v = &sim.entities().get(*vat).unwrap().position;
                        p.rx.abs_diff(v.rx) <= 3 && p.ry.abs_diff(v.ry) <= 3
                    }),
                    "clone exits its own vat"
                );
            }
        }
    }
}
#[test]
fn cloning_vats_blocked_exit_discards_clone_and_preserves_other_vats() {
    let Some((mut sim, rules, _, vats)) = fixture(3) else {
        return;
    };
    // A real output keeps vat[0]'s radio exit occupied; the other vats stay free.
    let held = sim
        .construct_object_limbo_at_height("E1", "Americans", 0, 0, 0, 0, &rules)
        .unwrap();
    assert_eq!(
        production_queue::exit_produced_object(
            &mut sim,
            &rules,
            vats[0],
            held,
            None,
            FrameEffects::default()
        ),
        crate::sim::ai_base_building::BuildingExit::Placed
    );
    assert!(
        sim.entities()
            .get(vats[0])
            .unwrap()
            .radio_contacts
            .first_free()
            .is_none()
    );
    place(&mut sim, &rules, "TANY");
    sim.flush_pending_delete();
    assert_eq!(
        products(&sim, "TANY").len(),
        3,
        "original + two unblocked vats"
    );
    assert!(
        !sim.entities()
            .values()
            .any(|e| sim.interner.resolve(e.type_ref()) == "TANY" && e.lifecycle.in_limbo),
        "no orphaned clone"
    );
}
#[test]
fn cloning_vats_follow_capture_deletion_rally_veterancy_and_saved_state() {
    let Some((mut sim, rules, _, vats)) = fixture(3) else {
        return;
    };
    let other = sim.interner.get("Russians").unwrap();
    sim.change_owner_with_rules(vats[0], other, &rules, None, FrameEffects::default());
    sim.uninit_with_rules(vats[1], &rules);
    sim.flush_pending_delete();
    sim.substrate
        .entities
        .get_mut(vats[2])
        .unwrap()
        .set_archive_target(Some(crate::sim::combat::TargetKind::Cell(23, 20)));
    let oid = owner(&sim);
    sim.houses.get_mut(&oid).unwrap().grant_veteran_infantry();
    let terrain = sim.resolved_terrain.as_ref().unwrap().clone();
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "cloning", 0);
    sim = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    sim.restore_after_snapshot_load().unwrap();
    sim.resolve_type_handles(&rules);
    sim.install_resolved_terrain_for_new_map(terrain);
    sim.rebuild_dynamic_navigation(&rules);
    let original = place(&mut sim, &rules, "E1");
    let ids = products(&sim, "E1");
    assert_eq!(ids.len(), 2);
    for id in ids {
        let e = sim.entities().get(id).unwrap();
        assert_eq!(
            crate::sim::combat::veterancy::rank_of(e.veterancy_raw),
            crate::sim::combat::veterancy::VeterancyRank::Veteran
        );
        if id != original {
            assert_eq!(
                e.archive_target(),
                Some(crate::sim::combat::TargetKind::Cell(23, 20))
            );
        }
    }
}
#[test]
fn cloning_vats_computer_building_factory_uses_the_same_delivery() {
    let Some((mut sim, rules, barracks, _)) = fixture(3) else {
        return;
    };
    let oid = owner(&sim);
    sim.houses.get_mut(&oid).unwrap().is_human = false;
    let ty = sim.interner.get("E1").unwrap();
    sim.production.factories.create_building_factory(
        barracks,
        oid,
        ProductionCategory::Infantry,
        ty,
        1,
        200,
    );
    factory_lifecycle::start_active_production(
        &mut sim,
        &rules,
        FactoryHolder::Building(barracks),
        ty,
        FrameEffects::default(),
    )
    .unwrap();
    let f = sim.production.factories.test_first_mut().unwrap();
    f.progress = PRODUCTION_STEPS;
    f.balance = 0;
    f.suspended = true;
    factory_ai::factory_ai(
        &mut sim,
        &rules,
        barracks,
        FactoryType::InfantryType,
        None,
        FrameEffects::default(),
    );
    assert_eq!(products(&sim, "E1").len(), 4);
    assert!(
        sim.production
            .factories
            .building_factory(barracks)
            .is_none()
    );
}
#[test]
fn cloning_vats_real_queue_charges_once_and_delivers_all_clones() {
    let Some((mut sim, rules, _, _)) = fixture(3) else {
        return;
    };
    let id = sim.interner.get("E1").unwrap();
    assert!(sim.apply_command(
        "Americans",
        &Command::QueueProduction { type_id: id },
        Some(&rules)
    ));
    let cash = credits_for_owner(&sim, "Americans");
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..6000 {
        publish_production_changes(&mut sim, &rules);
        let due = sim.take_due_commands();
        sim.advance_tick(&due, Some(&rules), Some(&grid), None, 33);
        if products(&sim, "E1").len() == 4 {
            break;
        }
    }
    assert_eq!(products(&sim, "E1").len(), 4);
    assert_eq!(
        cash - credits_for_owner(&sim, "Americans"),
        sim.cost_of(owner(&sim), rules.object("E1").unwrap(), &rules)
    );
}
