# Third-party material

Aztek is MIT-0. Two pieces of it come from elsewhere under the MIT licence, which asks for its notice to go with copies.

## Binary scene type ids

`crates/aztek_scene/data/mb/typeids.dat`, the four-character ids of node types in `.mb` files, is from
maya-scenefile-parser (https://github.com/mottosso/maya-scenefile-parser, also https://github.com/agibli/sansapp),
unmodified. It is built into the program.

Copyright (c) 2016 Alon Gibli. Licence: MIT, full text in `crates/aztek_scene/data/mb/LICENSE`.

## mGear's solvers

`plugins/mgear_solvers/` is the source of mGear's `mgear_solvers` plug-in (the `src/` folder of
https://github.com/mgear-dev/mgear at commit 62d9708 of 06/11/2026, after mGear 5.4.5), unmodified. It is built with
Aztek's plug-in kit and ships with the program.

Copyright (c) 2011-2018 Jeremie Passerin, Miquel Campos; 2018 onwards the mGear-Dev Organization. Licence: MIT, full
text in `plugins/mgear_solvers/LICENSE`.

## Test files

`crates/aztek_scene/tests/data/mb/` holds two `.mb` files used only by the tests, each with its MIT licence beside it;
`crates/aztek_scene/tests/data/mb/README.md` says where they are from. They are not part of the program.
