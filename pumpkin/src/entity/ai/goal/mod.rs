use crate::entity::mob::Mob;

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use parking_lot::RwLock;

pub mod active_target_goal;
pub mod ambient_stand_goal;
pub mod goal_selector;
pub mod look_around_goal;
pub mod look_at_entity;
mod melee_attack_goal;
pub mod move_to_target_pos_goal;
pub mod step_and_destroy_block_goal;
mod track_target_goal;
pub mod zombie_attack_goal;

#[must_use]
pub fn to_goal_ticks(server_ticks: i32) -> i32 {
    -(-server_ticks).div_euclid(2)
}


pub trait Goal: Send + Sync {
    /// How should the `Goal` initially start?
    fn can_start(&self, mob: &dyn Mob) -> bool;
    /// When it's started, how should it continue to run?
    fn should_continue(&self, mob: &dyn Mob) -> bool;
    /// Call when goal start
    fn start(&self, mob: &dyn Mob);
    /// Call when goal stop
    fn stop(&self, mob: &dyn Mob);
    /// If the `Goal` is running, this gets called every tick.
    fn tick(&self, mob: &dyn Mob);

    fn should_run_every_tick(&self) -> bool {
        false
    }

    fn can_stop(&self) -> bool {
        true
    }

    fn get_tick_count(&self, ticks: i32) -> i32 {
        if self.should_run_every_tick() {
            ticks
        } else {
            to_goal_ticks(ticks)
        }
    }

    fn get_goal_control(&self) -> &GoalControl;

    fn set_controls(&self, controls: &[Control]) {
        let mut self_controls = self.get_goal_control().controls.write();
        self_controls.clear();
        self_controls.extend(controls);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    Move,
    Look,
    Jump,
    Target,
}

pub struct GoalControl {
    pub controls: RwLock<HashSet<Control>>,
}

impl GoalControl {
    #[must_use]
    pub fn new(controls: HashSet<Control>) -> Self {
        Self {
            controls: RwLock::new(controls),
        }
    }

    #[must_use]
    pub fn from_array(controls: &[Control]) -> Self {
        Self::new(controls.iter().copied().collect())
    }
}

impl Default for GoalControl {
    fn default() -> Self {
        Self {
            controls: RwLock::new(HashSet::new()),
        }
    }
}

pub struct PrioritizedGoal {
    pub goal: Arc<dyn Goal>,
    pub running: AtomicBool,
    pub priority: u8,
}

impl PrioritizedGoal {
    pub fn new(priority: u8, goal: Arc<dyn Goal>) -> Self {
        Self {
            goal,
            running: AtomicBool::new(false),
            priority,
        }
    }

    fn can_be_replaced_by(&self, goal: &Arc<Self>) -> bool {
        self.can_stop() && goal.priority < self.priority
    }
}


impl Goal for PrioritizedGoal {
    fn can_start(&self, mob: &dyn Mob) -> bool {
        self.goal.can_start(mob)
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.goal.should_continue(mob)
    }

    fn start(&self, mob: &dyn Mob) {
        if !self.running.load(Relaxed) {
            self.running.store(true, Relaxed);
            self.goal.start(mob);
        }
    }

    fn stop(&self, mob: &dyn Mob) {
        if self.running.load(Relaxed) {
            self.running.store(false, Relaxed);
            self.goal.stop(mob);
        }
    }

    fn tick(&self, mob: &dyn Mob) {
        self.goal.tick(mob);
    }
    fn should_run_every_tick(&self) -> bool {
        self.goal.should_run_every_tick()
    }

    fn get_tick_count(&self, ticks: i32) -> i32 {
        self.goal.get_tick_count(ticks)
    }

    fn get_goal_control(&self) -> &GoalControl {
        self.goal.get_goal_control()
    }

    fn set_controls(&self, controls: &[Control]) {
        self.goal.set_controls(controls);
    }
}
