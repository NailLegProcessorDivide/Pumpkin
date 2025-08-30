use crate::block::UseWithItemArgs;
use crate::block::registry::BlockActionResult;
use crate::entity::Entity;
use crate::entity::item::ItemEntity;

use pumpkin_data::Block;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::item::ItemStack;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;
use uuid::Uuid;

#[pumpkin_block("minecraft:pumpkin")]
pub struct PumpkinBlock;

impl crate::block::BlockBehaviour for PumpkinBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if args.item_stack.lock().item != &Item::SHEARS {
            return BlockActionResult::Pass;
        }
        // TODO: set direction
        args.world.set_block_state(
            args.position,
            Block::CARVED_PUMPKIN.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        let entity = Entity::new(
            Uuid::new_v4(),
            args.world.clone(),
            args.position.to_f64(),
            &EntityType::ITEM,
            false,
        );
        let item_entity = Arc::new(ItemEntity::new(
            entity,
            ItemStack::new(4, &Item::PUMPKIN_SEEDS),
        ));
        args.world.spawn_entity(item_entity);
        BlockActionResult::Consume
    }
}
