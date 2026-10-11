use super::*;

impl GameState {
    pub(super) fn entity_snapshot(&self, entity: EntityId) -> Option<EntitySnapshot> {
        let key = entity_key(entity);
        let transform = *self.transforms.get(key)?;
        let health = self.health.get(key).copied().unwrap_or(Health {
            current: 0,
            maximum: 0,
        });
        let attack = self.attacks.get(key).copied().unwrap_or(Attack {
            damage: 0,
            range_milli: 0,
            cooldown_ticks: 0,
            cooldown_remaining: 0,
            projectile_speed_milli: 0,
        });
        let storage = self.storage.get(key).copied().unwrap_or(ResourceStorage {
            wood: 0,
            wood_capacity: 0,
        });
        let producer = self.producers.get(key).copied();
        let housing = self.housing.get(key).copied();

        if let Some(building) = self.buildings.get(key) {
            let tower = self.towers.get(key).copied();
            return Some(EntitySnapshot {
                id: entity,
                kind: match building.kind {
                    BuildingKind::TownHall => EntityKind::TownHall,
                    BuildingKind::Tower => EntityKind::Tower,
                    BuildingKind::Sawmill => EntityKind::Sawmill,
                    BuildingKind::StorageHouse => EntityKind::StorageHouse,
                    BuildingKind::House => EntityKind::House,
                    BuildingKind::Forest => EntityKind::Forest,
                },
                x_milli: transform.x_milli,
                z_milli: transform.z_milli,
                cell: building.cell,
                health: health.current,
                max_health: health.maximum,
                attack_damage: attack.damage,
                attack_range_milli: attack.range_milli,
                tower_archetype: tower.map(|tower| tower.archetype),
                tower_level: tower.map_or(0, |tower| tower.level),
                upgrade_cost: tower
                    .and_then(|tower| self.rules.tower_upgrade_cost(tower.archetype, tower.level)),
                raider_archetype: None,
                projectile_target: None,
                stored_wood: storage.wood,
                wood_capacity: storage.wood_capacity,
                production_resource: producer.map(|producer| producer.resource),
                production_amount: producer.map_or(0, |producer| producer.amount),
                production_interval_ticks: producer.map_or(0, |producer| producer.interval_ticks),
                production_progress_ticks: producer.map_or(0, |producer| producer.progress_ticks),
                housing_capacity: housing.map_or(0, |housing| housing.capacity),
                person_state: None,
                person_target_entity: None,
                cargo_wood: 0,
                cargo_capacity: 0,
            });
        }

        if let Some(person) = self.people.get(key).copied() {
            let cell = self
                .movements
                .get(key)
                .map_or_else(|| cell_for_transform(transform), |movement| movement.from);
            return Some(EntitySnapshot {
                id: entity,
                kind: EntityKind::Person,
                x_milli: transform.x_milli,
                z_milli: transform.z_milli,
                cell,
                health: health.current,
                max_health: health.maximum,
                attack_damage: 0,
                attack_range_milli: 0,
                tower_archetype: None,
                tower_level: 0,
                upgrade_cost: None,
                raider_archetype: None,
                projectile_target: None,
                stored_wood: 0,
                wood_capacity: 0,
                production_resource: None,
                production_amount: 0,
                production_interval_ticks: 0,
                production_progress_ticks: 0,
                housing_capacity: 0,
                person_state: Some(person.state),
                person_target_entity: person.target_entity,
                cargo_wood: person.cargo_wood,
                cargo_capacity: person.cargo_capacity,
            });
        }

        if let Some(raider) = self.raiders.get(key).copied() {
            let movement = self.movements.get(key)?;
            return Some(EntitySnapshot {
                id: entity,
                kind: EntityKind::Raider,
                x_milli: transform.x_milli,
                z_milli: transform.z_milli,
                cell: movement.from,
                health: health.current,
                max_health: health.maximum,
                attack_damage: attack.damage,
                attack_range_milli: attack.range_milli,
                tower_archetype: None,
                tower_level: 0,
                upgrade_cost: None,
                raider_archetype: Some(raider.archetype),
                projectile_target: None,
                stored_wood: 0,
                wood_capacity: 0,
                production_resource: None,
                production_amount: 0,
                production_interval_ticks: 0,
                production_progress_ticks: 0,
                housing_capacity: 0,
                person_state: None,
                person_target_entity: None,
                cargo_wood: 0,
                cargo_capacity: 0,
            });
        }

        let projectile = self.projectiles.get(key)?;
        Some(EntitySnapshot {
            id: entity,
            kind: EntityKind::Projectile,
            x_milli: transform.x_milli,
            z_milli: transform.z_milli,
            cell: cell_for_transform(transform),
            health: 0,
            max_health: 0,
            attack_damage: projectile.damage,
            attack_range_milli: 0,
            tower_archetype: Some(projectile.archetype),
            tower_level: 0,
            upgrade_cost: None,
            raider_archetype: None,
            projectile_target: Some(projectile.target),
            stored_wood: 0,
            wood_capacity: 0,
            production_resource: None,
            production_amount: 0,
            production_interval_ticks: 0,
            production_progress_ticks: 0,
            housing_capacity: 0,
            person_state: None,
            person_target_entity: None,
            cargo_wood: 0,
            cargo_capacity: 0,
        })
    }

    pub(super) fn tower_entity_at(&self, cell: Cell) -> Option<EntityId> {
        self.buildings.iter().find_map(|(key, building)| {
            if building.kind != BuildingKind::Tower || building.cell != cell {
                return None;
            }
            self.towers
                .get(key)
                .is_some_and(|tower| tower.level > 0)
                .then_some(key_entity(key))
        })
    }

    pub(super) fn cell_has_building(&self, cell: Cell) -> bool {
        self.buildings
            .iter()
            .any(|(_, building)| building.kind != BuildingKind::TownHall && building.cell == cell)
    }

    pub(super) fn unit_uses_cell(&self, cell: Cell) -> bool {
        self.movements
            .iter()
            .any(|(_, movement)| movement.from == cell || movement.to == cell)
    }

    pub(super) fn routes_remain_open(&self, extra_block: Option<Cell>) -> bool {
        let goals = town_goal_cells();
        Edge::ALL.into_iter().all(|edge| {
            self.next_path_step_to_any(edge.spawn_cell(), &goals, extra_block)
                .is_some()
        }) && self.raiders.iter().all(|(key, _)| {
            self.movements.get(key).is_some_and(|movement| {
                self.next_path_step_to_any(movement.from, &goals, extra_block)
                    .is_some()
            })
        })
    }

    pub(super) fn all_sawmills_accessible(
        &self,
        extra_block: Option<Cell>,
        hypothetical_sawmill: Option<Cell>,
    ) -> bool {
        let mut cells = self
            .buildings
            .iter()
            .filter_map(|(_, building)| {
                (building.kind == BuildingKind::Sawmill).then_some(building.cell)
            })
            .collect::<Vec<_>>();
        if let Some(cell) = hypothetical_sawmill {
            cells.push(cell);
        }
        cells.into_iter().all(|cell| {
            let goals = self.pickup_cells_for_sawmill_cell(cell, extra_block);
            !goals.is_empty()
                && self
                    .next_path_step_to_any(town_center(), &goals, extra_block)
                    .is_some()
        })
    }

    pub(super) fn worker_routes_remain_open(&self, extra_block: Option<Cell>) -> bool {
        self.people.iter().all(|(key, person)| {
            let start = self.movements.get(key).map_or_else(
                || {
                    self.transforms
                        .get(key)
                        .copied()
                        .map_or(town_center(), cell_for_transform)
                },
                |movement| movement.from,
            );
            let goals = match person.state {
                PersonState::IdleAtTownHall => return true,
                PersonState::ToTownHall => town_goal_cells(),
                PersonState::ToForest | PersonState::HarvestingForest => {
                    person.target_entity.map_or_else(Vec::new, |target| {
                        self.buildings
                            .get(entity_key(target))
                            .map_or_else(Vec::new, |building| {
                                self.adjacent_walkable_cells(building.cell, extra_block)
                            })
                    })
                }
                PersonState::ToSawmill | PersonState::ToSawmillPickup => {
                    person.target_entity.map_or_else(Vec::new, |target| {
                        self.sawmill_pickup_cells(target, extra_block)
                    })
                }
                PersonState::ToStorage => person.target_entity.map_or_else(Vec::new, |target| {
                    self.storage_goal_cells(target, extra_block)
                }),
                PersonState::ToConstructionStorage => {
                    person.target_entity.map_or_else(Vec::new, |site| {
                        self.nearest_storage_with_wood_for_site(site, start)
                            .map_or_else(Vec::new, |storage| {
                                self.storage_goal_cells(storage, extra_block)
                            })
                    })
                }
                PersonState::ToConstructionSite => {
                    person.target_entity.map_or_else(Vec::new, |site| {
                        self.construction_goal_cells(site, extra_block)
                    })
                }
            };
            !goals.is_empty()
                && self
                    .next_path_step_to_any(start, &goals, extra_block)
                    .is_some()
        })
    }

    pub(super) fn sawmill_pickup_cells(
        &self,
        entity: EntityId,
        extra_block: Option<Cell>,
    ) -> Vec<Cell> {
        let Some(building) = self.buildings.get(entity_key(entity)) else {
            return Vec::new();
        };
        self.pickup_cells_for_sawmill_cell(building.cell, extra_block)
    }

    pub(super) fn pickup_cells_for_sawmill_cell(
        &self,
        sawmill_cell: Cell,
        extra_block: Option<Cell>,
    ) -> Vec<Cell> {
        self.adjacent_walkable_cells(sawmill_cell, extra_block)
    }

    pub(super) fn path_cell_blocked(&self, cell: Cell, extra_block: Option<Cell>) -> bool {
        !is_town_cell(cell) && (extra_block == Some(cell) || self.cell_has_building(cell))
    }
}
