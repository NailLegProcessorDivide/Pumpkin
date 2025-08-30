use std::sync::Arc;


use pumpkin_data::fluid::Fluid;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{BlockStateId, tick::TickPriority};

use crate::{block::fluid::FluidBehaviour, entity::EntityBase, world::World};

use super::flowing::FlowingFluid;

#[pumpkin_block("minecraft:flowing_water")]
pub struct FlowingWater;

const WATER_FLOW_SPEED: u8 = 5;

impl FluidBehaviour for FlowingWater {
    fn placed(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        state_id: BlockStateId,
        block_pos: &BlockPos,
        old_state_id: BlockStateId,
        _notify: bool,
    ) {
        if old_state_id != state_id {
            world.schedule_fluid_tick(fluid, *block_pos, WATER_FLOW_SPEED, TickPriority::Normal);
        }
    }

    fn on_scheduled_tick(&self, world: &Arc<World>, fluid: &Fluid, block_pos: &BlockPos) {
        self.spread_fluid(world, fluid, block_pos);
    }

    fn on_neighbor_update(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        block_pos: &BlockPos,
        _notify: bool,
    ) {
        world.schedule_fluid_tick(fluid, *block_pos, WATER_FLOW_SPEED, TickPriority::Normal);
    }

    fn on_entity_collision(&self, entity: &dyn EntityBase) {
        entity.get_entity().extinguish();
    }
}

impl FlowingFluid for FlowingWater {
    fn get_drop_off(&self) -> i32 {
        1
    }

    fn get_slope_find_distance(&self) -> i32 {
        4
    }

    fn can_convert_to_source(&self, _world: &Arc<World>) -> bool {
        //TODO add game rule check for water conversion
        true
    }
}
