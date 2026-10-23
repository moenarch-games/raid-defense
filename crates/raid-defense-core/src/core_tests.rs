#[cfg(test)]
mod tests {
    use super::*;

    fn first_buildable_sawmill_cell(state: &GameState) -> Cell {
        for z in 1..GRID_HEIGHT - 1 {
            for x in 1..GRID_WIDTH - 1 {
                let cell = Cell::new(x, z);
                if state.validate_build_cell(cell).is_ok()
                    && state.validate_blocking_build(cell, Some(cell)).is_ok()
                {
                    return cell;
                }
            }
        }
        panic!("seeded world must expose a valid sawmill cell");
    }

    fn first_buildable_cell(state: &GameState) -> Cell {
        for z in 1..GRID_HEIGHT - 1 {
            for x in 1..GRID_WIDTH - 1 {
                let cell = Cell::new(x, z);
                if state.validate_build_cell(cell).is_ok()
                    && state.validate_blocking_build(cell, None).is_ok()
                {
                    return cell;
                }
            }
        }
        panic!("seeded world must expose a valid build cell");
    }

    fn building_entity_at(state: &GameState, kind: BuildingKind, cell: Cell) -> EntityId {
        state
            .buildings
            .iter()
            .find_map(|(key, building)| {
                (building.kind == kind && building.cell == cell).then_some(key_entity(key))
            })
            .expect("expected building must exist")
    }

    fn distance_sq(a: Cell, b: Cell) -> i32 {
        let dx = i32::from(a.x - b.x);
        let dz = i32::from(a.z - b.z);
        dx * dx + dz * dz
    }

    #[test]
    fn standard_map_is_twenty_one_square_with_seeded_blue_noise_forests() {
        let first = GameState::new(7);
        let second = GameState::new(7);
        let other = GameState::new(8);
        assert_eq!(GRID_WIDTH, 21);
        assert_eq!(GRID_HEIGHT, 21);
        assert_eq!(
            first.forest_count(),
            usize::from(first.rules.economy.forest_tile_count)
        );
        assert_eq!(first.snapshot(), second.snapshot());
        let first_forests = first
            .snapshot()
            .entities
            .into_iter()
            .filter(|entity| entity.kind == EntityKind::Forest)
            .map(|entity| entity.cell)
            .collect::<Vec<_>>();
        let other_forests = other
            .snapshot()
            .entities
            .into_iter()
            .filter(|entity| entity.kind == EntityKind::Forest)
            .map(|entity| entity.cell)
            .collect::<Vec<_>>();
        assert_ne!(first_forests, other_forests);

        for (index, forest) in first_forests.iter().enumerate() {
            for other_forest in &first_forests[index + 1..] {
                assert!(
                    distance_sq(*forest, *other_forest) >= 9,
                    "seeded forests must retain the minimum-distance blue-noise spacing"
                );
            }
        }
    }

    #[test]
    fn sawmill_does_not_harvest_forest_without_worker_labor() {
        let mut rules = STANDARD_RULES;
        rules.economy.sawmill_interval_ticks = 50;
        rules.economy.forest_regrowth_interval_ticks = u16::MAX;
        let mut state = GameState::with_rules(7, rules);
        let cell = first_buildable_sawmill_cell(&state);
        state
            .apply(Command::PlaceSawmill {
                x: cell.x,
                z: cell.z,
            })
            .expect("sawmill should build on any valid reachable cell");
        let forest_before: u32 = state
            .snapshot()
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Forest)
            .map(|entity| entity.stored_wood)
            .sum();

        for _ in 0..10 {
            let Event::TickAdvanced { wood_produced, .. } = state.advance_tick() else {
                unreachable!("advance_tick always yields a tick event");
            };
            assert_eq!(wood_produced, 0, "the sawmill itself must never cut trees");
        }
        let forest_before_worker_finishes: u32 = state
            .snapshot()
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Forest)
            .map(|entity| entity.stored_wood)
            .sum();
        assert_eq!(forest_before_worker_finishes, forest_before);

        let mut gathered = 0_u16;
        for _ in 0..400 {
            let Event::TickAdvanced { wood_produced, .. } = state.advance_tick() else {
                unreachable!("advance_tick always yields a tick event");
            };
            gathered = gathered.saturating_add(wood_produced);
            if gathered > 0 {
                break;
            }
        }
        assert!(gathered > 0, "a person should eventually finish gathering a tree batch");
        let forest_after: u32 = state
            .snapshot()
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Forest)
            .map(|entity| entity.stored_wood)
            .sum();
        assert!(forest_after < forest_before);
    }

    #[test]
    fn sawmill_has_no_forest_range_gate() {
        let mut rules = STANDARD_RULES;
        rules.economy.forest_tile_count = 1;
        rules.economy.sawmill_interval_ticks = 1;
        rules.economy.forest_regrowth_interval_ticks = u16::MAX;
        let mut state = GameState::with_rules(7, rules);
        let forest = state
            .snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == EntityKind::Forest)
            .expect("single seeded forest must exist");

        let mut distant_cell = None;
        for z in 1..GRID_HEIGHT - 1 {
            for x in 1..GRID_WIDTH - 1 {
                let cell = Cell::new(x, z);
                if distance_sq(cell, forest.cell) <= 25 {
                    continue;
                }
                if state.validate_build_cell(cell).is_ok()
                    && state.validate_blocking_build(cell, Some(cell)).is_ok()
                {
                    distant_cell = Some(cell);
                    break;
                }
            }
            if distant_cell.is_some() {
                break;
            }
        }
        let cell = distant_cell.expect("map must expose a cell beyond the former radius");
        state
            .apply(Command::PlaceSawmill {
                x: cell.x,
                z: cell.z,
            })
            .expect("forest distance must not reject a valid sawmill");
        assert_eq!(state.sawmill_count(), 1);

        let mut gathered = 0_u16;
        for _ in 0..500 {
            let Event::TickAdvanced { wood_produced, .. } = state.advance_tick() else {
                unreachable!("advance_tick always yields a tick event");
            };
            gathered = gathered.saturating_add(wood_produced);
            if gathered > 0 {
                break;
            }
        }
        assert!(gathered > 0, "workers should be able to service a distant sawmill");
        let forest_after = state
            .snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.id == forest.id)
            .expect("forest must persist after harvesting")
            .stored_wood;
        assert_eq!(forest.stored_wood - forest_after, u32::from(gathered));
    }

    #[test]
    fn depleted_forest_tile_persists_then_regenerates_to_full_capacity() {
        let mut rules = STANDARD_RULES;
        rules.economy.forest_tile_count = 1;
        rules.economy.forest_tile_wood = 5;
        rules.economy.forest_regrowth_amount = 2;
        rules.economy.forest_regrowth_interval_ticks = 3;
        let mut state = GameState::with_rules(19, rules);
        let forest = state
            .snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.kind == EntityKind::Forest)
            .expect("single seeded forest must exist");

        assert_eq!(state.take_wood_at(forest.id, u32::MAX), 5);
        assert_eq!(state.forest_count(), 1, "depletion must not despawn the tile");
        assert_eq!(
            state
                .snapshot()
                .entities
                .iter()
                .find(|entity| entity.id == forest.id)
                .expect("depleted forest remains in the snapshot")
                .stored_wood,
            0
        );
        assert_eq!(state.forest_regrowth_ready_tick.get(&forest.id), Some(&9));

        for _ in 0..8 {
            state.advance_tick();
        }
        assert_eq!(
            state
                .snapshot()
                .entities
                .iter()
                .find(|entity| entity.id == forest.id)
                .expect("dormant forest remains in the snapshot")
                .stored_wood,
            0
        );

        state.advance_tick();
        let regrown = state
            .snapshot()
            .entities
            .into_iter()
            .find(|entity| entity.id == forest.id)
            .expect("regenerated forest remains stable")
            .stored_wood;
        assert_eq!(regrown, 5);
        assert!(!state.forest_regrowth_ready_tick.contains_key(&forest.id));
        assert_eq!(state.forest_count(), 1);
    }

    #[test]
    fn raider_targets_the_nearest_stocked_storage() {
        let mut state = GameState::new(0x5eed);
        let storage_cell = Cell::new(2, GRID_HEIGHT / 2);
        state
            .apply(Command::PlaceStorageHouse {
                x: storage_cell.x,
                z: storage_cell.z,
            })
            .expect("west-side storage should be buildable");
        let storage = building_entity_at(&state, BuildingKind::StorageHouse, storage_cell);
        let moved = state.take_wood_at(TOWN_ENTITY, 30);
        assert_eq!(moved, 30);
        assert_eq!(state.store_wood_at(storage, moved), 30);

        state.spawn_raider(Edge::West);
        let raider = state
            .raiders
            .keys()
            .map(key_entity)
            .max()
            .expect("raider should spawn");
        assert_eq!(
            state
                .raiders
                .get(entity_key(raider))
                .expect("raider component should exist")
                .target_storage,
            storage
        );
    }

    #[test]
    fn construction_uses_the_nearest_stocked_storage_to_the_site() {
        let mut state = GameState::new(0x5eed);
        let storage_cell = Cell::new(2, GRID_HEIGHT / 2);
        state
            .apply(Command::PlaceStorageHouse {
                x: storage_cell.x,
                z: storage_cell.z,
            })
            .expect("storage should be buildable");
        let storage = building_entity_at(&state, BuildingKind::StorageHouse, storage_cell);
        let moved = state.take_wood_at(TOWN_ENTITY, 25);
        assert_eq!(state.store_wood_at(storage, moved), 25);

        let site_cell = Cell::new(3, GRID_HEIGHT / 2);
        state
            .apply(Command::PlaceTower {
                x: site_cell.x,
                z: site_cell.z,
                archetype: TowerArchetype::Arrow,
            })
            .expect("adjacent tower construction should start");
        let site = building_entity_at(&state, BuildingKind::Tower, site_cell);

        assert_eq!(
            state.nearest_storage_with_wood_for_site(site, town_center()),
            Some(storage)
        );
    }

    #[test]
    fn retained_construction_source_ranking_matches_a_full_scan() {
        let mut state = GameState::new(0x5eed);
        let mut storages = Vec::new();
        for cell in [
            Cell::new(2, GRID_HEIGHT / 2),
            Cell::new(GRID_WIDTH - 3, GRID_HEIGHT / 2),
            Cell::new(GRID_WIDTH / 2, 2),
        ] {
            if state
                .apply(Command::PlaceStorageHouse {
                    x: cell.x,
                    z: cell.z,
                })
                .is_ok()
            {
                storages.push(building_entity_at(&state, BuildingKind::StorageHouse, cell));
            }
        }
        assert!(storages.len() >= 2, "fixture needs storage houses");
        let mut sites = Vec::new();
        state.store_wood_at(TOWN_ENTITY, u32::MAX);
        for anchor in [
            Cell::new(3, 3),
            Cell::new(GRID_WIDTH - 4, GRID_HEIGHT - 4),
            Cell::new(GRID_WIDTH / 2 + 2, 3),
        ] {
            let mut cells = (1..GRID_HEIGHT - 1)
                .flat_map(|z| (1..GRID_WIDTH - 1).map(move |x| Cell::new(x, z)))
                .collect::<Vec<_>>();
            cells.sort_by_key(|cell| {
                ((cell.x - anchor.x).abs() + (cell.z - anchor.z).abs(), *cell)
            });
            let placed = cells.into_iter().find(|cell| {
                state
                    .apply(Command::PlaceTower {
                        x: cell.x,
                        z: cell.z,
                        archetype: TowerArchetype::Arrow,
                    })
                    .is_ok()
            });
            if let Some(cell) = placed {
                sites.push(building_entity_at(&state, BuildingKind::Tower, cell));
            }
        }
        assert!(sites.len() >= 2, "fixture needs construction sites");
        let starts = [town_center(), Cell::new(1, 1), Cell::new(GRID_WIDTH - 2, 5)];

        let check = |state: &GameState| {
            for &site in &sites {
                for &start in &starts {
                    assert_eq!(
                        state.nearest_storage_with_wood_for_site(site, start),
                        state.nearest_storage_with_wood_for_site_among(
                            site,
                            start,
                            &state.settlement_storage_ids()
                        ),
                        "site {site} from {start:?}"
                    );
                }
            }
        };
        check(&state);
        for &storage in &storages {
            let moved = state.take_wood_at(TOWN_ENTITY, 10);
            state.store_wood_at(storage, moved);
            check(&state);
        }
        let town_wood = state.storage.get(entity_key(TOWN_ENTITY)).unwrap().wood;
        state.take_wood_at(TOWN_ENTITY, town_wood);
        check(&state);
        state.take_wood_at(storages[0], 10);
        check(&state);
        // A topology change must drop the retained ranking.
        let extra = Cell::new(GRID_WIDTH / 2, GRID_HEIGHT - 3);
        state.store_wood_at(TOWN_ENTITY, 40);
        state
            .apply(Command::PlaceStorageHouse {
                x: extra.x,
                z: extra.z,
            })
            .expect("extra storage should be buildable");
        let extra = building_entity_at(&state, BuildingKind::StorageHouse, extra);
        state.store_wood_at(extra, 10);
        check(&state);
    }

    #[test]
    fn tower_is_inactive_until_workers_deliver_its_material() {
        let mut state = GameState::new(13);
        let cell = first_buildable_cell(&state);
        state
            .apply(Command::PlaceTower {
                x: cell.x,
                z: cell.z,
                archetype: TowerArchetype::Arrow,
            })
            .expect("tower construction should start");
        assert_eq!(state.tower_count(), 0);
        assert_eq!(state.construction_site_count(), 1);

        for _ in 0..200 {
            state.advance_tick();
            if state.tower_count() == 1 {
                break;
            }
        }
        assert_eq!(state.tower_count(), 1);
        assert_eq!(state.construction_site_count(), 0);
    }

    #[test]
    fn replay_includes_seeded_world_and_worker_construction() {
        let mut probe = GameState::new(29);
        let cell = first_buildable_cell(&probe);
        let mut commands = vec![Command::PlaceTower {
            x: cell.x,
            z: cell.z,
            archetype: TowerArchetype::Arrow,
        }];
        commands.extend(std::iter::repeat_n(Command::AdvanceTick, 100));
        let first = replay(29, &commands).expect("valid replay");
        let second = replay(29, &commands).expect("same replay remains valid");
        assert_eq!(first, second);
        assert_eq!(first.checksum(), second.checksum());
        probe
            .apply(Command::AdvanceTick)
            .expect("probe remains usable");
    }

    #[test]
    fn tower_targeting_prefers_nearest_then_lowest_entity_id() {
        let mut state = GameState::new(11);
        let tower = state.allocate_entity();
        state.transforms.insert(
            entity_key(tower),
            Transform {
                x_milli: 10_000,
                z_milli: 10_000,
            },
        );

        let lower_id = state.allocate_entity();
        let higher_id = state.allocate_entity();
        for (entity, x_milli) in [(lower_id, 9_000), (higher_id, 11_000)] {
            state.raiders.insert(
                entity_key(entity),
                Raider {
                    archetype: RaiderArchetype::Basic,
                    edge: Edge::North,
                    target_storage: TOWN_ENTITY,
                },
            );
            state.health.insert(
                entity_key(entity),
                Health {
                    current: 10,
                    maximum: 10,
                },
            );
            state.transforms.insert(
                entity_key(entity),
                Transform {
                    x_milli,
                    z_milli: 10_000,
                },
            );
        }

        assert_eq!(state.target_for_tower(tower, 2_000), Some(lower_id));

        state.transforms.insert(
            entity_key(higher_id),
            Transform {
                x_milli: 10_500,
                z_milli: 10_000,
            },
        );
        assert_eq!(state.target_for_tower(tower, 2_000), Some(higher_id));
    }

    #[test]
    fn integer_square_root_is_deterministic_at_boundaries() {
        assert_eq!(integer_sqrt(0), 0);
        assert_eq!(integer_sqrt(1), 1);
        assert_eq!(integer_sqrt(2), 1);
        assert_eq!(integer_sqrt(4), 2);
        assert_eq!(integer_sqrt(15), 3);
        assert_eq!(integer_sqrt(16), 4);
    }
}
