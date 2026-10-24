//! Player waypoint paths, separate from Foot NavQueue. The installed RA2
//! manual and gamemd PlanMgr (6379C0/637A10/637E00) establish held input,
//! streamed clicks, House-wide commit, editable nodes and looping routes.
//! This is VERA's deterministic command representation, not a claim that its
//! commands encode the original PLANCONNECT/PLANCOMMIT wire bytes.
use crate::rules::{overlay_types::OverlayTypeRegistry, ruleset::RuleSet};
use crate::sim::command::Command;
use crate::sim::intern::InternedId;
use crate::sim::world::{FrameEffects, Simulation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

const MAX_PATHS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlannedAction {
    Move,
    AttackMove,
    Attack {
        target: u64,
        force: bool,
    },
    ForceFire,
    Guard {
        target: Option<crate::sim::combat::TargetKind>,
    },
    Harvest,
    Capture {
        target: u64,
    },
    C4 {
        target: u64,
    },
    Enter {
        target: u64,
    },
    Repair {
        target: u64,
    },
    Deploy,
    InfantryDeploy,
    EnterBunker {
        target: u64,
    },
    Unload,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlannedOrder {
    pub actor: u64,
    pub cell: (u16, u16),
    pub action: PlannedAction,
}

impl PlannedOrder {
    pub(crate) fn from_command(command: &Command, sim: &Simulation) -> Option<Self> {
        use PlannedAction as A;
        let actor = command_actor(command)?;
        let mut cell = sim
            .entities()
            .get(actor)
            .map(|e| (e.position.rx, e.position.ry))?;
        let target_cell = |target| {
            sim.entities()
                .get(target)
                .map(|e| (e.position.rx, e.position.ry))
        };
        let action = match *command {
            Command::Move {
                target_rx,
                target_ry,
                ..
            } => {
                cell = (target_rx, target_ry);
                A::Move
            }
            Command::AttackMove {
                target_rx,
                target_ry,
                ..
            } => {
                cell = (target_rx, target_ry);
                A::AttackMove
            }
            Command::Attack { target_id, .. } | Command::ForceAttack { target_id, .. } => {
                cell = target_cell(target_id)?;
                A::Attack {
                    target: target_id,
                    force: matches!(command, Command::ForceAttack { .. }),
                }
            }
            Command::ForceAttackCell {
                target_rx,
                target_ry,
                ..
            } => {
                cell = (target_rx, target_ry);
                A::ForceFire
            }
            Command::Guard { target, .. } => {
                match target {
                    Some(crate::sim::combat::TargetKind::Cell(x, y)) => cell = (x, y),
                    Some(crate::sim::combat::TargetKind::Entity(id)) => cell = target_cell(id)?,
                    None => {}
                }
                A::Guard { target }
            }
            Command::HarvestCell {
                target_rx,
                target_ry,
                ..
            } => {
                cell = (target_rx, target_ry);
                A::Harvest
            }
            Command::CaptureBuilding {
                target_building_id, ..
            } => {
                cell = target_cell(target_building_id)?;
                A::Capture {
                    target: target_building_id,
                }
            }
            Command::PlantC4 {
                target_building_id, ..
            } => {
                cell = target_cell(target_building_id)?;
                A::C4 {
                    target: target_building_id,
                }
            }
            Command::EnterTransport { transport_id, .. } => {
                cell = target_cell(transport_id)?;
                A::Enter {
                    target: transport_id,
                }
            }
            Command::RepairAtDepot { depot_id, .. } => {
                cell = target_cell(depot_id)?;
                A::Repair { target: depot_id }
            }
            Command::DeployMcv { .. } => A::Deploy,
            Command::ToggleInfantryDeploy { .. } => A::InfantryDeploy,
            Command::EnterBunker { bunker_id, .. } => {
                cell = target_cell(bunker_id)?;
                A::EnterBunker { target: bunker_id }
            }
            Command::UnloadPassengers { .. } => A::Unload,
            _ => return None,
        };
        Some(Self {
            actor,
            cell,
            action,
        })
    }
    fn command(&self) -> Command {
        use PlannedAction as A;
        let entity_id = self.actor;
        let (target_rx, target_ry) = self.cell;
        match self.action {
            A::Move => Command::Move {
                entity_id,
                target_rx,
                target_ry,
                queue: false,
            },
            A::AttackMove => Command::AttackMove {
                entity_id,
                target_rx,
                target_ry,
                queue: false,
            },
            A::Attack {
                target,
                force: false,
            } => Command::Attack {
                attacker_id: entity_id,
                target_id: target,
            },
            A::Attack {
                target,
                force: true,
            } => Command::ForceAttack {
                attacker_id: entity_id,
                target_id: target,
            },
            A::ForceFire => Command::ForceAttackCell {
                attacker_id: entity_id,
                target_rx,
                target_ry,
            },
            A::Guard { target } => Command::Guard { entity_id, target },
            A::Harvest => Command::HarvestCell {
                entity_id,
                target_rx,
                target_ry,
            },
            A::Capture { target } => Command::CaptureBuilding {
                engineer_id: entity_id,
                target_building_id: target,
            },
            A::C4 { target } => Command::PlantC4 {
                attacker_id: entity_id,
                target_building_id: target,
            },
            A::Enter { target } => Command::EnterTransport {
                passenger_id: entity_id,
                transport_id: target,
            },
            A::Repair { target } => Command::RepairAtDepot {
                entity_id,
                depot_id: target,
            },
            A::Deploy => Command::DeployMcv { entity_id },
            A::InfantryDeploy => Command::ToggleInfantryDeploy { entity_id },
            A::EnterBunker { target } => Command::EnterBunker {
                unit_id: entity_id,
                bunker_id: target,
            },
            A::Unload => Command::UnloadPassengers {
                transport_id: entity_id,
            },
        }
    }
}

pub(crate) fn command_actor(command: &Command) -> Option<u64> {
    match *command {
        Command::Move { entity_id, .. }
        | Command::Stop { entity_id }
        | Command::Scatter { entity_id }
        | Command::AttackMove { entity_id, .. }
        | Command::Guard { entity_id, .. }
        | Command::DeployMcv { entity_id }
        | Command::ToggleInfantryDeploy { entity_id }
        | Command::HarvestCell { entity_id, .. }
        | Command::MinerReturn { entity_id, .. }
        | Command::RepairAtDepot { entity_id, .. }
        | Command::UndeployBuilding { entity_id } => Some(entity_id),
        Command::Attack { attacker_id, .. }
        | Command::ForceAttack { attacker_id, .. }
        | Command::ForceAttackCell { attacker_id, .. }
        | Command::PlantC4 { attacker_id, .. } => Some(attacker_id),
        Command::CaptureBuilding { engineer_id, .. } => Some(engineer_id),
        Command::EnterTransport { passenger_id, .. } => Some(passenger_id),
        Command::EnterBunker { unit_id, .. } => Some(unit_id),
        Command::UnloadPassengers { transport_id } => Some(transport_id),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct WaypointProgress {
    pub node: usize,
    issued: bool,
    issued_frame: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct WaypointPath {
    pub id: u32,
    pub members: BTreeMap<u64, WaypointProgress>,
    pub nodes: Vec<Vec<PlannedOrder>>,
    pub loop_to: Option<usize>,
    pub committed: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct WaypointPlans {
    next_id: u32,
    pub houses: BTreeMap<InternedId, Vec<WaypointPath>>,
}
impl WaypointPlans {
    pub(crate) fn hash_state(&self, hasher: &mut impl Hasher) {
        if self.next_id != 0 || !self.houses.is_empty() {
            b"waypoint-plans-v1".hash(hasher);
            self.hash(hasher);
        }
    }
    pub(crate) fn detach(&mut self, id: u64) {
        for paths in self.houses.values_mut() {
            for path in paths.iter_mut() {
                path.members.remove(&id);
            }
            paths.retain(|p| !p.members.is_empty());
        }
        self.houses.retain(|_, p| !p.is_empty());
    }
    pub(crate) fn commit(&mut self, owner: InternedId) {
        if let Some(paths) = self.houses.get_mut(&owner) {
            for path in paths {
                path.committed = true;
            }
        }
    }
    pub(crate) fn delete_node(&mut self, owner: InternedId, path_id: u32, node: usize) -> bool {
        let Some(paths) = self.houses.get_mut(&owner) else {
            return false;
        };
        let Some(path) = paths.iter_mut().find(|p| p.id == path_id) else {
            return false;
        };
        if node >= path.nodes.len() {
            return false;
        }
        path.nodes.remove(node);
        path.loop_to = path.loop_to.and_then(|n| {
            if path.nodes.len() < 2 {
                None
            } else {
                Some(if n > node {
                    n - 1
                } else {
                    n.min(path.nodes.len() - 1)
                })
            }
        });
        for progress in path.members.values_mut() {
            if progress.node > node {
                progress.node -= 1;
            } else if progress.node == node {
                progress.issued = false;
                if progress.node >= path.nodes.len() {
                    if let Some(loop_to) = path.loop_to {
                        progress.node = loop_to;
                    }
                }
            }
        }
        paths.retain(|p| !p.nodes.is_empty());
        self.houses.retain(|_, p| !p.is_empty());
        true
    }
    pub(crate) fn loop_path(&mut self, owner: InternedId, path_id: u32, node: usize) -> bool {
        let Some(path) = self
            .houses
            .get_mut(&owner)
            .and_then(|p| p.iter_mut().find(|p| p.id == path_id))
        else {
            return false;
        };
        if path.nodes.len() < 2 || node >= path.nodes.len() - 1 {
            return false;
        }
        path.loop_to = Some(node);
        true
    }
}
impl Simulation {
    pub(crate) fn delete_waypoint(
        &mut self,
        owner_name: &str,
        path_id: u32,
        node: usize,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        let Some(owner) = self.interner.get(owner_name) else {
            return false;
        };
        // Stop a removed current order without cancelling the remaining plan.
        let ids = self
            .waypoint_plans
            .houses
            .get(&owner)
            .and_then(|p| p.iter().find(|p| p.id == path_id))
            .map(|p| {
                p.members
                    .iter()
                    .filter(|(_, progress)| progress.node == node)
                    .map(|(&id, _)| id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut plans = std::mem::take(&mut self.waypoint_plans);
        let deleted = plans.delete_node(owner, path_id, node);
        if deleted {
            for id in ids {
                self.apply_command_with_overlays(
                    owner_name,
                    &Command::Stop { entity_id: id },
                    rules,
                    registry,
                    effects,
                );
            }
        }
        self.waypoint_plans = plans;
        deleted
    }

    pub(crate) fn connect_waypoints(
        &mut self,
        owner_name: &str,
        orders: &[PlannedOrder],
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        let Some(owner) = self.interner.get(owner_name) else {
            return false;
        };
        let orders: Vec<_> = orders
            .iter()
            .filter(|o| {
                self.order_actor_admits(o.actor)
                    && self.entities().get(o.actor).is_some_and(|e| {
                        e.owner() == owner
                            && e.category != crate::map::entities::EntityCategory::Structure
                    })
            })
            .cloned()
            .collect();
        if orders.is_empty() {
            return false;
        }
        let members: BTreeMap<_, _> = orders
            .iter()
            .map(|o| (o.actor, WaypointProgress::default()))
            .collect();
        let limit = rules.map_or(15, |r| r.general.max_waypoint_path_length);
        if limit == 0 {
            return false;
        }
        let existing = self.waypoint_plans.houses.get(&owner).and_then(|paths| {
            paths
                .iter()
                .position(|p| !p.committed && p.members.keys().eq(members.keys()))
        });
        if let Some(index) = existing {
            let path = &mut self.waypoint_plans.houses.get_mut(&owner).unwrap()[index];
            if path.nodes.len() >= limit || path.loop_to.is_some() {
                return false;
            }
            path.nodes.push(orders);
            log::debug!(
                "Waypoint path {}: {} nodes, {} members",
                path.id,
                path.nodes.len(),
                path.members.len()
            );
            return true;
        }
        if self.waypoint_plans.houses.get(&owner).is_some_and(|paths| {
            paths
                .iter()
                .filter(|p| p.members.keys().any(|id| !members.contains_key(id)))
                .count()
                >= MAX_PATHS
        }) {
            return false;
        }
        for &id in members.keys() {
            self.waypoint_plans.detach(id);
            self.apply_command_with_overlays(
                owner_name,
                &Command::Stop { entity_id: id },
                rules,
                registry,
                effects,
            );
        }
        let id = self.waypoint_plans.next_id;
        self.waypoint_plans.next_id = self.waypoint_plans.next_id.wrapping_add(1);
        log::debug!("Waypoint path {id}: 1 node, {} members", members.len());
        self.waypoint_plans
            .houses
            .entry(owner)
            .or_default()
            .push(WaypointPath {
                id,
                members,
                nodes: vec![orders],
                loop_to: None,
                committed: false,
            });
        true
    }
    /// Only planned actors are visited: an ordinary A20 frame pays an empty
    /// map check, not another scan across thousands of map objects.
    pub(crate) fn tick_waypoint_plans(
        &mut self,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) {
        if self.waypoint_plans.houses.is_empty() {
            return;
        }
        let mut plans = std::mem::take(&mut self.waypoint_plans);
        for (&owner, paths) in &mut plans.houses {
            let owner_name = self.interner.resolve(owner).to_owned();
            for path in paths.iter_mut() {
                path.members.retain(|&id, _| {
                    self.order_actor_admits(id)
                        && self.entities().get(id).is_some_and(|e| e.owner() == owner)
                });
                if !path.committed {
                    continue;
                }
                for (&id, progress) in &mut path.members {
                    if progress.issued && progress.issued_frame != self.session.binary_frame {
                        let order = path
                            .nodes
                            .get(progress.node)
                            .and_then(|n| n.iter().find(|o| o.actor == id));
                        if order.is_none_or(|o| self.planned_order_finished(o)) {
                            progress.node += 1;
                            if progress.node >= path.nodes.len() {
                                if let Some(loop_to) = path.loop_to {
                                    progress.node = loop_to;
                                }
                            }
                            progress.issued = false;
                        }
                    }
                    if !progress.issued && progress.node < path.nodes.len() {
                        let Some(order) = path.nodes[progress.node].iter().find(|o| o.actor == id)
                        else {
                            progress.node += 1;
                            continue;
                        };
                        if self.apply_command_with_overlays(
                            &owner_name,
                            &order.command(),
                            rules,
                            registry,
                            effects,
                        ) {
                            progress.issued = true;
                            progress.issued_frame = self.session.binary_frame;
                            log::debug!(
                                "Waypoint path {}: actor {id} executes node {} at {:?}",
                                path.id,
                                progress.node + 1,
                                order.cell
                            );
                        } else {
                            // A destroyed target or rejected special action cannot
                            // wedge every later node of the surviving unit's route.
                            progress.node += 1;
                        }
                    }
                }
                path.members.retain(|_, p| p.node < path.nodes.len());
            }
            paths.retain(|p| !p.members.is_empty());
        }
        plans.houses.retain(|_, p| !p.is_empty());
        self.waypoint_plans = plans;
    }
    fn planned_order_finished(&self, order: &PlannedOrder) -> bool {
        use PlannedAction as A;
        let Some(e) = self.entities().get(order.actor) else {
            return true;
        };
        let running = |mission| {
            e.mission.current().known() == Some(mission)
                || e.mission.queued().known() == Some(mission)
        };
        match order.action {
            A::Move | A::AttackMove => {
                let dx = i32::from(e.position.rx) - i32::from(order.cell.0);
                let dy = i32::from(e.position.ry) - i32::from(order.cell.1);
                (dx == 0 && dy == 0)
                    || (dx.abs() <= 1 && dy.abs() <= 1 && e.movement_target.is_none())
            }
            A::Attack { target, .. } | A::C4 { target } => {
                self.entities().get(target).is_none_or(|t| !t.is_alive())
            }
            A::Capture { target } => self
                .entities()
                .get(target)
                .is_none_or(|t| !t.is_alive() || t.owner() == e.owner()),
            A::Enter { .. } | A::EnterBunker { .. } => e.passenger_role.is_inside_transport(),
            A::Repair { target } => {
                self.entities().get(target).is_none_or(|t| !t.is_alive())
                    || (!running(crate::sim::mission::MissionType::Enter)
                        && !running(crate::sim::mission::MissionType::Repair)
                        && e.order_intent.is_none()
                        && e.movement_target.is_none())
            }
            A::Deploy | A::InfantryDeploy | A::Unload => {
                !running(crate::sim::mission::MissionType::Unload)
                    && e.order_intent.is_none()
                    && e.movement_target.is_none()
            }
            A::Guard { .. } | A::Harvest | A::ForceFire => false,
        }
    }
}

#[cfg(test)]
#[path = "planning_tests.rs"]
mod tests;
