use bevy::{
    ecs::{entity::Entity, query::ReadOnlyWorldQuery, system::Query, world::Mut},
    hierarchy::Children,
};

use super::{inventory_params::InventoryQuery, Inventory};

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
