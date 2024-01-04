use bevy::ecs::{
    entity::Entity,
    query::{ReadOnlyWorldQuery, With, Without, WorldQuery},
    system::{Query, SystemParam},
};

use super::{Fuel, Inventory, Output, Source, Storage};

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
    Without<Storage>,
)>;

pub type SourceInventoryQuery = InventoryQuery<(
    With<Source>,
    Without<Fuel>,
    Without<Output>,
    Without<Storage>,
)>;

pub type OutputInventoryQuery = InventoryQuery<(
    With<Output>,
    Without<Fuel>,
    Without<Source>,
    Without<Storage>,
)>;

pub type StorageInventoryQuery = InventoryQuery<(
    With<Storage>,
    Without<Fuel>,
    Without<Source>,
    Without<Output>,
)>;

#[derive(SystemParam)]
pub struct InventoryParams<'w, 's> {
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

impl InventoryParams<'_, '_> {
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
