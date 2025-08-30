use crate::block::{BlockBehaviour, OnPlaceArgs};

use pumpkin_data::block_properties::{BlockProperties, LadderLikeProperties};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::BlockStateId;

#[pumpkin_block("minecraft:ender_chest")]
pub struct EnderChestBlock;

impl BlockBehaviour for EnderChestBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = LadderLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }
}
