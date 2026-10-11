import { OrbitControls } from "@react-three/drei";
import { Canvas } from "@react-three/fiber";
import { useEffect, useMemo, useState } from "react";
import type { CellView, EntityView, SnapshotView } from "../simulationTypes";
import type { PlacementChoice } from "./viewModel";

// The 3D battlefield. Every transform derives from authoritative snapshot positions; the only
// browser-owned state is the hovered cell used to place the translucent placement ghost.

function worldX(value: number, width: number) {
  return value - width / 2;
}

function worldZ(value: number, height: number) {
  return value - height / 2;
}

function entityPosition(entity: EntityView, snapshot: SnapshotView): [number, number] {
  return [
    worldX(entity.x_milli / 1000, snapshot.grid_width),
    worldZ(entity.z_milli / 1000, snapshot.grid_height),
  ];
}

function isTownCell(cell: CellView, snapshot: SnapshotView) {
  const centerX = Math.floor(snapshot.grid_width / 2);
  const centerZ = Math.floor(snapshot.grid_height / 2);
  return Math.abs(cell.x - centerX) <= 1 && Math.abs(cell.z - centerZ) <= 1;
}

function isGateCell(cell: CellView, snapshot: SnapshotView) {
  const middleX = Math.floor(snapshot.grid_width / 2);
  const middleZ = Math.floor(snapshot.grid_height / 2);
  return (
    (cell.x === middleX && (cell.z === 0 || cell.z === snapshot.grid_height - 1)) ||
    (cell.z === middleZ && (cell.x === 0 || cell.x === snapshot.grid_width - 1))
  );
}

function GroundTile({
  cell,
  snapshot,
  selected,
  occupied,
  onHoverCell,
  onActivateCell,
}: {
  cell: CellView;
  snapshot: SnapshotView;
  selected: boolean;
  occupied: boolean;
  onHoverCell: (cell: CellView) => void;
  onActivateCell: (cell: CellView) => void;
}) {
  const protectedCell = isTownCell(cell, snapshot) || isGateCell(cell, snapshot);
  const base = protectedCell ? "#384238" : occupied ? "#3f372e" : "#263129";
  return (
    <mesh
      position={[
        worldX(cell.x + 0.5, snapshot.grid_width),
        -0.08,
        worldZ(cell.z + 0.5, snapshot.grid_height),
      ]}
      onPointerMove={(event) => {
        event.stopPropagation();
        onHoverCell(cell);
      }}
      onPointerOver={(event) => {
        event.stopPropagation();
        onHoverCell(cell);
      }}
      onClick={(event) => {
        event.stopPropagation();
        if (event.delta > 6) return;
        onActivateCell(cell);
      }}
      receiveShadow
    >
      <boxGeometry args={[0.94, 0.12, 0.94]} />
      <meshStandardMaterial color={selected ? "#d1af63" : base} roughness={0.92} />
    </mesh>
  );
}

function GhostMaterial() {
  return (
    <meshStandardMaterial
      color="#d8cfac"
      transparent
      opacity={0.42}
      depthWrite={false}
    />
  );
}

function PlacementGhost({
  placement,
  cell,
  snapshot,
}: {
  placement: PlacementChoice;
  cell: CellView;
  snapshot: SnapshotView;
}) {
  const position: [number, number, number] = [
    worldX(cell.x + 0.5, snapshot.grid_width),
    0,
    worldZ(cell.z + 0.5, snapshot.grid_height),
  ];

  if (placement.kind === "tower") {
    return (
      <group position={position}>
        <mesh position={[0, 0.28, 0]} raycast={() => undefined}>
          <cylinderGeometry args={[0.42, 0.48, 0.54, 8]} />
          <GhostMaterial />
        </mesh>
        <mesh position={[0, 0.82, 0]} raycast={() => undefined}>
          <boxGeometry args={[0.56, 0.58, 0.56]} />
          <GhostMaterial />
        </mesh>
      </group>
    );
  }

  if (placement.kind === "sawmill") {
    return (
      <group position={position}>
        <mesh position={[0, 0.3, 0]} raycast={() => undefined}>
          <boxGeometry args={[0.8, 0.54, 0.76]} />
          <GhostMaterial />
        </mesh>
        <mesh position={[0.43, 0.42, 0]} rotation={[Math.PI / 2, 0, 0]} raycast={() => undefined}>
          <cylinderGeometry args={[0.24, 0.24, 0.08, 16]} />
          <GhostMaterial />
        </mesh>
      </group>
    );
  }

  return (
    <group position={position}>
      <mesh position={[0, 0.32, 0]} raycast={() => undefined}>
        <boxGeometry args={[placement.kind === "storage_house" ? 0.9 : 0.82, 0.6, 0.78]} />
        <GhostMaterial />
      </mesh>
      <mesh position={[0, 0.78, 0]} rotation={[0, Math.PI / 4, 0]} raycast={() => undefined}>
        <coneGeometry args={[0.64, 0.5, 4]} />
        <GhostMaterial />
      </mesh>
    </group>
  );
}

function TownHallModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  return (
    <group position={[x, 0, z]}>
      <mesh position={[0, 0.38, 0]} castShadow receiveShadow>
        <boxGeometry args={[2.7, 0.72, 2.7]} />
        <meshStandardMaterial color="#a78b5a" roughness={0.82} />
      </mesh>
      <mesh position={[0, 1.08, 0]} castShadow>
        <boxGeometry args={[1.65, 0.75, 1.65]} />
        <meshStandardMaterial color="#d1c29b" roughness={0.76} />
      </mesh>
      <mesh position={[0, 1.72, 0]} rotation={[0, Math.PI / 4, 0]} castShadow>
        <coneGeometry args={[1.35, 1.05, 4]} />
        <meshStandardMaterial color="#725246" roughness={0.8} />
      </mesh>
    </group>
  );
}

function ForestModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  const ratio = entity.wood_capacity > 0 ? entity.stored_wood / entity.wood_capacity : 0;
  const scale = 0.72 + ratio * 0.28;
  return (
    <group position={[x, 0, z]} scale={scale}>
      <mesh position={[0, 0.32, 0]} castShadow>
        <cylinderGeometry args={[0.1, 0.14, 0.64, 7]} />
        <meshStandardMaterial color="#6f4b2f" roughness={0.95} />
      </mesh>
      <mesh position={[0, 0.86, 0]} castShadow>
        <coneGeometry args={[0.48, 0.95, 7]} />
        <meshStandardMaterial color="#365a35" roughness={0.92} />
      </mesh>
    </group>
  );
}

function TowerModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  if (entity.tower_level === 0) {
    const ratio = entity.wood_capacity > 0 ? entity.stored_wood / entity.wood_capacity : 0;
    return (
      <group position={[x, 0, z]}>
        <mesh position={[0, 0.14, 0]} castShadow>
          <boxGeometry args={[0.74, 0.22, 0.74]} />
          <meshStandardMaterial color="#806b4b" roughness={0.96} />
        </mesh>
        <mesh position={[-0.26, 0.52, -0.26]} castShadow>
          <boxGeometry args={[0.08, 0.8, 0.08]} />
          <meshStandardMaterial color="#a27a45" roughness={0.92} />
        </mesh>
        <mesh position={[0.26, 0.52, 0.26]} castShadow>
          <boxGeometry args={[0.08, 0.8, 0.08]} />
          <meshStandardMaterial color="#a27a45" roughness={0.92} />
        </mesh>
        <mesh position={[0, 0.38, 0]} castShadow scale={[Math.max(0.1, ratio), 1, 1]}>
          <boxGeometry args={[0.58, 0.12, 0.18]} />
          <meshStandardMaterial color="#c69b61" roughness={0.9} />
        </mesh>
      </group>
    );
  }

  const archetype = entity.tower_archetype ?? "arrow";
  const levelScale = 1 + Math.max(0, entity.tower_level - 1) * 0.1;
  return (
    <group position={[x, 0, z]} scale={levelScale}>
      <mesh position={[0, 0.28, 0]} castShadow receiveShadow>
        <cylinderGeometry args={[0.42, 0.48, 0.54, 8]} />
        <meshStandardMaterial color={archetype === "cannon" ? "#6d665e" : "#81705b"} roughness={0.86} />
      </mesh>
      <mesh position={[0, 0.82, 0]} castShadow>
        <boxGeometry args={[0.56, 0.58, 0.56]} />
        <meshStandardMaterial color={archetype === "cannon" ? "#8b806f" : "#b6a384"} roughness={0.78} />
      </mesh>
      {archetype === "cannon" ? (
        <mesh position={[0, 1.03, -0.32]} rotation={[Math.PI / 2, 0, 0]} castShadow>
          <cylinderGeometry args={[0.1, 0.14, 0.7, 10]} />
          <meshStandardMaterial color="#3d4140" metalness={0.35} roughness={0.58} />
        </mesh>
      ) : (
        <mesh position={[0, 1.3, 0]} rotation={[0, Math.PI / 4, 0]} castShadow>
          <coneGeometry args={[0.44, 0.46, 4]} />
          <meshStandardMaterial color="#5f493f" roughness={0.78} />
        </mesh>
      )}
    </group>
  );
}

function SawmillModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  return (
    <group position={[x, 0, z]}>
      <mesh position={[0, 0.3, 0]} castShadow>
        <boxGeometry args={[0.8, 0.54, 0.76]} />
        <meshStandardMaterial color="#7c5a38" roughness={0.9} />
      </mesh>
      <mesh position={[0.43, 0.42, 0]} rotation={[Math.PI / 2, 0, 0]} castShadow>
        <cylinderGeometry args={[0.24, 0.24, 0.08, 16]} />
        <meshStandardMaterial color="#aaa79a" metalness={0.35} roughness={0.6} />
      </mesh>
    </group>
  );
}

function StorageHouseModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  const ratio = entity.wood_capacity > 0 ? entity.stored_wood / entity.wood_capacity : 0;
  return (
    <group position={[x, 0, z]}>
      <mesh position={[0, 0.34, 0]} castShadow receiveShadow>
        <boxGeometry args={[0.9, 0.64, 0.84]} />
        <meshStandardMaterial color="#8a7353" roughness={0.9} />
      </mesh>
      <mesh position={[0, 0.82, 0]} rotation={[0, Math.PI / 4, 0]} castShadow>
        <coneGeometry args={[0.68, 0.5, 4]} />
        <meshStandardMaterial color="#514137" roughness={0.86} />
      </mesh>
      {ratio > 0 && (
        <mesh position={[0, 0.2, 0.46]} castShadow scale={[0.5 + ratio * 0.5, 1, 1]}>
          <boxGeometry args={[0.64, 0.18, 0.18]} />
          <meshStandardMaterial color="#9b6834" roughness={0.95} />
        </mesh>
      )}
    </group>
  );
}

function HouseModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  return (
    <group position={[x, 0, z]}>
      <mesh position={[0, 0.32, 0]} castShadow receiveShadow>
        <boxGeometry args={[0.82, 0.58, 0.72]} />
        <meshStandardMaterial color="#c0a27a" roughness={0.88} />
      </mesh>
      <mesh position={[0, 0.78, 0]} rotation={[0, Math.PI / 4, 0]} castShadow>
        <coneGeometry args={[0.62, 0.52, 4]} />
        <meshStandardMaterial color="#68483a" roughness={0.84} />
      </mesh>
    </group>
  );
}

function PersonModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  return (
    <group position={[x, 0, z]}>
      <mesh position={[0, 0.31, 0]} castShadow>
        <capsuleGeometry args={[0.12, 0.27, 3, 6]} />
        <meshStandardMaterial color="#4f6b55" roughness={0.8} />
      </mesh>
      <mesh position={[0, 0.63, 0]} castShadow>
        <sphereGeometry args={[0.12, 8, 6]} />
        <meshStandardMaterial color="#bd8f6d" roughness={0.82} />
      </mesh>
      {entity.cargo_wood > 0 && (
        <mesh position={[0.18, 0.32, 0]} castShadow>
          <boxGeometry args={[0.2, 0.24, 0.24]} />
          <meshStandardMaterial color="#916132" roughness={0.95} />
        </mesh>
      )}
    </group>
  );
}

function RaiderModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  const healthRatio = entity.max_health > 0 ? entity.health / entity.max_health : 0;
  return (
    <group position={[x, 0, z]}>
      <mesh position={[0, 0.38, 0]} castShadow>
        <capsuleGeometry args={[0.22, 0.4, 4, 8]} />
        <meshStandardMaterial color="#7a3030" roughness={0.72} />
      </mesh>
      <mesh position={[0, 0.82, 0]} castShadow>
        <sphereGeometry args={[0.2, 10, 8]} />
        <meshStandardMaterial color="#b78463" roughness={0.82} />
      </mesh>
      <mesh position={[0, 1.15, 0]}>
        <boxGeometry args={[0.58, 0.055, 0.07]} />
        <meshBasicMaterial color="#171b18" />
      </mesh>
      <mesh position={[-0.29 + 0.58 * healthRatio * 0.5, 1.15, -0.04]}>
        <boxGeometry args={[0.58 * healthRatio, 0.04, 0.04]} />
        <meshBasicMaterial color="#a9b75e" />
      </mesh>
    </group>
  );
}

function ProjectileModel({ entity, snapshot }: { entity: EntityView; snapshot: SnapshotView }) {
  const [x, z] = entityPosition(entity, snapshot);
  const cannon = entity.tower_archetype === "cannon";
  return (
    <mesh position={[x, cannon ? 0.72 : 0.9, z]} castShadow>
      <sphereGeometry args={[cannon ? 0.14 : 0.07, 10, 8]} />
      <meshStandardMaterial color={cannon ? "#d58a49" : "#d8c485"} roughness={0.45} />
    </mesh>
  );
}

function GateMarker({ cell, snapshot }: { cell: CellView; snapshot: SnapshotView }) {
  return (
    <group
      position={[
        worldX(cell.x + 0.5, snapshot.grid_width),
        0,
        worldZ(cell.z + 0.5, snapshot.grid_height),
      ]}
    >
      <mesh position={[-0.3, 0.38, 0]} castShadow>
        <boxGeometry args={[0.16, 0.76, 0.16]} />
        <meshStandardMaterial color="#655d50" />
      </mesh>
      <mesh position={[0.3, 0.38, 0]} castShadow>
        <boxGeometry args={[0.16, 0.76, 0.16]} />
        <meshStandardMaterial color="#655d50" />
      </mesh>
      <mesh position={[0, 0.72, 0]} castShadow>
        <boxGeometry args={[0.76, 0.14, 0.16]} />
        <meshStandardMaterial color="#655d50" />
      </mesh>
    </group>
  );
}

export function GameWorld({
  snapshot,
  selectedCell,
  placement,
  onSelectCell,
  onPlace,
}: {
  snapshot: SnapshotView;
  selectedCell: CellView | null;
  placement: PlacementChoice | null;
  onSelectCell: (cell: CellView) => void;
  onPlace: (cell: CellView) => void;
}) {
  const [hoveredCell, setHoveredCell] = useState<CellView | null>(null);
  useEffect(() => {
    setHoveredCell(null);
  }, [placement]);

  const cells = useMemo(() => {
    const result: CellView[] = [];
    for (let z = 0; z < snapshot.grid_height; z += 1) {
      for (let x = 0; x < snapshot.grid_width; x += 1) {
        result.push({ x, z });
      }
    }
    return result;
  }, [snapshot.grid_height, snapshot.grid_width]);
  const occupied = useMemo(
    () =>
      new Set(
        snapshot.entities
          .filter((entity) =>
            ["tower", "sawmill", "storage_house", "house", "forest"].includes(entity.kind),
          )
          .map((entity) => `${entity.cell.x}:${entity.cell.z}`),
      ),
    [snapshot.entities],
  );
  const gates = [
    { x: Math.floor(snapshot.grid_width / 2), z: 0 },
    { x: snapshot.grid_width - 1, z: Math.floor(snapshot.grid_height / 2) },
    { x: Math.floor(snapshot.grid_width / 2), z: snapshot.grid_height - 1 },
    { x: 0, z: Math.floor(snapshot.grid_height / 2) },
  ];
  const ghostCell = hoveredCell ?? selectedCell;

  return (
    <Canvas
      camera={{ position: [15, 18, 19], fov: 42, near: 0.1, far: 100 }}
      shadows
      dpr={[1, 1.6]}
      style={{ touchAction: "none" }}
    >
      <color attach="background" args={["#111712"]} />
      <fog attach="fog" args={["#111712", 26, 54]} />
      <ambientLight intensity={1.3} />
      <directionalLight position={[9, 16, 10]} intensity={2.1} castShadow />
      {cells.map((cell) => (
        <GroundTile
          key={`${cell.x}:${cell.z}`}
          cell={cell}
          snapshot={snapshot}
          selected={
            placement === null &&
            selectedCell !== null &&
            cell.x === selectedCell.x &&
            cell.z === selectedCell.z
          }
          occupied={occupied.has(`${cell.x}:${cell.z}`)}
          onHoverCell={setHoveredCell}
          onActivateCell={placement ? onPlace : onSelectCell}
        />
      ))}
      {placement && ghostCell && (
        <PlacementGhost placement={placement} cell={ghostCell} snapshot={snapshot} />
      )}
      {gates.map((cell) => (
        <GateMarker key={`gate-${cell.x}:${cell.z}`} cell={cell} snapshot={snapshot} />
      ))}
      {snapshot.entities.map((entity) => {
        switch (entity.kind) {
          case "town_hall":
            return <TownHallModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "tower":
            return <TowerModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "sawmill":
            return <SawmillModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "storage_house":
            return <StorageHouseModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "house":
            return <HouseModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "forest":
            return <ForestModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "person":
            return <PersonModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "raider":
            return <RaiderModel key={entity.id} entity={entity} snapshot={snapshot} />;
          case "projectile":
            return <ProjectileModel key={entity.id} entity={entity} snapshot={snapshot} />;
        }
      })}
      <OrbitControls
        makeDefault
        target={[0, 0.4, 0]}
        minDistance={9}
        maxDistance={42}
        maxPolarAngle={Math.PI * 0.47}
        enableDamping
      />
    </Canvas>
  );
}
