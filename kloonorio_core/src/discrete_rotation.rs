use bevy::prelude::*;

pub struct DiscreteRotationPlugin;

impl Plugin for DiscreteRotationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DiscreteRotation>();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompassDirection {
    North = 0,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

pub enum SideCount {
    One = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
}

impl TryFrom<u32> for SideCount {
    type Error = &'static str;

    fn try_from(side_count: u32) -> Result<Self, Self::Error> {
        match side_count {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            4 => Ok(Self::Four),
            8 => Ok(Self::Eight),
            _ => Err("Invalid side count"),
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
pub struct DiscreteRotation {
    current: usize,
    max: usize,
}

impl DiscreteRotation {
    pub fn new(num_sides: SideCount) -> Self {
        Self {
            current: 0,
            max: num_sides as usize,
        }
    }

    pub fn rotate(&mut self) {
        self.current = (self.current + 1) % self.max;
    }

    pub fn set(&mut self, direction: CompassDirection) {
        self.current = match self.max {
            1 => 0,
            2 => match direction {
                CompassDirection::North => 0,
                CompassDirection::South => 1,
                _ => panic!("Invalid direction for 2 sides"),
            },
            4 => match direction {
                CompassDirection::North => 0,
                CompassDirection::East => 1,
                CompassDirection::South => 2,
                CompassDirection::West => 3,
                _ => panic!("Invalid direction for 4 sides"),
            },
            8 => match direction {
                CompassDirection::North => 0,
                CompassDirection::NorthEast => 1,
                CompassDirection::East => 2,
                CompassDirection::SouthEast => 3,
                CompassDirection::South => 4,
                CompassDirection::SouthWest => 5,
                CompassDirection::West => 6,
                CompassDirection::NorthWest => 7,
            },
            _ => unreachable!(),
        };
    }

    pub fn get(&self) -> usize {
        self.current
    }

    pub fn compass_direction(&self) -> CompassDirection {
        match self.max {
            1 => CompassDirection::North,
            2 => match self.current {
                0 => CompassDirection::North,
                1 => CompassDirection::South,
                _ => unreachable!(),
            },
            4 => match self.current {
                0 => CompassDirection::North,
                1 => CompassDirection::East,
                2 => CompassDirection::South,
                3 => CompassDirection::West,
                _ => unreachable!(),
            },
            8 => match self.current {
                0 => CompassDirection::North,
                1 => CompassDirection::NorthEast,
                2 => CompassDirection::East,
                3 => CompassDirection::SouthEast,
                4 => CompassDirection::South,
                5 => CompassDirection::SouthWest,
                6 => CompassDirection::West,
                7 => CompassDirection::NorthWest,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    pub fn radians(&self) -> f32 {
        match self.max {
            1 => 0.0,
            2 => self.current as f32 * std::f32::consts::PI,
            4 => self.current as f32 * std::f32::consts::PI / 2.0,
            8 => self.current as f32 * std::f32::consts::PI / 4.0,
            _ => unreachable!(),
        }
    }
}
