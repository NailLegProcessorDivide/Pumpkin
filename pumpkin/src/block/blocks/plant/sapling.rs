
use pumpkin_data::block_properties::{BlockProperties, Integer0To1};
use pumpkin_data::tag::{RegistryKey, get_tag_values};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::BlockStateId;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, RandomTickArgs};
use crate::world::World;

type SaplingProperties = pumpkin_data::block_properties::OakSaplingLikeProperties;

#[pumpkin_block_from_tag("minecraft:saplings")]
pub struct SaplingBlock;

impl SaplingBlock {
    fn generate(&self, world: &Arc<World>, pos: &BlockPos) {
        let (block, state) = world.get_block_and_state_id(pos);
        let mut props = SaplingProperties::from_state_id(state, block);
        if props.stage == Integer0To1::L0 {
            props.stage = Integer0To1::L1;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
        } else {
            //TODO generate tree
        }
    }
}

impl BlockBehaviour for SaplingBlock {
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

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        self.generate(args.world, args.position);
    }
}

impl PlantBlockBase for SaplingBlock {}
