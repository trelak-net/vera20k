use super::*;
use crate::sim::{combat::AttackTarget, command::Command, power_system::PowerState};

pub(crate) fn fixture() -> (Simulation, RuleSet, u64, Vec<u64>) {
    let rules = RuleSet::from_ini_with_fixed_art_for_test(
        &crate::rules::ini_parser::IniFile::from_str(
            r#"
[InfantryTypes]
0=SHK
1=GI
[BuildingTypes]
0=TESLA
1=POWER
[VehicleTypes]
0=TANK
[SHK]
Strength=125
Speed=4
SpeedType=Foot
Locomotor={4A582744-9839-11d1-B709-00A024DDAFD1}
Primary=Shock
Secondary=Charge
CanPassiveAquire=no
[GI]
Strength=125
Speed=4
SpeedType=Foot
Locomotor={4A582744-9839-11d1-B709-00A024DDAFD1}
Primary=Shock
[TESLA]
Strength=1000
Powered=yes
Power=-75
Overpowerable=yes
Primary=CoilBolt
Secondary=OPCoilBolt
[POWER]
Strength=1000
Power=150
[TANK]
Strength=10000
Speed=4
SpeedType=Track
Locomotor={4A582745-9839-11d1-B709-00A024DDAFD1}
[Shock]
Damage=10
ROF=5
Range=5
Projectile=Invisible
Speed=100
Warhead=SA
[Charge]
Damage=10
ROF=5
Range=5
Projectile=Invisible
Speed=100
Warhead=ChargeWH
[CoilBolt]
Damage=50
ROF=5
Range=5
Projectile=Invisible
Speed=100
Warhead=SA
[OPCoilBolt]
Damage=100
ROF=5
Range=6
Projectile=Invisible
Speed=100
Warhead=SA
[Invisible]
Inviso=yes
Image=none
AG=yes
[Warheads]
0=SA
1=ChargeWH
[SA]
Verses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%
[ChargeWH]
Verses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%
ElectricAssault=yes
"#,
        ),
        &crate::rules::ini_parser::IniFile::from_str(
            "[TESLA]\nFoundation=1x1\n[POWER]\nFoundation=1x1\n",
        ),
    )
    .unwrap();
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 450590);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    for house in sim.houses.values_mut() {
        house.is_human = true;
        house.player_control = true;
    }
    sim.session.current_house = sim.interner.get("Americans");
    sim.session.game_mode_nonzero = true;
    let coil = sim
        .spawn_object_at_height("TESLA", "Americans", 15, 15, 0, 0, &rules)
        .unwrap();
    let plant = sim
        .spawn_object_at_height("POWER", "Americans", 20, 15, 0, 0, &rules)
        .unwrap();
    for id in [coil, plant] {
        sim.substrate
            .entities
            .get_mut(id)
            .unwrap()
            .finish_building_construction_for_test();
    }
    let chargers = [(14, 15), (16, 14), (16, 16)]
        .into_iter()
        .map(|(x, y)| {
            sim.spawn_object_at_height("SHK", "Americans", x, y, 0, 0, &rules)
                .unwrap()
        })
        .collect();
    sim.spawn_object_at_height("TANK", "Russians", 28, 15, 0, 0, &rules)
        .unwrap();
    assert!(sim.rebuild_dynamic_navigation(&rules));
    (sim, rules, coil, chargers)
}

#[test]
fn update_matches_sixty_original_executable_cases() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../tools/projectile_oracle/tesla_charging.json"
    ))
    .unwrap();
    let rows = data["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 60);
    for row in rows {
        let input = &row["input"];
        let targets = input["targeting"].as_array().unwrap();
        let mut charging = TeslaCharging::default();
        for i in 0..targets.len() {
            charging.register(i as u64);
        }
        let mut power = PowerState::default();
        power.total_output = input["power"].as_i64().unwrap() as i32;
        power.total_drain = input["drain"].as_i64().unwrap() as i32;
        charging.update(
            |id| targets[*id as usize].as_bool().unwrap(),
            input["online"] == 1,
            power.has_full_power(),
        );
        assert_eq!(
            serde_json::json!(charging.chargers),
            row["remaining"],
            "{input}"
        );
        assert_eq!(
            serde_json::json!(charging.overpowered),
            row["overpowered"],
            "{input}"
        );
    }
}

#[test]
fn registration_power_thresholds_retarget_and_expiry_use_live_entities() {
    let (mut sim, rules, coil, chargers) = fixture();
    let owner = sim.entities().get(coil).unwrap().owner();
    let mut power = PowerState::default();
    power.total_drain = 75;
    sim.power_states.insert(owner, power);
    for (index, &trooper) in chargers.iter().enumerate() {
        sim.substrate
            .entities
            .get_mut(trooper)
            .unwrap()
            .attack_target = Some(AttackTarget::new(coil));
        sim.tesla_charge_detonation(trooper, Some(coil));
        sim.update_tesla_charging(coil, &rules);
        assert_eq!(
            sim.building_operational_state(coil, &rules),
            Some(index >= 1)
        );
        assert_eq!(
            sim.entities()
                .get(coil)
                .unwrap()
                .tesla_charging
                .is_overpowered(),
            index >= 2
        );
    }
    sim.tesla_charge_detonation(chargers[0], Some(coil));
    assert_eq!(
        sim.entities().get(coil).unwrap().tesla_charging.chargers,
        vec![chargers[1], chargers[2], chargers[0]]
    );
    sim.uninit_with_rules(chargers[1], &rules);
    assert_eq!(sim.entities().get(coil).unwrap().tesla_charging.count(), 2);
    assert!(
        sim.entities()
            .get(coil)
            .unwrap()
            .tesla_charging
            .is_overpowered(),
        "expiry does not sample the latch early"
    );
    sim.update_tesla_charging(coil, &rules);
    assert!(
        !sim.entities()
            .get(coil)
            .unwrap()
            .tesla_charging
            .is_overpowered()
    );
    sim.substrate
        .entities
        .get_mut(chargers[2])
        .unwrap()
        .attack_target = None;
    sim.power_states.get_mut(&owner).unwrap().total_output = 150;
    sim.update_tesla_charging(coil, &rules);
    assert_eq!(
        sim.entities().get(coil).unwrap().tesla_charging.chargers,
        vec![chargers[0]]
    );
    assert!(
        sim.entities()
            .get(coil)
            .unwrap()
            .tesla_charging
            .is_overpowered()
    );
    let obj = rules.object("TESLA").unwrap();
    let facts =
        crate::sim::combat::combat_weapon::attacker_facts(sim.entities().get(coil).unwrap(), obj);
    assert!(facts.is_overpowered_building);
    // A building and vehicle cannot become chargers by entering this receiver.
    sim.tesla_charge_detonation(coil, Some(coil));
    assert_eq!(sim.entities().get(coil).unwrap().tesla_charging.count(), 1);
}

#[test]
fn friendly_attack_fires_charge_without_damage_and_stop_disconnects() {
    let (mut sim, rules, coil, chargers) = fixture();
    let command = Command::Attack {
        attacker_id: chargers[0],
        target_id: coil,
    };
    assert!(!sim.apply_command("Russians", &command, Some(&rules)));
    assert!(sim.apply_command("Americans", &command, Some(&rules)));
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..120 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
        if sim
            .entities()
            .get(coil)
            .unwrap()
            .tesla_charging
            .is_overpowered()
        {
            break;
        }
    }
    let entity = sim.entities().get(coil).unwrap();
    assert!(
        entity.tesla_charging.is_overpowered(),
        "real AssaultBolt must charge the coil"
    );
    assert_eq!(entity.health.current, 1000, "charging is not friendly fire");
    assert!(sim.apply_command(
        "Americans",
        &Command::Stop {
            entity_id: chargers[0]
        },
        Some(&rules)
    ));
    sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
    assert_eq!(
        sim.entities().get(coil).unwrap().tesla_charging,
        TeslaCharging::default()
    );
}

#[test]
fn charger_links_and_latch_survive_save_and_change_hash() {
    let (mut sim, rules, coil, chargers) = fixture();
    let before = sim.state_hash();
    sim.tesla_charge_detonation(chargers[0], Some(coil));
    assert_ne!(before, sim.state_hash());
    sim.substrate
        .entities
        .get_mut(chargers[0])
        .unwrap()
        .attack_target = Some(AttackTarget::new(coil));
    sim.update_tesla_charging(coil, &rules);
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "tesla", 0);
    let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    restored.restore_after_snapshot_load().unwrap();
    assert_eq!(
        restored.entities().get(coil).unwrap().tesla_charging,
        sim.entities().get(coil).unwrap().tesla_charging
    );
    restored
        .substrate
        .entities
        .get_mut(chargers[0])
        .unwrap()
        .attack_target = None;
    restored.update_tesla_charging(coil, &rules);
    assert_eq!(
        restored.entities().get(coil).unwrap().tesla_charging,
        TeslaCharging::default()
    );
    sim.substrate
        .entities
        .get_mut(coil)
        .unwrap()
        .tesla_charging
        .register(999999);
    let invalid = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "bad-tesla", 0);
    let mut invalid = crate::sim::snapshot::GameSnapshot::load(&invalid)
        .unwrap()
        .sim;
    assert!(matches!(
        invalid.restore_after_snapshot_load(),
        Err(
            crate::sim::snapshot::SnapshotRestoreError::UnresolvedObjectReference {
                field: "tesla_charging",
                ..
            }
        )
    ));
}
