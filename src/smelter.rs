use bevy::prelude::*;

use crate::inventory::{Output, Source};
use crate::recipe_loader::Recipe;
use crate::types::{ActiveCraft, CraftingQueue, Powered, Resource, Working};

#[derive(Component)]
pub struct Smelter;

pub fn smelter_tick(
    mut commands: Commands,
    mut smelter_query: Query<
        (Entity, &mut CraftingQueue, &mut Source, &mut Output),
        (With<Smelter>, With<Powered>),
    >,
    time: Res<Time>,
) {
    for (entity, mut crafting_queue, mut source, mut output) in smelter_query.iter_mut() {
        if source.0.has_items(&[(Resource::IronOre, 1)])
            && crafting_queue.0.is_empty()
            && output.0.can_add(&[(Resource::IronPlate, 1)])
        {
            source.0.remove_items(&[(Resource::IronOre, 1)]);
            crafting_queue.0.push_back(ActiveCraft {
                timer: Timer::from_seconds(1., false),
                blueprint: Recipe {
                    materials: vec![(Resource::IronOre, 1u32)],
                    products: vec![(Resource::IronPlate, 1u32)],
                    crafting_time: 0.5,
                    name: "Iron Plate".into(),
                },
            });
            commands.entity(entity).insert(Working);
        }

        if let Some(active_build) = crafting_queue.0.front_mut() {
            if active_build.timer.tick(time.delta()).just_finished() {
                output.0.add_items(&active_build.blueprint.products);
                crafting_queue.0.pop_front();
                commands.entity(entity).remove::<Working>();
            }
        }
    }
}
