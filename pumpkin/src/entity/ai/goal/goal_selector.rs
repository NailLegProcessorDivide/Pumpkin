use crate::entity::ai::goal::{Control, Goal, GoalControl, PrioritizedGoal};
use crate::entity::mob::Mob;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, LazyLock};
use parking_lot::Mutex;

static REPLACEABLE_GOAL: LazyLock<Arc<PrioritizedGoal>> = LazyLock::new(|| {
    Arc::new(PrioritizedGoal::new(
        u8::MAX,
        Arc::new(DummyGoal {
            goal_control: GoalControl::default(),
        }),
    ))
});

pub struct GoalSelector {
    goals_by_control: Mutex<HashMap<Control, Arc<PrioritizedGoal>>>,
    goals: Mutex<Vec<Arc<PrioritizedGoal>>>,
    disabled_controls: Mutex<HashSet<Control>>,
}

impl Default for GoalSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl GoalSelector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            goals_by_control: Mutex::new(HashMap::new()),
            goals: Mutex::new(Vec::new()),
            disabled_controls: Mutex::new(HashSet::new()),
        }
    }

    pub fn add_goal(&self, priority: u8, goal: Arc<dyn Goal>) {
        let mut goals = self.goals.lock();
        goals.push(Arc::new(PrioritizedGoal::new(priority, goal)));
    }

    pub fn remove_goal(&self, goal: Arc<dyn Goal>, mob: &dyn Mob) {
        let mut goals = self.goals.lock();
        for prioritized_goal in goals.iter() {
            if Arc::ptr_eq(&prioritized_goal.goal, &goal) && prioritized_goal.running.load(Relaxed)
            {
                prioritized_goal.stop(mob);
            }
        }

        goals.retain(|prioritized_goal| !Arc::ptr_eq(&prioritized_goal.goal, &goal));
    }

    pub fn uses_any(
        prioritized_goal: Arc<PrioritizedGoal>,
        controls: HashSet<Control>,
    ) -> bool {
        let goal_control = prioritized_goal.get_goal_control();
        let goal_controls = goal_control.controls.read();
        for control in goal_controls.iter() {
            if controls.contains(control) {
                return true;
            }
        }

        false
    }

    pub fn can_replace_all(
        goal: Arc<PrioritizedGoal>,
        goals_by_control: &HashMap<Control, Arc<PrioritizedGoal>>,
    ) -> bool {
        let controls_lock = goal.get_goal_control().controls.read();
        for control in controls_lock.iter() {
            let existing: &Arc<PrioritizedGoal> =
                goals_by_control.get(control).unwrap_or(&*REPLACEABLE_GOAL);

            if !existing.can_be_replaced_by(&goal) {
                return false;
            }
        }
        true
    }

    pub fn tick(&self, mob: &dyn Mob) {
        let goals = self.goals.lock();
        let disabled_controls = self.disabled_controls.lock();
        for prioritized_goal in goals.iter() {
            if prioritized_goal.running.load(Relaxed)
                && (Self::uses_any(prioritized_goal.clone(), disabled_controls.clone())
                    || !prioritized_goal.should_continue(mob))
            {
                prioritized_goal.stop(mob);
            }
        }

        let mut goals_by_control = self.goals_by_control.lock();
        goals_by_control.retain(|_, prioritized_goal| prioritized_goal.running.load(Relaxed));

        for prioritized_goal in goals.iter() {
            if !prioritized_goal.running.load(Relaxed)
                && !Self::uses_any(prioritized_goal.clone(), disabled_controls.clone())
                && Self::can_replace_all(prioritized_goal.clone(), &goals_by_control)
                && prioritized_goal.can_start(mob)
            {
                let controls = prioritized_goal.get_goal_control().controls.read();
                for control in controls.iter() {
                    let goal = goals_by_control.get(control).unwrap_or(&*REPLACEABLE_GOAL);
                    goal.stop(mob);
                    goals_by_control.insert(*control, prioritized_goal.clone());
                }
                drop(controls); // Drop lock
                prioritized_goal.start(mob);
            }
        }
        // Drop locks
        drop(goals);
        drop(disabled_controls);
        drop(goals_by_control);

        self.tick_goals(mob, true);
    }

    pub fn tick_goals(&self, mob: &dyn Mob, tick_all: bool) {
        for prioritized_goal in self.goals.lock().iter() {
            if prioritized_goal.running.load(Relaxed)
                && (tick_all || prioritized_goal.should_run_every_tick())
            {
                prioritized_goal.tick(mob);
            }
        }
    }

    pub fn disable_control(&self, control: Control) {
        self.disabled_controls.lock().insert(control);
    }

    pub fn enable_control(&self, control: Control) {
        self.disabled_controls.lock().remove(&control);
    }

    pub fn set_control_enabled(&self, control: Control, enabled: bool) {
        if enabled {
            self.enable_control(control);
        } else {
            self.disable_control(control);
        }
    }
}

pub struct DummyGoal {
    goal_control: GoalControl,
}


impl Goal for DummyGoal {
    fn can_start(&self, _mob: &dyn Mob) -> bool {
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
