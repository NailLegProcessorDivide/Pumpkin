use pumpkin_util::{
    math::vector2::Vector2,
    text::{
        TextComponent,
        color::{Color, NamedColor},
    },
};

use crate::{
    command::{
        CommandError, CommandExecutor, CommandSender,
        args::{
            ConsumedArgs, DefaultNameArgConsumer, FindArgDefaultName,
            bounded_num::BoundedNumArgumentConsumer, position_2d::Position2DArgumentConsumer,
        },
        tree::CommandTree,
        tree::builder::{argument_default_name, literal},
    },
    server::Server,
};

const NAMES: [&str; 1] = ["worldborder"];

const DESCRIPTION: &str = "Worldborder command.";

const NOTHING_CHANGED_EXCEPTION: &str = "commands.worldborder.set.failed.nochange";

fn distance_consumer() -> BoundedNumArgumentConsumer<f64> {
    BoundedNumArgumentConsumer::new().min(0.0).name("distance")
}

fn time_consumer() -> BoundedNumArgumentConsumer<i32> {
    BoundedNumArgumentConsumer::new().min(0).name("time")
}

fn damage_per_block_consumer() -> BoundedNumArgumentConsumer<f32> {
    BoundedNumArgumentConsumer::new()
        .min(0.0)
        .name("damage_per_block")
}

fn damage_buffer_consumer() -> BoundedNumArgumentConsumer<f32> {
    BoundedNumArgumentConsumer::new().min(0.0).name("buffer")
}

fn warning_distance_consumer() -> BoundedNumArgumentConsumer<i32> {
    BoundedNumArgumentConsumer::new().min(0).name("distance")
}

struct GetExecutor;

impl CommandExecutor for GetExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        _args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let border = world.worldborder.lock();

        let diameter = border.new_diameter.round() as i32;
        sender.send_message(TextComponent::translate(
            "commands.worldborder.get",
            [TextComponent::text(diameter.to_string())],
        ));
        Ok(())
    }
}

struct SetExecutor;

impl CommandExecutor for SetExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(distance) = distance_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    distance_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        if (distance - border.new_diameter).abs() < f64::EPSILON {
            sender.send_message(
                TextComponent::translate(NOTHING_CHANGED_EXCEPTION, [])
                    .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        }

        let dist = format!("{distance:.1}");
        sender.send_message(TextComponent::translate(
            "commands.worldborder.set.immediate",
            [TextComponent::text(dist)],
        ));
        border.set_diameter(world, distance, None);
        Ok(())
    }
}

struct SetTimeExecutor;

impl CommandExecutor for SetTimeExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(distance) = distance_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    distance_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };
        let Ok(time) = time_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    time_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        match distance.total_cmp(&border.new_diameter) {
            std::cmp::Ordering::Equal => {
                sender.send_message(
                    TextComponent::translate(NOTHING_CHANGED_EXCEPTION, [])
                        .color(Color::Named(NamedColor::Red)),
                );
                return Ok(());
            }
            std::cmp::Ordering::Less => {
                let dist = format!("{distance:.1}");
                sender.send_message(TextComponent::translate(
                    "commands.worldborder.set.shrink",
                    [
                        TextComponent::text(dist),
                        TextComponent::text(time.to_string()),
                    ],
                ));
            }
            std::cmp::Ordering::Greater => {
                let dist = format!("{distance:.1}");
                sender.send_message(TextComponent::translate(
                    "commands.worldborder.set.grow",
                    [
                        TextComponent::text(dist),
                        TextComponent::text(time.to_string()),
                    ],
                ));
            }
        }

        border.set_diameter(world, distance, Some(i64::from(time) * 1000));
        Ok(())
    }
}

struct AddExecutor;

impl CommandExecutor for AddExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(distance) = distance_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    distance_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        if distance == 0.0 {
            sender.send_message(
                TextComponent::translate(NOTHING_CHANGED_EXCEPTION, [])
                    .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        }

        let distance = border.new_diameter + distance;

        let dist = format!("{distance:.1}");
        sender.send_message(TextComponent::translate(
            "commands.worldborder.set.immediate",
            [TextComponent::text(dist)],
        ));
        border.set_diameter(world, distance, None);
        Ok(())
    }
}

struct AddTimeExecutor;

impl CommandExecutor for AddTimeExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(distance) = distance_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    distance_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };
        let Ok(time) = time_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    time_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        let distance = distance + border.new_diameter;

        match distance.total_cmp(&border.new_diameter) {
            std::cmp::Ordering::Equal => {
                sender.send_message(
                    TextComponent::translate(NOTHING_CHANGED_EXCEPTION, [])
                        .color(Color::Named(NamedColor::Red)),
                );
                return Ok(());
            }
            std::cmp::Ordering::Less => {
                let dist = format!("{distance:.1}");
                sender.send_message(TextComponent::translate(
                    "commands.worldborder.set.shrink",
                    [
                        TextComponent::text(dist),
                        TextComponent::text(time.to_string()),
                    ],
                ));
            }
            std::cmp::Ordering::Greater => {
                let dist = format!("{distance:.1}");
                sender.send_message(TextComponent::translate(
                    "commands.worldborder.set.grow",
                    [
                        TextComponent::text(dist),
                        TextComponent::text(time.to_string()),
                    ],
                ));
            }
        }

        border.set_diameter(world, distance, Some(i64::from(time) * 1000));
        Ok(())
    }
}

struct CenterExecutor;

impl CommandExecutor for CenterExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Vector2 { x, y } = Position2DArgumentConsumer.find_arg_default_name(args)?;

        sender.send_message(TextComponent::translate(
            "commands.worldborder.center.success",
            [
                TextComponent::text(format!("{x:.2}")),
                TextComponent::text(format!("{y:.2}")),
            ],
        ));
        border.set_center(world, x, y);
        Ok(())
    }
}

struct DamageAmountExecutor;

impl CommandExecutor for DamageAmountExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(damage_per_block) = damage_per_block_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    damage_per_block_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        if (damage_per_block - border.damage_per_block).abs() < f32::EPSILON {
            sender.send_message(
                TextComponent::translate("commands.worldborder.damage.amount.failed", [])
                    .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        }

        let damage = format!("{damage_per_block:.2}");
        sender.send_message(TextComponent::translate(
            "commands.worldborder.damage.amount.success",
            [TextComponent::text(damage)],
        ));
        border.damage_per_block = damage_per_block;
        Ok(())
    }
}

struct DamageBufferExecutor;

impl CommandExecutor for DamageBufferExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(buffer) = damage_buffer_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    damage_buffer_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        if (buffer - border.buffer).abs() < f32::EPSILON {
            sender.send_message(
                TextComponent::translate("commands.worldborder.damage.buffer.failed", [])
                    .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        }

        let buf = format!("{buffer:.2}");
        sender.send_message(TextComponent::translate(
            "commands.worldborder.damage.buffer.success",
            [TextComponent::text(buf)],
        ));
        border.buffer = buffer;
        Ok(())
    }
}

struct WarningDistanceExecutor;

impl CommandExecutor for WarningDistanceExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(distance) = warning_distance_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    warning_distance_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        if distance == border.warning_blocks {
            sender.send_message(
                TextComponent::translate("commands.worldborder.warning.distance.failed", [])
                    .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        }

        sender.send_message(TextComponent::translate(
            "commands.worldborder.warning.distance.success",
            [TextComponent::text(distance.to_string())],
        ));
        border.set_warning_distance(world, distance);
        Ok(())
    }
}

struct WarningTimeExecutor;

impl CommandExecutor for WarningTimeExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        // TODO: Maybe ask player for world, or get the current world
        let worlds = server.worlds.read();
        let world = worlds
            .first()
            .expect("There should always be at least one world");
        let mut border = world.worldborder.lock();

        let Ok(time) = time_consumer().find_arg_default_name(args)? else {
            sender.send_message(
                TextComponent::text(format!(
                    "{} is out of bounds.",
                    time_consumer().default_name()
                ))
                .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        };

        if time == border.warning_time {
            sender.send_message(
                TextComponent::translate("commands.worldborder.warning.time.failed", [])
                    .color(Color::Named(NamedColor::Red)),
            );
            return Ok(());
        }

        sender.send_message(TextComponent::translate(
            "commands.worldborder.warning.time.success",
            [TextComponent::text(time.to_string())],
        ));
        border.set_warning_delay(world, time);
        Ok(())
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(
            literal("add").then(
                argument_default_name(distance_consumer())
                    .execute(AddExecutor)
                    .then(argument_default_name(time_consumer()).execute(AddTimeExecutor)),
            ),
        )
        .then(
            literal("center")
                .then(argument_default_name(Position2DArgumentConsumer).execute(CenterExecutor)),
        )
        .then(
            literal("damage")
                .then(
                    literal("amount").then(
                        argument_default_name(damage_per_block_consumer())
                            .execute(DamageAmountExecutor),
                    ),
                )
                .then(literal("buffer").then(
                    argument_default_name(damage_buffer_consumer()).execute(DamageBufferExecutor),
                )),
        )
        .then(literal("get").execute(GetExecutor))
        .then(
            literal("set").then(
                argument_default_name(distance_consumer())
                    .execute(SetExecutor)
                    .then(argument_default_name(time_consumer()).execute(SetTimeExecutor)),
            ),
        )
        .then(
            literal("warning")
                .then(
                    literal("distance").then(
                        argument_default_name(warning_distance_consumer())
                            .execute(WarningDistanceExecutor),
                    ),
                )
                .then(
                    literal("time")
                        .then(argument_default_name(time_consumer()).execute(WarningTimeExecutor)),
                ),
        )
}
