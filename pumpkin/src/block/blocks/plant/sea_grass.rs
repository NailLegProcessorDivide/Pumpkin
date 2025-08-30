
use pumpkin_data::{Block, BlockDirection};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::BlockStateId;

use crate::block::{
    blocks::plant::PlantBlockBase,
    {BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs},
};

#[pumpkin_block("minecraft:seagrass")]
pub struct SeaGrassBlock;

impl BlockBehaviour for SeaGrassBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }
}

impl PlantBlockBase for SeaGrassBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
    ) -> bool {
        let block = block_accessor.get_block(pos);
        let block_state = block_accessor.get_block_state(pos);
        block_state.is_side_solid(BlockDirection::Up) && block != &Block::MAGMA_BLOCK
    }
}
