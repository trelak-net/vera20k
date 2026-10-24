//! Actual A20 definitions and a full-map launch, plus a bounded entry/launch
//! arena. A20's secret-house Tag is unreferenced in the authored map; only the
//! arena attaches it, explicitly, to exercise the otherwise dormant action33.
use super::*;
use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::{command::Command, snapshot::GameSnapshot};

fn map_path() -> Option<String> {
    let path = std::env::var("VERA20K_WOJTEK_A20").ok();
    assert!(
        path.is_some() || std::env::var_os("VERA20K_REQUIRE_RETAIL_ASSETS").is_none(),
        "required A20 path missing"
    );
    path
}
fn crate_cells(sim: &Simulation) -> Vec<(i16, i16)> {
    sim.crate_authority
        .slots()
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| (s.cell_x, s.cell_y))
        .collect()
}

#[test]
fn wojtek_full_a20_takeovers_follow_start_slots_and_pickup_springs_authored_crate() {
    use crate::skirmish_launch::*;
    let Some(path) = map_path() else {
        return;
    };
    let dir = std::env::var("RA2_DIR").expect("retail asset directory");
    let mut launch = crate::headless_scenario::battle_session(&path);
    launch.local.start_position = LaunchStartPosition::Position(0);
    launch.options.short_game = false;
    launch.options.crates = false;
    launch.options.unit_count = 0;
    launch.opponents = (1..8)
        .map(|slot| SkirmishAiSlot {
            country: LaunchCountry::Russia,
            country_random: false,
            color_index: slot,
            color_random: false,
            start_position: LaunchStartPosition::Position(slot),
            team: LaunchTeam::None,
            difficulty: AiDifficulty::Easy,
        })
        .collect();
    launch.pre_fill_house_roster = PreFillHouseRoster::from_compact_skirmish(7);
    let launch =
        crate::sim::scenario_bootstrap::MatchLaunchDescriptor::from_resolved(launch).unwrap();
    let mut scenario = crate::headless_scenario::load_with_launch(
        std::path::Path::new(&dir),
        &path,
        68_050,
        launch,
    )
    .unwrap();
    let sim = &scenario.runtime.simulation;
    let expected: Vec<_> = sim
        .trigger_runtime
        .attachments
        .iter()
        .filter_map(|(attachment, &tag)| {
            let TagAttachment::Object(id) = attachment else {
                return None;
            };
            let live = sim.trigger_runtime.tag(tag)?;
            let trigger = sim.trigger_runtime.trigger(live.triggers[0])?;
            let ty = &sim.trigger_runtime.definitions.trigger_types[trigger.type_index];
            let action = ty.actions.iter().find(|a| a.entry.kind == 14)?;
            let slot = (action.value? - 4475) as u32;
            Some((*id, slot, sim.session.start_slot_houses[&slot]))
        })
        .collect();
    assert!(expected.len() > 100, "must load actual authored structures");
    assert_eq!(
        expected
            .iter()
            .map(|(_, slot, _)| *slot)
            .collect::<BTreeSet<_>>()
            .len(),
        7 // A20 defines eight Takeovers but attaches only seven families.
    );
    scenario.runtime.advance_frame_for_tooling(&[], 33).unwrap();
    let crate::sim::runtime::SimRuntime {
        simulation: sim,
        resources,
    } = &mut scenario.runtime;
    for &(id, slot, owner) in &expected {
        assert_eq!(
            sim.entities().get(id).unwrap().owner(),
            owner,
            "takeover slot{slot}, object{id}"
        );
    }
    assert_eq!(
        sim.trigger_runtime
            .action_trace
            .iter()
            .filter(|(kind, _)| *kind == 14)
            .count(),
        8
    );
    let rules = &resources.rules;
    let registry = &resources.overlay_registry;
    let owner = sim.session.start_slot_houses[&0];
    let actor = sim
        .entities()
        .values()
        .find(|e| e.owner() == owner && e.category == crate::map::entities::EntityCategory::Unit)
        .unwrap()
        .stable_id();
    let pos = sim.entities().get(actor).unwrap().position;
    assert!(crate::sim::crates::place_crate(
        sim,
        rules,
        registry,
        (pos.rx as i16, pos.ry as i16),
        Some(0)
    ));
    let cell = crate_cells(sim)[0];
    sim.session.binary_frame = 451; // elapsed10 predicate is now true
    crate::sim::crates::pickup_crate(sim, rules, registry, cell, actor, FrameEffects::default());
    assert!(sim.trigger_runtime.dirty[3]);
    let before = crate_cells(sim).len();
    sim.advance_triggers(
        TriggerInputs {
            events: &resources.events,
            actions: &resources.actions,
            rules: Some(rules),
        },
        Some(registry),
        FrameEffects::default(),
    );
    assert_eq!(
        crate_cells(sim).len(),
        before + 1,
        "authored action108 places one money crate"
    );
    assert_eq!(
        sim.trigger_runtime
            .action_trace
            .iter()
            .filter(|(kind, _)| *kind == 108)
            .count(),
        1
    );
    // Repeat0: a second pickup cannot rerun this deleted trigger.
    sim.raise_crate_pickup(
        actor,
        crate::sim::world::UninitContext::new(Some(rules), Some(registry)),
    );
    sim.advance_triggers(
        TriggerInputs {
            events: &resources.events,
            actions: &resources.actions,
            rules: Some(rules),
        },
        Some(registry),
        FrameEffects::default(),
    );
    assert_eq!(
        sim.trigger_runtime
            .action_trace
            .iter()
            .filter(|(kind, _)| *kind == 108)
            .count(),
        1
    );
    eprintln!(
        "A20: transferred {} structures over8 starts; pickup -> elapsed timer -> money crate verified",
        expected.len()
    );
}

#[test]
fn wojtek_entry_reward_is_ready_survives_refresh_save_and_fires_once_without_silo() {
    let Some(path) = map_path() else {
        return;
    };
    let retail = crate::rules::retail_ini_fixture::retail_battle_rules_for_map(&path).unwrap();
    let rules = retail.rules;
    let map = crate::map::map_file::MapFile::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let sounds = crate::rules::sound_ini::SoundRegistry::from_ini(
        &IniFile::from_bytes(
            &crate::rules::retail_ini_fixture::retail_ini_bytes("soundmd.ini").unwrap(),
        )
        .unwrap(),
    );
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 33);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    let owner = sim.interner.get("Americans").unwrap();
    let other = sim.interner.get("Russians").unwrap();
    sim.houses.get_mut(&owner).unwrap().country = Some(owner);
    sim.houses.get_mut(&other).unwrap().country = Some(other);
    sim.session.current_house = Some(owner);
    sim.session.start_slot_houses.insert(0, owner);
    sim.initialize_trigger_definitions(
        &map,
        &crate::rules::team_ai_ini::TeamAiIniRegistry::default(),
        None,
        Some(&sounds),
    );
    sim.spawn_object("GAPOWR", "Americans", 8, 17, 0, &rules)
        .unwrap();
    let building = sim
        .spawn_object("NAPSYA", "Russians", 17, 17, 0, &rules)
        .unwrap();
    sim.spawn_object("NAPOWR", "Russians", 26, 26, 0, &rules)
        .unwrap();
    let engineer = sim
        .spawn_object("ENGINEER", "Americans", 16, 17, 0, &rules)
        .unwrap();
    sim.attach_object_trigger_tag(building, Some("01000010"));
    // Wrong owner must not trip the EnteredBy start0 predicate.
    let context = crate::sim::world::UninitContext::new(Some(&rules), None);
    sim.raise_object_tag(building, building, 1, context);
    assert!(
        !sim.trigger_runtime
            .action_trace
            .iter()
            .any(|(kind, _)| *kind == 33)
    );
    sim.fog.reveal_all_for_owner(owner);
    assert!(sim.apply_command(
        "Americans",
        &Command::CaptureBuilding {
            engineer_id: engineer,
            target_building_id: building
        },
        Some(&rules)
    ));
    sim.rebuild_dynamic_navigation(&rules);
    let grid = sim.path_grid_snapshot().unwrap();
    for _ in 0..900 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
        if sim.entities().get(building).unwrap().owner() == owner {
            break;
        }
    }
    assert_eq!(sim.entities().get(building).unwrap().owner(), owner);
    assert_eq!(
        sim.trigger_runtime
            .action_trace
            .iter()
            .filter(|(kind, _)| *kind == 33)
            .count(),
        1
    );
    assert_eq!(
        sim.trigger_runtime
            .action_trace
            .iter()
            .filter(|(kind, _)| *kind == 19)
            .count(),
        1
    );
    let nuke = sim.interner.get("NukeSpecial").unwrap();
    let inst = &sim.super_weapons[&owner][&nuke];
    assert!(inst.one_time && inst.is_ready && inst.is_active);
    crate::sim::superweapon::refresh_super_weapons_for_owner(&mut sim, &rules, owner);
    assert!(sim.super_weapons[&owner][&nuke].is_active);
    sim.power_states.get_mut(&owner).unwrap().is_low_power = true;
    crate::sim::superweapon::tick_superweapon_instances(&mut sim, &rules);
    assert!(!sim.super_weapons[&owner][&nuke].is_suspended);
    let hash = sim.state_hash();
    sim.super_weapons
        .get_mut(&owner)
        .unwrap()
        .get_mut(&nuke)
        .unwrap()
        .one_time = false;
    assert_ne!(sim.state_hash(), hash);
    sim.super_weapons
        .get_mut(&owner)
        .unwrap()
        .get_mut(&nuke)
        .unwrap()
        .one_time = true;
    let bytes = GameSnapshot::save(&sim, 0, 0, "map-reward", 0);
    let mut restored = GameSnapshot::load(&bytes).unwrap().sim;
    restored.restore_after_snapshot_load().unwrap();
    restored.resolve_type_handles(&rules);
    assert!(
        restored.super_weapons[&owner][&nuke].one_time
            && restored.super_weapons[&owner][&nuke].is_ready
    );
    // A silo built during the gift makes the next grant an ordinary recharge.
    let mut provider_sim = GameSnapshot::load(&bytes).unwrap().sim;
    provider_sim.restore_after_snapshot_load().unwrap();
    provider_sim.resolve_type_handles(&rules);
    crate::sim::arena_fixture::flat_ground(&mut provider_sim, &rules);
    provider_sim
        .spawn_object("NAMISL", "Americans", 23, 15, 0, &rules)
        .unwrap();
    assert!(provider_sim.super_weapons[&owner][&nuke].one_time);
    assert!(provider_sim.fire_super_weapon(
        &rules,
        owner,
        nuke,
        (24, 24),
        None,
        FrameEffects::default()
    ));
    let regular = &provider_sim.super_weapons[&owner][&nuke];
    assert!(regular.is_active && !regular.one_time && !regular.is_ready);
    restored.houses.get_mut(&owner).unwrap().is_defeated = true;
    crate::sim::superweapon::tick_superweapon_instances(&mut restored, &rules);
    assert!(!restored.super_weapons[&owner][&nuke].is_active);
    // Continue the source arena to retain its resource-bound terrain.
    assert!(sim.fire_super_weapon(&rules, owner, nuke, (24, 24), None, FrameEffects::default()));
    assert!(!sim.super_weapons[&owner][&nuke].is_active);
    assert!(!sim.fire_super_weapon(&rules, owner, nuke, (24, 24), None, FrameEffects::default()));
    let (_, bullet) = sim
        .projectiles
        .iter()
        .find(|(_, b)| sim.interner.resolve(b.payload.weapon) == "NukePayload")
        .expect("falling nuclear warhead");
    assert_eq!(bullet.position.z, 20000);
    assert_eq!(bullet.source_id, 0);
    let victim = sim
        .spawn_object("HTNK", "Russians", 24, 24, 0, &rules)
        .unwrap();
    let hp = sim.entities().get(victim).unwrap().health.current;
    for _ in 0..500 {
        sim.advance_tick(&[], Some(&rules), Some(&grid), None, 33);
        if sim
            .entities()
            .get(victim)
            .is_none_or(|e| e.health.current < hp)
        {
            break;
        }
    }
    assert!(
        sim.entities()
            .get(victim)
            .is_none_or(|e| e.health.current < hp),
        "nuclear payload must actually damage"
    );
    assert!(
        !sim.super_weapons[&owner][&nuke].is_active,
        "must not regenerate without silo"
    );
}

#[test]
fn original_change_house_and_one_time_grant_controls_match() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/spatial_oracle/map_actions.json")).unwrap();
    let rules=RuleSet::from_ini(&IniFile::from_str("[Countries]\n0=Americans\n1=Russians\n[Americans]\nSide=GDI\n[Russians]\nSide=Nod\n[VehicleTypes]\n0=TANK\n[TANK]\nStrength=100\nSpeed=4\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\n[SuperWeaponTypes]\n0=NukeSpecial\n[NukeSpecial]\nType=MultiMissile\nRechargeTime=5\n")).unwrap();
    for row in rows["change"].as_array().unwrap() {
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 1);
        let owner = sim.interner.get("Americans").unwrap();
        let other = sim.interner.get("Russians").unwrap();
        let resolve = row["resolve"].as_bool().unwrap();
        if resolve {
            sim.houses.get_mut(&owner).unwrap().country = Some(owner);
            sim.session.start_slot_houses.insert(0, owner);
            sim.session.start_slot_houses.insert(7, owner);
        }
        let value = row["value"].as_i64().unwrap();
        let map=crate::map::map_file::MapFile::from_bytes(format!("[Map]\nSize=0,0,20,20\nLocalSize=0,0,20,20\nTheater=TEMPERATE\n[IsoMapPack5]\n1=DwALABwBAAIA/////wAAABEAAA==\n[Triggers]\nTR=Americans,<none>,Transfer,0,1,1,1,0\n[Events]\nTR=1,8,0,0\n[Actions]\nTR=1,14,0,{value},0,0,0,0,A\n[Tags]\nTAG=2,Transfer,TR\n").as_bytes()).unwrap();
        sim.initialize_trigger_definitions(
            &map,
            &crate::rules::team_ai_ini::TeamAiIniRegistry::default(),
            None,
            None,
        );
        let mut ids = Vec::new();
        for (ordinal, bits) in row["rows"].as_array().unwrap().iter().enumerate() {
            let id = sim
                .spawn_object(
                    "TANK",
                    "Russians",
                    20 + (ordinal % 4) as u16,
                    22 + (ordinal / 4) as u16,
                    0,
                    &rules,
                )
                .unwrap();
            if bits[3] == 1 {
                sim.attach_object_trigger_tag(id, Some("TAG"));
            }
            let entity = sim.substrate.entities.get_mut(id).unwrap();
            entity.lifecycle.object_alive = bits[0] == 1;
            entity.lifecycle.cell_marked = bits[1] == 1;
            entity.lifecycle.in_limbo = bits[2] == 1;
            ids.push(id);
        }
        let typ = 0;
        let action = sim.trigger_runtime.definitions.trigger_types[typ].actions[0].clone();
        let live = (row["live"] != 0).then_some(0);
        let result = sim.apply_live_trigger_action_for_instance(
            &action,
            typ,
            live,
            &mut Vec::new(),
            Some(&rules),
            None,
            FrameEffects::default(),
        );
        assert_eq!(result, row["result"] == 1, "{row}");
        let changed: Vec<_> = ids
            .iter()
            .enumerate()
            .filter(|(_, id)| sim.entities().get(**id).unwrap().owner() != other)
            .map(|(i, _)| i as i64)
            .collect();
        let native: Vec<_> = row["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c[0].as_i64().unwrap())
            .collect();
        assert_eq!(changed, native, "{row}");
    }
    for row in rows["grant"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["once"] == 1)
    {
        let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 1);
        let owner = sim.interner.get("Americans").unwrap();
        let ty = sim.interner.intern("NukeSpecial");
        let mut inst = crate::sim::superweapon::SuperWeaponInstance::new(ty, owner, 11);
        inst.charge_duration = 123;
        inst.is_active = row["active"] == 1;
        inst.is_ready = row["ready"] == 1;
        sim.super_weapons.entry(owner).or_default().insert(ty, inst);
        sim.session.binary_frame = 100;
        assert!(crate::sim::superweapon::grant_one_time(
            &mut sim, &rules, owner, 0
        ));
        let inst = &sim.super_weapons[&owner][&ty];
        assert_eq!(
            serde_json::json!([
                inst.is_active as u8,
                inst.one_time as u8,
                inst.is_ready as u8,
                inst.is_suspended as u8
            ]),
            row["flags"]
        );
        assert_eq!(
            serde_json::json!([inst.charge_start_tick, inst.charge_duration]),
            row["timer"]
        );
    }
}
