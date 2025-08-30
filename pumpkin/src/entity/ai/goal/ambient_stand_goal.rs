use super::{Goal, GoalControl};
use crate::entity::mob::Mob;

use rand::Rng;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;

#[allow(dead_code)]
pub struct AmbientStandGoal {
    goal_control: GoalControl,
    cooldown: AtomicI32,
}

impl Default for AmbientStandGoal {
    fn default() -> Self {
        let entity = Self {
            goal_control: GoalControl::default(),
            cooldown: AtomicI32::new(0),
        };
        entity.reset_cooldown();

        entity
    }
}

impl AmbientStandGoal {
    fn reset_cooldown(&self) {
        // TODO: should be: this.cooldown = -entity.getMinAmbientStandDelay();
        // TODO: implement when Horses are implemented
        self.cooldown.store(0, Relaxed);
    }
}


impl Goal for AmbientStandGoal {
    fn can_start(&self, mob: &dyn Mob) -> bool {
        let cooldown = self.cooldown.fetch_add(1, Relaxed) + 1;
        if cooldown > 0 && mob.get_random().random_range(0..1000) < cooldown {
            self.reset_cooldown();
        }

        false
    }
    fn should_continue(&self, _mob: &dyn Mob) -> bool {
        false
    }

    fn start(&self, _mob: &dyn Mob) {}

    fn stop(&self, _mob: &dyn Mob) {}

    fn tick(&self, _mob: &dyn Mob) {}

    fn get_goal_control(&self) -> &GoalControl {
        &self.goal_control
    }
}
