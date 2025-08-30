use std::sync::Arc;

use crate::block::BlockBehaviour;
use crate::block::OnEntityCollisionArgs;
use crate::block::PlacedArgs;

use pumpkin_macros::pumpkin_block;
use pumpkin_registry::VanillaDimensionType;
use pumpkin_world::block::entities::end_portal::EndPortalBlockEntity;

#[pumpkin_block("minecraft:end_portal")]
pub struct EndPortalBlock;

impl BlockBehaviour for EndPortalBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        let world = if args.world.dimension_type == VanillaDimensionType::TheEnd {
            args.server
                .get_world_from_dimension(VanillaDimensionType::Overworld)
        } else {
            args.server
                .get_world_from_dimension(VanillaDimensionType::TheEnd)
        };
        args.entity
            .get_entity()
            .try_use_portal(0, world, *args.position);
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        args.world
            .add_block_entity(Arc::new(EndPortalBlockEntity::new(*args.position)));
    }
}
