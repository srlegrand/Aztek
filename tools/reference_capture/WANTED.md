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

`crates/aztek_host/tests/reference_fixtures.rs` reads `calls/`. To refresh the type list, run
`cargo run -p aztek_host --example list_node_types > tools/reference_capture/node_types.txt`.

## What tests still need

Each is a guess in a test, marked `// unverified:` at the line. `calls.json` carries a call for each where one fits.

| Question | Test | Call |
| --- | --- | --- |
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

Add a row, and a call in `calls.json`, whenever a test takes a guess.
