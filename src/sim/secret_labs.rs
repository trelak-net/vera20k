//! Secret Lab rewards: scenario selection68C050, GetSecretProduction459840,
//! CanBuild4F78E4. House+110 has the same append/removal events as House+68,
//! filtered by SecretLab (Unlimbo441449, ChangeOwner448989/449091, expiry).
use crate::rules::{
    object_type::{ObjectCategory, ObjectType},
    ruleset::RuleSet,
};
use crate::sim::{intern::InternedId, world::Simulation};

const CATEGORIES: [ObjectCategory; 3] = [
    ObjectCategory::Infantry,
    ObjectCategory::Vehicle,
    ObjectCategory::Building,
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct SecretLabReward {
    category: ObjectCategory,
    type_id: InternedId,
}

/// Native removes an index from a temporary vector but uses the drawn index
/// itself, NOT the removed value, to address the original pools. Thus rewards
/// can repeat and later draws see a shrinking PREFIX. Preserve that behavior.
fn select_rewards<T: Copy>(
    pool: &[T],
    count: usize,
    mut draw: impl FnMut(u32, u32) -> u32,
) -> Vec<T> {
    if count == 0 || pool.is_empty() || pool.len() < count {
        return Vec::new();
    }
    (0..count)
        .map(|i| pool[draw(0, (pool.len() - i - 1) as u32) as usize])
        .collect()
}
impl Simulation {
    pub(crate) fn initialize_secret_labs(&mut self, rules: &RuleSet) {
        // Stable IDs preserve building construction order. Includes labs with
        // an explicit reward: original still assigns their unused fallback.
        let labs: Vec<_> = self
            .entities()
            .values_sorted()
            .filter(|e| {
                e.category == crate::map::entities::EntityCategory::Structure
                    && self
                        .object_type(e.type_ref(), rules)
                        .is_some_and(|ty| ty.secret_lab)
            })
            .map(|e| e.stable_id())
            .collect();
        if labs.is_empty() {
            return;
        }
        let mut pool = Vec::new();
        for (category, ids) in CATEGORIES.into_iter().zip(&rules.general.secret_lab_pools) {
            for id in ids {
                pool.push(SecretLabReward {
                    category,
                    type_id: self.interner.intern(id),
                });
            }
        }
        let rewards = select_rewards(&pool, labs.len(), |lo, hi| {
            self.scenario_rng.next_range_u32_inclusive(lo, hi)
        });
        for (id, reward) in labs.into_iter().zip(rewards) {
            self.substrate
                .entities
                .get_mut(id)
                .unwrap()
                .secret_lab_reward = Some(reward);
        }
    }
    fn secret_production(&self, id: u64, ty: &ObjectType) -> Option<SecretLabReward> {
        for (category, name) in CATEGORIES.into_iter().zip(&ty.secret_lab_fixed) {
            if let Some(type_id) = name.as_ref().and_then(|id| self.interner.get(id)) {
                return Some(SecretLabReward { category, type_id });
            }
        }
        self.entities().get(id)?.secret_lab_reward
    }
    pub(crate) fn secret_lab_grants(
        &self,
        rules: &RuleSet,
        owner: InternedId,
        obj: &ObjectType,
    ) -> bool {
        if !obj.secret_lab_grantable {
            return false;
        }
        let Some(type_id) = self.interner.get(&obj.id) else {
            return false;
        };
        let Some(house) = self.houses.get(&owner) else {
            return false;
        };
        let reward = SecretLabReward {
            category: obj.category,
            type_id,
        };
        house.base_projection.buildings().iter().any(|&id| {
            let Some(e) = self.entities().get(id) else {
                return false;
            };
            let Some(ty) = self.object_type(e.type_ref(), rules) else {
                return false;
            };
            ty.secret_lab && self.secret_production(id, ty) == Some(reward)
        })
    }
}
#[cfg(test)]
#[path = "secret_labs_tests.rs"]
mod tests;
