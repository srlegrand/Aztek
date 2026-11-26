# Capture kit

Nothing in Aztek's tests has yet been compared with a running copy of the reference. This kit gets the answers.

## Running it

    mayapy tools/reference_capture/capture.py

or in the Script Editor of a running copy, with a Python tab:

    import os; os.environ["CAPTURE_DIR"] = "/path/to/aztek/tools/reference_capture"
    exec(open(os.environ["CAPTURE_DIR"] + "/capture.py").read())

Use a clean preferences folder, so the hotkeys and units are the defaults. It writes to `tests/data/reference/`; commit
that folder. `CAPTURE_ONLY=calls,attributes,commands,scenes` runs one part again, and `errors.json` lists any part that
failed. A second argument to `mayapy` names another output folder.

| File | What it says |
| --- | --- |
| `meta.json` | The version captured from |
| `calls/<area>.json` | Result or error of each call in `calls.json` |
| `attributes/<nodeType>.json` | `attributeInfo -all`, names, types, defaults, `listAttr -k`, `-cb`, for each type in `node_types.txt` |
| `commands/runtime.json`, `name_commands.json`, `hotkeys.json` | The default runtime commands, name commands and hotkeys |
| `scenes/*.ma` | The scenes of `scenes.json`, saved by the reference |
| `posed/<scene>.json` | World matrices and sample points of those scenes, reopened from the file, at their frames |

`xgen_capture.py` is separate: it records a groom made by hand in two steps (see its header) and writes
`tests/data/reference/xgen/`, which XJEN's tests read (docs/XJEN.md).

`crates/aztek_host/tests/reference_fixtures.rs` reads `calls/`. To refresh the type list, run
`cargo run -p aztek_host --example list_node_types > tools/reference_capture/node_types.txt`.

## The inventory

`inventory.py` lists everything the reference exposes, for the parity ledger (`parity/README.md`): commands and their
flags, node types and their attributes, both scripting APIs, the C++ headers, environment variables (set, in the
environment file, or read by the shipped scripts), preferences, runtime and name commands, hotkeys, menus, shelves,
panels, tool contexts, translators, project rules and plug-ins. Names and interfaces only, with the product's and
vendor's names and the user's paths taken out. In a Python tab of the Script Editor of a running copy (menus, hotkeys
and shelves need the interface):

    exec(open("/path/to/aztek/tools/reference_capture/inventory.py").read())

or `mayapy tools/reference_capture/inventory.py`. It writes `reference_inventory_<stamp>/` and a zip of it in the home
folder; the header lists its settings (`INVENTORY_ONLY`, `INVENTORY_DEVKIT` for the C++ headers, and others). Then
`python3 tools/parity/import_inventory.py <zip>` merges it into `parity/`.

## What tests still need

Each is a guess in a test, marked `// unverified:` at the line. `calls.json` carries a call for each where one fits.

| Question | Test | Call |
| --- | --- | --- |
| Where do the default Move and Scale plane handles sit relative to the axis tips in a front view? | `plane_handles.rs` | Front view screenshots |
| Is an instanced shape listed by its path (`pCube3\|pCubeShape1`)? | `basics.rs` `duplicate_and_instance` | `instance.*` |
| `duplicate -st` after a moved duplicate: is the offset repeated? | `duplicate_and_instance` | `duplicate.*` |
| Does `rename` rename the shape with its transform? | `basics.rs` rename test | `rename.*` |
| What does `rename` on a shape return? | `basics.rs` rename test | `rename.*` |
| Does `duplicate` keep the padding (`arm_01` to `arm_02`), past a carry (`ctl_L_009` to `ctl_L_010`), and after a letter (`box5`)? | `duplicate_keeps_padding` | `duplicate.padded*`, `duplicate.letter.digit` |
| Does current time keep its seconds across a unit change? | `time_units_convert` | `units.pal.time`, `units.ntsc.time` |
| Does inverting components stay on the object? | `invert_selection` | `select.invert` |
| Does `connectAttr` make a cycle and warn at evaluation, rather than refuse? | `cycles_connect` | `cycle.connects` |
| Does `displaySmoothness -q -po` answer per object as an array? | `smooth_preview_attributes` | `smooth.query` |
| What does `hyperShade -listUpstreamShaderNodes` return? | `shading_node_colour_managed` | none yet |
| Does joint `stiffness` change an ikRPsolver or ikSCsolver solve (a three-joint chain, the middle joint's stiffnessY at 90, the handle dragged in)? Does `priority` change anything for two RP handles sharing a joint? | `ik.rs` (none yet) | none yet |
| Spline IK `twistType` Ease In / Ease Out / Ease In Out and `rootTwistMode`: each joint's twist (five joints along X, `twist` 90) | `ik_spline_options.rs` `ease_in_gathers_the_twist_toward_the_end` | none yet |
| Every XGen default, the commands the Generate menu and the grooming tools run, how a groom is stored in `.ma` and `.mb` | XJEN, all steps | `xgen_capture.py` |

Add a row, and a call in `calls.json`, whenever a test takes a guess.
