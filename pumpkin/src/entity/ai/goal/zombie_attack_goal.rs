use super::{Goal, GoalControl};
use crate::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::entity::mob::Mob;

use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;

pub struct ZombieAttackGoal {
    melee_attack_goal: MeleeAttackGoal,
    ticks: AtomicI32,
}

impl ZombieAttackGoal {
    #[must_use]
    pub fn new(speed: f64, pause_when_mob_idle: bool) -> Self {
        Self {
            melee_attack_goal: MeleeAttackGoal::new(speed, pause_when_mob_idle),
            ticks: AtomicI32::new(0),
        }
    }
}


impl Goal for ZombieAttackGoal {
    fn can_start(&self, mob: &dyn Mob) -> bool {
        self.melee_attack_goal.can_start(mob)
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.melee_attack_goal.should_continue(mob)
    }

    fn start(&self, mob: &dyn Mob) {
        self.melee_attack_goal.start(mob);
        self.ticks.store(0, Relaxed);
    }

    fn stop(&self, mob: &dyn Mob) {
        self.melee_attack_goal.stop(mob);
        mob.get_mob_entity().set_attacking(false);
    }

    fn tick(&self, mob: &dyn Mob) {
        self.melee_attack_goal.tick(mob);
        let ticks = self.ticks.fetch_add(1, Relaxed) + 1;
        if ticks >= 5
            && self.melee_attack_goal.cooldown.load(Relaxed)
                < self.melee_attack_goal.get_max_cooldown()
        {
            mob.get_mob_entity().set_attacking(true);
        } else {
            mob.get_mob_entity().set_attacking(false);
        }
    }

    fn should_run_every_tick(&self) -> bool {
        self.melee_attack_goal.should_run_every_tick()
    }

    fn get_goal_control(&self) -> &GoalControl {
        self.melee_attack_goal.get_goal_control()
    }
}
