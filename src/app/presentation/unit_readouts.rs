//! Live unit readouts from PIPS2.SHP. The original DrawPipScalePips
//! 709B6E/709D1D/70A083 draws docked spawns, passenger Size/Pip colors,
//! mind-control capacity and ammo. No duplicate counters or gameplay writes.
use crate::app::AppState;
use crate::map::entities::EntityCategory;
use crate::render::batch::SpriteInstance;
use crate::rules::object_type::{ObjectType, PipScale};
use crate::rules::ruleset::RuleSet;
use crate::sim::game_entity::GameEntity;
use crate::sim::world::Simulation;

#[derive(Debug, PartialEq)]
struct Pip {
    frame: usize,
    x: f32,
    y: f32,
}

// Bound malformed map values without limiting the supplied 40/50-place transports.
const MAX_READOUT_PIPS: usize = 512;
fn count(value: i32) -> usize {
    value.clamp(0, MAX_READOUT_PIPS as i32) as usize
}

fn readout_pips(sim: &Simulation, e: &GameEntity, obj: &ObjectType, rules: &RuleSet) -> Vec<Pip> {
    let mut pips = Vec::new();
    let mut row = |frames: Vec<usize>, y| {
        pips.extend(frames.into_iter().enumerate().map(|(i, frame)| Pip {
            frame,
            x: i as f32 * 4.0,
            y,
        }));
    };
    if obj.spawns_number > 0 {
        let docked = e
            .spawn_manager
            .as_ref()
            .map_or(0, |manager| manager.count_docked_spawns());
        row(
            (0..count(obj.spawns_number))
                .map(|i| usize::from(i < docked))
                .collect(),
            0.0,
        );
    }
    // Passengers is gated by capacity, not PipScale (709D28).
    if obj.passengers > 0 {
        // TechnoType::GetPipMax 716290 selects the width by PipScale.
        let maximum = match obj.pip_scale {
            PipScale::Passengers => count(obj.passengers),
            PipScale::Ammo => count(obj.ammo),
            PipScale::Tiberium => {
                if e.category == EntityCategory::Infantry {
                    3
                } else {
                    5
                }
            }
            PipScale::MindControl => e
                .capture_manager
                .as_ref()
                .map_or(0, |m| count(m.capacity())),
            PipScale::None => 0,
        };
        let mut frames = vec![0; maximum];
        let mut occupied = Vec::new();
        if let Some(cargo) = e.passenger_role.cargo() {
            // Cargo is LIFO; native fills backwards from total Size-1.
            for id in cargo.passengers.iter().rev() {
                let Some(passenger) = sim.entities().get(*id) else {
                    continue;
                };
                let Some(kind) = rules.object(sim.interner.resolve(passenger.type_ref())) else {
                    continue;
                };
                let frame = match passenger.category {
                    EntityCategory::Infantry => kind.passenger_pip,
                    EntityCategory::Unit => 5,
                    _ => 1,
                };
                occupied.push(frame);
                for _ in 1..kind.size.clamp(1, MAX_READOUT_PIPS as u32) {
                    if occupied.len() >= frames.len() {
                        break;
                    }
                    occupied.push(3);
                }
                if occupied.len() >= frames.len() {
                    break;
                }
            }
        }
        for (slot, frame) in frames.iter_mut().zip(occupied) {
            *slot = frame;
        }
        row(frames, 0.0);
    } else {
        match obj.pip_scale {
            PipScale::Tiberium => {
                if let Some(miner) = &e.miner {
                    // Preserve the existing ore/gem display proportions.
                    let filled = usize::from(miner.cargo_pips()).min(5);
                    let gems = miner
                        .cargo
                        .iter()
                        .filter(|b| b.resource_type == crate::sim::miner::ResourceType::Gem)
                        .count();
                    let gem_pips = if miner.cargo.is_empty() {
                        0
                    } else {
                        (gems * filled).div_ceil(miner.cargo.len())
                    };
                    row(
                        (0..5)
                            .map(|i| {
                                if i < gem_pips {
                                    5
                                } else if i < filled {
                                    2
                                } else {
                                    0
                                }
                            })
                            .collect(),
                        0.0,
                    );
                }
            }
            PipScale::Ammo => {
                let rounds = e
                    .aircraft_ammo
                    .as_ref()
                    .map_or(
                        if obj.initial_ammo == -1 {
                            obj.ammo
                        } else {
                            obj.initial_ammo
                        },
                        |a| a.current,
                    )
                    .max(0);
                if obj.pip_wrap > 0 {
                    let columns = count(obj.pip_wrap);
                    let layers = obj.ammo / obj.pip_wrap;
                    row(
                        (0..columns)
                            .map(|i| {
                                let filled = ((rounds - 1 - i as i32) / obj.pip_wrap + 1)
                                    .clamp(0, layers.max(0));
                                if rounds <= i as i32 || filled == 0 {
                                    14
                                } else {
                                    14 + filled as usize
                                }
                            })
                            .collect(),
                        -3.0,
                    );
                } else {
                    // Native draws frame13 for live rounds only, no empty bullets.
                    row(vec![13; count(rounds.min(obj.ammo))], -3.0);
                }
            }
            PipScale::MindControl => {
                if let Some(manager) = &e.capture_manager {
                    let capacity = count(manager.capacity());
                    let mut frames: Vec<_> = (0..capacity)
                        .map(|i| usize::from(i < count(manager.count())))
                        .collect();
                    if manager.is_overloaded() {
                        frames.push(4);
                    }
                    row(frames, 0.0);
                }
            }
            _ => {}
        }
    }
    pips
}

pub(crate) fn build_cargo_pip_instances(state: &AppState, sw: f32, sh: f32) -> Vec<SpriteInstance> {
    let (Some(runtime), Some(overlay)) = (
        &state.match_state.sim_runtime,
        &state.match_state.match_presentation.selection_overlay,
    ) else {
        return Vec::new();
    };
    let Some(art) = overlay.unit_readout_pips() else {
        return Vec::new();
    };
    let sim = &runtime.simulation;
    let owner = crate::app::input::commands::preferred_local_owner(state)
        .and_then(|n| sim.interner.get(&n));
    let mut instances = Vec::new();
    for e in sim.entities().values() {
        if !e.selected
            || e.category == EntityCategory::Structure
            || !e.lifecycle.object_alive
            || e.lifecycle.in_limbo
            || e.health.current <= 0
            || e.sinking.is_active()
            || e.passenger_role.is_inside_transport()
        {
            continue;
        }
        if !super::ui_overlays::status_entity_visible_plain(
            owner,
            &sim.fog,
            &e.position,
            e.owner(),
            state.match_state.sandbox_full_visibility,
        ) {
            continue;
        }
        let Some(obj) = runtime
            .resources
            .rules
            .object(sim.interner.resolve(e.type_ref()))
        else {
            continue;
        };
        let (sx, sy) = super::instances::interpolated_screen_position_entity(e);
        let x = sx - 15.0
            + if e.category == EntityCategory::Infantry {
                11.0
            } else {
                0.0
            };
        let y = sy + 10.0 + obj.pixel_selection_bracket_delta as f32;
        // Cull before reading passenger lists; large A20 selections stay bounded.
        if x + MAX_READOUT_PIPS as f32 * 4.0 < state.match_state.input.camera_x - 48.0
            || x > state.match_state.input.camera_x + sw + 48.0
            || y < state.match_state.input.camera_y - 48.0
            || y > state.match_state.input.camera_y + sh + 48.0
        {
            continue;
        }
        for pip in readout_pips(sim, e, obj, &runtime.resources.rules) {
            if let Some(instance) = art.instance(pip.frame, (x + pip.x, y + pip.y), 0.0004) {
                instances.push(instance);
            }
        }
    }
    instances
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Simulation, RuleSet, u64, u64, u64) {
        let (mut sim, rules, _) = crate::sim::world::entry_test_fixture::fixture_with_rules(
            "[VehicleTypes]\n0=BUS\n[BUS]\nStrength=300\nPassengers=50\nPipScale=Passengers\n[InfantryTypes]\n2=RED\n3=BLUE\n[RED]\nStrength=100\nPip=red\nSize=1\n[BLUE]\nStrength=100\nPip=blue\nSize=2\n[HORNET]\nAmmo=4\nPipScale=Ammo\n",
        );
        let bus = sim
            .spawn_object("BUS", "Americans", 16, 15, 0, &rules)
            .unwrap();
        let red = sim
            .spawn_object("RED", "Americans", 17, 15, 0, &rules)
            .unwrap();
        let blue = sim
            .spawn_object("BLUE", "Americans", 17, 16, 0, &rules)
            .unwrap();
        (sim, rules, bus, red, blue)
    }
    #[test]
    fn unit_readouts_transport_colors_size_and_unloading() {
        let (mut sim, rules, bus, red, blue) = fixture();
        let cargo = sim
            .substrate
            .entities
            .get_mut(bus)
            .unwrap()
            .passenger_role
            .cargo_mut()
            .unwrap();
        cargo.board(red, 1);
        cargo.board(blue, 2);
        let frames = |sim: &Simulation| {
            readout_pips(
                sim,
                sim.entities().get(bus).unwrap(),
                rules.object("BUS").unwrap(),
                &rules,
            )
        };
        let pips = frames(&sim);
        assert_eq!(pips.len(), 50);
        assert_eq!(
            pips.iter().take(4).map(|p| p.frame).collect::<Vec<_>>(),
            vec![4, 5, 3, 0]
        );
        assert_eq!(pips[49].x, 196.0);
        sim.substrate
            .entities
            .get_mut(bus)
            .unwrap()
            .passenger_role
            .cargo_mut()
            .unwrap()
            .disembark(blue);
        assert_eq!(
            frames(&sim)
                .iter()
                .take(3)
                .map(|p| p.frame)
                .collect::<Vec<_>>(),
            vec![4, 0, 0]
        );
    }
    #[test]
    fn unit_readouts_ammo_tracks_live_magazine_and_wraps() {
        let (mut sim, rules, _, _, _) = fixture();
        let id = sim
            .spawn_object("HORNET", "Americans", 15, 15, 0, &rules)
            .unwrap();
        for n in [4, 2, 0] {
            sim.substrate
                .entities
                .get_mut(id)
                .unwrap()
                .aircraft_ammo
                .as_mut()
                .unwrap()
                .current = n;
            let pips = readout_pips(
                &sim,
                sim.entities().get(id).unwrap(),
                rules.object("HORNET").unwrap(),
                &rules,
            );
            assert_eq!(pips.len(), n as usize);
            assert!(pips.iter().all(|p| p.frame == 13 && p.y == -3.0));
        }
        let mut obj = rules.object("HORNET").unwrap().clone();
        obj.pip_wrap = 2;
        sim.substrate
            .entities
            .get_mut(id)
            .unwrap()
            .aircraft_ammo
            .as_mut()
            .unwrap()
            .current = 3;
        assert_eq!(
            readout_pips(&sim, sim.entities().get(id).unwrap(), &obj, &rules)
                .iter()
                .map(|p| p.frame)
                .collect::<Vec<_>>(),
            vec![16, 15]
        );
    }
    #[test]
    fn unit_readouts_spawn_launch_rearm_and_mind_control_capacity() {
        use crate::sim::house_state::HouseState;
        use crate::sim::spawn_manager::SpawnSlotState;
        let (mut sim, rules, _) = crate::sim::world::entry_test_fixture::fixture_with_rules(
            "[VehicleTypes]\n0=MIND\n1=SHIP\n[MIND]\nStrength=500\nPrimary=Control\nPipScale=MindControl\n[SHIP]\nStrength=500\nSpawns=HORNET\nSpawnsNumber=3\n[Control]\nDamage=3\nROF=100\nRange=7\nProjectile=Invisible\nWarhead=Controller\nInfiniteMindControl=yes\n[Warheads]\n2=Controller\n[Controller]\nMindControl=yes\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n",
        );
        for name in ["Americans", "Russians"] {
            let owner = sim.interner.intern(name);
            sim.houses
                .insert(owner, HouseState::new(owner, 0, None, true, 0, 10));
            sim.session.house_order.push(owner);
        }
        let carrier = sim
            .spawn_object("SHIP", "Americans", 16, 15, 0, &rules)
            .unwrap();
        {
            let manager = sim
                .substrate
                .entities
                .get_mut(carrier)
                .unwrap()
                .spawn_manager
                .as_mut()
                .unwrap();
            manager.slots[0].state = SpawnSlotState::ReadyDocked;
            manager.slots[1].state = SpawnSlotState::Reloading;
            manager.slots[2].state = SpawnSlotState::Regenerating;
        }
        let frames = |sim: &Simulation, id, name| {
            readout_pips(
                sim,
                sim.entities().get(id).unwrap(),
                rules.object(name).unwrap(),
                &rules,
            )
            .into_iter()
            .map(|p| p.frame)
            .collect::<Vec<_>>()
        };
        assert_eq!(frames(&sim, carrier, "SHIP"), vec![1, 1, 0]);
        sim.substrate
            .entities
            .get_mut(carrier)
            .unwrap()
            .spawn_manager
            .as_mut()
            .unwrap()
            .slots[0]
            .state = SpawnSlotState::Regenerating;
        assert_eq!(frames(&sim, carrier, "SHIP"), vec![1, 0, 0]);
        let mind = sim
            .spawn_object("MIND", "Americans", 17, 15, 0, &rules)
            .unwrap();
        assert_eq!(frames(&sim, mind, "MIND"), vec![0, 0, 0]);
        let mut victims = Vec::new();
        for n in 0..4 {
            let victim = sim
                .spawn_object("ENGINEER", "Russians", 17, 16 + n, 0, &rules)
                .unwrap();
            assert!(sim.capture_unit(
                mind,
                victim,
                &rules,
                None,
                crate::sim::world::FrameEffects::default()
            ));
            victims.push(victim);
        }
        assert_eq!(frames(&sim, mind, "MIND"), vec![1, 1, 1, 4]);
        sim.free_unit(
            mind,
            victims[0],
            &rules,
            None,
            crate::sim::world::FrameEffects::default(),
        );
        assert_eq!(frames(&sim, mind, "MIND"), vec![1, 1, 1]);
    }
}
