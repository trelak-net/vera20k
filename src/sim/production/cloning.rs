//! Infantry cloning at factory delivery. Original ExitObject443C60's
//! 4449D9..444A53 visits every owned Cloning building, constructs the produced
//! type for that vat's owner and calls its normal ExitObject. Factory=Infantry
//! and !Cloning gate dispatch, so clone exits cannot recursively clone.
use crate::rules::{
    object_type::{FactoryType, ObjectCategory},
    overlay_types::OverlayTypeRegistry,
    ruleset::RuleSet,
};
use crate::sim::{
    ai_base_building::BuildingExit,
    world::{FrameEffects, Simulation},
};

pub(super) fn clone_produced_infantry(
    sim: &mut Simulation,
    rules: &RuleSet,
    producer: u64,
    product: u64,
    overlays: Option<&OverlayTypeRegistry>,
    effects: FrameEffects<'_>,
) {
    let Some(factory) = sim.entities().get(producer) else {
        return;
    };
    let Some(factory_type) = sim.object_type(factory.type_ref(), rules) else {
        return;
    };
    if factory_type.factory != Some(FactoryType::InfantryType) || factory_type.cloning {
        return;
    }
    let Some(infantry) = sim.entities().get(product) else {
        return;
    };
    let Some(infantry_type) = sim.object_type(infantry.type_ref(), rules) else {
        return;
    };
    if infantry_type.category != ObjectCategory::Infantry {
        return;
    }
    let owner = factory.owner();
    let type_name = infantry_type.id.clone();
    let owner_name = sim.interner.resolve(owner).to_owned();
    // Reuse the lifecycle-owned house building order; no per-frame census or
    // separate save-state registry. Original has no power or BuildLimit check.
    let vats: Vec<_> = sim
        .houses
        .get(&owner)
        .into_iter()
        .flat_map(|house| house.base_projection.buildings().iter().copied())
        .filter(|&id| {
            sim.entities().get(id).is_some_and(|e| {
                e.owner() == owner
                    && e.health.current > 0
                    && !e.lifecycle.in_limbo
                    && sim
                        .object_type(e.type_ref(), rules)
                        .is_some_and(|t| t.cloning)
            })
        })
        .collect();
    for vat in vats {
        let Some(clone) =
            sim.construct_object_limbo_at_height(&type_name, &owner_name, 0, 0, 0, 0, rules)
        else {
            continue;
        };
        if super::production_queue::exit_produced_object(sim, rules, vat, clone, overlays, effects)
            == BuildingExit::Placed
        {
            sim.mission_spawned_entities = true;
        } else {
            // A blocked/busy vat misses this clone. It neither charges the
            // owner nor leaves an orphaned limbo object/factory to retry.
            sim.discard_constructed_limbo(clone, Some(rules), effects);
        }
    }
}
