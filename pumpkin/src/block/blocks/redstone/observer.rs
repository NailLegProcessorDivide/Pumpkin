use std::sync::Arc;

use crate::block::{
    EmitsRedstonePowerArgs, GetRedstonePowerArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnScheduledTickArgs, OnStateReplacedArgs,
};

use pumpkin_data::{
    Block, FacingExt,
    block_properties::{BlockProperties, ObserverLikeProperties},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{BlockStateId, tick::TickPriority, world::BlockFlags};

use crate::{
    block::{BlockBehaviour, OnNeighborUpdateArgs},
    world::World,
};

#[pumpkin_block("minecraft:observer")]
pub struct ObserverBlock;

impl BlockBehaviour for ObserverBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = ObserverLikeProperties::default(args.block);
        props.facing = args.player.living_entity.entity.get_facing();
        props.to_state_id(args.block)
    }

    fn on_neighbor_update(&self, _args: OnNeighborUpdateArgs<'_>) {}

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = ObserverLikeProperties::from_state_id(state.id, args.block);

        if props.powered {
            props.powered = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        } else {
            props.powered = true;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
            args.world
                .schedule_block_tick(args.block, *args.position, 2, TickPriority::Normal);
        }

        Self::update_neighbors(args.world, args.block, args.position, &props);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = ObserverLikeProperties::from_state_id(args.state_id, args.block);

        if props.facing.to_block_direction() == args.direction && !props.powered {
            Self::schedule_tick(args.world, args.position);
        }

        args.state_id
    }

    fn emits_redstone_power(&self, args: EmitsRedstonePowerArgs<'_>) -> bool {
        let props = ObserverLikeProperties::from_state_id(args.state.id, args.block);
        props.facing.to_block_direction() == args.direction
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = ObserverLikeProperties::from_state_id(args.state.id, args.block);
        if props.facing.to_block_direction() == args.direction && props.powered {
            15
        } else {
            0
        }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        self.get_weak_redstone_power(args)
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            let props = ObserverLikeProperties::from_state_id(args.old_state_id, args.block);
            if props.powered
                && args
                    .world
                    .is_block_tick_scheduled(args.position, &Block::OBSERVER)
            {
                Self::update_neighbors(args.world, args.block, args.position, &props);
            }
        }
    }
}

impl ObserverBlock {
    fn update_neighbors(
        world: &Arc<World>,
        block: &Block,
        block_pos: &BlockPos,
        props: &ObserverLikeProperties,
    ) {
        let facing = props.facing;
        let opposite_facing_pos =
            block_pos.offset(facing.to_block_direction().opposite().to_offset());
        world.update_neighbor(&opposite_facing_pos, block);
        world.update_neighbors(&opposite_facing_pos, Some(facing.to_block_direction()));
    }

    fn schedule_tick(world: &World, block_pos: &BlockPos) {
        world.schedule_block_tick(&Block::OBSERVER, *block_pos, 2, TickPriority::Normal);
    }
}
