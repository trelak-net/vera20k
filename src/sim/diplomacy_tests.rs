use super::*;
use crate::sim::{command::Command, house_state::HouseState};
fn fixture() -> (Simulation, RuleSet, OverlayTypeRegistry, Vec<InternedId>) {
    let (mut sim, rules, registry) =
        crate::sim::world::entry_test_fixture::fixture_with_rules_and_fixed_art(
            "[Sides]\nGDI=Americans\nNod=Russians\nThirdSide=YuriCountry\nCivilian=Neutral\n[VehicleTypes]\n0=MTNK\n[MTNK]\nStrength=400\nSpeed=6\nSpeedType=Track\nMovementZone=Normal\nThreatPosed=10\nSight=3\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\n",
            &crate::rules::ini_parser::IniFile::from_str(""),
        );
    let owners: Vec<_> = ["Americans", "Russians", "Other"]
        .into_iter()
        .map(|name| {
            let owner = sim.interner.intern(name);
            sim.houses
                .insert(owner, HouseState::new(owner, 0, None, true, 0, 10));
            owner
        })
        .collect();
    sim.session.house_order = owners.clone();
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.short_game = false;
    (sim, rules, registry, owners)
}
#[test]
fn alliance_commands_clear_targets_and_anger_and_break_both_directions() {
    let (mut sim, rules, registry, owners) = fixture();
    let [a, b, _] = owners[..] else {
        unreachable!()
    };
    let actor = sim
        .spawn_object("MTNK", "Americans", 10, 10, 0, &rules)
        .unwrap();
    let victim = sim
        .spawn_object("MTNK", "Russians", 12, 10, 0, &rules)
        .unwrap();
    sim.assign_target_represented(
        actor,
        Some(crate::sim::combat::TargetKind::Entity(victim)),
        Some(&rules),
        Some(&registry),
        FrameEffects::default(),
    )
    .unwrap();
    let house = sim.houses.get_mut(&a).unwrap();
    house.grudge_scores.insert(b, 45);
    house.enemy_house = Some(b);
    assert!(sim.apply_command(
        "Americans",
        &Command::ToggleAlliance { target: b },
        Some(&rules)
    ));
    assert!(sim.house_considers_ally(a, b));
    assert!(!sim.house_considers_ally(b, a));
    assert!(sim.entities().get(actor).unwrap().attack_target.is_none());
    assert_eq!(sim.houses[&a].grudge_scores[&b], 0);
    assert_eq!(sim.houses[&a].enemy_house, None);
    assert!(crate::map::houses::is_allied_with(
        &sim.fog.alliances,
        "Americans",
        "Russians"
    ));
    assert!(sim.apply_command(
        "Russians",
        &Command::ToggleAlliance { target: a },
        Some(&rules)
    ));
    assert!(sim.house_considers_ally(b, a));
    assert!(sim.apply_command(
        "Americans",
        &Command::ToggleAlliance { target: b },
        Some(&rules)
    ));
    assert!(!sim.house_considers_ally(a, b));
    assert!(!sim.house_considers_ally(b, a));
    assert_eq!(sim.houses[&a].grudge_scores[&b], 1);
    assert!(!sim.houses[&b].grudge_scores.contains_key(&b));
}
#[test]
fn invalid_commands_cannot_change_ai_teams_fixed_modes_or_the_last_enemy() {
    let (mut sim, rules, _, owners) = fixture();
    let [a, b, c] = owners[..] else {
        unreachable!()
    };
    let command = Command::ToggleAlliance { target: b };
    for gate in 0..6 {
        sim.houses.get_mut(&b).unwrap().is_human = gate != 0;
        sim.houses.get_mut(&a).unwrap().is_defeated = gate == 1;
        sim.session.diplomacy.fixed_alliance = gate == 2;
        sim.session.diplomacy.allies_allowed = gate != 3;
        sim.session.game_options.ally_change_allowed = gate != 4;
        sim.session.game_mode_nonzero = gate != 5;
        let hash = sim.state_hash();
        assert!(
            !sim.apply_command("Americans", &command, Some(&rules)),
            "gate {gate}"
        );
        assert_eq!(sim.state_hash(), hash, "gate {gate}");
    }
    sim.session.game_mode_nonzero = true;
    sim.houses.get_mut(&c).unwrap().is_defeated = true;
    assert_eq!(
        sim.alliance_change_block(a, b, &rules),
        Some(AllianceChangeBlock::LastEnemy)
    );
    assert!(!sim.apply_command("Americans", &command, Some(&rules)));
    assert!(!sim.apply_command(
        "Americans",
        &Command::ToggleAlliance { target: a },
        Some(&rules)
    ));
}
#[test]
fn diplomacy_policy_and_alliances_survive_saved_games_and_change_hashes() {
    let (mut sim, rules, _, owners) = fixture();
    assert!(sim.apply_command(
        "Americans",
        &Command::ToggleAlliance { target: owners[1] },
        Some(&rules)
    ));
    let hash = sim.state_hash();
    sim.session.diplomacy.fixed_alliance = true;
    assert_ne!(sim.state_hash(), hash);
    sim.session.diplomacy.allies_allowed = false;
    let queued = crate::sim::command::CommandEnvelope::new(
        owners[0],
        sim.session.tick + 1,
        Command::ToggleAlliance { target: owners[1] },
    );
    sim.queue_command(queued.clone());
    let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "diplomacy", 0);
    let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
        .unwrap()
        .sim;
    restored.restore_after_snapshot_load().unwrap();
    assert_eq!(restored.pending_command_snapshot(), vec![queued]);
    assert!(restored.session.diplomacy.fixed_alliance);
    assert!(!restored.session.diplomacy.allies_allowed);
    assert!(restored.house_considers_ally(owners[0], owners[1]));
    let command = Command::ToggleAlliance { target: owners[1] };
    let bytes = bincode::serialize(&command).unwrap();
    assert_eq!(bincode::deserialize::<Command>(&bytes).unwrap(), command);
}

#[test]
fn original_can_ally_and_selected_object_hotkey_controls_match() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/spatial_oracle/diplomacy.json")).unwrap();
    let (_, rules, _, _) = fixture();
    let civilian = rules.side_index("Civilian").unwrap().0;
    for row in corpus["can_ally"].as_array().unwrap() {
        let mut sim = Simulation::new();
        let owners: Vec<_> = (0..row["count"].as_u64().unwrap())
            .map(|i| {
                let owner = sim.interner.intern(&format!("House{i}"));
                let mut house = HouseState::new(owner, 0, None, true, 0, 10);
                house.is_defeated = row["defeated"].as_u64().unwrap() & (1 << i) != 0;
                house.multiplay_passive = row["passive"].as_u64().unwrap() & (1 << i) != 0;
                if i == 1 && row["civilian"].as_bool().unwrap() {
                    house.side_index = civilian;
                }
                sim.houses.insert(owner, house);
                owner
            })
            .collect();
        sim.session.house_order = owners.clone();
        sim.session.game_mode_nonzero = row["mode"].as_bool().unwrap();
        let graph = sim.house_alliances.entry("HOUSE0".into()).or_default();
        for i in 0..owners.len() {
            if row["allies"].as_u64().unwrap() & (1 << i) != 0 {
                graph.insert(format!("HOUSE{i}"));
            }
        }
        let target = owners[usize::from(!row["same"].as_bool().unwrap())];
        assert_eq!(
            sim.can_make_player_ally(owners[0], target, &rules),
            row["admitted"].as_bool().unwrap(),
            "{row}"
        );
    }
    let (mut sim, rules, _, owners) = fixture();
    for row in corpus["hotkey"].as_array().unwrap() {
        if !row["selected"].as_bool().unwrap() {
            assert!(!row["admitted"].as_bool().unwrap());
            continue;
        }
        let target = owners[usize::from(!row["same"].as_bool().unwrap())];
        sim.houses.get_mut(&owners[0]).unwrap().is_human = true;
        sim.houses.get_mut(&owners[0]).unwrap().is_defeated = row["dead"].as_bool().unwrap();
        sim.houses.get_mut(&target).unwrap().is_human = row["human"].as_bool().unwrap();
        sim.session.game_mode_nonzero = row["mode"].as_bool().unwrap();
        sim.session.diplomacy.fixed_alliance = row["fixed"].as_bool().unwrap();
        sim.session.diplomacy.allies_allowed = row["allowed"].as_bool().unwrap();
        sim.session.game_options.ally_change_allowed = row["change"].as_bool().unwrap();
        assert_eq!(
            sim.alliance_change_block(owners[0], target, &rules)
                .is_none(),
            row["admitted"].as_bool().unwrap(),
            "{row}"
        );
    }
}

#[test]
fn make_enemy_matches_original_reciprocal_bits_and_anger_targets() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/spatial_oracle/diplomacy.json")).unwrap();
    for row in corpus["break_alliance"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["ours"].as_bool().unwrap())
    {
        let (mut sim, rules, _, owners) = fixture();
        sim.house_alliances
            .entry("AMERICANS".into())
            .or_default()
            .insert("RUSSIANS".into());
        if row["theirs"].as_bool().unwrap() {
            sim.house_alliances
                .entry("RUSSIANS".into())
                .or_default()
                .insert("AMERICANS".into());
        }
        assert!(sim.apply_command(
            "Americans",
            &Command::ToggleAlliance { target: owners[1] },
            Some(&rules)
        ));
        for (index, other) in [(0, 1), (1, 0)] {
            assert_eq!(
                sim.house_considers_ally(owners[index], owners[other]),
                row["after"][index].as_u64().unwrap() & (1 << other) != 0
            );
        }
        for anger in row["anger"].as_array().unwrap() {
            let source = owners[anger[0].as_u64().unwrap() as usize];
            let target = owners[anger[1].as_u64().unwrap() as usize];
            if source == target {
                assert!(!sim.houses[&source].grudge_scores.contains_key(&target));
            } else {
                assert_eq!(
                    sim.houses[&source].grudge_scores[&target],
                    anger[2].as_i64().unwrap() as i32
                );
            }
        }
    }
}

#[test]
fn forming_and_breaking_refresh_live_sight_and_spatial_threat() {
    let (mut sim, rules, _, owners) = fixture();
    let a = owners[0];
    let b = owners[1];
    sim.spawn_object("MTNK", "Americans", 10, 10, 0, &rules)
        .unwrap();
    sim.spawn_object("MTNK", "Russians", 20, 18, 0, &rules)
        .unwrap();
    sim.refresh_diplomacy_sight(&rules);
    assert!(!sim.fog.is_cell_visible(b, 10, 10));
    let threat = sim.house_threat_at_cell(b, (10, 10)).unwrap();
    assert!(threat > 0);
    assert!(sim.apply_command(
        "Americans",
        &Command::ToggleAlliance { target: b },
        Some(&rules)
    ));
    assert!(sim.fog.is_cell_visible(b, 10, 10));
    assert!(sim.apply_command(
        "Russians",
        &Command::ToggleAlliance { target: a },
        Some(&rules)
    ));
    assert_eq!(sim.house_threat_at_cell(b, (10, 10)).unwrap(), 0);
    assert!(sim.apply_command(
        "Americans",
        &Command::ToggleAlliance { target: b },
        Some(&rules)
    ));
    assert!(!sim.fog.is_cell_visible(b, 10, 10));
    assert!(sim.fog.is_cell_revealed(b, 10, 10));
    assert_eq!(sim.house_threat_at_cell(b, (10, 10)).unwrap(), threat);
}

#[test]
fn a20_battle_and_ffa_keep_their_launched_ai_team_policy() {
    use crate::skirmish_launch::*;
    let path = std::env::var("VERA20K_WOJTEK_A20").ok();
    assert!(
        path.is_some() || std::env::var_os("VERA20K_REQUIRE_RETAIL_ASSETS").is_none(),
        "required A20 map missing"
    );
    let Some(path) = path else {
        return;
    };
    let dir = std::env::var("RA2_DIR").expect("retail directory");
    let assets = crate::assets::asset_manager::AssetManager::new(
        std::path::Path::new(&dir),
        crate::assets::asset_manager::MediaArchiveMode::STOCK_DIGITAL,
    )
    .unwrap();
    let modes = crate::skirmish_modes::skirmish_modes_from_assets(&assets).unwrap();
    for id in [1, 2] {
        let mode = modes.iter().find(|mode| mode.id == id).unwrap();
        let mut launch = crate::headless_scenario::battle_session(&path);
        launch.mode = SkirmishLaunchMode::from_game_mode(mode);
        launch.local.team = LaunchTeam::Team(0);
        launch.local.start_position = LaunchStartPosition::Position(0);
        launch.options.short_game = false;
        launch.options.unit_count = 0;
        launch.opponents = (1..8)
            .map(|slot| SkirmishAiSlot {
                country: LaunchCountry::Russia,
                country_random: false,
                color_index: slot,
                color_random: false,
                start_position: LaunchStartPosition::Position(slot),
                team: if slot == 1 {
                    LaunchTeam::Team(0)
                } else {
                    LaunchTeam::None
                },
                difficulty: AiDifficulty::Easy,
            })
            .collect();
        launch.pre_fill_house_roster = PreFillHouseRoster::from_compact_skirmish(7);
        let launch =
            crate::sim::scenario_bootstrap::MatchLaunchDescriptor::from_resolved(launch).unwrap();
        let mut scenario = crate::headless_scenario::load_with_launch(
            std::path::Path::new(&dir),
            &path,
            68050,
            launch,
        )
        .unwrap();
        let sim = &mut scenario.runtime.simulation;
        assert_eq!(sim.session.diplomacy.allies_allowed, mode.allies_allowed);
        let a = sim.session.house_order[0];
        let b = sim.session.house_order[1];
        assert_eq!(sim.house_considers_ally(a, b), id == 1);
        assert_eq!(sim.house_considers_ally(b, a), id == 1);
        let graph = sim.house_alliances.clone();
        let owner = sim.interner.resolve(a).to_string();
        assert!(!sim.apply_command(
            &owner,
            &Command::ToggleAlliance { target: b },
            Some(&scenario.runtime.resources.rules)
        ));
        assert_eq!(sim.house_alliances, graph);
    }
}
