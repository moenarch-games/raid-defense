use super::*;

impl GameState {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::with_rules(seed, STANDARD_RULES)
    }

    #[must_use]
    pub fn with_rules(seed: u64, rules: GameRules) -> Self {
        Self::try_with_rules(seed, rules).expect("game rules must be valid")
    }

    pub fn try_with_rules(seed: u64, rules: GameRules) -> Result<Self, RulesError> {
        rules.validate()?;
        let mut state = Self {
            seed,
            rules,
            tick: 0,
            wave: 0,
            completed_waves: 0,
            day_ticks_remaining: rules.cycle.day_length_ticks,
            raid_rally_ticks_remaining: 0,
            wave_schedule: WaveSchedule::default(),
            next_entity: 1,
            transforms: SparseMap::new(),
            health: SparseMap::new(),
            attacks: SparseMap::new(),
            buildings: SparseMap::new(),
            towers: SparseMap::new(),
            raiders: SparseMap::new(),
            movements: SparseMap::new(),
            projectiles: SparseMap::new(),
            storage: SparseMap::new(),
            producers: SparseMap::new(),
            housing: SparseMap::new(),
            people: SparseMap::new(),
            work_ticks: SparseMap::new(),
            forest_regrowth_ready_tick: BTreeMap::new(),
            alive: SparseSet::new(),
            instrumentation: navigation::Instrumentation::default(),
        };

        let town_cell = town_center();
        state.spawn_entity(TOWN_ENTITY);
        state
            .transforms
            .insert(entity_key(TOWN_ENTITY), Transform::at_cell(town_cell));
        state.health.insert(
            entity_key(TOWN_ENTITY),
            Health {
                current: rules.buildings.town_hall.max_health,
                maximum: rules.buildings.town_hall.max_health,
            },
        );
        state.buildings.insert(
            entity_key(TOWN_ENTITY),
            Building {
                kind: BuildingKind::TownHall,
                cell: town_cell,
            },
        );
        state.invalidate_navigation();
        state.storage.insert(
            entity_key(TOWN_ENTITY),
            ResourceStorage {
                wood: rules.economy.starting_wood,
                wood_capacity: rules.economy.town_wood_capacity,
            },
        );
        state.housing.insert(
            entity_key(TOWN_ENTITY),
            Housing {
                capacity: rules.population.base_capacity,
            },
        );
        for _ in 0..rules.population.starting_people {
            state.spawn_person();
        }
        state.spawn_seeded_forests();
        Ok(state)
    }

    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    #[must_use]
    pub const fn rules(&self) -> GameRules {
        self.rules
    }

    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub const fn wave(&self) -> u32 {
        self.wave
    }

    #[must_use]
    pub const fn completed_waves(&self) -> u32 {
        self.completed_waves
    }

    #[must_use]
    pub const fn day_ticks_remaining(&self) -> u16 {
        self.day_ticks_remaining
    }

    #[must_use]
    pub const fn raid_rally_ticks_remaining(&self) -> u16 {
        self.raid_rally_ticks_remaining
    }

    #[must_use]
    pub const fn is_rallying(&self) -> bool {
        self.raid_rally_ticks_remaining != 0
    }

    #[must_use]
    pub fn is_night(&self) -> bool {
        self.wave_schedule.remaining_raiders != 0 || self.raider_count() != 0
    }

    #[must_use]
    pub fn houses_unlocked(&self) -> bool {
        self.rules.houses_unlocked(self.completed_waves)
    }

    #[must_use]
    pub fn wood(&self) -> u32 {
        self.settlement_wood_total()
    }

    #[must_use]
    pub fn wood_capacity(&self) -> u32 {
        self.settlement_wood_capacity_total()
    }

    #[must_use]
    pub fn population_capacity(&self) -> u16 {
        self.housing.iter().fold(0_u16, |total, (_, housing)| {
            total.saturating_add(housing.capacity)
        })
    }

    #[must_use]
    pub fn people_count(&self) -> usize {
        self.people.iter().count()
    }

    #[must_use]
    pub fn town_health(&self) -> u16 {
        self.health
            .get(entity_key(TOWN_ENTITY))
            .map_or(0, |health| health.current)
    }

    #[must_use]
    pub fn raider_count(&self) -> usize {
        self.raiders.iter().count()
    }

    #[must_use]
    pub fn tower_count(&self) -> usize {
        self.towers
            .iter()
            .filter(|(_, tower)| tower.level > 0)
            .count()
    }

    #[must_use]
    pub fn construction_site_count(&self) -> usize {
        self.towers
            .iter()
            .filter(|(_, tower)| tower.level == 0)
            .count()
    }

    #[must_use]
    pub fn sawmill_count(&self) -> usize {
        self.producers.iter().count()
    }

    #[must_use]
    pub fn storage_house_count(&self) -> usize {
        self.buildings
            .iter()
            .filter(|(_, building)| building.kind == BuildingKind::StorageHouse)
            .count()
    }

    #[must_use]
    pub fn forest_count(&self) -> usize {
        self.forest_count_internal()
    }

    #[must_use]
    pub fn house_count(&self) -> usize {
        self.buildings
            .iter()
            .filter(|(_, building)| building.kind == BuildingKind::House)
            .count()
    }

    #[must_use]
    pub fn projectile_count(&self) -> usize {
        self.projectiles.iter().count()
    }

    pub fn apply(&mut self, command: Command) -> Result<Event, GameError> {
        match command {
            Command::PlaceTower { x, z, archetype } => self.place_tower(Cell::new(x, z), archetype),
            Command::PlaceSawmill { x, z } => self.place_sawmill(Cell::new(x, z)),
            Command::PlaceStorageHouse { x, z } => self.place_storage_house(Cell::new(x, z)),
            Command::PlaceHouse { x, z } => self.place_house(Cell::new(x, z)),
            Command::UpgradeTower { x, z } => self.upgrade_tower(Cell::new(x, z)),
            Command::StartWave => self.start_wave(),
            Command::AdvanceTick => Ok(self.advance_tick()),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> GameSnapshot {
        let mut ids = self.alive.iter().map(key_entity).collect::<Vec<_>>();
        ids.sort_unstable();
        let entities = ids
            .into_iter()
            .filter_map(|id| self.entity_snapshot(id))
            .collect();

        GameSnapshot {
            seed: self.seed,
            tick: self.tick,
            wood: self.wood(),
            wood_capacity: self.wood_capacity(),
            wave: self.wave,
            completed_waves: self.completed_waves,
            people: u16::try_from(self.people_count()).unwrap_or(u16::MAX),
            population_capacity: self.population_capacity(),
            houses_unlocked: self.houses_unlocked(),
            town_health: self.town_health(),
            town_max_health: self.rules.buildings.town_hall.max_health,
            grid_width: GRID_WIDTH,
            grid_height: GRID_HEIGHT,
            entities,
        }
    }

    #[must_use]
    pub fn checksum(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        feed_u64(&mut hash, self.seed);
        feed_u64(&mut hash, self.rules.fingerprint());
        feed_u64(&mut hash, self.tick);
        feed_u64(&mut hash, u64::from(self.wave));
        feed_u64(&mut hash, u64::from(self.completed_waves));
        feed_u16(&mut hash, self.day_ticks_remaining);
        feed_u16(&mut hash, self.raid_rally_ticks_remaining);
        feed_u16(&mut hash, self.wave_schedule.remaining_raiders);
        feed_u16(&mut hash, self.wave_schedule.spawn_ticks_remaining);
        feed_byte(&mut hash, self.wave_schedule.group_index);
        feed_u16(&mut hash, self.wave_schedule.remaining_in_group);
        feed_u64(&mut hash, u64::from(self.next_entity));

        for entity in &self.snapshot().entities {
            feed_u64(&mut hash, u64::from(entity.id));
            feed_byte(
                &mut hash,
                match entity.kind {
                    EntityKind::TownHall => 0,
                    EntityKind::Tower => 1,
                    EntityKind::Sawmill => 2,
                    EntityKind::StorageHouse => 3,
                    EntityKind::House => 4,
                    EntityKind::Forest => 5,
                    EntityKind::Person => 6,
                    EntityKind::Raider => 7,
                    EntityKind::Projectile => 8,
                },
            );
            feed_i32(&mut hash, entity.x_milli);
            feed_i32(&mut hash, entity.z_milli);
            feed_i16(&mut hash, entity.cell.x);
            feed_i16(&mut hash, entity.cell.z);
            feed_u16(&mut hash, entity.health);
            feed_u16(&mut hash, entity.max_health);

            if let Some(attack) = self.attacks.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_u16(&mut hash, attack.damage);
                feed_i32(&mut hash, attack.range_milli);
                feed_byte(&mut hash, attack.cooldown_ticks);
                feed_byte(&mut hash, attack.cooldown_remaining);
                feed_u16(&mut hash, attack.projectile_speed_milli);
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(tower) = self.towers.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_byte(&mut hash, tower_archetype_code(tower.archetype));
                feed_byte(&mut hash, tower.level);
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(movement) = self.movements.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_i16(&mut hash, movement.from.x);
                feed_i16(&mut hash, movement.from.z);
                feed_i16(&mut hash, movement.to.x);
                feed_i16(&mut hash, movement.to.z);
                feed_u16(&mut hash, movement.progress_milli);
                feed_u16(&mut hash, movement.speed_milli);
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(raider) = self.raiders.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_byte(&mut hash, raider_archetype_code(raider.archetype));
                feed_byte(
                    &mut hash,
                    match raider.edge {
                        Edge::North => 0,
                        Edge::East => 1,
                        Edge::South => 2,
                        Edge::West => 3,
                    },
                );
                feed_u64(&mut hash, u64::from(raider.target_storage));
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(projectile) = self.projectiles.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_u64(&mut hash, u64::from(projectile.target));
                feed_u16(&mut hash, projectile.damage);
                feed_u16(&mut hash, projectile.speed_milli);
                feed_byte(&mut hash, tower_archetype_code(projectile.archetype));
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(storage) = self.storage.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_u64(&mut hash, u64::from(storage.wood));
                feed_u64(&mut hash, u64::from(storage.wood_capacity));
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(ready_tick) = self.forest_regrowth_ready_tick.get(&entity.id) {
                feed_byte(&mut hash, 1);
                feed_u64(&mut hash, *ready_tick);
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(producer) = self.producers.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_byte(&mut hash, resource_kind_code(producer.resource));
                feed_u16(&mut hash, producer.amount);
                feed_u16(&mut hash, producer.interval_ticks);
                feed_u16(&mut hash, producer.progress_ticks);
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(housing) = self.housing.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_u16(&mut hash, housing.capacity);
            } else {
                feed_byte(&mut hash, 0);
            }

            if let Some(person) = self.people.get(entity_key(entity.id)) {
                feed_byte(&mut hash, 1);
                feed_byte(&mut hash, person_state_code(person.state));
                if let Some(target) = person.target_entity {
                    feed_byte(&mut hash, 1);
                    feed_u64(&mut hash, u64::from(target));
                } else {
                    feed_byte(&mut hash, 0);
                }
                feed_u16(&mut hash, person.cargo_wood);
                feed_u16(&mut hash, person.cargo_capacity);
                if let Some(work_ticks) = self.work_ticks.get(entity_key(entity.id)) {
                    feed_byte(&mut hash, 1);
                    feed_u16(&mut hash, *work_ticks);
                } else {
                    feed_byte(&mut hash, 0);
                }
            } else {
                feed_byte(&mut hash, 0);
            }
        }

        hash
    }
}
