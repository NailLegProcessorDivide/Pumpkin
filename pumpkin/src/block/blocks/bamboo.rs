use pumpkin_data::tag;
use pumpkin_data::tag::Taggable;
use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, CanPlaceAtArgs};

#[pumpkin_block("minecraft:bamboo")]
pub struct BambooBlock;

impl BlockBehaviour for BambooBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let block_below = args.block_accessor.get_block(&args.position.down());
        block_below.is_tagged_with_by_tag(&tag::Block::MINECRAFT_BAMBOO_PLANTABLE_ON)
    }
}
