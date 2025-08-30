use std::sync::atomic::Ordering;

use pumpkin_data::Block;
use pumpkin_util::{GameMode, math::position::BlockPos};
use pumpkin_world::{block::entities::command_block::CommandBlockEntity, tick::TickPriority};

use crate::{
    block::{
        BlockBehaviour, BlockMetadata, CanPlaceAtArgs, OnNeighborUpdateArgs, OnScheduledTickArgs,
    },
    world::World,
};

use super::redstone::block_receives_redstone_power;

pub struct CommandBlock;

impl CommandBlock {
    pub fn update(
        world: &World,
        block: &Block,
        command_block: &CommandBlockEntity,
        pos: &BlockPos,
        powered: bool,
    ) {
        if command_block.powered.load(Ordering::Relaxed) == powered {
            return;
        }
        command_block.powered.store(powered, Ordering::Relaxed);
        if powered {
            world.schedule_block_tick(block, *pos, 1, TickPriority::Normal);
        }
    }
}

impl BlockMetadata for CommandBlock {
    fn namespace(&self) -> &'static str {
        "minecraft"
    }

    fn ids(&self) -> &'static [&'static str] {
        &[
            Block::COMMAND_BLOCK.name,
            Block::CHAIN_COMMAND_BLOCK.name,
            Block::REPEATING_COMMAND_BLOCK.name,
        ]
    }
}

impl BlockBehaviour for CommandBlock {
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if let Some(block_entity) = args.world.get_block_entity(args.position) {
            if block_entity.resource_location() != CommandBlockEntity::ID {
                return;
            }
            let command_entity = block_entity
                .as_any()
                .downcast_ref::<CommandBlockEntity>()
                .unwrap();

            Self::update(
                args.world,
                args.block,
                command_entity,
                args.position,
                block_receives_redstone_power(args.world, args.position),
            );
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && block_entity.resource_location() != CommandBlockEntity::ID
        {
            return;
        }
        // TODO
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        if let Some(player) = args.player
            && player.gamemode.load() == GameMode::Creative
        {
            return true;
        }

        false
    }
}
