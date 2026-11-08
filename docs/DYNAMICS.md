# Dynamics

One solver moves cloth, particles and rigid bodies, so they all collide with and push each other in the same step.
There is no separate particle system, cloth system or rigid body system: each is particles held by different
constraints. Files from the reference's several older dynamics systems load into this one.

The solver is the crate `aztek_dyn`; the scene wraps it in `crates/aztek_scene/src/dynamics.rs` (nodes),
`dyn_solve.rs` (stepping with time), `dyn_cmds.rs` and `dyn_particles.rs` (commands) and `ma_file/dynamics.rs`
(files).

## Using it

The Dynamics menu set (F5, or the menu set box) has Cloth, Particles, Rigid Bodies, Fields, Constraints, Cache and
Solver menus, and the Dynamics shelf the common items.

- **Cloth.** Select a mesh, Cloth > Create Cloth. The mesh as it comes in (skinned, keyed, deformed) is where the
  cloth starts and what it can be drawn to; the simulated mesh goes out to the shape. Material presets (silk, cotton,
  leather, heavy denim, rubber) are in the Cloth menu and at the top of the cloth's Attribute Editor tab.
- **Painted maps.** With the cloth's mesh selected, the Paint Attributes tool paints `cloth.cloth1.inputAttractPerVertex`
  (how strongly each point follows the incoming mesh), `massPerVertex` and `pinPerVertex` (1 holds a point to the
  incoming mesh).
- **Colliders.** Select a mesh, Cloth > Create Collider. It may be animated or skinned.
- **Particles.** Particles > Create Emitter makes an emitter and the particles it feeds; with a mesh selected,
  Emit from Object emits off its surface. Particles draw as points, spheres or blobs (`particleRenderType`).
- **Rigid bodies.** Select objects, Rigid Bodies > Create Active (moved by the solver) or Passive (only pushes).
- **Fields.** Select bodies and choose a field: air, drag, gravity, newton, radial, turbulence, uniform, vortex,
  volume axis. Fields > Affect Selected Objects connects a field to more bodies.
- **Constraints.** Component to component, point to surface, weld, tearable surface, nail, pin, hinge, spring.
- **Cache.** Create New Cache solves the playback range and writes it to disk; the cloth then plays from it.
- **Solver.** Gravity, wind, air density, ground plane, substeps, start frame; Interactive Playback plays every frame
  in a loop so things can be moved while it solves.

Time behaves as the reference's solvers do: nothing is solved going backwards; going to the start frame starts again;
playing forward solves each frame; jumping forward several frames solves one. Frames already solved are kept in memory
and shown again when scrubbing back. `dynInfo` reports the solver's state.

## Nodes

| Node | Made by | What it is |
|---|---|---|
| `solver` (shape under `solver1`) | any body, `createSolver` | Time, gravity, wind, ground plane, substeps. Bodies connect its `message` to their `solver`. |
| `cloth` (`cloth1`) | `createCloth` | A deformer: mesh in, simulated mesh out. |
| `particles` (shape under `particles1`) | `particle`, `emitter` | Particles with position, velocity, age, lifespan, colour, opacity, radius. |
| `emitter` (shape under `emitter1`) | `emitter` | Feeds particles through their `newParticles[]`. |
| `rigidBody` (`rigidBody1`) | `rigidBody` | Drives its object's translate and rotate; passive ones only push. |
| `collider` (`collider1`) | `collider` | An animated mesh the others hit. |
| `field` (shape under `gravityField1` and the like) | the field commands | Acts on bodies connected to its `message` through their `inputForce[]`. |
| `dynamicConstraint` | `dynamicConstraint` | Holds points of bodies to each other, to a surface or in place. |

Attributes keep the reference's names where they mean the same thing (`thickness`, `stretchResistance`,
`bendResistance`, `pointMass`, `inputMeshAttract`, `subSteps`, `gravity`, `lifespanMode`, `magnitude`, `attenuation`
and so on), so files map across. Resistances become compliance as `1e-5 / stretchResistance`,
`1e-5 / compressionResistance` and `1e-4 / bendResistance`.

## Commands

New: `createCloth`, `collider`, `createSolver`, `clothPreset`, `dynamicConstraint`, `dynCache`, `dynInfo`.
Kept from the reference for scripts: `nCloth`, `createNCloth`, `makeCollideNCloth`, `createNConstraint`, `particle`,
`nParticle`, `emitter`, `rigidBody`, `gravity`, `air`, `drag`, `newton`, `radial`, `turbulence`, `uniform`, `vortex`,
`volumeAxis`, `connectDynamic`, `getParticleAttr`, `setParticleAttr`, `dynExpression`. Flags are in
[COMMANDS.md](COMMANDS.md).

## The solver

Extended position based dynamics on particles. Each frame is split into substeps (12 by default); each substep
predicts positions from velocity, gravity, fields and air drag, projects the constraints once (stretch twice), resolves
contacts, and takes the velocity from what moved. Small steps rather than many iterations.

- **Cloth:** stretch and compression (separate stiffness), dihedral bending, optional area, attraction to the input
  mesh with a per point map, pins, stretch limits and long range attachments. Stretch limits hold vertices several
  edges apart (3, 9, 27...) no further apart than the path between them along the rest mesh; long range attachments
  keep every vertex within its distance along the rest mesh from the nearest pin. Neither acts on folds, only on
  stretch, and they carry tension across a fine mesh in one step.
- **Parallel and repeatable:** constraints are coloured so the constraints of one colour share no particle and solve
  at once; contacts are summed per particle in a fixed order. The result is the same, bit for bit, on any number of
  threads.
- **Rigid bodies:** particles filling the shape (its mesh's volume, a box, sphere or capsule) plus its own corners on
  the surface, put back on the rigid shape after each substep (shape matching).
- **Particles:** born from emitters (omni, directional, surface, volume, curve, point) at a rate carried over between
  frames and seeded by emitter and frame; lifespans constant, random or per particle.
- **Fields:** force `magnitude * (1 / (1 + d))^attenuation`, times `(1 - d / maxDistance)` and nothing past it when
  `useMaxDistance` is on; gravity is an acceleration, the rest forces divided by mass.

### Contacts

- **Colliders:** each particle finds, per collider, its nearest triangle among those that can reach it this frame
  (a spatial hash over swept triangle bounds). The thickness is a soft zone: a point below the surface goes back to
  it at once, and a point in the zone rises a quarter of the thickness per substep at most, so a contact never adds
  energy. Friction acts against the collider's own motion at that place.
- **Sides:** a closed collider has one outside, from its normals. For an open one (a plane, a garment) each particle
  learns which side it is on when it first meets it, and keeps it: a point found on the wrong side within the
  collider's motion this frame, twice the thickness and a quarter of the triangle is put back. A fast collider cannot
  tunnel through; deeper than that is reported as impossible. Round an open border, the side is learnt again.
- **Self and between bodies:** cloth vertices, particles and collider vertices against cloth triangles (over the face
  only, the one-ring excluded), and particles against particles, found every 4 substeps over that much motion.

## Impossible states

What the solve cannot place, it does not force. After each frame every vertex is checked for: left through a
collider by more than half the thickness, pushed through deeper than it can be brought back, pressed into its
thickness from opposite sides at once, stretched past the stretch limit (3 times rest), swinging back and forth faster
than its thickness each frame, or a value that stopped being finite. Each adds to a trouble score that decays by 0.6 a
frame; at 2 the vertex leaves the simulation.

Leaving, its weight in the simulation ramps from 1 to 0 over `blendFrames` (4), smoothed at both ends, and it blends
to its fallback: its animated input (Animation), the average of where its neighbours are (Neighbours), or, by default
(Auto), the input where that is near and not itself inside something, else the neighbours. Its speed goes with its
weight, so nothing it was moved by while blending is carried back into the simulation. Out, it is treated as kinematic by the constraints and checked each frame at its fallback place; after
`returnFrames` (4) frames in a row in a possible state it ramps back the same way, learning its sides afresh. It comes
back after `mostFramesOut` (48) frames whatever happens, so none stays out for good. A patch whose neighbours are all
out can only gather in, never drift.

The viewport marks vertices out in orange and those blending back in yellow. `outOfSimulation` on each body and the
solver, and `dynInfo`, count them.

Hard limits on every substep: speed at most `maxVelocity`; a particle's step at most twice that over the substep; a
position that is not finite is put back where it was and stopped; a constraint whose correction is not finite is
skipped, so nothing spreads to neighbours. Masses of zero pin; tiny and huge masses work; degenerate faces are
skipped; a topology change builds the solver again; an input that jumps further than its own size in a frame starts
that body again from it; a frame longer than 1/12 s is solved as 1/12 s.

## How it is tested

`cargo test -p aztek_dyn` and `-p aztek_scene --test dynamics --test dynamics_ma`:

- free fall against the formula (within 1%), a pendulum's period (within 2%), a drape on a sphere (nothing inside,
  the middle on top at the thickness, stretch under 1.15, still, nothing out of the simulation), a stack of three
  boxes that stays standing and upright, the same answer bit for bit on 1 and 8 threads;
- cloth squeezed between two plates that close to nothing, hold and open: vertices leave, stay out while squeezed,
  come back within 6 frames of the plates opening, and none leaves more than twice;
- a ball driven through pinned cloth at 12 units a second: everything finite and near its animation each frame, no edge
  past 10 times its rest length (the ball's width), and every vertex back within 60 frames of the ball leaving;
- a 3,721 vertex drape with self collision: finite, nothing above where it started, nothing into the ball, no edge
  past 12 times rest;
- 24 random cases of 48 frames: random grids with degenerate faces, masses of 0, 1e-9 and 1e9, a mass that is not a
  number, colliders scaled 0.01 to 100, turned and thrown 100 units each frame, inputs teleporting 50 units and
  turning to NaN, particles fired at 1e6, time steps of 1e-7, 10 and -1: all finite, within bounds, none out longer
  than allowed;
- in the scene: drape, scrubbing back, starting again, jumping ahead, undo, emitters with gravity, a rigid body on the
  ground plane and a tilted one landing on its corners, presets and old command names;
- files: own nodes saved and read back to the same text and the same simulation; a nucleus scene with nCloth and
  nRigid read as cloth and a collider that drape; classic particles with fields, an emitter, a rigid body, a rigid
  solver and hair.

## Speed

Measured on 32 threads, release build.

| Scene | Vertices | Milliseconds a frame |
|---|---|---|
| Cloth drape on a sphere (`drape` example, grid 30) | 961 | 13 |
| Free cloth, no contacts (grid 141) | 20,164 | about 100 (60 of it constraints) |
| The same falling onto a sphere, self collision on | 20,164 | 140 to 270 while landing, more as it folds |
| Shirt on a skinned character, self collision, gentle swing | 3,018 against 15,277 | 130 to 190 solving |
| The same shirt smoothed once | 12,153 against 15,277 | 240 to 320 solving |

Where the time goes on the character: finding contacts (cloth against itself and the body's vertices against cloth
triangles) about half, cloth constraints a quarter, resolving collider contacts the rest. `cargo run --release -p
aztek_scene --example cloth_rig -- file.ma garment body control` prints it per frame for a rig; `dynInfo` shows it per
part.

## Files

`.ma` nodes of the older systems are rewritten as these before the scene is built:

| From | To |
|---|---|
| `nucleus`, `rigidSolver` | a transform with a `solver` shape |
| `bulletSolverShape` | `solver` |
| `nCloth` | a `cloth` deformer between the mesh it read and the mesh it drove; its transform goes |
| `nParticle`, `particle` | `particles`; stored positions (`pos0`, else `pos`) become start positions |
| `nRigid` | `collider` |
| `rigidBody`, `bulletRigidBodyShape` | `rigidBody` moving the object it was under |
| `pointEmitter` | a transform with an `emitter` shape |
| `airField`, `dragField`, `gravityField`, `newtonField`, `radialField`, `turbulenceField`, `uniformField`, `vortexField`, `volumeAxisField` | a transform with a `field` shape |
| `dynamicConstraint` | held as one, without its components |
| `hairSystem`, `nComponent`, fluids, `cacheFile`, `geoConnector`, the soft body and spring nodes, and nodes of the molecular matter plug-in | `heldNode`: not simulated, written back as they came |

`follicle` stays as it is. What a node set that is not used here is kept on it as the file's own lines
(`fileAttributes`) and its old type in `fileType`; a warning names those attributes. Connections that mean nothing
here are dropped and counted in a warning. Substeps count four times the old ones (between 8 and 64), as the old
solvers iterate more within a step.

Saving writes the nodes above as themselves, under `requires "aztekDynamics" "1.0"`. Read back here they are the same,
attribute for attribute; held nodes are written back in their original form. The reference reads such a file but
keeps these nodes as unknown nodes of a plug-in it does not have, so the dynamics do not play there.

Not honoured from the older systems: nCloth's lift, stickiness, pressure, rigidity, wind shadow, self collision flags
and every per vertex map but input attract and mass (all kept); nucleus wind noise, plane bounce and normal (the plane
always faces up), collision layers; particle ramps and per particle expressions (`dynExpression` keeps the text but
does not run it); fields' volume shapes and per field falloff curves; rigid solver settings; Bullet's constraints and
soft bodies; dynamic constraints' components (made again with `dynamicConstraint`); hair, fluids and molecular matter
simulation.

## The disk cache

`dynCache -create` writes one file per cloth (the temporary folder unless `-file` names one); `-append`, `-replace`
and `-delete` act on it. The format is Aztek's own, not the reference's `.mcx`:

- `AZDC`, then a little endian `u32` version (1);
- records to the end of the file: `f64` frame, `u32` channel count, and per channel a `u32` name length, the name in
  UTF-8, a `u32` point count and `x y z` as `f32` for each point, object space.

A later record for the same frame replaces an earlier one, so appending never rewrites the file.

## Room to grow

Particles carry a phase (cloth, particle, rigid, fluid); `Extension` lets later systems such as liquids, smoke and fire
take part in every substep of the same solve.
