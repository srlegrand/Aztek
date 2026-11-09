# Grox

`examples/characters/grox.ma`: a horned brute, generated end to end by the scripts here. Rough on purpose: it shows a
full character rig, not good modelling.

Needs Python 3 with numpy, scipy, scikit-image, pyfqmr and Pillow, and a built Aztek. From an empty work folder:

```bash
T=<this folder>
cp $T/design.py .
python3 $T/sculpt.py 1.25 60000                         # grox_body.obj, 30k vertices
GROX_WORK=$PWD aztek --script $T/uvs.py                 # grox_geo.ma, uvdata.json
python3 $T/bake.py 2048                                 # grox_body_{baseColor,normal,ORM,mask}.png
GROX_WORK=$PWD GROX_TOOLS=$T aztek --script $T/rig.py   # out/grox_rig.ma
python3 $T/weights.py out/grox_rig.ma                   # skin weights by bone, smoothed over the surface
```

The copy in `examples` keeps every map at 1024, in `examples/textures/grox`.

## The rig

- `C_global_CTRL`: `geometry` (full, proxy, none), `skeleton`, `geoLock`. Under it `C_local_CTRL`, then `C_cog`, `C_hips`,
  three spine controls and `C_chest`; two neck controls; `C_head` with `follow` (neck, chest, world) and `blink`; `C_jaw`.
- Arms: clavicle, FK and IK blended by `<side>_armSettings_CTRL.ikFk`; RP IK with a pole vector, `stretch` and `follow`
  (world, chest, cog). `<side>_hand_CTRL` has `curl` and `spread` for the three claws; a two-joint thumb.
- Legs: the same blend; reverse foot on `<side>_legIK_CTRL` with `roll`, `rollBreak`, `toeTap`, `heelTwist`, `toeTwist`
  and `bank`; three toes.
- Five-joint ram horns, brow knobs, eyes aimed at `C_eyes_CTRL`, eyelids on the blink, an eight-control tail.
- One skinCluster, 81 influences, four per vertex. Proxy pieces follow the joints.
- Joints `<side>_<part>_JNT`, controls `<side>_<part>_CTRL` inside `_OFF` and `_DRV` groups; left blue, right red,
  centre yellow.
