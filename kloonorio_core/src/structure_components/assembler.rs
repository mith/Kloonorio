use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        event::{Event, EventReader},
        query::{With, Without},
        system::{Commands, Query, Res},
    },
    hierarchy::Children,
    reflect::Reflect,
    time::{Time, Timer, TimerMode},
};

use crate::{
    inventory::{Inventory, ItemFilter, Output, Source},
    item::Item,
    recipe::Recipe,
    types::{ActiveCraft, CraftingQueue, Powered, Working},
};

pub struct AssemblerPlugin;

impl Plugin for AssemblerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (assembler_change_recipe, assembler_tick))
            .add_event::<ChangeAssemblerRecipeEvent>();
    }
}
#[derive(Component, Default, Debug, Reflect)]
pub struct Assembler {
    pub recipe: Option<Recipe>,
}

#[derive(Debug, Event)]
pub struct ChangeAssemblerRecipeEvent {
    pub entity: Entity,
    pub recipe: Recipe,
}

fn assembler_change_recipe(
    mut assembler_query: Query<(&mut Assembler, &mut CraftingQueue, &Children)>,
    mut change_recipe_events: EventReader<ChangeAssemblerRecipeEvent>,
    mut source_query: Query<&mut Inventory, (With<Source>, Without<Output>)>,
) {
    for event in change_recipe_events.read() {
        if let Ok((mut assembler, mut crafting_queue, children)) =
            assembler_query.get_mut(event.entity)
        {
            assembler.recipe = Some(event.recipe.clone());
            crafting_queue.0.clear();

            let source_entity = children.iter().find(|c| source_query.get(**c).is_ok());
            let mut source = source_query.get_mut(*source_entity.unwrap()).unwrap();
            source.allowed_items = ItemFilter::Only(
                event
                    .recipe
                    .ingredients
                    .iter()
                    .map(|(p, _)| Item::new(p.to_string()))
                    .collect(),
            );
        }
    }
}

pub fn assembler_tick(
    mut commands: Commands,
    mut assembler_query: Query<(Entity, &Assembler, &mut CraftingQueue, &Children), With<Powered>>,
    mut source_query: Query<&mut Inventory, (With<Source>, Without<Output>)>,
    mut output_query: Query<&mut Inventory, (With<Output>, Without<Source>)>,
    time: Res<Time>,
) {
    for (entity, assembler, mut crafting_queue, children) in assembler_query.iter_mut() {
        let source_entity = children.iter().find(|c| source_query.get(**c).is_ok());
        let output_entity = children.iter().find(|c| output_query.get(**c).is_ok());

        let mut source = source_query.get_mut(*source_entity.unwrap()).unwrap();
        let mut output = output_query.get_mut(*output_entity.unwrap()).unwrap();

        let Some(recipe) = &assembler.recipe else {
            continue;
        };

        if source.has_items(&recipe.ingredients)
            && crafting_queue.0.is_empty()
            && output.can_add(&recipe.products)
        {
            source.remove_items(&recipe.ingredients);
            crafting_queue.0.push_back(ActiveCraft {
                timer: Timer::from_seconds(recipe.crafting_time, TimerMode::Repeating),
                recipe: recipe.clone(),
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
