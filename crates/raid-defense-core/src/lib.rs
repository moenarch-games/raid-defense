#![forbid(unsafe_code)]

use std::collections::{BTreeMap, VecDeque};

use collection_kernels::{SparseMap, SparseSet};

mod default_rules;
mod navigation;
pub mod rules;
mod state_basic;
mod state_combat;
mod state_economy;
mod state_logistics;
mod state_snapshot_path;
mod world;

pub use default_rules::STANDARD_RULES;
pub use navigation::WorkCounters;
pub use rules::*;

pub type EntityId = u32;

// Topology and deterministic representation are engine invariants, not balance knobs.
pub const GRID_WIDTH: i16 = 21;
pub const GRID_HEIGHT: i16 = 21;
pub const CELL_SCALE: i32 = 1_000;

// Compatibility aliases for callers that use the standard profile. New simulation
// code must read the immutable `GameState::rules()` value instead.
pub const STARTING_WOOD: u32 = STANDARD_RULES.economy.starting_wood;
pub const TOWN_WOOD_CAPACITY: u32 = STANDARD_RULES.economy.town_wood_capacity;
pub const SAWMILL_COST: u32 = STANDARD_RULES.buildings.sawmill.wood_cost;
pub const SAWMILL_OUTPUT: u16 = STANDARD_RULES.economy.sawmill_output;
pub const SAWMILL_INTERVAL_TICKS: u16 = STANDARD_RULES.economy.sawmill_interval_ticks;
pub const SAWMILL_LOCAL_WOOD_CAPACITY: u32 = STANDARD_RULES.economy.sawmill_local_wood_capacity;
pub const STORAGE_HOUSE_COST: u32 = STANDARD_RULES.buildings.storage_house.wood_cost;
pub const STORAGE_HOUSE_WOOD_CAPACITY: u32 = STANDARD_RULES.economy.storage_house_wood_capacity;
pub const ARROW_TOWER_COST: u32 = STANDARD_RULES.towers.arrow.build_cost;
pub const CANNON_TOWER_COST: u32 = STANDARD_RULES.towers.cannon.build_cost;
pub const MAX_TOWER_LEVEL: u8 = STANDARD_RULES.towers.max_level;
pub const HOUSE_UNLOCK_COMPLETED_WAVES: u32 = STANDARD_RULES.buildings.house.unlock_completed_waves;
pub const HOUSE_COST: u32 = STANDARD_RULES.buildings.house.wood_cost;
pub const BASE_POPULATION_CAPACITY: u16 = STANDARD_RULES.population.base_capacity;
pub const HOUSE_POPULATION_CAPACITY: u16 = STANDARD_RULES.buildings.house.population_capacity;
pub const PERSON_CARRY_CAPACITY: u16 = STANDARD_RULES.population.carry_capacity;
pub const PERSON_SPEED_MILLI: u16 = STANDARD_RULES.population.speed_milli;
pub const TOWN_MAX_HEALTH: u16 = STANDARD_RULES.buildings.town_hall.max_health;
pub const DAY_LENGTH_TICKS: u16 = STANDARD_RULES.cycle.day_length_ticks;
pub const RAID_RALLY_TICKS: u16 = STANDARD_RULES.cycle.raid_rally_ticks;

const TOWN_ENTITY: EntityId = 0;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Cell {
    pub x: i16,
    pub z: i16,
}

impl Cell {
    #[must_use]
    pub const fn new(x: i16, z: i16) -> Self {
        Self { x, z }
    }

    #[must_use]
    pub fn center_x_milli(self) -> i32 {
        i32::from(self.x) * CELL_SCALE + CELL_SCALE / 2
    }

    #[must_use]
    pub fn center_z_milli(self) -> i32 {
        i32::from(self.z) * CELL_SCALE + CELL_SCALE / 2
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edge {
    North,
    East,
    South,
    West,
}

impl Edge {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    #[must_use]
    pub const fn spawn_cell(self) -> Cell {
        match self {
            Self::North => Cell::new(GRID_WIDTH / 2, 0),
            Self::East => Cell::new(GRID_WIDTH - 1, GRID_HEIGHT / 2),
            Self::South => Cell::new(GRID_WIDTH / 2, GRID_HEIGHT - 1),
            Self::West => Cell::new(0, GRID_HEIGHT / 2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Transform {
    pub x_milli: i32,
    pub z_milli: i32,
}

impl Transform {
    fn at_cell(cell: Cell) -> Self {
        Self {
            x_milli: cell.center_x_milli(),
            z_milli: cell.center_z_milli(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Health {
    pub current: u16,
    pub maximum: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attack {
    pub damage: u16,
    pub range_milli: i32,
    pub cooldown_ticks: u8,
    pub cooldown_remaining: u8,
    pub projectile_speed_milli: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Wood,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceStorage {
    pub wood: u32,
    pub wood_capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceProducer {
    pub resource: ResourceKind,
    pub amount: u16,
    pub interval_ticks: u16,
    pub progress_ticks: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Housing {
    pub capacity: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildingKind {
    TownHall,
    Tower,
    Sawmill,
    StorageHouse,
    House,
    Forest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Building {
    pub kind: BuildingKind,
    pub cell: Cell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TowerArchetype {
    Arrow,
    Cannon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tower {
    pub archetype: TowerArchetype,
    /// Level zero is an authoritative construction site. It becomes active only
    /// after workers have delivered the full build cost to the site.
    pub level: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TowerStats {
    pub damage: u16,
    pub range_milli: i32,
    pub cooldown_ticks: u8,
    pub projectile_speed_milli: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Raider {
    pub archetype: RaiderArchetype,
    pub edge: Edge,
    pub target_storage: EntityId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaveSchedule {
    remaining_raiders: u16,
    spawn_ticks_remaining: u16,
    wave: WaveRules,
    group_index: u8,
    remaining_in_group: u16,
}

impl Default for WaveSchedule {
    fn default() -> Self {
        Self {
            remaining_raiders: 0,
            spawn_ticks_remaining: 0,
            wave: WaveRules::EMPTY,
            group_index: 0,
            remaining_in_group: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Movement {
    pub from: Cell,
    pub to: Cell,
    pub progress_milli: u16,
    pub speed_milli: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Projectile {
    pub target: EntityId,
    pub damage: u16,
    pub speed_milli: u16,
    pub archetype: TowerArchetype,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersonState {
    IdleAtTownHall,
    ToForest,
    HarvestingForest,
    ToSawmill,
    ToSawmillPickup,
    ToStorage,
    ToConstructionStorage,
    ToConstructionSite,
    ToTownHall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Person {
    pub state: PersonState,
    pub target_entity: Option<EntityId>,
    pub cargo_wood: u16,
    pub cargo_capacity: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    PlaceTower {
        x: i16,
        z: i16,
        archetype: TowerArchetype,
    },
    PlaceSawmill {
        x: i16,
        z: i16,
    },
    PlaceStorageHouse {
        x: i16,
        z: i16,
    },
    PlaceHouse {
        x: i16,
        z: i16,
    },
    UpgradeTower {
        x: i16,
        z: i16,
    },
    StartWave,
    AdvanceTick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    TowerConstructionStarted {
        entity: EntityId,
        cell: Cell,
        archetype: TowerArchetype,
        wood_required: u32,
    },
    SawmillBuilt {
        entity: EntityId,
        cell: Cell,
        wood_cost: u32,
    },
    StorageHouseBuilt {
        entity: EntityId,
        cell: Cell,
        wood_cost: u32,
        wood_capacity: u32,
    },
    HouseBuilt {
        entity: EntityId,
        cell: Cell,
        wood_cost: u32,
        people_added: u16,
        population_capacity: u16,
    },
    TowerUpgraded {
        entity: EntityId,
        archetype: TowerArchetype,
        level: u8,
        wood_cost: u32,
    },
    WaveStarted {
        wave: u32,
        raiders: u16,
        rally_ticks: u16,
    },
    TickAdvanced {
        tick: u64,
        shots: u16,
        impacts: u16,
        kills: u16,
        /// Wood cut by people at forests during this tick.
        wood_produced: u16,
        wood_picked_up: u16,
        wood_delivered: u16,
        towers_completed: u16,
        wood_stolen: u16,
        town_damage: u16,
        completed_wave: Option<u32>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameError {
    OutOfBounds,
    CellOccupied,
    ProtectedCell,
    PathBlocked,
    InsufficientWood,
    NoForestInRange,
    HouseLocked,
    NoTower,
    MaxTowerLevel,
    RaidersStillActive,
    ScenarioComplete,
    GameOver,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityKind {
    TownHall,
    Tower,
    Sawmill,
    StorageHouse,
    House,
    Forest,
    Person,
    Raider,
    Projectile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub kind: EntityKind,
    pub x_milli: i32,
    pub z_milli: i32,
    pub cell: Cell,
    pub health: u16,
    pub max_health: u16,
    pub attack_damage: u16,
    pub attack_range_milli: i32,
    pub tower_archetype: Option<TowerArchetype>,
    pub tower_level: u8,
    pub upgrade_cost: Option<u32>,
    pub raider_archetype: Option<RaiderArchetype>,
    pub projectile_target: Option<EntityId>,
    pub stored_wood: u32,
    pub wood_capacity: u32,
    pub production_resource: Option<ResourceKind>,
    pub production_amount: u16,
    pub production_interval_ticks: u16,
    pub production_progress_ticks: u16,
    pub housing_capacity: u16,
    pub person_state: Option<PersonState>,
    pub person_target_entity: Option<EntityId>,
    pub cargo_wood: u16,
    pub cargo_capacity: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameSnapshot {
    pub seed: u64,
    pub tick: u64,
    pub wood: u32,
    pub wood_capacity: u32,
    pub wave: u32,
    pub completed_waves: u32,
    pub people: u16,
    pub population_capacity: u16,
    pub houses_unlocked: bool,
    pub town_health: u16,
    pub town_max_health: u16,
    pub grid_width: i16,
    pub grid_height: i16,
    pub entities: Vec<EntitySnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameState {
    seed: u64,
    rules: GameRules,
    tick: u64,
    wave: u32,
    completed_waves: u32,
    day_ticks_remaining: u16,
    raid_rally_ticks_remaining: u16,
    wave_schedule: WaveSchedule,
    next_entity: EntityId,
    transforms: SparseMap<Transform>,
    health: SparseMap<Health>,
    attacks: SparseMap<Attack>,
    buildings: SparseMap<Building>,
    towers: SparseMap<Tower>,
    raiders: SparseMap<Raider>,
    movements: SparseMap<Movement>,
    projectiles: SparseMap<Projectile>,
    storage: SparseMap<ResourceStorage>,
    producers: SparseMap<ResourceProducer>,
    housing: SparseMap<Housing>,
    people: SparseMap<Person>,
    work_ticks: SparseMap<u16>,
    forest_regrowth_ready_tick: BTreeMap<EntityId, u64>,
    alive: SparseSet,
    /// Retained navigation and work counters; never authoritative.
    instrumentation: navigation::Instrumentation,
}

include!("helpers.rs");
include!("core_tests.rs");
