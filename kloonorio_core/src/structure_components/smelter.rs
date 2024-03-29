use bevy::prelude::*;

use crate::types::{ActiveCraft, CraftingQueue, Powered, Working};
use crate::{
    inventory::{Inventory, Output, Source},
    item::Item,
    recipe::Recipe,
};

#[derive(Component)]
pub struct Smelter;

pub fn smelter_tick(
    mut commands: Commands,
    mut smelter_query: Query<
        (Entity, &mut CraftingQueue, &Children),
        (With<Smelter>, With<Powered>),
    >,
    mut source_query: Query<&mut Inventory, (With<Source>, Without<Output>)>,
    mut output_query: Query<&mut Inventory, (With<Output>, Without<Source>)>,
    time: Res<Time>,
) {
    for (entity, mut crafting_queue, children) in smelter_query.iter_mut() {
        let source_entity = children.iter().find(|c| source_query.get(**c).is_ok());
        let output_entity = children.iter().find(|c| output_query.get(**c).is_ok());

        let mut source = source_query.get_mut(*source_entity.unwrap()).unwrap();
        let mut output = output_query.get_mut(*output_entity.unwrap()).unwrap();

        if source.has_items(&[(Item::new("Iron ore"), 1)])
            && crafting_queue.0.is_empty()
            && output.can_add(&[(Item::new("Iron plate"), 1)])
        {
            source.remove_items(&[(Item::new("Iron ore"), 1)]);
            crafting_queue.0.push_back(ActiveCraft {
                timer: Timer::from_seconds(1., TimerMode::Repeating),
                recipe: Recipe {
                    ingredients: vec![(Item::new("Iron ore"), 1u32)],
                    products: vec![(Item::new("Iron plate"), 1u32)],
                    crafting_time: 0.5,
                    name: "Iron Plate".into(),
                },
            });
            commands.entity(entity).insert(Working);
        }

        if let Some(active_build) = crafting_queue.0.front_mut() {
            if active_build.timer.tick(time.delta()).just_finished() {
                output.add_items(&active_build.recipe.products);
                crafting_queue.0.pop_front();
                commands.entity(entity).remove::<Working>();
            }
        }
    }
}
