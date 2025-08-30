use crate::block::blocks::falling::FallingBlock;
use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, OnPlaceArgs, OnScheduledTickArgs, PlacedArgs,
};

use pumpkin_data::block_properties::{BlockProperties, WallTorchLikeProperties};
use pumpkin_data::tag::{RegistryKey, get_tag_values};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_world::BlockStateId;

#[pumpkin_block_from_tag("minecraft:anvil")]
pub struct AnvilBlock;

impl BlockBehaviour for AnvilBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let dir = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .rotate_clockwise();

        let mut props = WallTorchLikeProperties::default(args.block);

        props.facing = dir;
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        FallingBlock::placed(&FallingBlock, args);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        FallingBlock::get_state_for_neighbor_update(&FallingBlock, args)
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        FallingBlock::on_scheduled_tick(&FallingBlock, args);
    }
}
