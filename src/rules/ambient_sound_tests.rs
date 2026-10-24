//! `AmbientSound=` (`ObjectTypeClass::ReadINI @ 0x005F93C1`) as the production
//! process owner binds it from the extracted retail INIs. Its reader is the
//! shared registered-reference one `turret_rotate_sound_tests` covers.

use std::sync::Arc;

use super::NativeRulesProcessOwner;
use super::NativeScenarioRulesPrefix;
use crate::rules::ini_parser::IniFile;
use crate::rules::sound_ini::SoundRegistry;

/// Every active-YR author: animals, two vehicles, two ships, the shipyards and
/// psychic amplifiers, and two civilian props. All are TechnoTypes, which is
/// why `sim::world::object_sound` plays only Technos' type sounds.
#[test]
fn retail_ambient_sound_authors() {
    let (Some(rules_ini), Some(art_ini), Some(sound_ini)) = (
        crate::rules::retail_ini_fixture::retail_ini("rulesmd.ini"),
        crate::rules::retail_ini_fixture::retail_ini("artmd.ini"),
        crate::rules::retail_ini_fixture::retail_ini("soundmd.ini"),
    ) else {
        return;
    };
    let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
        rules_ini,
        None,
        art_ini,
        Arc::new(SoundRegistry::from_ini(&sound_ini)),
    )
    .unwrap();
    let rules = owner
        .load_scenario(
            NativeScenarioRulesPrefix::NonCampaign(None),
            &IniFile::empty(),
        )
        .unwrap()
        .into_parts()
        .0;
    let mut authors = rules
        .all_objects()
        .filter_map(|object| Some((object.id.as_str(), object.ambient_sound.as_deref()?)))
        .collect::<Vec<_>>();
    authors.sort_unstable();
    assert_eq!(
        authors,
        [
            ("CAMSC06", "CampfireLoop"),
            ("CAWASH18", "FountainLoop"),
            ("COP", "PoliceSirenLoop"),
            ("COW", "CowAmbient"),
            ("CRUISE", "CruiseShipAmbience"),
            ("DNOB", "BrontoAmbient"),
            ("GAYARD", "_Amb_WavesLake"),
            ("JOSH", "ChimpAmbient"),
            ("NAPSYA", "PsychicAmplifierLoop"),
            ("NAPSYB", "PsychicAmplifierLoop"),
            ("NAYARD", "_Amb_WavesLake"),
            ("PROPA", "PropagandaTruck"),
            ("TUG", "TugBoatAmbience"),
            ("YAYARD", "_Amb_WavesLake"),
        ]
    );
}
