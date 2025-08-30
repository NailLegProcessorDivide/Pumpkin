use std::sync::Arc;


use pumpkin_data::block_properties::{
    BlockProperties, EastWireConnection, EnumVariants, Integer0To15, NorthWireConnection,
    ObserverLikeProperties, RedstoneWireLikeProperties, RepeaterLikeProperties,
    SouthWireConnection, WestWireConnection,
};
use pumpkin_data::{Block, BlockDirection, BlockState, HorizontalFacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::BlockStateId;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::registry::BlockActionResult;
use crate::block::{
    BrokenArgs, CanPlaceAtArgs, GetRedstonePowerArgs, GetStateForNeighborUpdateArgs,
    OnNeighborUpdateArgs, OnPlaceArgs, PlacedArgs, PrepareArgs,
};
use crate::{
    block::{BlockBehaviour, NormalUseArgs},
    world::World,
};

use super::turbo::RedstoneWireTurbo;
use super::{get_redstone_power_no_dust, update_wire_neighbors};

type RedstoneWireProperties = RedstoneWireLikeProperties;

#[pumpkin_block("minecraft:redstone_wire")]
pub struct RedstoneWireBlock;

impl BlockBehaviour for RedstoneWireBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut wire = RedstoneWireProperties::default(args.block);
        wire.power = Integer0To15::from_index(calculate_power(args.world, args.position).into());
        wire = get_regulated_sides(wire, args.world, args.position);
        if is_dot(wire) {
            wire = make_cross(wire.power);
        }

        wire.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut wire = RedstoneWireProperties::from_state_id(args.state_id, args.block);
        let old_state = wire;
        let new_side: WireConnection;

        match args.direction {
            BlockDirection::Up => {
                return args.state_id;
            }
            BlockDirection::Down => {
                return get_regulated_sides(wire, args.world, args.position)
                    .to_state_id(args.block);
            }
            BlockDirection::North => {
                let side = get_side(args.world, args.position, BlockDirection::North);
                wire.north = side.to_north();
                new_side = side;
            }
            BlockDirection::South => {
                let side = get_side(args.world, args.position, BlockDirection::South);
                wire.south = side.to_south();
                new_side = side;
            }
            BlockDirection::East => {
                let side = get_side(args.world, args.position, BlockDirection::East);
                wire.east = side.to_east();
                new_side = side;
            }
            BlockDirection::West => {
                let side = get_side(args.world, args.position, BlockDirection::West);
                wire.west = side.to_west();
                new_side = side;
            }
        }

        wire = get_regulated_sides(wire, args.world, args.position);
        if is_cross(old_state) && new_side.is_none() {
            return wire.to_state_id(args.block);
        }
        if !is_dot(old_state) && is_dot(wire) {
            let power = wire.power;
            wire = make_cross(power);
        }
        wire.to_state_id(args.block)
    }

    fn prepare(&self, args: PrepareArgs<'_>) {
        let wire_props =
            RedstoneWireLikeProperties::from_state_id(args.state_id, &Block::REDSTONE_WIRE);

        for direction in BlockDirection::horizontal() {
            let other_block_pos = args.position.offset(direction.to_offset());
            let other_block = args.world.get_block(&other_block_pos);

            if wire_props.is_side_connected(direction) && other_block != &Block::REDSTONE_WIRE {
                let up_block_pos = other_block_pos.up();
                let up_block = args.world.get_block(&up_block_pos);
                if up_block == &Block::REDSTONE_WIRE {
                    args.world.replace_with_state_for_neighbor_update(
                        &up_block_pos,
                        direction.opposite(),
                        args.flags,
                    );
                }

                let down_block_pos = other_block_pos.down();
                let down_block = args.world.get_block(&down_block_pos);
                if down_block == &Block::REDSTONE_WIRE {
                    args.world.replace_with_state_for_neighbor_update(
                        &down_block_pos,
                        direction.opposite(),
                        args.flags,
                    );
                }
            }
        }
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        let wire = RedstoneWireProperties::from_state_id(state.id, args.block);
        if on_use(wire, args.world, args.position) {
            BlockActionResult::Success
        } else {
            BlockActionResult::Pass
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if can_place_at(args.world.as_ref(), args.position) {
            let state = args.world.get_block_state(args.position);
            let mut wire = RedstoneWireProperties::from_state_id(state.id, args.block);
            let new_power = calculate_power(args.world, args.position);
            if wire.power.to_index() as u8 != new_power {
                wire.power = Integer0To15::from_index(new_power.into());
                args.world.set_block_state(
                    args.position,
                    wire.to_state_id(&Block::REDSTONE_WIRE),
                    BlockFlags::empty(),
                );
                RedstoneWireTurbo::update_surrounding_neighbors(args.world, *args.position);
            }
        } else {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let wire = RedstoneWireProperties::from_state_id(args.state.id, args.block);
        if args.direction == BlockDirection::Up || wire.is_side_connected(args.direction.opposite())
        {
            wire.power.to_index() as u8
        } else {
            0
        }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let wire = RedstoneWireProperties::from_state_id(args.state.id, args.block);
        if args.direction == BlockDirection::Up || wire.is_side_connected(args.direction.opposite())
        {
            wire.power.to_index() as u8
        } else {
            0
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        update_wire_neighbors(args.world, args.position);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        update_wire_neighbors(args.world, args.position);
    }
}

fn can_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
    let floor = world.get_block_state(&block_pos.down());
    floor.is_side_solid(BlockDirection::Up)
}

fn on_use(wire: RedstoneWireProperties, world: &Arc<World>, block_pos: &BlockPos) -> bool {
    if is_cross(wire) || is_dot(wire) {
        let mut new_wire = if is_cross(wire) {
            RedstoneWireProperties::default(&Block::REDSTONE_WIRE)
        } else {
            make_cross(wire.power)
        };
        new_wire.power = wire.power;

        new_wire = get_regulated_sides(new_wire, world, block_pos);
        if wire != new_wire {
            world.set_block_state(
                block_pos,
                new_wire.to_state_id(&Block::REDSTONE_WIRE),
                BlockFlags::empty(),
            );
            update_wire_neighbors(world, block_pos);
            return true;
        }
    }
    false
}

#[must_use]
pub fn make_cross(power: Integer0To15) -> RedstoneWireProperties {
    RedstoneWireProperties {
        north: NorthWireConnection::Side,
        south: SouthWireConnection::Side,
        east: EastWireConnection::Side,
        west: WestWireConnection::Side,
        power,
    }
}

fn can_connect_to(world: &World, block: &Block, side: BlockDirection, state: &BlockState) -> bool {
    if world
        .block_registry
        .emits_redstone_power(block, state, side)
    {
        return true;
    }
    if block == &Block::REPEATER {
        let repeater_props = RepeaterLikeProperties::from_state_id(state.id, block);
        return repeater_props.facing.to_block_direction() == side
            || repeater_props.facing.to_block_direction() == side.opposite();
    } else if block == &Block::OBSERVER {
        let observer_props = ObserverLikeProperties::from_state_id(state.id, block);
        return observer_props.facing == side.to_facing();
    } else if block == &Block::REDSTONE_WIRE {
        return true;
    }
    false
}

fn can_connect_diagonal_to(block: &Block) -> bool {
    block == &Block::REDSTONE_WIRE
}

pub fn get_side(world: &World, pos: &BlockPos, side: BlockDirection) -> WireConnection {
    let neighbor_pos: BlockPos = pos.offset(side.to_offset());
    let (neighbor, state) = world.get_block_and_state(&neighbor_pos);

    if can_connect_to(world, neighbor, side, state) {
        return WireConnection::Side;
    }

    let up_pos = pos.offset(BlockDirection::Up.to_offset());
    let up_state = world.get_block_state(&up_pos);

    if !up_state.is_solid_block()
        && state.is_side_solid(side.opposite())
        && can_connect_diagonal_to(
            world.get_block(&neighbor_pos.offset(BlockDirection::Up.to_offset())),
        )
    {
        WireConnection::Up
    } else if !state.is_solid_block()
        && can_connect_diagonal_to(
            world.get_block(&neighbor_pos.offset(BlockDirection::Down.to_offset())),
        )
    {
        WireConnection::Side
    } else {
        WireConnection::None
    }
}

fn get_all_sides(
    mut wire: RedstoneWireProperties,
    world: &World,
    pos: &BlockPos,
) -> RedstoneWireProperties {
    wire.north = get_side(world, pos, BlockDirection::North).to_north();
    wire.south = get_side(world, pos, BlockDirection::South).to_south();
    wire.east = get_side(world, pos, BlockDirection::East).to_east();
    wire.west = get_side(world, pos, BlockDirection::West).to_west();
    wire
}

#[must_use]
pub fn is_dot(wire: RedstoneWireProperties) -> bool {
    wire.north == NorthWireConnection::None
        && wire.south == SouthWireConnection::None
        && wire.east == EastWireConnection::None
        && wire.west == WestWireConnection::None
}

#[must_use]
pub fn is_cross(wire: RedstoneWireProperties) -> bool {
    wire.north == NorthWireConnection::Side
        && wire.south == SouthWireConnection::Side
        && wire.east == EastWireConnection::Side
        && wire.west == WestWireConnection::Side
}

pub fn get_regulated_sides(
    wire: RedstoneWireProperties,
    world: &World,
    pos: &BlockPos,
) -> RedstoneWireProperties {
    let mut state = get_all_sides(wire, world, pos);
    if is_dot(wire) && is_dot(state) {
        return state;
    }
    let north_none = state.north.is_none();
    let south_none = state.south.is_none();
    let east_none = state.east.is_none();
    let west_none = state.west.is_none();
    let north_south_none = north_none && south_none;
    let east_west_none = east_none && west_none;
    if north_none && east_west_none {
        state.north = NorthWireConnection::Side;
    }
    if south_none && east_west_none {
        state.south = SouthWireConnection::Side;
    }
    if east_none && north_south_none {
        state.east = EastWireConnection::Side;
    }
    if west_none && north_south_none {
        state.west = WestWireConnection::Side;
    }
    state
}

trait RedstoneWireLikePropertiesExt {
    fn is_side_connected(&self, direction: BlockDirection) -> bool;
    //fn get_connection_type(&self, direction: BlockDirection) -> WireConnection;
}

impl RedstoneWireLikePropertiesExt for RedstoneWireLikeProperties {
    fn is_side_connected(&self, direction: BlockDirection) -> bool {
        match direction {
            BlockDirection::North => self.north.to_wire_connection().is_connected(),
            BlockDirection::South => self.south.to_wire_connection().is_connected(),
            BlockDirection::East => self.east.to_wire_connection().is_connected(),
            BlockDirection::West => self.west.to_wire_connection().is_connected(),
            _ => false,
        }
    }

    /*
    fn get_connection_type(&self, direction: BlockDirection) -> WireConnection {
        match direction {
            BlockDirection::North => self.north.to_wire_connection(),
            BlockDirection::South => self.south.to_wire_connection(),
            BlockDirection::East => self.east.to_wire_connection(),
            BlockDirection::West => self.west.to_wire_connection(),
            _ => WireConnection::None,
        }
    }
     */
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireConnection {
    Up,
    Side,
    None,
}

impl WireConnection {
    fn is_connected(self) -> bool {
        self != Self::None
    }

    fn is_none(self) -> bool {
        self == Self::None
    }

    fn to_north(self) -> NorthWireConnection {
        match self {
            Self::Up => NorthWireConnection::Up,
            Self::Side => NorthWireConnection::Side,
            Self::None => NorthWireConnection::None,
        }
    }

    fn to_south(self) -> SouthWireConnection {
        match self {
            Self::Up => SouthWireConnection::Up,
            Self::Side => SouthWireConnection::Side,
            Self::None => SouthWireConnection::None,
        }
    }

    fn to_east(self) -> EastWireConnection {
        match self {
            Self::Up => EastWireConnection::Up,
            Self::Side => EastWireConnection::Side,
            Self::None => EastWireConnection::None,
        }
    }

    fn to_west(self) -> WestWireConnection {
        match self {
            Self::Up => WestWireConnection::Up,
            Self::Side => WestWireConnection::Side,
            Self::None => WestWireConnection::None,
        }
    }
}
trait CardinalWireConnectionExt {
    fn to_wire_connection(&self) -> WireConnection;
    fn is_none(&self) -> bool;
}

impl CardinalWireConnectionExt for NorthWireConnection {
    fn to_wire_connection(&self) -> WireConnection {
        match self {
            Self::Side => WireConnection::Side,
            Self::Up => WireConnection::Up,
            Self::None => WireConnection::None,
        }
    }

    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

impl CardinalWireConnectionExt for SouthWireConnection {
    fn to_wire_connection(&self) -> WireConnection {
        match self {
            Self::Side => WireConnection::Side,
            Self::Up => WireConnection::Up,
            Self::None => WireConnection::None,
        }
    }

    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

impl CardinalWireConnectionExt for EastWireConnection {
    fn to_wire_connection(&self) -> WireConnection {
        match self {
            Self::Side => WireConnection::Side,
            Self::Up => WireConnection::Up,
            Self::None => WireConnection::None,
        }
    }

    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

impl CardinalWireConnectionExt for WestWireConnection {
    fn to_wire_connection(&self) -> WireConnection {
        match self {
            Self::Side => WireConnection::Side,
            Self::Up => WireConnection::Up,
            Self::None => WireConnection::None,
        }
    }

    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

fn max_wire_power(wire_power: u8, world: &World, pos: BlockPos) -> u8 {
    let (block, block_state) = world.get_block_and_state(&pos);
    if block == &Block::REDSTONE_WIRE {
        let wire = RedstoneWireProperties::from_state_id(block_state.id, block);
        wire_power.max(wire.power.to_index() as u8)
    } else {
        wire_power
    }
}

fn calculate_power(world: &World, pos: &BlockPos) -> u8 {
    let mut block_power: u8 = 0;
    let mut wire_power: u8 = 0;

    let up_pos = pos.offset(BlockDirection::Up.to_offset());
    let up_state = world.get_block_state(&up_pos);

    for side in BlockDirection::all() {
        let neighbor_pos = pos.offset(side.to_offset());
        wire_power = max_wire_power(wire_power, world, neighbor_pos);
        let (neighbor, neighbor_state) = world.get_block_and_state(&neighbor_pos);
        block_power = block_power.max(get_redstone_power_no_dust(
            neighbor,
            neighbor_state,
            world,
            neighbor_pos,
            side,
        ));
        if side.is_horizontal() {
            if !up_state.is_solid_block() && neighbor_state.is_solid_block() {
                wire_power = max_wire_power(
                    wire_power,
                    world,
                    neighbor_pos.offset(BlockDirection::Up.to_offset()),
                );
            }

            if !neighbor_state.is_solid_block() {
                wire_power = max_wire_power(
                    wire_power,
                    world,
                    neighbor_pos.offset(BlockDirection::Down.to_offset()),
                );
            }
        }
    }

    block_power.max(wire_power.saturating_sub(1))
}
