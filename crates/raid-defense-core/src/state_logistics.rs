use super::*;

impl GameState {
    pub(super) fn run_person_logistics_system(&mut self) -> (u16, u16, u16, u16) {
        let mut gathered_total = 0_u16;
        let mut picked_up_total = 0_u16;
        let mut delivered_total = 0_u16;
        let mut towers_completed = 0_u16;

        if !self.is_rallying() {
            self.assign_idle_people();
        }

        let mut person_ids = self.people.keys().map(key_entity).collect::<Vec<_>>();
        person_ids.sort_unstable();
        for entity in person_ids {
            let Some(mut person) = self.people.get(entity_key(entity)).copied() else {
                continue;
            };
            if person.state == PersonState::IdleAtTownHall {
                continue;
            }

            if person.state == PersonState::HarvestingForest {
                let remaining = self
                    .work_ticks
                    .get(entity_key(entity))
                    .copied()
                    .unwrap_or(0);
                if remaining > 1 {
                    self.work_ticks.insert(entity_key(entity), remaining - 1);
                    continue;
                }
                self.work_ticks.remove(entity_key(entity));

                let Some(forest) = person.target_entity else {
                    let start = self.person_cell(entity);
                    self.send_person_home(entity, &mut person, start);
                    self.people.insert(entity_key(entity), person);
                    continue;
                };
                let amount =
                    u32::from(person.cargo_capacity.min(self.rules.economy.sawmill_output));
                let gathered = self.take_wood_at(forest, amount);
                let gathered_u16 = u16::try_from(gathered).unwrap_or(u16::MAX);
                if gathered_u16 == 0 {
                    let start = self.person_cell(entity);
                    self.send_person_home(entity, &mut person, start);
                    self.people.insert(entity_key(entity), person);
                    continue;
                }

                person.cargo_wood = gathered_u16;
                gathered_total = gathered_total.saturating_add(gathered_u16);
                let start = self.person_cell(entity);
                let sawmill = self
                    .nearest_sawmill_with_capacity(start)
                    .or_else(|| self.nearest_sawmill(start));
                if let Some(sawmill) = sawmill {
                    person.state = PersonState::ToSawmill;
                    person.target_entity = Some(sawmill);
                    self.route_person_to_goals(
                        entity,
                        start,
                        &self.sawmill_pickup_cells(sawmill, None),
                    );
                } else {
                    // A sawmill was required before the forestry job was assigned. If it
                    // disappears from consideration, keep the worker and cargo fail-closed
                    // rather than teleporting raw wood into settlement storage.
                    self.movements.remove(entity_key(entity));
                }
                self.people.insert(entity_key(entity), person);
                continue;
            }

            let Some(mut movement) = self.movements.get(entity_key(entity)).copied() else {
                self.rebuild_person_movement(entity, person);
                continue;
            };

            movement.progress_milli = movement.progress_milli.saturating_add(movement.speed_milli);
            if movement.progress_milli < CELL_SCALE as u16 {
                self.movements.insert(entity_key(entity), movement);
                self.transforms
                    .insert(entity_key(entity), interpolate_transform(movement));
                continue;
            }

            movement.progress_milli -= CELL_SCALE as u16;
            movement.from = movement.to;
            self.transforms
                .insert(entity_key(entity), Transform::at_cell(movement.from));

            match person.state {
                PersonState::IdleAtTownHall => {}
                PersonState::ToForest => {
                    let Some(target) = person.target_entity else {
                        self.send_person_home(entity, &mut person, movement.from);
                        self.people.insert(entity_key(entity), person);
                        continue;
                    };
                    let goals = self.forest_goal_cells(target, None);
                    if goals.contains(&movement.from) {
                        person.state = PersonState::HarvestingForest;
                        self.movements.remove(entity_key(entity));
                        self.work_ticks.insert(
                            entity_key(entity),
                            self.rules.economy.sawmill_interval_ticks,
                        );
                    } else {
                        self.route_person_to_goals(entity, movement.from, &goals);
                    }
                }
                PersonState::HarvestingForest => {
                    unreachable!("harvesting is handled before movement")
                }
                PersonState::ToSawmill => {
                    let Some(target) = person.target_entity else {
                        self.movements.remove(entity_key(entity));
                        self.people.insert(entity_key(entity), person);
                        continue;
                    };
                    let goals = self.sawmill_pickup_cells(target, None);
                    if goals.contains(&movement.from) {
                        let delivered = self.store_wood_at(target, u32::from(person.cargo_wood));
                        let delivered_u16 = u16::try_from(delivered).unwrap_or(u16::MAX);
                        person.cargo_wood = person.cargo_wood.saturating_sub(delivered_u16);
                        delivered_total = delivered_total.saturating_add(delivered_u16);
                        if person.cargo_wood == 0 {
                            self.send_person_home(entity, &mut person, movement.from);
                        } else if let Some(next_sawmill) =
                            self.nearest_sawmill_with_capacity(movement.from)
                        {
                            person.target_entity = Some(next_sawmill);
                            self.route_person_to_goals(
                                entity,
                                movement.from,
                                &self.sawmill_pickup_cells(next_sawmill, None),
                            );
                        } else {
                            // Keep raw cargo at the sawmill until local capacity opens.
                            self.movements.remove(entity_key(entity));
                        }
                    } else {
                        self.route_person_to_goals(entity, movement.from, &goals);
                    }
                }
                PersonState::ToSawmillPickup => {
                    let Some(target) = person.target_entity else {
                        self.send_person_home(entity, &mut person, movement.from);
                        self.people.insert(entity_key(entity), person);
                        continue;
                    };
                    let goals = self.sawmill_pickup_cells(target, None);
                    if goals.contains(&movement.from) {
                        let pickup = self.take_sawmill_wood(target, person.cargo_capacity);
                        person.cargo_wood = pickup;
                        picked_up_total = picked_up_total.saturating_add(pickup);
                        if pickup == 0 {
                            self.send_person_home(entity, &mut person, movement.from);
                        } else if let Some(storage) =
                            self.nearest_storage_with_capacity(movement.from, None)
                        {
                            person.state = PersonState::ToStorage;
                            person.target_entity = Some(storage);
                            self.route_person_to_goals(
                                entity,
                                movement.from,
                                &self.storage_goal_cells(storage, None),
                            );
                        } else {
                            self.send_person_home(entity, &mut person, movement.from);
                        }
                    } else {
                        self.route_person_to_goals(entity, movement.from, &goals);
                    }
                }
                PersonState::ToStorage => {
                    let Some(target) = person.target_entity else {
                        self.send_person_home(entity, &mut person, movement.from);
                        self.people.insert(entity_key(entity), person);
                        continue;
                    };
                    let goals = self.storage_goal_cells(target, None);
                    if goals.contains(&movement.from) {
                        let delivered = self.store_wood_at(target, u32::from(person.cargo_wood));
                        let delivered_u16 = u16::try_from(delivered).unwrap_or(u16::MAX);
                        person.cargo_wood = person.cargo_wood.saturating_sub(delivered_u16);
                        delivered_total = delivered_total.saturating_add(delivered_u16);
                        if person.cargo_wood == 0 {
                            self.send_person_home(entity, &mut person, movement.from);
                        } else if let Some(next_storage) =
                            self.nearest_storage_with_capacity(movement.from, None)
                        {
                            person.target_entity = Some(next_storage);
                            self.route_person_to_goals(
                                entity,
                                movement.from,
                                &self.storage_goal_cells(next_storage, None),
                            );
                        } else {
                            self.send_person_home(entity, &mut person, movement.from);
                        }
                    } else {
                        self.route_person_to_goals(entity, movement.from, &goals);
                    }
                }
                PersonState::ToConstructionStorage => {
                    let Some(site) = person.target_entity else {
                        self.send_person_home(entity, &mut person, movement.from);
                        self.people.insert(entity_key(entity), person);
                        continue;
                    };
                    let Some(source) = self.nearest_storage_with_wood_for_site(site, movement.from)
                    else {
                        self.send_person_home(entity, &mut person, movement.from);
                        self.people.insert(entity_key(entity), person);
                        continue;
                    };
                    let source_goals = self.storage_goal_cells(source, None);
                    if source_goals.contains(&movement.from) {
                        let remaining = self.construction_remaining(site);
                        let pickup_limit = u32::from(person.cargo_capacity).min(remaining);
                        let pickup = self.take_wood_at(source, pickup_limit);
                        let pickup_u16 = u16::try_from(pickup).unwrap_or(u16::MAX);
                        person.cargo_wood = pickup_u16;
                        picked_up_total = picked_up_total.saturating_add(pickup_u16);
                        if pickup_u16 == 0 {
                            self.send_person_home(entity, &mut person, movement.from);
                        } else {
                            person.state = PersonState::ToConstructionSite;
                            person.target_entity = Some(site);
                            let goals = self.construction_goal_cells(site, None);
                            self.route_person_to_goals(entity, movement.from, &goals);
                        }
                    } else {
                        self.route_person_to_goals(entity, movement.from, &source_goals);
                    }
                }
                PersonState::ToConstructionSite => {
                    let Some(site) = person.target_entity else {
                        self.send_person_home(entity, &mut person, movement.from);
                        self.people.insert(entity_key(entity), person);
                        continue;
                    };
                    let goals = self.construction_goal_cells(site, None);
                    if goals.contains(&movement.from) {
                        let delivered = self.deliver_construction_wood(site, person.cargo_wood);
                        person.cargo_wood = person.cargo_wood.saturating_sub(delivered);
                        if self.complete_construction_if_ready(site) {
                            towers_completed = towers_completed.saturating_add(1);
                        }
                        self.send_person_home(entity, &mut person, movement.from);
                    } else {
                        self.route_person_to_goals(entity, movement.from, &goals);
                    }
                }
                PersonState::ToTownHall => {
                    if is_town_cell(movement.from) {
                        if person.cargo_wood > 0 {
                            let delivered =
                                self.store_wood_at(TOWN_ENTITY, u32::from(person.cargo_wood));
                            let delivered_u16 = u16::try_from(delivered).unwrap_or(u16::MAX);
                            person.cargo_wood = person.cargo_wood.saturating_sub(delivered_u16);
                            delivered_total = delivered_total.saturating_add(delivered_u16);
                        }
                        if person.cargo_wood == 0 {
                            person.state = PersonState::IdleAtTownHall;
                            person.target_entity = None;
                            self.movements.remove(entity_key(entity));
                        } else if let Some(storage) =
                            self.nearest_storage_with_capacity(movement.from, None)
                        {
                            person.state = PersonState::ToStorage;
                            person.target_entity = Some(storage);
                            self.route_person_to_goals(
                                entity,
                                movement.from,
                                &self.storage_goal_cells(storage, None),
                            );
                        } else {
                            self.movements.remove(entity_key(entity));
                        }
                    } else {
                        self.route_person_to_goals(entity, movement.from, &town_goal_cells());
                    }
                }
            }
            self.people.insert(entity_key(entity), person);
        }

        (
            gathered_total,
            picked_up_total,
            delivered_total,
            towers_completed,
        )
    }

    pub(super) fn recall_people_for_raid(&mut self) {
        let mut ids = self.people.keys().map(key_entity).collect::<Vec<_>>();
        ids.sort_unstable();
        for entity in ids {
            let Some(mut person) = self.people.get(entity_key(entity)).copied() else {
                continue;
            };
            let start = self.person_cell(entity);
            let should_finish_delivery = matches!(
                person.state,
                PersonState::ToSawmill | PersonState::ToStorage | PersonState::ToConstructionSite
            ) && person.cargo_wood > 0;
            if matches!(
                person.state,
                PersonState::IdleAtTownHall | PersonState::ToTownHall
            ) || should_finish_delivery
            {
                continue;
            }
            self.send_person_home(entity, &mut person, start);
            self.people.insert(entity_key(entity), person);
        }
    }

    pub(super) fn assign_idle_people(&mut self) {
        if self.is_rallying() {
            return;
        }

        let mut reserved_construction = BTreeMap::<EntityId, u32>::new();
        let mut reserved_sawmills = BTreeMap::<EntityId, u32>::new();
        let mut reserved_forests = BTreeMap::<EntityId, u32>::new();
        for (_, person) in self.people.iter() {
            match person.state {
                PersonState::ToConstructionStorage | PersonState::ToConstructionSite => {
                    if let Some(target) = person.target_entity {
                        let entry = reserved_construction.entry(target).or_default();
                        *entry = entry.saturating_add(if person.cargo_wood > 0 {
                            u32::from(person.cargo_wood)
                        } else {
                            u32::from(person.cargo_capacity)
                        });
                    }
                }
                PersonState::ToSawmillPickup => {
                    if let Some(target) = person.target_entity {
                        let entry = reserved_sawmills.entry(target).or_default();
                        *entry = entry.saturating_add(u32::from(person.cargo_capacity));
                    }
                }
                PersonState::ToForest | PersonState::HarvestingForest => {
                    if let Some(target) = person.target_entity {
                        let harvest_amount =
                            u32::from(person.cargo_capacity.min(self.rules.economy.sawmill_output));
                        let entry = reserved_forests.entry(target).or_default();
                        *entry = entry.saturating_add(harvest_amount);
                    }
                }
                _ => {}
            }
        }

        let mut ids = self.people.keys().map(key_entity).collect::<Vec<_>>();
        ids.sort_unstable();
        for entity in ids {
            let Some(mut person) = self.people.get(entity_key(entity)).copied() else {
                continue;
            };
            if person.state != PersonState::IdleAtTownHall || person.cargo_wood != 0 {
                continue;
            }
            let start = self.person_cell(entity);

            if let Some(site) = self.best_construction_for_person(&reserved_construction)
                && self
                    .nearest_storage_with_wood_for_site(site, start)
                    .is_some()
            {
                person.state = PersonState::ToConstructionStorage;
                person.target_entity = Some(site);
                self.people.insert(entity_key(entity), person);
                self.rebuild_person_movement(entity, person);
                let entry = reserved_construction.entry(site).or_default();
                *entry = entry.saturating_add(u32::from(person.cargo_capacity));
                continue;
            }

            if let Some(target) = self.best_sawmill_for_person(&reserved_sawmills) {
                let goals = self.sawmill_pickup_cells(target, None);
                if let Some(next) = self.next_path_step_to_any(start, &goals, None) {
                    person.state = PersonState::ToSawmillPickup;
                    person.target_entity = Some(target);
                    self.people.insert(entity_key(entity), person);
                    self.movements.insert(
                        entity_key(entity),
                        Movement {
                            from: start,
                            to: next,
                            progress_milli: 0,
                            speed_milli: self.rules.population.speed_milli,
                        },
                    );
                    let entry = reserved_sawmills.entry(target).or_default();
                    *entry = entry.saturating_add(u32::from(person.cargo_capacity));
                    continue;
                }
            }

            let harvest_amount =
                u32::from(person.cargo_capacity.min(self.rules.economy.sawmill_output));
            let Some(forest) =
                self.best_forest_for_person(start, harvest_amount, &reserved_forests)
            else {
                continue;
            };
            let goals = self.forest_goal_cells(forest, None);
            let Some(next) = self.next_path_step_to_any(start, &goals, None) else {
                continue;
            };
            person.state = PersonState::ToForest;
            person.target_entity = Some(forest);
            self.people.insert(entity_key(entity), person);
            self.movements.insert(
                entity_key(entity),
                Movement {
                    from: start,
                    to: next,
                    progress_milli: 0,
                    speed_milli: self.rules.population.speed_milli,
                },
            );
            let entry = reserved_forests.entry(forest).or_default();
            *entry = entry.saturating_add(harvest_amount);
        }
    }

    pub(super) fn best_construction_for_person(
        &self,
        reserved: &BTreeMap<EntityId, u32>,
    ) -> Option<EntityId> {
        self.construction_sites().into_iter().find(|site| {
            self.count_work(|work| work.logistics_candidate_searches += 1);
            let remaining = self.construction_remaining(*site);
            let reserved = reserved.get(site).copied().unwrap_or(0);
            remaining.saturating_sub(reserved) > 0
        })
    }

    pub(super) fn best_sawmill_for_person(
        &self,
        reserved: &BTreeMap<EntityId, u32>,
    ) -> Option<EntityId> {
        let mut candidates = self
            .producers
            .keys()
            .map(key_entity)
            .filter_map(|entity| {
                self.count_work(|work| work.logistics_candidate_searches += 1);
                let stored = self.storage.get(entity_key(entity))?.wood;
                let reserved = reserved.get(&entity).copied().unwrap_or(0);
                let available = stored.saturating_sub(reserved);
                (available > 0).then_some((available, entity))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(available, entity)| (std::cmp::Reverse(*available), *entity));
        candidates.first().map(|(_, entity)| *entity)
    }

    fn best_forest_for_person(
        &self,
        start: Cell,
        harvest_amount: u32,
        reserved: &BTreeMap<EntityId, u32>,
    ) -> Option<EntityId> {
        self.nearest_sawmill_with_capacity(start)?;
        let mut candidates = self
            .buildings
            .iter()
            .filter_map(|(key, building)| {
                if building.kind != BuildingKind::Forest {
                    return None;
                }
                self.count_work(|work| work.logistics_candidate_searches += 1);
                let storage = self.storage.get(key)?;
                let entity = key_entity(key);
                let reserved_wood = reserved.get(&entity).copied().unwrap_or(0);
                let available = storage.wood.saturating_sub(reserved_wood);
                if available == 0 {
                    return None;
                }
                let goals = self.forest_goal_cells(entity, None);
                let distance = self.path_distance_to_any(start, &goals, None)?;
                let reserved_batches = reserved_wood.checked_div(harvest_amount).unwrap_or(0);
                Some((reserved_batches, reserved_wood, distance, entity))
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.first().map(|(_, _, _, entity)| *entity)
    }

    fn nearest_sawmill_with_capacity(&self, start: Cell) -> Option<EntityId> {
        self.nearest_sawmill_matching(start, true)
    }

    fn nearest_sawmill(&self, start: Cell) -> Option<EntityId> {
        self.nearest_sawmill_matching(start, false)
    }

    fn nearest_sawmill_matching(&self, start: Cell, require_capacity: bool) -> Option<EntityId> {
        let mut candidates = self
            .producers
            .keys()
            .map(key_entity)
            .filter_map(|entity| {
                self.count_work(|work| work.logistics_candidate_searches += 1);
                let storage = self.storage.get(entity_key(entity))?;
                if require_capacity && storage.wood >= storage.wood_capacity {
                    return None;
                }
                let goals = self.sawmill_pickup_cells(entity, None);
                let distance = self.path_distance_to_any(start, &goals, None)?;
                Some((distance, entity))
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.first().map(|(_, entity)| *entity)
    }

    fn forest_goal_cells(&self, entity: EntityId, extra_block: Option<Cell>) -> Vec<Cell> {
        let Some(building) = self.buildings.get(entity_key(entity)) else {
            return Vec::new();
        };
        if building.kind != BuildingKind::Forest {
            return Vec::new();
        }
        self.adjacent_walkable_cells(building.cell, extra_block)
    }

    fn person_cell(&self, entity: EntityId) -> Cell {
        self.movements.get(entity_key(entity)).map_or_else(
            || {
                self.transforms
                    .get(entity_key(entity))
                    .copied()
                    .map_or(town_center(), cell_for_transform)
            },
            |movement| movement.from,
        )
    }

    pub(super) fn take_sawmill_wood(&mut self, sawmill: EntityId, capacity: u16) -> u16 {
        let Some(storage) = self.storage.get_mut(entity_key(sawmill)) else {
            return 0;
        };
        let amount = storage.wood.min(u32::from(capacity));
        storage.wood -= amount;
        u16::try_from(amount).unwrap_or(u16::MAX)
    }

    pub(super) fn send_person_home(&mut self, entity: EntityId, person: &mut Person, start: Cell) {
        person.state = PersonState::ToTownHall;
        person.target_entity = None;
        self.work_ticks.remove(entity_key(entity));
        self.route_person_to_goals(entity, start, &town_goal_cells());
    }

    pub(super) fn route_person_to_goals(&mut self, entity: EntityId, start: Cell, goals: &[Cell]) {
        if let Some(next) = self.next_path_step_to_any(start, goals, None) {
            self.movements.insert(
                entity_key(entity),
                Movement {
                    from: start,
                    to: next,
                    progress_milli: 0,
                    speed_milli: self.rules.population.speed_milli,
                },
            );
        } else {
            self.movements.remove(entity_key(entity));
        }
    }

    pub(super) fn rebuild_person_movement(&mut self, entity: EntityId, person: Person) {
        let start = self.person_cell(entity);
        let goals = match person.state {
            PersonState::IdleAtTownHall | PersonState::HarvestingForest => return,
            PersonState::ToTownHall => town_goal_cells(),
            PersonState::ToForest => person
                .target_entity
                .map_or_else(Vec::new, |target| self.forest_goal_cells(target, None)),
            PersonState::ToSawmill | PersonState::ToSawmillPickup => person
                .target_entity
                .map_or_else(Vec::new, |target| self.sawmill_pickup_cells(target, None)),
            PersonState::ToStorage => person
                .target_entity
                .map_or_else(Vec::new, |target| self.storage_goal_cells(target, None)),
            PersonState::ToConstructionStorage => {
                person.target_entity.map_or_else(Vec::new, |site| {
                    self.nearest_storage_with_wood_for_site(site, start)
                        .map_or_else(Vec::new, |storage| self.storage_goal_cells(storage, None))
                })
            }
            PersonState::ToConstructionSite => person
                .target_entity
                .map_or_else(Vec::new, |site| self.construction_goal_cells(site, None)),
        };
        self.route_person_to_goals(entity, start, &goals);
    }

    pub(super) fn construction_goal_cells(
        &self,
        site: EntityId,
        extra_block: Option<Cell>,
    ) -> Vec<Cell> {
        let Some(building) = self.buildings.get(entity_key(site)) else {
            return Vec::new();
        };
        self.adjacent_walkable_cells(building.cell, extra_block)
    }
}
