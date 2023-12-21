use bevy::{
    ecs::{
        query::{ReadOnlyWorldQuery, WorldQuery},
        system::SystemParam,
    },
    math::Vec3Swizzles,
    prelude::*,
};
use bevy_ecs_tilemap::tiles::TileTextureIndex;
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};
use tracing::instrument;

use crate::{
    inventory::{Fuel, Inventory, Output, Source, Stack, Storage},
    placeable::Building,
    structure_components::{burner::Burner, transport_belt::TransportBelt},
    terrain::{COAL, IRON, STONE, TREE},
    types::Item,
};

pub fn texture_id_to_product(index: TileTextureIndex) -> Item {
    match index.0 {
        COAL => Item::new("Coal"),
        IRON => Item::new("Iron ore"),
        STONE => Item::new("Stone"),
        TREE => Item::new("Wood"),
        _ => panic!("Unknown product"),
    }
}

pub fn product_to_texture(product: &Item) -> String {
    match product {
        // Product::name) => name.to_lowercase().replace(" ", "_"),
        _ => "no_icon".to_string(),
    }
}

/// Spawn a stack of items at the given position
#[instrument(skip(commands, asset_server))]
pub fn spawn_stack(
    commands: &mut Commands,
    stack: Stack,
    asset_server: &AssetServer,
    position: Vec3,
) {
    let path = format!("textures/icons/{}.png", product_to_texture(&stack.item));
    debug!("Loading texture at {:?}", path);
    commands.spawn((
        Name::new(stack.item.to_string()),
        stack,
        Collider::cuboid(3., 3.),
        SpriteBundle {
            texture: asset_server.load(path),
            transform: Transform::from_translation(position),
            sprite: Sprite {
                custom_size: Some(Vec2::new(6., 6.)),
                ..default()
            },
            ..default()
        },
    ));
}

#[instrument(skip(inventories_query, children))]
pub fn drop_into_entity_inventory(
    inventories_query: &mut Query<&mut Inventory, Without<Output>>,
    collider_entity: Entity,
    stack: Stack,
    children: &Query<&Children>,
) -> bool {
    if let Ok(inventory) = inventories_query.get_mut(collider_entity).as_mut() {
        if inventory.can_add(&[(stack.item.clone(), stack.amount)]) {
            inventory.add_stack(stack);
            debug!("Dropped into inventory");
            return true;
        } else {
            debug!("No space in inventory");
            return false;
        }
    } else {
        debug!("No inventory component found on entity, searching children.");
        for child in children.iter_descendants(collider_entity) {
            if let Ok(inventory) = inventories_query.get_mut(child).as_mut() {
                if inventory.can_add(&[(stack.item.clone(), stack.amount)]) {
                    debug!("Dropped into child inventory");
                    inventory.add_stack(stack);
                    return true;
                }
            }
        }
        debug!("No inventory found on children.");
        return false;
    }
}

/// Drop a stack in a suitable inventory or drop it on the floor. Returns false when neither could
/// be done
#[instrument(skip(
    commands,
    rapier_context,
    asset_server,
    inventories_query,
    belts_query,
    children
))]
pub fn drop_stack_at_point(
    commands: &mut Commands,
    rapier_context: &RapierContext,
    asset_server: &AssetServer,
    inventories_query: &mut Query<&mut Inventory, Without<Output>>,
    belts_query: &mut Query<&mut TransportBelt>,
    children: &Query<&Children>,
    stack: Stack,
    drop_point: Vec3,
) -> bool {
    if let Some(collider_entity) = rapier_context.intersection_with_shape(
        drop_point.xy(),
        0.,
        &Collider::ball(0.2),
        QueryFilter::new().exclude_sensors(),
    ) {
        debug!(collider_entity = ?collider_entity, "Found entity at drop point");
        if let Ok(mut belt) = belts_query.get_mut(collider_entity) {
            debug!("Found belt at drop point");
            if belt.add(1, stack.item.clone()) {
                debug!("Added to belt");
                return true;
            } else {
                debug!("Belt full");
                return false;
            }
        } else {
            drop_into_entity_inventory(inventories_query, collider_entity, stack, children)
        }
    } else {
        debug!("No entity found at drop point, dropping on the ground");
        spawn_stack(commands, stack, asset_server, drop_point);
        true
    }
}

#[derive(WorldQuery)]
#[world_query(mutable)]
pub struct InventoryQuery<F>
where
    F: ReadOnlyWorldQuery,
{
    pub inventory: &'static mut Inventory,
    _filter: F,
}

pub type FuelInventoryQuery = InventoryQuery<(
    With<Fuel>,
    Without<Source>,
    Without<Output>,
    Without<Building>,
)>;

pub type SourceInventoryQuery = InventoryQuery<(
    With<Source>,
    Without<Fuel>,
    Without<Output>,
    Without<Building>,
)>;

pub type OutputInventoryQuery = InventoryQuery<(
    With<Output>,
    Without<Fuel>,
    Without<Source>,
    Without<Building>,
)>;

pub type StorageInventoryQuery = InventoryQuery<(
    With<Storage>,
    Without<Fuel>,
    Without<Source>,
    Without<Output>,
    Without<Building>,
    Without<Burner>,
)>;

#[derive(SystemParam)]
pub struct Inventories<'w, 's> {
    pub fuel_inventories: Query<'w, 's, FuelInventoryQuery>,
    pub source_inventories: Query<'w, 's, SourceInventoryQuery>,
    pub output_inventories: Query<'w, 's, OutputInventoryQuery>,
    pub storage_inventories: Query<'w, 's, StorageInventoryQuery>,
}

#[derive(Debug, Clone, Copy)]
pub enum InventoryType {
    Fuel,
    Source,
    Output,
    Storage,
}

impl Inventories<'_, '_> {
    pub fn get_inventory_component(
        &self,
        entity: Entity,
        inventory_type: InventoryType,
    ) -> Option<&Inventory> {
        match inventory_type {
            InventoryType::Fuel => self.fuel_inventories.get(entity).ok().map(|i| i.inventory),
            InventoryType::Source => self
                .source_inventories
                .get(entity)
                .ok()
                .map(|i| i.inventory),
            InventoryType::Output => self
                .output_inventories
                .get(entity)
                .ok()
                .map(|i| i.inventory),
            InventoryType::Storage => self
                .storage_inventories
                .get(entity)
                .ok()
                .map(|i| i.inventory),
        }
    }
}

/// Get the inventory of a child entity.
/// Returns a tuple of the child entity and the inventory.
pub fn get_inventory_child<'b, I>(
    children: &Children,
    output_query: &'b Query<InventoryQuery<I>>,
) -> (Entity, &'b Inventory)
where
    I: ReadOnlyWorldQuery,
{
    let output = children
        .iter()
        .flat_map(|c| output_query.get(*c).map(|i| (*c, i.inventory)))
        .next()
        .unwrap();
    output
}

/// Get the inventory of a child entity.
/// Returns a tuple of the child entity and the inventory.
pub fn get_inventory_child_mut<'b, I>(
    children: &Children,
    output_query: &'b mut Query<InventoryQuery<I>>,
) -> (Entity, Mut<'b, Inventory>)
where
    I: ReadOnlyWorldQuery,
{
    let child_id = children.iter().find(|c| output_query.get(**c).is_ok());
    if let Some(child_id) = child_id {
        let output = output_query.get_mut(*child_id).unwrap();
        (*child_id, output.inventory)
    } else {
        panic!("no child with inventory found");
    }
}

pub fn try_get_inventory_child<'b, I>(
    children: &Children,
    output_query: &'b Query<InventoryQuery<I>>,
) -> Option<(Entity, &'b Inventory)>
where
    I: ReadOnlyWorldQuery,
{
    let output = children
        .iter()
        .flat_map(|c| output_query.get(*c).map(|i| (*c, i.inventory)))
        .next();
    output
}

pub fn find_entities_on_position(
    rapier_context: &RapierContext,
    position: Vec2,
    filter: Option<QueryFilter>,
) -> Vec<Entity> {
    let mut entities = Vec::new();
    rapier_context.intersections_with_point(
        position,
        filter.unwrap_or_else(QueryFilter::new),
        |entity| {
            entities.push(entity);
            true
        },
    );
    entities
}

pub fn try_subtract(value: &mut u32, subtract_amount: u32) -> u32 {
    let original_value = *value;
    *value = value.saturating_sub(subtract_amount);
    original_value - *value
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn try_subtract_enough() {
        let mut value = 10;
        let subtracted = try_subtract(&mut value, 5);
        assert_eq!(subtracted, 5);
        assert_eq!(value, 5);
    }

    #[test]
    fn try_subtract_not_enough() {
        let mut value = 10;
        let subtracted = try_subtract(&mut value, 15);
        assert_eq!(subtracted, 10);
        assert_eq!(value, 0);
    }

    #[test]
    fn try_subtract_exactly_enough() {
        let mut value = 10;
        let subtracted = try_subtract(&mut value, 0);
        assert_eq!(subtracted, 0);
        assert_eq!(value, 10);
    }
}
