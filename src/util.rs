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
    inventory::{Fuel, Inventory, Output, Source, Stack},
    placeable::Building,
    terrain::{COAL, IRON, STONE, TREE},
    transport_belt::TransportBelt,
    types::Product,
};

pub fn texture_id_to_product(index: TileTextureIndex) -> Product {
    match index.0 {
        COAL => Product::Intermediate("Coal".into()),
        IRON => Product::Intermediate("Iron ore".into()),
        STONE => Product::Intermediate("Stone".into()),
        TREE => Product::Intermediate("Wood".into()),
        _ => panic!("Unknown product"),
    }
}

pub fn product_to_texture(product: &Product) -> String {
    match product {
        // Product::Intermediate(name) => name.to_lowercase().replace(" ", "_"),
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
    let path = format!("textures/icons/{}.png", product_to_texture(&stack.resource));
    debug!("Loading texture at {:?}", path);
    commands.spawn((
        Name::new(stack.resource.to_string()),
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
        if inventory.can_add(&[(stack.resource.clone(), stack.amount)]) {
            inventory.add_stack(stack);
            info!("Dropped into inventory");
            return true;
        } else {
            debug!("No space in inventory");
            return false;
        }
    } else {
        debug!("No inventory component found on entity, searching children.");
        for child in children.iter_descendants(collider_entity) {
            if let Ok(inventory) = inventories_query.get_mut(child).as_mut() {
                if inventory.can_add(&[(stack.resource.clone(), stack.amount)]) {
                    info!("Dropped into child inventory");
                    inventory.add_stack(stack);
                    return true;
                }
            }
        }
        debug!("No inventory found on children.");
        return false;
    }
}

#[instrument(skip(inventories_query, children))]
pub fn take_stack_from_entity_inventory(
    inventories_query: &mut Query<&mut Inventory, (Without<Fuel>, Without<Source>)>,
    target_entity: Entity,
    children: &Query<&Children>,
    max_size: u32,
) -> Option<Stack> {
    if let Ok(inventory) = inventories_query.get_mut(target_entity).as_mut() {
        let taken = inventory.take_stack(max_size);
        if let Some(ref stack) = taken {
            info!(stack = ?stack, "Found stack in inventory");
        }
        return taken;
    } else {
        debug!("No inventory component found on entity, searching children.");
        for child in children.iter_descendants(target_entity) {
            debug!(child = ?child, "Checking child");
            if let Ok(inventory) = inventories_query.get_mut(child).as_mut() {
                debug!("Found inventory on child");
                if let Some(stack) = inventory.take_stack(max_size) {
                    info!(stack = ?stack, "Found stack in child entity");
                    return Some(stack);
                } else {
                    debug!("No stack found in child entity");
                }
            }
        }
        debug!("No inventory found on children.");
        None
    }
}

#[instrument(skip(belts_query))]
pub fn take_stack_from_entity_belt(
    belts_query: &mut Query<&mut TransportBelt>,
    target_entity: Entity,
    max_size: u32,
) -> Option<Stack> {
    if let Ok(mut belt) = belts_query.get_mut(target_entity) {
        belt.take().map(|product| Stack {
            resource: product,
            amount: 1,
        })
    } else {
        None
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
        info!(collider_entity = ?collider_entity, "Found entity at drop point");
        if let Ok(mut belt) = belts_query.get_mut(collider_entity) {
            info!("Found belt at drop point");
            if belt.add(1, stack.resource.clone()) {
                info!("Added to belt");
                return true;
            } else {
                info!("Belt full");
                return false;
            }
        } else {
            drop_into_entity_inventory(inventories_query, collider_entity, stack, children)
        }
    } else {
        info!("No entity found at drop point, dropping on the ground");
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

#[derive(SystemParam)]
pub struct Inventories<'w, 's> {
    pub inventories: Query<'w, 's, &'static mut Inventory>,
    pub fuel_inventories: Query<'w, 's, FuelInventoryQuery>,
    pub source_inventories: Query<'w, 's, SourceInventoryQuery>,
    pub output_inventories: Query<'w, 's, OutputInventoryQuery>,
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

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Resource)]
    struct Target(Entity);

    #[derive(Resource)]
    struct Result(Option<Stack>);

    fn test_system(
        mut commands: Commands,
        mut inventories_query: Query<&mut Inventory, (Without<Fuel>, Without<Source>)>,
        target_entity: Res<Target>,
        children: Query<&Children>,
    ) {
        let result =
            take_stack_from_entity_inventory(&mut inventories_query, target_entity.0, &children, 1);
        commands.insert_resource(Result(result));
    }

    #[test]
    fn take_stack_from_entity_inventory_no_child() {
        let mut app = App::new();

        let mut inventory = Inventory::new(1);
        inventory.add_item(Product::Intermediate("Coal".into()), 1);

        let target_entity_id = app.world.spawn(inventory).id();

        app.insert_resource(Target(target_entity_id));

        app.add_systems(Update, test_system);
        app.update();

        let result = app.world.get_resource::<Result>().unwrap();

        assert_eq!(
            result.0,
            Some(Stack::new(Product::Intermediate("Coal".into()), 1))
        );
    }

    #[test]
    fn take_stack_from_entity_inventory_child_output() {
        let mut app = App::new();

        let mut inventory = Inventory::new(1);
        inventory.add_item(Product::Intermediate("Coal".into()), 1);

        let target_entity_id = app
            .world
            .spawn_empty()
            .with_children(|p| {
                p.spawn((Output, Inventory::new(1)));
            })
            .id();

        app.insert_resource(Target(target_entity_id));

        app.add_systems(Update, test_system);
        app.update();

        let result = app.world.get_resource::<Result>().unwrap();
        assert_eq!(
            result.0,
            Some(Stack::new(Product::Intermediate("Coal".into()), 1))
        );
    }
}
