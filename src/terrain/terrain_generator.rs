use std::{
    hash::{BuildHasher, Hasher},
    sync::Arc,
};

use ndarray::Array2;
use rand::seq::SliceRandom;

use rand::SeedableRng;

use ahash::{AHasher, RandomState};
use bevy::{ecs::component::Component, math::IVec2};
use fast_poisson::Poisson2D;
use noise::{NoiseFn, OpenSimplex, ScalePoint, Seedable, SuperSimplex, Turbulence};
use rand_xoshiro::Xoshiro256StarStar;

use super::{
    ChunkData, CHUNK_SIZE, COAL, DEEP_WATER, GRASS, GROUND, IRON, STONE, TALL_GRASS, TREE, WATER,
};

#[derive(Component, Clone)]
pub struct TerrainGenerator {
    generator: Arc<Box<dyn ChunkGenerator>>,
}

impl TerrainGenerator {
    pub fn new(generator: Box<dyn ChunkGenerator>) -> Self {
        Self {
            generator: Arc::new(generator),
        }
    }

    pub fn generate_chunk(&self, chunk_position: IVec2) -> ChunkData {
        self.generator.generate_chunk(chunk_position)
    }
}

pub trait ChunkGenerator: Send + Sync {
    fn generate_chunk(&self, chunk_position: IVec2) -> ChunkData;
}

pub struct FlatChunkGenerator {
    tile_type: TileType,
}

impl FlatChunkGenerator {
    pub fn new(tile_type: TileType) -> Self {
        Self { tile_type }
    }
}

impl ChunkGenerator for FlatChunkGenerator {
    fn generate_chunk(&self, _chunk_position: IVec2) -> ChunkData {
        let mut chunk =
            Array2::<Option<TileType>>::default((CHUNK_SIZE.x as usize, CHUNK_SIZE.y as usize));
        chunk.fill(Some(self.tile_type));
        ChunkData(chunk)
    }
}

pub struct NoiseChunkGenerator {
    seed: u32,
}

impl NoiseChunkGenerator {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }
}

impl ChunkGenerator for NoiseChunkGenerator {
    fn generate_chunk(&self, chunk_position: IVec2) -> ChunkData {
        let seed = self.seed;
        generate_chunk_noise(seed, chunk_position)
    }
}

#[derive(Debug)]
struct RadiusNoise {
    location: [f64; 2],
    radius: f64,
}

impl NoiseFn<f64, 2> for RadiusNoise {
    /// Return 1. if the point is within the radius, 0. otherwise
    fn get(&self, point: [f64; 2]) -> f64 {
        let dist = (point[0] - self.location[0]).powi(2) + (point[1] - self.location[1]).powi(2);
        if dist < self.radius.powi(2) {
            1.
        } else {
            0.
        }
    }
}

type TileType = u32;

struct Region {
    ores: Vec<(u32, Turbulence<RadiusNoise, OpenSimplex>)>,
}

fn generate_region(seed: u32, region_location: IVec2) -> Region {
    let useed = seed as u64;
    let mut hasher: AHasher = RandomState::with_seeds(
        useed,
        useed.swap_bytes(),
        useed.count_ones() as u64,
        useed.rotate_left(32),
    )
    .build_hasher();
    hasher.write_i32(region_location.x);
    hasher.write_i32(region_location.y);
    let region_seed = hasher.finish();

    // Generate a list of ore locations for the region
    let ore_locations = Poisson2D::new()
        .with_dimensions(
            [(CHUNK_SIZE.x * 10) as f64, (CHUNK_SIZE.y * 10) as f64],
            30.,
        )
        .with_seed(region_seed)
        .iter()
        .take(10)
        .collect::<Vec<_>>();

    let ore_noise = ore_locations
        .iter()
        .map(|&location| RadiusNoise {
            location,
            radius: 5.,
        })
        .map(|noise| {
            Turbulence::<_, OpenSimplex>::new(noise)
                .set_seed(seed + 11)
                .set_frequency(0.1)
                .set_power(10.)
        });

    let mut rng = Xoshiro256StarStar::seed_from_u64(region_seed);
    let ore_types = ore_locations.iter().map(|_| {
        let ore_types = [(COAL, 2), (IRON, 2), (STONE, 1)];

        let ore_type = ore_types
            .choose_weighted(&mut rng, |item| item.1)
            .unwrap()
            .0;
        ore_type
    });

    Region {
        ores: ore_types.into_iter().zip(ore_noise).collect::<Vec<_>>(),
    }
}

fn generate_chunk_noise(seed: u32, chunk_position: IVec2) -> ChunkData {
    let mut chunk =
        Array2::<Option<TileType>>::default((CHUNK_SIZE.x as usize, CHUNK_SIZE.y as usize));

    let open_simplex = SuperSimplex::new(seed);
    let scale_point = ScalePoint::new(open_simplex).set_scale(0.005);
    let turbulence = Turbulence::<_, SuperSimplex>::new(scale_point)
        .set_seed(seed + 9)
        .set_frequency(0.001)
        .set_power(100.);
    let turbulence_2 = Turbulence::<_, OpenSimplex>::new(turbulence)
        .set_seed(seed + 10)
        .set_frequency(0.1)
        .set_power(10.)
        .set_roughness(103);
    for ((x, y), tile) in chunk.indexed_iter_mut() {
        let noise = turbulence_2.get([
            (chunk_position.x * CHUNK_SIZE.x as i32 + x as i32).into(),
            (chunk_position.y * CHUNK_SIZE.y as i32 + y as i32).into(),
        ]);
        if noise > 0.4 {
            *tile = Some(TREE);
        } else if noise > 0.2 {
            *tile = Some(TALL_GRASS);
        } else if noise > -0.1 {
            *tile = Some(GRASS);
        } else if noise > -0.3 {
            *tile = Some(GROUND);
        } else if noise > -0.4 {
            *tile = Some(WATER);
        } else {
            *tile = Some(DEEP_WATER);
        }
    }

    let region_location = chunk_position / 10 * 10;
    let region = generate_region(seed, region_location);

    for ((x, y), tile) in chunk.indexed_iter_mut() {
        let ore_type = region.ores.iter().fold(None, |acc, (ore_type, noise)| {
            let amount = noise.get([
                ((chunk_position.x - region_location.x) * CHUNK_SIZE.x as i32 + x as i32).into(),
                ((chunk_position.y - region_location.y) * CHUNK_SIZE.y as i32 + y as i32).into(),
            ]);
            if amount > 0. {
                Some(*ore_type)
            } else {
                acc
            }
        });
        if ore_type.is_some() && !matches!(tile, Some(WATER) | Some(DEEP_WATER)) {
            *tile = ore_type;
        }
    }

    ChunkData(chunk)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn generate_chunk_is_reproducible() {
        let seed = 123456789;
        let position = IVec2::new(100, 100);
        let chunk_a = generate_chunk_noise(seed, position);
        let chunk_b = generate_chunk_noise(seed, position);
        assert_eq!(chunk_a, chunk_b);
    }
}
