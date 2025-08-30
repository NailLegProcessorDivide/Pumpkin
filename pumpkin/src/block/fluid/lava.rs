use std::sync::Arc;


use pumpkin_data::{
    Block, BlockDirection,
    fluid::{Falling, Fluid, FluidProperties, Level},
    world::WorldEvent,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{BlockStateId, tick::TickPriority, world::BlockFlags};

use crate::{block::fluid::FluidBehaviour, entity::EntityBase, world::World};

use super::flowing::FlowingFluid;
type FlowingFluidProperties = pumpkin_data::fluid::FlowingWaterLikeFluidProperties;

#[pumpkin_block("minecraft:flowing_lava")]
pub struct FlowingLava;

impl FlowingLava {
    fn receive_neighbor_fluids(
        &self,
        world: &Arc<World>,
        _fluid: &Fluid,
        block_pos: &BlockPos,
    ) -> bool {
        // Logic to determine if we should replace the fluid with any of (cobble, obsidian, stone or basalt)
        let below_is_soul_soil = world
            .get_block(&block_pos.offset(BlockDirection::Down.to_offset()))
            == &Block::SOUL_SOIL;
        let is_still = world.get_block_state_id(block_pos) == Block::LAVA.default_state.id;

        for dir in BlockDirection::flow_directions() {
            let neighbor_pos = block_pos.offset(dir.opposite().to_offset());
            if world.get_block(&neighbor_pos) == &Block::WATER {
                let block = if is_still {
                    Block::OBSIDIAN
                } else {
                    Block::COBBLESTONE
                };
                world.set_block_state(
                    block_pos,
                    block.default_state.id,
                    BlockFlags::NOTIFY_NEIGHBORS,
                );
                world.sync_world_event(WorldEvent::LavaExtinguished, *block_pos, 0);
                return false;
            }
            if below_is_soul_soil && world.get_block(&neighbor_pos) == &Block::BLUE_ICE {
                world.set_block_state(
                    block_pos,
                    Block::BASALT.default_state.id,
                    BlockFlags::NOTIFY_NEIGHBORS,
                );
                world.sync_world_event(WorldEvent::LavaExtinguished, *block_pos, 0);
                return false;
            }
        }
        true
    }
}

const LAVA_FLOW_SPEED: u8 = 30;

impl FluidBehaviour for FlowingLava {
    fn placed(
        &self,
        world: &Arc<World>,
        fluid: &Fluid,
        state_id: BlockStateId,
        block_pos: &BlockPos,
        old_state_id: BlockStateId,
        _notify: bool,
    ) {
        if old_state_id != state_id && self.receive_neighbor_fluids(world, fluid, block_pos) {
            world.schedule_fluid_tick(fluid, *block_pos, LAVA_FLOW_SPEED, TickPriority::Normal);
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
        if self.receive_neighbor_fluids(world, fluid, block_pos) {
            world.schedule_fluid_tick(fluid, *block_pos, LAVA_FLOW_SPEED, TickPriority::Normal);
        }
    }

    fn on_entity_collision(&self, entity: &dyn EntityBase) {
        let base_entity = entity.get_entity();
        if !base_entity.entity_type.fire_immune {
            base_entity.set_on_fire_for(15.0);
        }
    }
}

impl FlowingFluid for FlowingLava {
    //TODO implement ultrawarm logic
    fn get_drop_off(&self) -> i32 {
        2
    }

    fn get_slope_find_distance(&self) -> i32 {
        2
    }

    fn can_convert_to_source(&self, _world: &Arc<World>) -> bool {
        //TODO add game rule check for lava conversion
        false
    }

    fn spread_to(&self, world: &Arc<World>, fluid: &Fluid, pos: &BlockPos, state_id: BlockStateId) {
        let mut new_props = FlowingFluidProperties::default(fluid);
        new_props.level = Level::L8;
        new_props.falling = Falling::True;
        if state_id == new_props.to_state_id(fluid) {
            // STONE creation
            if world.get_block(pos) == &Block::WATER {
                world.set_block_state(pos, Block::STONE.default_state.id, BlockFlags::NOTIFY_ALL);
                world.sync_world_event(WorldEvent::LavaExtinguished, *pos, 0);
                return;
            }
        }

        if self.is_waterlogged(world, pos).is_some() {
            return;
        }

        world.set_block_state(pos, state_id, BlockFlags::NOTIFY_ALL);
    }
}
