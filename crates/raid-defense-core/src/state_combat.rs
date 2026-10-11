use super::*;

/// Live raiders bucketed by grid cell for one targeting pass.
///
/// Tower targeting examines only the raiders in the cells overlapped by the
/// tower's range square, so its candidate work follows the local candidate set
/// rather than every raider on the map.
pub(super) struct RaiderCellIndex {
    cells: Vec<Vec<EntityId>>,
}

impl RaiderCellIndex {
    fn cell(&self, x: i16, z: i16) -> &[EntityId] {
        cell_index(Cell::new(x, z))
            .and_then(|index| self.cells.get(index))
            .map_or(&[], Vec::as_slice)
    }
}

fn grid_axis_cell(milli: i64, last: i16) -> i16 {
    let cell = milli.div_euclid(i64::from(CELL_SCALE));
    i16::try_from(cell.clamp(0, i64::from(last))).unwrap_or(last)
}

impl GameState {
    pub(super) fn run_tower_attack_system(&mut self) -> u16 {
        let mut tower_ids = self
            .towers
            .iter()
            .filter_map(|(key, tower)| (tower.level > 0).then_some(key_entity(key)))
            .collect::<Vec<_>>();
        tower_ids.sort_unstable();

        let mut shots = 0_u16;
        let mut raider_index = None;
        for tower in tower_ids {
            let Some(mut attack) = self.attacks.get(entity_key(tower)).copied() else {
                continue;
            };
            if attack.cooldown_remaining > 0 {
                attack.cooldown_remaining -= 1;
                self.attacks.insert(entity_key(tower), attack);
                continue;
            }

            let index = raider_index.get_or_insert_with(|| self.raider_cell_index());
            let Some(target) = self.target_for_tower_in(tower, attack.range_milli, index) else {
                continue;
            };
            let tower_component = self
                .towers
                .get(entity_key(tower))
                .copied()
                .expect("tower ids come from the tower component store");
            let tower_position = self
                .transforms
                .get(entity_key(tower))
                .copied()
                .expect("towers always have transforms");
            let projectile_entity = self.allocate_entity();
            self.transforms
                .insert(entity_key(projectile_entity), tower_position);
            self.projectiles.insert(
                entity_key(projectile_entity),
                Projectile {
                    target,
                    damage: attack.damage,
                    speed_milli: attack.projectile_speed_milli,
                    archetype: tower_component.archetype,
                },
            );

            shots = shots.saturating_add(1);
            attack.cooldown_remaining = attack.cooldown_ticks;
            self.attacks.insert(entity_key(tower), attack);
        }

        shots
    }

    pub(super) fn run_projectile_system(&mut self) -> (u16, Vec<EntityId>) {
        let mut ids = self.projectiles.keys().map(key_entity).collect::<Vec<_>>();
        ids.sort_unstable();
        let mut impacts = 0_u16;
        let mut killed = Vec::new();
        let mut despawn = Vec::new();

        for entity in ids {
            let Some(projectile) = self.projectiles.get(entity_key(entity)).copied() else {
                continue;
            };
            let Some(target_health) = self.health.get(entity_key(projectile.target)).copied()
            else {
                despawn.push(entity);
                continue;
            };
            if target_health.current == 0 {
                despawn.push(entity);
                continue;
            }
            let Some(position) = self.transforms.get(entity_key(entity)).copied() else {
                despawn.push(entity);
                continue;
            };
            let Some(target_position) = self.transforms.get(entity_key(projectile.target)).copied()
            else {
                despawn.push(entity);
                continue;
            };

            let dx = i64::from(target_position.x_milli - position.x_milli);
            let dz = i64::from(target_position.z_milli - position.z_milli);
            let distance_sq = dx * dx + dz * dz;
            let speed = i64::from(projectile.speed_milli);

            if distance_sq <= speed * speed {
                let target_died = {
                    let health = self
                        .health
                        .get_mut(entity_key(projectile.target))
                        .expect("target health was checked above");
                    health.current = health.current.saturating_sub(projectile.damage);
                    health.current == 0
                };
                impacts = impacts.saturating_add(1);
                despawn.push(entity);
                if target_died {
                    killed.push(projectile.target);
                }
                continue;
            }

            let distance = i64::try_from(integer_sqrt(distance_sq as u64))
                .unwrap_or(i64::MAX)
                .max(1);
            let step_x = dx * speed / distance;
            let step_z = dz * speed / distance;
            let next = Transform {
                x_milli: position
                    .x_milli
                    .saturating_add(saturating_i64_to_i32(step_x)),
                z_milli: position
                    .z_milli
                    .saturating_add(saturating_i64_to_i32(step_z)),
            };
            self.transforms.insert(entity_key(entity), next);
        }

        for entity in despawn {
            self.despawn_projectile(entity);
        }
        killed.sort_unstable();
        killed.dedup();
        (impacts, killed)
    }

    pub(super) fn raider_cell_index(&self) -> RaiderCellIndex {
        let cell_count = usize::try_from(GRID_WIDTH).unwrap_or_default()
            * usize::try_from(GRID_HEIGHT).unwrap_or_default();
        let mut cells = vec![Vec::new(); cell_count];
        for key in self.raiders.keys() {
            let Some(transform) = self.transforms.get(key) else {
                continue;
            };
            if let Some(index) = cell_index(cell_for_transform(*transform)) {
                cells[index].push(key_entity(key));
            }
        }
        RaiderCellIndex { cells }
    }

    #[cfg(test)]
    pub(super) fn target_for_tower(&self, tower: EntityId, range_milli: i32) -> Option<EntityId> {
        self.target_for_tower_in(tower, range_milli, &self.raider_cell_index())
    }

    /// Nearest live raider within range (ties by entity id), examining only the
    /// raiders whose cell overlaps the tower's range square.
    pub(super) fn target_for_tower_in(
        &self,
        tower: EntityId,
        range_milli: i32,
        index: &RaiderCellIndex,
    ) -> Option<EntityId> {
        let tower_position = *self.transforms.get(entity_key(tower))?;
        let range = i64::from(range_milli);
        let range_sq = range * range;
        let tower_x = i64::from(tower_position.x_milli);
        let tower_z = i64::from(tower_position.z_milli);
        let min_x = grid_axis_cell(tower_x - range, GRID_WIDTH - 1);
        let max_x = grid_axis_cell(tower_x + range, GRID_WIDTH - 1);
        let min_z = grid_axis_cell(tower_z - range, GRID_HEIGHT - 1);
        let max_z = grid_axis_cell(tower_z + range, GRID_HEIGHT - 1);

        let mut best: Option<(i64, EntityId)> = None;
        let mut evaluated = 0_u64;
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                for &entity in index.cell(x, z) {
                    evaluated += 1;
                    if !self
                        .health
                        .get(entity_key(entity))
                        .is_some_and(|health| health.current > 0)
                    {
                        continue;
                    }
                    let Some(position) = self.transforms.get(entity_key(entity)) else {
                        continue;
                    };
                    let dx = i64::from(position.x_milli) - tower_x;
                    let dz = i64::from(position.z_milli) - tower_z;
                    let distance_sq = dx * dx + dz * dz;
                    if distance_sq <= range_sq
                        && best.is_none_or(|current| (distance_sq, entity) < current)
                    {
                        best = Some((distance_sq, entity));
                    }
                }
            }
        }
        self.count_work(|work| work.target_candidate_evaluations += evaluated);
        best.map(|(_, entity)| entity)
    }

    pub(super) fn run_raider_movement_system(&mut self) -> (u16, u16) {
        let mut raiders = self.raiders.keys().map(key_entity).collect::<Vec<_>>();
        raiders.sort_unstable();
        let mut wood_stolen = 0_u16;
        let mut town_damage = 0_u16;
        let mut finished = Vec::new();

        for entity in raiders {
            let Some(mut movement) = self.movements.get(entity_key(entity)).copied() else {
                continue;
            };
            movement.progress_milli = movement.progress_milli.saturating_add(movement.speed_milli);

            if movement.progress_milli >= CELL_SCALE as u16 {
                movement.progress_milli -= CELL_SCALE as u16;
                movement.from = movement.to;
                let mut raider = self
                    .raiders
                    .get(entity_key(entity))
                    .copied()
                    .expect("raider ids come from the raider store");

                if self.storage_reached(raider.target_storage, movement.from) {
                    let stolen = self
                        .take_wood_at(raider.target_storage, self.raider_wood_steal_amount(entity));
                    if stolen > 0 {
                        wood_stolen =
                            wood_stolen.saturating_add(u16::try_from(stolen).unwrap_or(u16::MAX));
                        finished.push(entity);
                        continue;
                    }
                    if raider.target_storage == TOWN_ENTITY {
                        let damage = self
                            .attacks
                            .get(entity_key(entity))
                            .map_or(0, |attack| attack.damage);
                        town_damage = town_damage.saturating_add(damage);
                        finished.push(entity);
                        continue;
                    }
                    raider.target_storage = self.nearest_raider_storage_from_fields(movement.from);
                    self.raiders.insert(entity_key(entity), raider);
                }

                let mut next = self.raider_next_step(raider.target_storage, movement.from);
                if next.is_none() && raider.target_storage != TOWN_ENTITY {
                    raider.target_storage = TOWN_ENTITY;
                    self.raiders.insert(entity_key(entity), raider);
                    next = self.raider_next_step(TOWN_ENTITY, movement.from);
                }
                let Some(next) = next else {
                    movement.to = movement.from;
                    self.movements.insert(entity_key(entity), movement);
                    self.transforms
                        .insert(entity_key(entity), Transform::at_cell(movement.from));
                    continue;
                };
                movement.to = next;
            }

            let transform = interpolate_transform(movement);
            self.movements.insert(entity_key(entity), movement);
            self.transforms.insert(entity_key(entity), transform);
        }

        if town_damage > 0
            && let Some(health) = self.health.get_mut(entity_key(TOWN_ENTITY))
        {
            health.current = health.current.saturating_sub(town_damage);
        }
        for entity in finished {
            self.despawn_raider(entity);
        }

        (wood_stolen, town_damage)
    }

    /// Next step toward a storage target along its retained distance field.
    fn raider_next_step(&self, target_storage: EntityId, from: Cell) -> Option<Cell> {
        self.count_work(|work| work.path_follow_steps += 1);
        let goals = self.storage_goal_cells(target_storage, None);
        self.with_distance_field(&goals, None, |field| field?.next_step(from))
    }

    /// Nearest stocked settlement storage by retained distance field, falling
    /// back to the Town Hall.
    fn nearest_raider_storage_from_fields(&self, start: Cell) -> EntityId {
        let mut candidates = self
            .settlement_storage_ids()
            .into_iter()
            .filter_map(|entity| {
                let storage = self.storage.get(entity_key(entity))?;
                if storage.wood == 0 {
                    return None;
                }
                let goals = self.storage_goal_cells(entity, None);
                let distance =
                    self.with_distance_field(&goals, None, |field| field?.distance(start))?;
                Some((distance, entity))
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates
            .first()
            .map_or(TOWN_ENTITY, |(_, entity)| *entity)
    }

    pub(super) fn raider_wood_steal_amount(&self, entity: EntityId) -> u32 {
        if self.rules.raids.wave_plan.is_explicit() {
            return self
                .raiders
                .get(entity_key(entity))
                .map_or(0, |raider| self.rules.raider(raider.archetype).wood_steal);
        }
        self.rules.raids.base_wood_steal.saturating_add(
            self.wave
                .saturating_sub(1)
                .saturating_mul(self.rules.raids.wood_steal_per_wave),
        )
    }

    #[cfg(test)]
    pub(super) fn spawn_raider(&mut self, edge: Edge) {
        self.spawn_raider_as(edge, RaiderArchetype::Basic);
    }

    pub(super) fn spawn_raider_as(&mut self, edge: Edge, archetype: RaiderArchetype) {
        let from = edge.spawn_cell();
        let target_storage = self.nearest_raider_storage(from);
        let goals = self.storage_goal_cells(target_storage, None);
        let to = self
            .next_path_step_to_any(from, &goals, None)
            .or_else(|| self.next_path_step_to_any(from, &town_goal_cells(), None))
            .expect("validated building placements keep every edge connected to town hall");
        let entity = self.allocate_entity();

        let (max_health, damage, speed_milli) = if self.rules.raids.wave_plan.is_explicit() {
            let profile = self.rules.raider(archetype);
            (profile.health, profile.damage, profile.speed_milli)
        } else {
            let wave_bonus = u16::try_from(self.wave.saturating_sub(1))
                .unwrap_or(u16::MAX)
                .saturating_mul(self.rules.raids.health_per_wave);
            let max_health = self.rules.raids.base_health.saturating_add(wave_bonus);
            let damage_steps =
                u16::try_from(self.wave / self.rules.raids.damage_increase_every_waves)
                    .unwrap_or(u16::MAX);
            let damage_bonus = damage_steps.saturating_mul(self.rules.raids.damage_increase_amount);
            (
                max_health,
                self.rules.raids.base_damage.saturating_add(damage_bonus),
                self.rules.raids.speed_milli,
            )
        };

        self.transforms
            .insert(entity_key(entity), Transform::at_cell(from));
        self.health.insert(
            entity_key(entity),
            Health {
                current: max_health,
                maximum: max_health,
            },
        );
        self.attacks.insert(
            entity_key(entity),
            Attack {
                damage,
                range_milli: 0,
                cooldown_ticks: 0,
                cooldown_remaining: 0,
                projectile_speed_milli: 0,
            },
        );
        self.raiders.insert(
            entity_key(entity),
            Raider {
                archetype,
                edge,
                target_storage,
            },
        );
        self.movements.insert(
            entity_key(entity),
            Movement {
                from,
                to,
                progress_milli: 0,
                speed_milli,
            },
        );
    }

    pub(super) fn spawn_person(&mut self) -> EntityId {
        let entity = self.allocate_entity();
        self.transforms
            .insert(entity_key(entity), Transform::at_cell(town_center()));
        self.health.insert(
            entity_key(entity),
            Health {
                current: self.rules.population.person_health,
                maximum: self.rules.population.person_health,
            },
        );
        self.people.insert(
            entity_key(entity),
            Person {
                state: PersonState::IdleAtTownHall,
                target_entity: None,
                cargo_wood: 0,
                cargo_capacity: self.rules.population.carry_capacity,
            },
        );
        entity
    }

    pub(super) fn despawn_raider(&mut self, entity: EntityId) {
        let mut dependent_projectiles = self
            .projectiles
            .iter()
            .filter_map(|(key, projectile)| {
                (projectile.target == entity).then_some(key_entity(key))
            })
            .collect::<Vec<_>>();
        dependent_projectiles.sort_unstable();
        for projectile in dependent_projectiles {
            self.despawn_projectile(projectile);
        }

        let key = entity_key(entity);
        self.transforms.remove(key);
        self.health.remove(key);
        self.attacks.remove(key);
        self.raiders.remove(key);
        self.movements.remove(key);
        self.alive.remove(key);
    }

    pub(super) fn despawn_projectile(&mut self, entity: EntityId) {
        let key = entity_key(entity);
        self.transforms.remove(key);
        self.projectiles.remove(key);
        self.alive.remove(key);
    }

    pub(super) fn allocate_entity(&mut self) -> EntityId {
        let entity = self.next_entity;
        self.next_entity = self
            .next_entity
            .checked_add(1)
            .expect("entity id space exhausted");
        self.spawn_entity(entity);
        entity
    }

    pub(super) fn spawn_entity(&mut self, entity: EntityId) {
        let inserted = self.alive.insert(entity_key(entity));
        assert!(inserted, "entity ids must be unique");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raiders_crossing_cells(state: &mut GameState) {
        let raiders = state.raiders.keys().collect::<Vec<_>>();
        for key in raiders {
            let movement = state
                .movements
                .get_mut(key)
                .expect("spawned raider must move");
            movement.progress_milli = (CELL_SCALE as u16).saturating_sub(movement.speed_milli);
        }
    }

    #[test]
    fn raider_distance_field_matches_existing_bfs_routes() {
        let state = GameState::new(0x5eed);
        let goals = town_goal_cells();

        for z in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let cell = Cell::new(x, z);
                if state.path_cell_blocked(cell, None) {
                    continue;
                }
                assert_eq!(
                    state.raider_next_step(TOWN_ENTITY, cell),
                    state.next_path_step_to_any(cell, &goals, None),
                    "distance field must preserve deterministic BFS routing from {cell:?}"
                );
            }
        }
    }

    #[test]
    fn raiders_between_cells_do_no_navigation_work() {
        let mut state = GameState::new(17);
        state.wave = 1;
        state.spawn_raider(Edge::North);
        let before = state.work_counters();
        state.run_raider_movement_system();
        let after = state.work_counters();

        assert_eq!(after.flow_field_rebuilds, before.flow_field_rebuilds);
        assert_eq!(after.path_follow_steps, before.path_follow_steps);
    }

    #[test]
    fn raiders_crossing_cells_reuse_retained_fields_across_ticks() {
        let mut state = GameState::new(17);
        state.wave = 1;
        state.spawn_raider(Edge::North);
        state.spawn_raider(Edge::South);
        raiders_crossing_cells(&mut state);
        state.run_raider_movement_system();
        let warmed = state.work_counters();

        raiders_crossing_cells(&mut state);
        state.run_raider_movement_system();
        let after = state.work_counters();
        assert_eq!(after.flow_field_rebuilds, warmed.flow_field_rebuilds);
        assert_eq!(
            after.navigation_cells_visited,
            warmed.navigation_cells_visited
        );
        assert_eq!(after.path_follow_steps - warmed.path_follow_steps, 2);
    }

    #[test]
    fn field_retargeting_matches_existing_bfs_storage_choice() {
        let mut state = GameState::new(17);
        let storage_cell = (1..GRID_HEIGHT - 1)
            .flat_map(|z| (1..GRID_WIDTH - 1).map(move |x| Cell::new(x, z)))
            .find(|cell| {
                state
                    .validate_command(Command::PlaceStorageHouse {
                        x: cell.x,
                        z: cell.z,
                    })
                    .is_ok()
            })
            .expect("seeded map must expose an accepted storage-house cell");
        state
            .apply(Command::PlaceStorageHouse {
                x: storage_cell.x,
                z: storage_cell.z,
            })
            .expect("storage house should build");
        let storage_house = state
            .settlement_storage_ids()
            .into_iter()
            .find(|entity| *entity != TOWN_ENTITY)
            .expect("storage house must join settlement storage");
        let transferred = state.take_wood_at(TOWN_ENTITY, 20);
        assert_eq!(transferred, 20);
        assert_eq!(state.store_wood_at(storage_house, transferred), transferred);

        for z in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let cell = Cell::new(x, z);
                if state.path_cell_blocked(cell, None) {
                    continue;
                }
                assert_eq!(
                    state.nearest_raider_storage_from_fields(cell),
                    state.nearest_raider_storage(cell),
                    "field retargeting must preserve BFS storage choice from {cell:?}"
                );
            }
        }
    }

    #[test]
    fn spatial_targeting_matches_a_full_raider_scan() {
        let mut state = GameState::new(23);
        state.wave = 1;
        for edge in Edge::ALL {
            for _ in 0..3 {
                state.spawn_raider(edge);
            }
        }
        // Spread raiders over many fixed-point positions.
        let raiders = state.raiders.keys().collect::<Vec<_>>();
        for (step, key) in raiders.into_iter().enumerate() {
            let offset = i32::try_from(step).unwrap() * 1_317;
            state.transforms.insert(
                key,
                Transform {
                    x_milli: (offset * 7) % (i32::from(GRID_WIDTH) * CELL_SCALE),
                    z_milli: (offset * 3) % (i32::from(GRID_HEIGHT) * CELL_SCALE),
                },
            );
        }
        let index = state.raider_cell_index();
        let tower = state.allocate_entity();
        for z in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                state
                    .transforms
                    .insert(entity_key(tower), Transform::at_cell(Cell::new(x, z)));
                for range in [0, 999, 1_000, 2_000, 2_500, 30_000] {
                    let tower_position = state.transforms.get(entity_key(tower)).copied().unwrap();
                    let expected = state
                        .raiders
                        .keys()
                        .filter_map(|key| {
                            let position = state.transforms.get(key)?;
                            let dx = i64::from(position.x_milli - tower_position.x_milli);
                            let dz = i64::from(position.z_milli - tower_position.z_milli);
                            let distance_sq = dx * dx + dz * dz;
                            (distance_sq <= i64::from(range) * i64::from(range))
                                .then_some((distance_sq, key_entity(key)))
                        })
                        .min()
                        .map(|(_, entity)| entity);
                    assert_eq!(
                        state.target_for_tower_in(tower, range, &index),
                        expected,
                        "tower at {x},{z} range {range}"
                    );
                }
            }
        }
    }
}
