use std::sync::Arc;

use pumpkin_data::block_properties::{BlockProperties, FurnaceLikeProperties};
use pumpkin_inventory::{
    furnace::furnace_screen_handler::FurnaceScreenHandler, screen_handler::ScreenHandlerFactory,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;
use pumpkin_world::{
    block::entities::{BlockEntity, furnace::FurnaceBlockEntity},
    inventory::Inventory,
};
use parking_lot::Mutex;

use crate::block::BlockBehaviour;

struct FurnaceScreenFactory {
    inventory: Arc<dyn Inventory>,
    block_entity: Arc<dyn BlockEntity>,
}

impl FurnaceScreenFactory {
    fn new(inventory: Arc<dyn Inventory>, block_entity: Arc<dyn BlockEntity>) -> Self {
        Self {
            inventory,
            block_entity,
        }
    }
}

impl ScreenHandlerFactory for FurnaceScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<pumpkin_inventory::player::player_inventory::PlayerInventory>,
        _player: &dyn pumpkin_inventory::screen_handler::InventoryPlayer,
    ) -> Option<Arc<Mutex<dyn pumpkin_inventory::screen_handler::ScreenHandler>>> {
        let furnace_screen_handler = FurnaceScreenHandler::new(
            sync_id,
            player_inventory,
            self.inventory.clone(),
            self.block_entity.clone(),
        );
        Some(Arc::new(Mutex::new(furnace_screen_handler)))
    }

    fn get_display_name(&self) -> pumpkin_util::text::TextComponent {
        TextComponent::translate("container.furnace", &[])
    }
}

#[pumpkin_block("minecraft:furnace")]
pub struct FurnaceBlock;

impl BlockBehaviour for FurnaceBlock {
    fn normal_use(
        &self,
        args: crate::block::NormalUseArgs<'_>,
    ) -> crate::block::registry::BlockActionResult {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(inventory) = block_entity.clone().get_inventory()
        {
            let furnace_screen_factory = FurnaceScreenFactory::new(inventory, block_entity);
            args.player.open_handled_screen(&furnace_screen_factory);
        }
        crate::block::registry::BlockActionResult::Consume
    }

    //Same to normal_use
    fn use_with_item(
        &self,
        _args: crate::block::UseWithItemArgs<'_>,
    ) -> crate::block::registry::BlockActionResult {
        crate::block::registry::BlockActionResult::PassToDefaultBlockAction
    }

    fn on_entity_collision(&self, _args: crate::block::OnEntityCollisionArgs<'_>) {}

    fn should_drop_items_on_explosion(&self) -> bool {
        true
    }

    fn explode(&self, _args: crate::block::ExplodeArgs<'_>) {}

    fn on_synced_block_event(&self, _args: crate::block::OnSyncedBlockEventArgs<'_>) -> bool {
        false
    }

    fn on_place(&self, args: crate::block::OnPlaceArgs<'_>) -> pumpkin_world::BlockStateId {
        let mut props = FurnaceLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();

        props.to_state_id(args.block)
    }

    fn random_tick(&self, _args: crate::block::RandomTickArgs<'_>) {}

    fn can_place_at(&self, _args: crate::block::CanPlaceAtArgs<'_>) -> bool {
        true
    }

    fn can_update_at(&self, _args: crate::block::CanUpdateAtArgs<'_>) -> bool {
        false
    }

    fn placed(&self, args: crate::block::PlacedArgs<'_>) {
        let furnace_block_entity = FurnaceBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(furnace_block_entity));
    }

    fn player_placed(&self, _args: crate::block::PlayerPlacedArgs<'_>) {}

    fn broken(&self, args: crate::block::BrokenArgs<'_>) {
        args.world.remove_block_entity(args.position);
    }

    fn on_neighbor_update(&self, _args: crate::block::OnNeighborUpdateArgs<'_>) {}

    fn prepare(&self, _args: crate::block::PrepareArgs<'_>) {}

    fn get_state_for_neighbor_update(
        &self,
        args: crate::block::GetStateForNeighborUpdateArgs<'_>,
    ) -> pumpkin_world::BlockStateId {
        args.state_id
    }

    fn on_scheduled_tick(&self, _args: crate::block::OnScheduledTickArgs<'_>) {}

    fn on_state_replaced(&self, _args: crate::block::OnStateReplacedArgs<'_>) {}

    fn emits_redstone_power(&self, _args: crate::block::EmitsRedstonePowerArgs<'_>) -> bool {
        false
    }

    fn get_weak_redstone_power(&self, _args: crate::block::GetRedstonePowerArgs<'_>) -> u8 {
        0
    }

    fn get_strong_redstone_power(&self, _args: crate::block::GetRedstonePowerArgs<'_>) -> u8 {
        0
    }

    fn get_comparator_output(
        &self,
        _args: crate::block::GetComparatorOutputArgs<'_>,
    ) -> Option<u8> {
        None
    }
}
