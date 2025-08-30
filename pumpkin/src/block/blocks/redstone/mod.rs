use std::sync::Arc;

/**
 * This implementation is heavily based on <https://github.com/MCHPR/MCHPRS>
 * Updated to fit pumpkin by 4lve
 */
use pumpkin_data::{Block, BlockDirection, BlockState};
use pumpkin_util::math::position::BlockPos;

use crate::world::World;

pub mod buttons;
pub mod comparator;
pub mod copper_bulb;
pub mod dropper;
pub mod lever;
pub mod observer;
pub mod pressure_plate;
pub mod rails;
pub mod redstone_block;
pub mod redstone_lamp;
pub mod redstone_torch;
pub mod redstone_wire;
pub mod repeater;
pub mod target_block;
pub mod tripwire;
pub mod tripwire_hook;
pub mod turbo;

// abstruct
pub mod abstruct_redstone_gate;
pub mod dispenser;

pub fn update_wire_neighbors(world: &Arc<World>, pos: &BlockPos) {
    for direction in BlockDirection::all() {
        let neighbor_pos = pos.offset(direction.to_offset());
        let block = world.get_block(&neighbor_pos);
        world
            .block_registry
            .on_neighbor_update(world, block, &neighbor_pos, block, true);

        for n_direction in BlockDirection::all() {
            let n_neighbor_pos = neighbor_pos.offset(n_direction.to_offset());
            let block = world.get_block(&n_neighbor_pos);
            world
                .block_registry
                .on_neighbor_update(world, block, &n_neighbor_pos, block, true);
        }
    }
}

pub fn is_emitting_redstone_power(
    block: &Block,
    state: &BlockState,
    world: &World,
    pos: &BlockPos,
    facing: BlockDirection,
) -> bool {
    get_redstone_power(block, state, world, pos, facing) > 0
}

pub fn get_redstone_power(
    block: &Block,
    state: &BlockState,
    world: &World,
    pos: &BlockPos,
    facing: BlockDirection,
) -> u8 {
    if state.is_solid_block() {
        return std::cmp::max(
            get_max_strong_power(world, pos, true),
            get_weak_power(block, state, world, pos, facing, true),
        );
    }
    get_weak_power(block, state, world, pos, facing, true)
}

fn get_redstone_power_no_dust(
    block: &Block,
    state: &BlockState,
    world: &World,
    pos: BlockPos,
    facing: BlockDirection,
) -> u8 {
    if state.is_solid_block() {
        return std::cmp::max(
            get_max_strong_power(world, &pos, false),
            get_weak_power(block, state, world, &pos, facing, false),
        );
    }
    get_weak_power(block, state, world, &pos, facing, false)
}

fn get_max_strong_power(world: &World, pos: &BlockPos, dust_power: bool) -> u8 {
    let mut max_power = 0;
    for side in BlockDirection::all() {
        let (block, state) = world.get_block_and_state(&pos.offset(side.to_offset()));
        max_power = max_power.max(get_strong_power(
            block,
            state,
            world,
            &pos.offset(side.to_offset()),
            side,
            dust_power,
        ));
    }
    max_power
}

fn get_max_weak_power(world: &World, pos: &BlockPos, dust_power: bool) -> u8 {
    let mut max_power = 0;
    for side in BlockDirection::all() {
        let (block, state) = world.get_block_and_state(&pos.offset(side.to_offset()));
        max_power = max_power.max(get_weak_power(
            block,
            state,
            world,
            &pos.offset(side.to_offset()),
            side,
            dust_power,
        ));
    }
    max_power
}

fn get_weak_power(
    block: &Block,
    state: &BlockState,
    world: &World,
    pos: &BlockPos,
    side: BlockDirection,
    dust_power: bool,
) -> u8 {
    if !dust_power && block == &Block::REDSTONE_WIRE {
        return 0;
    }
    world
        .block_registry
        .get_weak_redstone_power(block, world, pos, state, side)
}

fn get_strong_power(
    block: &Block,
    state: &BlockState,
    world: &World,
    pos: &BlockPos,
    side: BlockDirection,
    dust_power: bool,
) -> u8 {
    if !dust_power && block == &Block::REDSTONE_WIRE {
        return 0;
    }
    world
        .block_registry
        .get_strong_redstone_power(block, world, pos, state, side)
}

pub fn block_receives_redstone_power(world: &World, pos: &BlockPos) -> bool {
    for facing in BlockDirection::all() {
        let neighbor_pos = pos.offset(facing.to_offset());
        let (block, state) = world.get_block_and_state(&neighbor_pos);
        if is_emitting_redstone_power(block, state, world, &neighbor_pos, facing) {
            return true;
        }
    }
    false
}

#[must_use]
pub fn is_diode(block: &Block) -> bool {
    block == &Block::REPEATER || block == &Block::COMPARATOR
}

pub fn diode_get_input_strength(world: &World, pos: &BlockPos, facing: BlockDirection) -> u8 {
    let input_pos = pos.offset(facing.to_offset());
    let (input_block, input_state) = world.get_block_and_state(&input_pos);
    let power: u8 = get_redstone_power(input_block, input_state, world, &input_pos, facing);
    if power == 0 && input_state.is_solid_block() {
        return get_max_weak_power(world, &input_pos, true);
    }
    power
}
