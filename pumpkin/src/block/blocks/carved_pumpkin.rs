use pumpkin_data::{
    Block,
    block_properties::{BlockProperties, WallTorchLikeProperties},
};
use pumpkin_world::BlockStateId;

use crate::block::{BlockBehaviour, BlockMetadata, OnPlaceArgs};

pub struct CarvedPumpkinBlock;

impl BlockMetadata for CarvedPumpkinBlock {
    fn namespace(&self) -> &'static str {
        "minecraft"
    }

    fn ids(&self) -> &'static [&'static str] {
        &[Block::JACK_O_LANTERN.name, Block::CARVED_PUMPKIN.name]
    }
}

impl BlockBehaviour for CarvedPumpkinBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = WallTorchLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }
}
