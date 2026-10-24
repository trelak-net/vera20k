//! Offline-factory regressions use live Temporal links and the production
//! frame phase, so time, money, queue identity and save restoration are covered.
use super::*;
use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::{
    intern::InternedId,
    snapshot::GameSnapshot,
    world::{FrameEffects, Simulation},
};

fn fixture() -> (Simulation, RuleSet, InternedId, [u64; 2]) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[Countries]\n0=Americans\n1=Russians\n[Americans]\nSide=GDI\n[Russians]\nSide=Nod\n\
         [InfantryTypes]\n0=E1\n1=CLEG\n[BuildingTypes]\n0=GAPILE\n\
         [E1]\nStrength=100\nCost=200\nSpeed=4\nTechLevel=1\nOwner=Americans\n\
         [CLEG]\nStrength=125\nCost=1500\nSpeed=4\nTechLevel=1\nOwner=Americans,Russians\nPrimary=CHRONO\n\
         Locomotor={4A582744-9839-11d1-B709-00A024DDAFD1}\n\
         [GAPILE]\nStrength=1000\nFactory=InfantryType\nOwner=Americans,Russians\n\
         [CHRONO]\nDamage=8\nRange=10\nROF=120\nProjectile=INVIS\nWarhead=TEMP\n\
         [INVIS]\nInviso=yes\n[TEMP]\nTemporal=yes\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n"
    )).unwrap();
    let mut sim = Simulation::with_seed(0x509140);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
    tests::spawn_structure(&mut sim, &rules, 1, "Americans", "GAPILE", 10, 10);
    tests::spawn_structure(&mut sim, &rules, 2, "Americans", "GAPILE", 14, 10);
    tests::spawn_structure(&mut sim, &rules, 3, "Russians", "GAPILE", 20, 20);
    let attackers = [
        sim.spawn_object_at_height("CLEG", "Russians", 18, 12, 0, 0, &rules)
            .unwrap(),
        sim.spawn_object_at_height("CLEG", "Russians", 19, 12, 0, 0, &rules)
            .unwrap(),
    ];
    assert!(sim.rebuild_dynamic_navigation(&rules));
    let owner = sim.interner.get("Americans").unwrap();
    sim.houses
        .get_mut(&owner)
        .unwrap()
        .economy
        .set_credits_for_test(5000);
    for type_name in ["E1", "CLEG"] {
        assert!(enqueue_by_type(
            &mut sim,
            &rules,
            "Americans",
            type_name,
            FrameEffects::default()
        ));
    }
    (sim, rules, owner, attackers)
}

fn step(sim: &mut Simulation, rules: &RuleSet, frames: u32) {
    for _ in 0..frames {
        sim.session.binary_frame += 1;
        revalidate_and_step_factories(sim, rules, FrameEffects::default());
    }
}
fn held(sim: &Simulation, owner: InternedId) -> &Factory {
    sim.production
        .factories
        .factory(FactoryHolder::House(owner, ProductionCategory::Infantry))
        .unwrap()
}
fn warp(sim: &mut Simulation, rules: &RuleSet, attacker: u64, building: u64) {
    sim.temporal_initiate_warp(
        attacker,
        Some(building),
        rules,
        None,
        FrameEffects::default(),
    );
    assert!(!sim.entities().get(building).unwrap().building_online());
}

#[test]
fn offline_factory_preserves_progress_money_queue_and_save_then_resumes() {
    let (mut sim, rules, owner, attackers) = fixture();
    step(&mut sim, &rules, 30);
    let progress = held(&sim, owner).progress;
    assert!(progress > 0 && progress < PRODUCTION_STEPS);
    warp(&mut sim, &rules, attackers[0], 1);
    step(&mut sim, &rules, 20);
    assert!(
        held(&sim, owner).progress > progress,
        "one online factory still produces"
    );
    warp(&mut sim, &rules, attackers[1], 2);
    step(&mut sim, &rules, 1);
    let frozen = held(&sim, owner).clone();
    let credits = credits_for_owner(&sim, "Americans");
    assert!(frozen.suspended && !frozen.manual);
    assert_eq!(
        queue_view_for_owner(&sim, &rules, "Americans")[0].state,
        BuildQueueState::Paused
    );
    assert_eq!(frozen.queue.len(), 1);
    step(&mut sim, &rules, 300);
    assert_eq!(held(&sim, owner), &frozen);
    assert_eq!(credits_for_owner(&sim, "Americans"), credits);
    assert!(!enqueue_by_type(
        &mut sim,
        &rules,
        "Americans",
        "E1",
        FrameEffects::default()
    ));
    assert_eq!(held(&sim, owner).queue.len(), 1);

    let bytes = GameSnapshot::save(&sim, 0, 0, "offline_factory", 0);
    let mut restored = GameSnapshot::load(&bytes)
        .expect("save of live temporal hold")
        .sim;
    assert_eq!(held(&restored, owner), &frozen);
    restored.resolve_type_handles(&rules);
    step(&mut restored, &rules, 30);
    assert_eq!(held(&restored, owner), &frozen);
    restored.temporal_let_go(attackers[0]);
    step(&mut restored, &rules, 1);
    let resumed = held(&restored, owner);
    assert!(!resumed.suspended && !resumed.manual);
    assert_eq!(
        resumed.progress, frozen.progress,
        "resume restarts a full step interval"
    );
    let rate = resumed.step_rate_frames;
    step(&mut restored, &rules, u32::from(rate));
    assert_eq!(held(&restored, owner).progress, frozen.progress + 1);
    assert!(credits_for_owner(&restored, "Americans") < credits);
    assert_eq!(held(&restored, owner).object, frozen.object);
    assert_eq!(held(&restored, owner).queue, frozen.queue);
}

#[test]
fn offline_factory_never_resumes_manual_pause_or_completed_product() {
    for completed in [false, true] {
        let (mut sim, rules, owner, attackers) = fixture();
        if completed {
            assert!(
                sim.production
                    .factories
                    .test_arm_ready(owner, ProductionCategory::Infantry)
            );
        } else {
            assert!(suspend_production(
                &mut sim,
                "Americans",
                ProductionCategory::Infantry
            ));
        }
        let before = held(&sim, owner).clone();
        let credits = credits_for_owner(&sim, "Americans");
        for (attacker, building) in attackers.into_iter().zip([1, 2]) {
            warp(&mut sim, &rules, attacker, building);
        }
        step(&mut sim, &rules, 20);
        for attacker in attackers {
            sim.temporal_let_go(attacker);
        }
        step(&mut sim, &rules, 60);
        assert_eq!(held(&sim, owner), &before);
        assert_eq!(credits_for_owner(&sim, "Americans"), credits);
        assert_eq!(
            queue_view_for_owner(&sim, &rules, "Americans")[0].state,
            if completed {
                BuildQueueState::Done
            } else {
                BuildQueueState::Paused
            }
        );
    }
}

#[test]
fn offline_factory_cancellation_promotes_a_held_successor_and_loss_abandons() {
    let (mut sim, rules, owner, attackers) = fixture();
    step(&mut sim, &rules, 20);
    for (attacker, building) in attackers.into_iter().zip([1, 2]) {
        warp(&mut sim, &rules, attacker, building);
    }
    step(&mut sim, &rules, 1);
    // Cancel the active GI while a different, eligible product waits behind it.
    let old = held(&sim, owner).object.as_ref().unwrap().entity_id.unwrap();
    let tail = sim.interner.get("CLEG").unwrap();
    assert!(cancel_by_type_for_owner(
        &mut sim,
        &rules,
        "Americans",
        "E1",
        false,
        FrameEffects::default()
    ));
    assert!(!sim.entities().contains(old));
    let successor = held(&sim, owner).clone();
    assert_eq!(successor.object.as_ref().unwrap().type_id, tail);
    assert!(successor.suspended && !successor.manual);
    assert_eq!(successor.progress, 0);
    step(&mut sim, &rules, 20);
    assert_eq!(held(&sim, owner), &successor);
    sim.uninit_with_rules(1, &rules);
    sim.uninit_with_rules(2, &rules);
    step(&mut sim, &rules, 1);
    assert!(
        sim.production
            .factories
            .view(owner, ProductionCategory::Infantry)
            .is_none()
    );
    assert!(queue_view_for_owner(&sim, &rules, "Americans").is_empty());
}
