use super::*;
use crate::sim::{
    combat::ScanMission,
    command::Command,
    mission::{MissionId, MissionType},
};

#[test]
fn selection_and_history_match_thirty_original_executable_cases() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../tools/projectile_oracle/distributed_fire.json"
    ))
    .unwrap();
    let rows = data["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 30);
    for row in rows {
        let mut state = DistributedFire {
            candidates: serde_json::from_value(row["input"]["candidates"].clone()).unwrap(),
            history: row["input"]["history"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| TargetKind::Entity(id.as_u64().unwrap()))
                .collect(),
        };
        let assigned = state
            .choose()
            .map(|id| vec![id.unwrap_or(0)])
            .unwrap_or_default();
        assert_eq!(
            serde_json::json!(assigned),
            row["assigned"],
            "{}",
            row["input"]
        );
        let history: Vec<_> = state.references().skip(state.candidates.len()).collect();
        assert_eq!(
            serde_json::json!(history),
            row["history"],
            "{}",
            row["input"]
        );
    }
}
fn fixture() -> (Simulation, RuleSet, u64, [u64; 2]) {
    let rules = RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str(
        r#"
[VehicleTypes]
0=DISTR
1=TARGET
[DISTR]
Strength=500
Speed=5
SpeedType=Track
Locomotor={4A582745-9839-11d1-B709-00A024DDAFD1}
Primary=GUN
DistributedFire=yes
[TARGET]
Strength=500
Speed=5
SpeedType=Track
Locomotor={4A582745-9839-11d1-B709-00A024DDAFD1}
CanPassiveAquire=no
[GUN]
Damage=1
ROF=1
Range=8
OmniFire=yes
Projectile=Invisible
Warhead=SA
[Invisible]
Inviso=yes
AG=yes
[SA]
Verses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%
"#,
    ))
    .unwrap();
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 709550);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let firer = sim
        .spawn_object("DISTR", "Americans", 10, 10, 0, &rules)
        .unwrap();
    let targets = [
        sim.spawn_object("TARGET", "Russians", 11, 10, 0, &rules)
            .unwrap(),
        sim.spawn_object("TARGET", "Russians", 12, 10, 0, &rules)
            .unwrap(),
    ];
    sim.mission_assign_exact(firer, MissionId::from_known(MissionType::Guard), 0)
        .unwrap();
    (sim, rules, firer, targets)
}
fn scan(sim: &mut Simulation, rules: &RuleSet, id: u64, mask: ScanMission) -> Option<u64> {
    sim.greatest_threat_represented(
        rules,
        None,
        id,
        mask,
        None,
        crate::sim::combat::combat_targeting::greatest_threat_for_entity,
    )
}
#[test]
fn passive_firing_spreads_and_an_explicit_attack_stays_on_its_target() {
    let (mut sim, rules, firer, targets) = fixture();
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..180 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
        if targets
            .iter()
            .all(|id| sim.entities().get(*id).unwrap().health.current < 500)
        {
            break;
        }
    }
    assert!(
        targets
            .iter()
            .all(|id| sim.entities().get(*id).unwrap().health.current < 500),
        "passive distribution must actually hit both targets"
    );
    sim.substrate
        .entities
        .get_mut(firer)
        .unwrap()
        .distributed_fire
        .history
        .clear();
    let before = sim.entities().get(targets[1]).unwrap().health.current;
    assert!(sim.apply_command(
        "Americans",
        &Command::Attack {
            attacker_id: firer,
            target_id: targets[0]
        },
        Some(&rules)
    ));
    for _ in 0..90 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
    }
    assert_eq!(
        sim.entities().get(targets[1]).unwrap().health.current,
        before
    );
    assert!(
        sim.entities()
            .get(firer)
            .unwrap()
            .distributed_fire
            .history
            .is_empty()
    );
    assert_eq!(
        sim.entities()
            .get(firer)
            .unwrap()
            .attack_target
            .as_ref()
            .map(|a| a.target),
        Some(TargetKind::Entity(targets[0]))
    );
}
#[test]
fn candidates_history_save_hash_and_pointer_expiry_stay_consistent() {
    let (mut sim, rules, firer, targets) = fixture();
    let before = sim.state_hash();
    scan(&mut sim, &rules, firer, ScanMission::Guard);
    assert_ne!(before, sim.state_hash());
    sim.substrate
        .entities
        .get_mut(firer)
        .unwrap()
        .distributed_fire
        .record(TargetKind::Entity(targets[0]));
    let expected = sim.entities().get(firer).unwrap().distributed_fire.clone();
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "spread", 0);
    let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    restored.restore_after_snapshot_load().unwrap();
    assert_eq!(
        restored.entities().get(firer).unwrap().distributed_fire,
        expected
    );
    restored.uninit_with_rules(targets[0], &rules);
    let state = &restored.entities().get(firer).unwrap().distributed_fire;
    assert!(!state.names(targets[0]));
    assert!(state.names(targets[1]));
    // The flat Hunt pass clears the candidate vectors, but the original only
    // collects spread candidates in radius scans.
    scan(&mut sim, &rules, firer, ScanMission::Hunt);
    assert!(
        sim.entities()
            .get(firer)
            .unwrap()
            .distributed_fire
            .candidates
            .is_empty()
    );
    sim.distribute_fire(firer, &rules, None, FrameEffects::default());
    assert!(
        sim.entities()
            .get(firer)
            .unwrap()
            .distributed_fire
            .history
            .is_empty()
    );
}

#[test]
fn retail_aegis_automatically_fires_at_two_airships_over_water() {
    let Some((ini, art)) = crate::rules::retail_ini_fixture::retail_rules_and_art() else {
        return;
    };
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&ini, &art).unwrap();
    use crate::rules::terrain_rules::{LandType, SpeedCostProfile, TerrainClass};
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 709550);
    let water = LandType::Water.as_index();
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    sim.session.current_house = sim.interner.get("Americans");
    let costs = SpeedCostProfile {
        float: Some(100),
        hover: Some(100),
        ..Default::default()
    };
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
    sim.install_resolved_terrain_for_new_map(
        crate::map::resolved_terrain::ResolvedTerrainGrid::from_cells(32, 32, cells),
    );
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    assert!(sim.rebuild_dynamic_navigation(&rules));
    let ship = sim
        .spawn_object("AEGIS", "Americans", 10, 10, 0, &rules)
        .unwrap();
    let targets = [
        sim.spawn_object("ZEP", "Russians", 12, 10, 0, &rules)
            .unwrap(),
        sim.spawn_object("ZEP", "Russians", 13, 10, 0, &rules)
            .unwrap(),
    ];
    let strength = rules.object("ZEP").unwrap().strength;
    let grid = sim.path_grid_snapshot().unwrap();
    let mut hit = [false, false];
    for _ in 0..900 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
        for (i, id) in targets.iter().enumerate() {
            hit[i] |= sim
                .entities()
                .get(*id)
                .is_none_or(|e| e.health.current < strength);
        }
        if hit == [true, true] {
            break;
        }
    }
    if hit != [true, true] {
        for id in [ship, targets[0], targets[1]] {
            let entity = sim.entities().get(id).unwrap();
            eprintln!(
                "id={id} health={} position={:?} mission={:?} limbo={} marked={} air={:?} target={:?}",
                entity.health.current,
                entity.position,
                entity.mission.current(),
                entity.lifecycle.in_limbo,
                entity.lifecycle.cell_marked,
                entity.air_spatial_bucket(),
                entity.attack_target
            );
        }
        eprintln!(
            "direct guard={:?}",
            scan(&mut sim, &rules, ship, ScanMission::Guard)
        );
    }
    assert_eq!(
        hit,
        [true, true],
        "stock AEGIS must acquire and hit both airships; state={:?}",
        sim.entities().get(ship).map(|e| &e.distributed_fire)
    );
}
