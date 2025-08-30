use pumpkin_data::BlockDirection;
use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, CanPlaceAtArgs};

#[pumpkin_block("minecraft:vine")]
pub struct VineBlock;

impl BlockBehaviour for VineBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        // TODO: This is bad and not vanilla, just a "hotfix"
        for dir in BlockDirection::all() {
            if !args
                .block_accessor
                .get_block_state(&args.position.offset(dir.to_offset()))
                .is_air()
            {
                return true;
            }
        }
        false
    }
}
