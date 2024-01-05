use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        query::With,
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Query, Res},
    },
    time::Time,
};
use kloonorio_core::{
    inventory::Inventory,
    player::Player,
    types::{AppState, CraftingQueue},
};

pub struct CraftPlugin;

impl Plugin for CraftPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, craft_ticker.run_if(in_state(AppState::Running)));
    }
}

fn craft_ticker(
    mut player_query: Query<(&mut Inventory, &mut CraftingQueue), With<Player>>,
    time: Res<Time>,
) {
    for (mut inventory, mut build_queue) in &mut player_query {
        if let Some(active_build) = build_queue.0.front_mut() {
            if active_build.timer.tick(time.delta()).just_finished() {
                inventory.add_items(&active_build.recipe.products);
                build_queue.0.pop_front();
            }
        }
    }
}
