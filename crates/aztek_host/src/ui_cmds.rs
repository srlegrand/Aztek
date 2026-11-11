//! Commands that belong to the application rather than the scene: optionVar, runTimeCommand, nameCommand, hotkey,
//! selectType, selectPref, headsUpDisplay, lookThru, isolateSelect, workspace, toggle and cleanUpScene. State lives here;
//! requests the Qt shell must carry out go through the UI request list as "name|payload" strings.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use aztek_ops::{fail, ArgType, CommandDef, FlagDef};
use aztek_scene::graph::Graph;
use aztek_scene::plug::{NodeId, NO_NODE};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

#[derive(Default)]
struct Ui {
    /// Mask categories switched off: polymesh, curve, joint, ikHandle, deformer, locator, camera, group.
    masks_off: BTreeSet<String>,
    /// 0 hierarchy (roots), 1 object, 2 component.
    scope: i32,
    lock_selection: bool,
    prefs: Map<String, Value>,
    option_vars: Map<String, Value>,
    loaded: bool,
    run_time: BTreeMap<String, Value>,
    name_commands: BTreeMap<String, Value>,
    huds: BTreeMap<String, Value>,
    /// Which of the built-in heads up displays are on.
    hud_on: BTreeSet<String>,
    /// What `hotkey` has bound, for hotkeyCheck.
    hotkeys: Vec<Value>,
    workspace: String,
    /// Per-node display toggles the Display menu sets: localAxis, rotatePivot, scalePivot, selectHandle.
    node_flags: BTreeMap<String, HashSet<NodeId>>,
}

static UI: Mutex<Option<Ui>> = Mutex::new(None);

fn with_ui<R>(f: impl FnOnce(&mut Ui) -> R) -> R {
    let mut g = UI.lock().unwrap();
    let u = g.get_or_insert_with(|| Ui { scope: 1, hud_on: ["viewAxis".to_string(), "cameraNames".to_string()].into_iter().collect(), ..Default::default() });
    f(u)
}

fn option_var_file() -> PathBuf {
    let base = std::env::var_os("AZTEK_PREFS").map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default();
        PathBuf::from(home).join(".aztek")
    });
    base.join("optionVars.json")
}

fn load_option_vars(u: &mut Ui) {
    if u.loaded {
        return;
    }
    u.loaded = true;
    if let Ok(t) = std::fs::read_to_string(option_var_file()) {
        if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&t) {
            u.option_vars = m;
        }
    }
}

fn save_option_vars(u: &Ui) {
    let p = option_var_file();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, Value::Object(u.option_vars.clone()).to_string());
}

/// Does the selection mask let this category be picked?
pub fn mask_allows(cat: &str) -> bool {
    with_ui(|u| !u.masks_off.contains(cat))
}

/// What a pick selects, by the selection scope: the root of the hierarchy, or the node itself.
pub fn scoped(g: &Graph, id: NodeId) -> NodeId {
    if with_ui(|u| u.scope) != 0 {
        return id;
    }
    let mut n = id;
    loop {
        let p = g.parent(n);
        if p == NO_NODE {
            return n;
        }
        n = p;
    }
}

pub fn selection_locked() -> bool {
    with_ui(|u| u.lock_selection)
}

thread_local! {
    static KEEP_PREFS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether optionVars are saved between sessions: the application turns it on; tests and other embedders leave the
/// user's preferences alone.
pub fn keeps_prefs() -> bool {
    KEEP_PREFS.with(std::cell::Cell::get)
}

pub fn set_keeps_prefs(on: bool) {
    KEEP_PREFS.with(|c| c.set(on));
}

/// The optionVars saved by the last session.
pub fn saved_option_vars() -> std::collections::BTreeMap<String, Value> {
    std::fs::read_to_string(option_var_file())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .map(|m| m.into_iter().collect())
        .unwrap_or_default()
}

/// Saves the optionVars for the next session, when preferences are kept.
pub fn save_option_var_map(vars: &std::collections::BTreeMap<String, Value>) {
    if !keeps_prefs() {
        return;
    }
    let p = option_var_file();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let m: Map<String, Value> = vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let _ = std::fs::write(p, Value::Object(m).to_string());
}

/// What the preferences say before the first scene: the up axis (`upAxisDirection`) and the units a new scene starts
/// in (`workingUnitLinearDefault`, `workingUnitAngularDefault`, `workingUnitTimeDefault`), as the reference reads them
/// from its preferences at start-up.
pub fn apply_startup_prefs(vars: &std::collections::BTreeMap<String, Value>) {
    let text = |k: &str| vars.get(k).and_then(|v| v.as_str().map(str::to_string));
    if let Some(a) = text("upAxisDirection") {
        aztek_scene::world::set_up_z(a.eq_ignore_ascii_case("z"));
    }
    let lin = text("workingUnitLinearDefault").and_then(|u| aztek_ops::units::linear_factor(&u));
    let ang = text("workingUnitAngularDefault").and_then(|u| aztek_ops::units::angle_factor(&u));
    aztek_ops::units::set_defaults(lin.unwrap_or(0.0), ang.unwrap_or(0.0));
    aztek_ops::units::reset();
    if let Some(t) = text("workingUnitTimeDefault") {
        if aztek_scene::anim_time::set_default_time_unit(&t) {
            aztek_scene::anim_time::set_time_unit(&t);
        }
    }
}

pub fn pref_bool(name: &str, fallback: bool) -> bool {
    // From MEL a flag arrives as 0 or 1, in a number or a word.
    let truth = |v: &Value| match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_f64().map(|x| x != 0.0),
        Value::String(t) => match t.as_str() {
            "1" | "true" | "on" | "yes" => Some(true),
            "0" | "false" | "off" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    };
    with_ui(|u| u.prefs.get(name).and_then(truth).unwrap_or(fallback))
}

pub fn pref_f64(name: &str, fallback: f64) -> f64 {
    with_ui(|u| u.prefs.get(name).and_then(Value::as_f64).unwrap_or(fallback))
}

pub fn set_pref(name: &str, value: Value) {
    with_ui(|u| {
        u.prefs.insert(name.to_string(), value);
    });
}

pub fn set_hud(name: &str, on: bool) {
    with_ui(|u| {
        if on {
            u.hud_on.insert(name.to_string());
        } else {
            u.hud_on.remove(name);
        }
    });
}

pub fn hud_on(name: &str) -> bool {
    with_ui(|u| u.hud_on.contains(name))
}

/// The scripted heads up displays, as (section, block, label, command).
/// The buttons and sliders in the views, by name, as their commands gave them.
pub fn hud_controls() -> Vec<(String, Value)> {
    with_ui(|u| u.huds.iter().filter(|(_, h)| h.get("kind").is_some() && h.get("visible").and_then(Value::as_bool).unwrap_or(true)).map(|(n, h)| (n.clone(), h.clone())).collect())
}

/// A slider dragged in a view.
pub fn hud_set_value(name: &str, v: f64) {
    with_ui(|u| {
        if let Some(h) = u.huds.get_mut(name) {
            h["value"] = json!(v);
        }
    });
}

/// hudButton, hudSlider and hudSliderButton: one record in the heads up display's list, so -rem, -ex and -lh of
/// headsUpDisplay see them, as in the reference.
fn hud_control(kind: &'static str, a: &aztek_ops::Args) -> aztek_ops::CmdResult {
    let name = a.pos_str(0).map(str::to_string);
    if a.bool("exists") && !a.bool("query") {
        return Ok(json!(name.is_some_and(|n| with_ui(|u| u.huds.get(&n).is_some_and(|h| h["kind"] == kind)))));
    }
    let mut flags = a.flags.clone();
    flags.retain(|k, v| !matches!(k.as_str(), "query" | "edit" | "queried" | "exists") && !v.is_null());
    for (k, v) in flags.iter_mut() {
        if crate::scriptui::is_callback(k) {
            *v = crate::scriptui::callback_value(v);
        }
    }
    if a.bool("query") {
        let Some(n) = name else { fail!("{kind}: name it.") };
        let q = a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| flags.keys().next().cloned());
        let Some(q) = q else { fail!("{kind}: name what to ask about.") };
        return with_ui(|u| {
            let Some(h) = u.huds.get(&n) else { fail!("{kind}: Object '{n}' not found.") };
            Ok(h.get(&q).cloned().unwrap_or_else(|| match q.as_str() {
                "visible" => json!(true),
                "section" | "block" => json!(0),
                "minValue" | "value" => json!(0.0),
                "maxValue" => json!(100.0),
                _ => Value::Null,
            }))
        });
    }
    let Some(n) = name else { fail!("{kind}: name it.") };
    with_ui(|u| {
        if a.bool("edit") {
            let Some(h) = u.huds.get_mut(&n).and_then(Value::as_object_mut) else { fail!("{kind}: Object '{n}' not found.") };
            h.extend(flags);
        } else {
            if u.huds.contains_key(&n) {
                fail!("{kind}: Object's name '{n}' is not unique.");
            }
            flags.insert("kind".to_string(), json!(kind));
            u.huds.insert(n.clone(), Value::Object(flags));
        }
        Ok(json!(n))
    })
}

pub fn custom_huds() -> Vec<(i64, i64, String, String)> {
    with_ui(|u| {
        u.huds
            .values()
            .filter(|h| h.get("kind").is_none() && h.get("visible").and_then(Value::as_bool).unwrap_or(true))
            .map(|h| {
                (
                    h.get("section").and_then(Value::as_i64).unwrap_or(0),
                    h.get("block").and_then(Value::as_i64).unwrap_or(0),
                    h.get("label").and_then(Value::as_str).unwrap_or("").to_string(),
                    h.get("command").and_then(Value::as_str).unwrap_or("").to_string(),
                )
            })
            .collect()
    })
}

pub fn node_flag(flag: &str, id: NodeId) -> bool {
    with_ui(|u| u.node_flags.get(flag).is_some_and(|s| s.contains(&id)))
}

/// The nodes a display toggle is on for.
pub fn node_flag_nodes(flag: &str) -> Vec<NodeId> {
    with_ui(|u| u.node_flags.get(flag).map(|s| s.iter().copied().collect()).unwrap_or_default())
}

/// JSON the shell reads: the runtime commands, for the Hotkey Editor.
pub fn runtime_commands() -> Value {
    with_ui(|u| {
        json!({
            "runTime": u.run_time.values().cloned().collect::<Vec<_>>(),
            "nameCommands": u.name_commands.values().cloned().collect::<Vec<_>>(),
        })
    })
}

/// The Qt shell's named actions, callable from MEL and Python as the reference's runtime commands are. Names already taken stay
/// as they are.
pub fn register_ui_names(s: &mut Session, list: &Value) {
    for it in list.as_array().into_iter().flatten() {
        let Some(name) = it.get(0).and_then(Value::as_str) else { continue };
        if s.commands.find(name).is_some() {
            continue;
        }
        let doc = it.get(1).and_then(Value::as_str).unwrap_or("");
        let n = name.to_string();
        s.register(
            Cmd::new(name, doc, move |_s, _a| {
                crate::core::push_ui_request(n.clone());
                Ok(Value::Null)
            })
            .undoable(false),
        );
    }
}

fn reg(s: &mut Session, c: Cmd) {
    // A command the scene layer already has wins.
    if s.commands.find(&c.name).is_none() {
        s.register(c);
    }
}

pub fn register(s: &mut Session) {
    reg(
        s,
        Cmd::new(
            "newSceneUnits",
            "The units a new scene starts in, as File > New Scene's options set them: -linear, -angle, -time. -q gives them.",
            |_s, a| {
                use aztek_ops::units;
                if a.bool("query") {
                    let name = |f: f64| units::LINEAR_UNITS.iter().find(|u| (u.2 - f).abs() < 1e-12).map(|u| u.0).unwrap_or("cm");
                    let ang = if (units::default_rad_per_unit() - 1.0).abs() < 1e-12 { "rad" } else { "deg" };
                    return Ok(json!({"linear": name(units::default_cm_per_unit()), "angle": ang, "time": aztek_scene::anim_time::default_time_unit()}));
                }
                let lin = match a.get("linear").and_then(Value::as_str) {
                    Some(u) => units::linear_factor(u).ok_or_else(|| aztek_ops::CommandError::new(format!("newSceneUnits: unknown linear unit '{u}'")))?,
                    None => 0.0,
                };
                let ang = match a.get("angle").and_then(Value::as_str) {
                    Some(u) => units::angle_factor(u).ok_or_else(|| aztek_ops::CommandError::new(format!("newSceneUnits: unknown angle unit '{u}'")))?,
                    None => 0.0,
                };
                if let Some(t) = a.get("time").and_then(Value::as_str) {
                    if !aztek_scene::anim_time::set_default_time_unit(t) {
                        fail!("newSceneUnits: unknown time unit '{t}'");
                    }
                }
                units::set_defaults(lin, ang);
                Ok(Value::Null)
            },
        )
        .flag(f("linear", "l", ArgType::String, "Linear unit."))
        .flag(f("angle", "a", ArgType::String, "Angle unit."))
        .flag(f("time", "t", ArgType::String, "Time unit."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .undoable(false),
    );
    reg(
        s,
        Cmd::new("optionVar", "Named preference values kept between sessions. -iv, -fv, -sv set; -q reads; -ex tests; -rm removes.", |_s, a| {
            with_ui(|u| {
                load_option_vars(u);
                let mut changed = false;
                let pair = |v: &Value| -> Option<(String, Value)> {
                    let arr = v.as_array()?;
                    Some((arr.first()?.as_str()?.to_string(), arr.get(1)?.clone()))
                };
                for flag in ["intValue", "floatValue", "stringValue"] {
                    if let Some((k, v)) = a.get(flag).and_then(pair) {
                        u.option_vars.insert(k, v);
                        changed = true;
                    }
                }
                if let Some((k, v)) = a.get("stringValueAppend").and_then(pair) {
                    let e = u.option_vars.entry(k).or_insert(json!([]));
                    if !e.is_array() {
                        *e = json!([e.clone()]);
                    }
                    e.as_array_mut().unwrap().push(v);
                    changed = true;
                }
                if let Some(k) = a.get("remove").and_then(Value::as_str) {
                    u.option_vars.remove(k);
                    changed = true;
                }
                if changed {
                    save_option_vars(u);
                    return Ok(Value::Null);
                }
                if let Some(k) = a.get("exists").and_then(Value::as_str) {
                    return Ok(json!(u.option_vars.contains_key(k)));
                }
                if a.bool("list") {
                    return Ok(json!(u.option_vars.keys().cloned().collect::<Vec<_>>()));
                }
                if let Some(k) = a.pos_str(0) {
                    return Ok(u.option_vars.get(k).cloned().unwrap_or(json!(0)));
                }
                Ok(Value::Null)
            })
        })
        .positional("name")
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool, "Read the value named."))
        .flag(f("intValue", "iv", ArgType::Any, "Name and integer value.").multi(2))
        .flag(f("floatValue", "fv", ArgType::Any, "Name and float value.").multi(2))
        .flag(f("stringValue", "sv", ArgType::Any, "Name and string value.").multi(2))
        .flag(f("stringValueAppend", "sva", ArgType::Any, "Name and a string to add to its array.").multi(2))
        .flag(f("exists", "ex", ArgType::String, "Whether the name is set."))
        .flag(f("remove", "rm", ArgType::String, "Remove the name."))
        .flag(f("list", "l", ArgType::Bool, "Every name.")),
    );

    reg(
        s,
        Cmd::new("runTimeCommand", "Create a named command that runs a script: -c the script, -ann its annotation, -cat its category, -cl mel or python.", |s, a| {
            let Some(name) = a.pos_str(0).map(str::to_string) else {
                return Ok(with_ui(|u| json!(u.run_time.keys().cloned().collect::<Vec<_>>())));
            };
            if a.bool("exists") {
                return Ok(json!(with_ui(|u| u.run_time.contains_key(&name))));
            }
            if a.bool("query") {
                return Ok(with_ui(|u| {
                    let e = u.run_time.get(&name).cloned().unwrap_or(json!({}));
                    if a.bool("command") { e["command"].clone() } else if a.bool("annotation") { e["annotation"].clone() } else if a.bool("category") { e["category"].clone() } else { e }
                }));
            }
            if a.bool("delete") {
                with_ui(|u| u.run_time.remove(&name));
                s.commands.unregister(&name);
                return Ok(Value::Null);
            }
            let cmd = a.string("command", "");
            let lang = a.string("commandLanguage", "mel");
            let entry = json!({"name": name, "command": cmd, "annotation": a.string("annotation", ""), "category": a.string("category", "User"), "language": lang, "default": a.bool("default")});
            with_ui(|u| u.run_time.insert(name.clone(), entry));
            let doc = a.string("annotation", "");
            let code = format!("{}|{}", if lang == "python" { "py" } else { "mel" }, cmd);
            s.register(
                Cmd::new(&name, &doc, move |_s, _a| {
                    crate::core::push_ui_request(code.clone());
                    Ok(Value::Null)
                })
                .undoable(false),
            );
            Ok(json!(name))
        })
        .positional("name")
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool, "Query."))
        .flag(f("edit", "e", ArgType::Bool, "Edit."))
        .flag(f("exists", "ex", ArgType::Bool, "Whether the command exists."))
        .flag(f("delete", "del", ArgType::Bool, "Delete it."))
        .flag(f("command", "c", ArgType::Any, "The script."))
        .flag(f("annotation", "ann", ArgType::Any, "What it does."))
        .flag(f("category", "cat", ArgType::Any, "Its category in the Hotkey Editor."))
        .flag(f("commandLanguage", "cl", ArgType::String, "mel or python."))
        .flag(f("default", "d", ArgType::Bool, "A factory command.")),
    );

    reg(
        s,
        Cmd::new("nameCommand", "Create a name a hotkey can run: -c the command, -ann its annotation.", |_s, a| {
            let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("nameCommand: give a name") };
            let e = json!({"name": name, "command": a.string("command", ""), "annotation": a.string("annotation", ""), "language": a.string("sourceType", "mel")});
            with_ui(|u| u.name_commands.insert(name.clone(), e));
            Ok(json!(name))
        })
        .positional("name")
        .undoable(false)
        .flag(f("command", "c", ArgType::String, "The command."))
        .flag(f("annotation", "ann", ArgType::String, "What it does."))
        .flag(f("sourceType", "stp", ArgType::String, "mel or python.")),
    );

    reg(
        s,
        Cmd::new("hotkeyCheck", "The annotation of the nameCommand a key is bound to, or an empty string: -k the key, -alt, -ctl, -cmd, -opt modifiers, -kup for release. Less than the reference: it sees keys bound with the hotkey command, not the built-in table the shell keeps.", |_s, a| {
            let key = a.string("keyString", "");
            if key.is_empty() {
                fail!("hotkeyCheck: give -k");
            }
            let (ctrl, alt) = (a.bool("ctrlModifier") || a.bool("commandModifier"), a.bool("altModifier") || a.bool("optionModifier"));
            let release = a.bool("keyUp");
            Ok(with_ui(|u| {
                let found = u.hotkeys.iter().find(|x| x["key"] == key.as_str() && x["ctrl"] == ctrl && x["alt"] == alt);
                let name = found.and_then(|x| x[if release { "releaseName" } else { "name" }].as_str()).unwrap_or("").to_string();
                let ann = u.name_commands.get(&name).and_then(|e| e["annotation"].as_str()).unwrap_or("").to_string();
                json!(ann)
            }))
        })
        .flag(f("altModifier", "alt", ArgType::Bool, "With Alt."))
        .flag(f("commandModifier", "cmd", ArgType::Bool, "With Command: as Ctrl here."))
        .flag(f("ctrlModifier", "ctl", ArgType::Bool, "With Ctrl."))
        .flag(f("isRepeatable", "ir", ArgType::Bool, "Accepted for existing scripts; has no effect."))
        .flag(f("keyString", "k", ArgType::String, "The key."))
        .flag(f("keyUp", "kup", ArgType::Bool, "On release."))
        .flag(f("optionModifier", "opt", ArgType::Bool, "With Option: as Alt here."))
        .undoable(false),
    );

    reg(
        s,
        Cmd::new("hotkey", "Bind a key: -k the key, -alt -ctl -sht modifiers, -n the name command for press, -rn for release.", |_s, a| {
            let key = a.string("keyShortcut", "");
            if key.is_empty() {
                fail!("hotkey: give -k");
            }
            if a.bool("query") {
                let release = a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| matches!(x.as_str(), Some("releaseName" | "rn"))));
                let found = with_ui(|u| {
                    u.hotkeys
                        .iter()
                        .find(|x| x["key"] == json!(key) && x["ctrl"] == json!(a.bool("ctrlModifier")) && x["alt"] == json!(a.bool("altModifier")) && x["shift"] == json!(a.bool("shiftModifier")))
                        .cloned()
                });
                let field = if release { "releaseName" } else { "name" };
                return Ok(found.map_or(json!(""), |h| h[field].clone()));
            }
            let hk = json!({
                "key": key,
                "ctrl": a.bool("ctrlModifier"),
                "alt": a.bool("altModifier"),
                "shift": a.bool("shiftModifier"),
                "name": a.string("name", ""),
                "releaseName": a.string("releaseName", ""),
            });
            with_ui(|u| {
                u.hotkeys.retain(|x| !(x["key"] == hk["key"] && x["ctrl"] == hk["ctrl"] && x["alt"] == hk["alt"] && x["shift"] == hk["shift"]));
                u.hotkeys.push(hk.clone());
            });
            crate::core::push_ui_request(format!("hotkey|{hk}"));
            Ok(Value::Null)
        })
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool, "Ask what the key is bound to: -n for press, -rn for release."))
        .flag(f("queried", "", ArgType::Any, "What a query asks for."))
        .flag(f("keyShortcut", "k", ArgType::String, "The key."))
        .flag(f("altModifier", "alt", ArgType::Bool, "With Alt."))
        .flag(f("ctrlModifier", "ctl", ArgType::Bool, "With Ctrl."))
        .flag(f("shiftModifier", "sht", ArgType::Bool, "With Shift."))
        .flag(f("name", "n", ArgType::String, "The name command for press."))
        .flag(f("releaseName", "rn", ArgType::String, "The name command for release.")),
    );

    let mut st = Cmd::new("selectType", "Selection masks: -polymesh, -joint, -nurbsCurve, -ikHandle, -locator, -cluster, -camera and so on, each on or off; -cv, -v, -pv points, -pe edges, -pf faces pick the component kind. -q reads one.", |s, a| {
        use aztek_scene::components::{self, ComponentMode};
        let kinds = [("controlVertex", ComponentMode::Vertex), ("vertex", ComponentMode::Vertex), ("polymeshVertex", ComponentMode::Vertex), ("polymeshEdge", ComponentMode::Edge), ("polymeshFace", ComponentMode::Face)];
        for (flag, mode) in kinds {
            if a.has(flag) {
                if a.bool("query") {
                    return Ok(json!(components::mode(&s.graph) == mode));
                }
                if a.bool(flag) {
                    components::set_mode(&mut s.graph, mode);
                }
            }
        }
        with_ui(|u| {
            let names = [
                ("polymesh", "polymesh"),
                ("nurbsCurve", "curve"),
                ("nurbsSurface", "surface"),
                ("joint", "joint"),
                ("ikHandle", "ikHandle"),
                ("handle", "ikHandle"),
                ("cluster", "deformer"),
                ("nonlinear", "deformer"),
                ("locator", "locator"),
                ("camera", "camera"),
                ("group", "group"),
            ];
            let query = a.bool("query");
            if a.bool("allObjects") {
                u.masks_off.clear();
            }
            for (flag, cat) in names {
                if let Some(v) = a.get(flag) {
                    if query {
                        return Ok(json!(!u.masks_off.contains(cat)));
                    }
                    if v.as_bool().unwrap_or(false) {
                        u.masks_off.remove(cat);
                    } else {
                        u.masks_off.insert(cat.to_string());
                    }
                }
            }
            Ok(Value::Null)
        })
    })
    .undoable(false)
    .flag(f("query", "q", ArgType::Bool, "Query."))
    .flag(f("allObjects", "alc", ArgType::Bool, "Everything on."));
    for (n, sh) in [("polymesh", "pm"), ("nurbsCurve", "nc"), ("nurbsSurface", "ns"), ("joint", "j"), ("ikHandle", "ikh"), ("handle", "ha"), ("cluster", "cl"), ("nonlinear", "nl"), ("locator", "lc"), ("camera", "ca"), ("group", "gr")] {
        st = st.flag(f(n, sh, ArgType::Bool, "This kind can be selected."));
    }
    for (n, sh, doc) in [("controlVertex", "cv", "CVs."), ("vertex", "v", "Points."), ("polymeshVertex", "pv", "Mesh vertices."), ("polymeshEdge", "pe", "Mesh edges."), ("polymeshFace", "pf", "Mesh faces.")] {
        st = st.flag(f(n, sh, ArgType::Bool, doc));
    }
    s.register(st);

    reg(
        s,
        Cmd::new("selectScope", "Status line scope: hierarchy (roots), object or component. No argument returns it.", |s, a| {
            if let Some(v) = a.pos_str(0) {
                let n = match v {
                    "hierarchy" => 0,
                    "component" => 2,
                    _ => 1,
                };
                with_ui(|u| u.scope = n);
                if n != 2 {
                    let _ = s.run("selectMode", json!({"args": ["object"]}));
                }
            }
            Ok(json!(["hierarchy", "object", "component"][with_ui(|u| u.scope) as usize]))
        })
        .positional("[scope]")
        .undoable(false),
    );

    reg(
        s,
        Cmd::new("selectPref", "Selection preferences: -clickDrag, -trackSelectionOrder, -preSelectHilite, -xformNoSelect, -useDepth, -affectsActive, -lockSelection. -q reads.", |_s, a| {
            with_ui(|u| {
                let query = a.bool("query");
                let mut out = Value::Null;
                for name in ["clickDrag", "trackSelectionOrder", "preSelectHilite", "xformNoSelect", "useDepth", "affectsActive", "lockSelection"] {
                    if let Some(v) = a.get(name) {
                        if query {
                            out = if name == "lockSelection" { json!(u.lock_selection) } else { u.prefs.get(name).cloned().unwrap_or(json!(false)) };
                        } else if name == "lockSelection" {
                            u.lock_selection = v.as_bool().unwrap_or(false);
                        } else {
                            u.prefs.insert(name.to_string(), v.clone());
                        }
                    }
                }
                Ok(out)
            })
        })
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool, "Query."))
        .flag(f("clickDrag", "cld", ArgType::Bool, "Click and drag to select or to move what's under the cursor."))
        .flag(f("trackSelectionOrder", "tso", ArgType::Bool, "Keep the order things were picked in."))
        .flag(f("preSelectHilite", "psh", ArgType::Bool, "Highlight what's under the cursor."))
        .flag(f("xformNoSelect", "xns", ArgType::Bool, "A click on an object under the transform tools doesn't reselect."))
        .flag(f("useDepth", "ud", ArgType::Bool, "Marquee only picks what's visible."))
        .flag(f("affectsActive", "aa", ArgType::Bool, "Selection affects the active list."))
        .flag(f("lockSelection", "lck", ArgType::Bool, "The selection doesn't change on a click.")),
    );

    const HUD_COMMON: &str = "section:s:a block:b:a blockSize:bs:a blockAlignment:ba:a padding:p:a visible:vis:b allowOverlap:ao:b";
    const HUD_BUTTON: &str = "label:l:a labelFontSize:lfs:a buttonWidth:bw:a buttonShape:bsh:a pressCommand:pc:a releaseCommand:rc:a";
    const HUD_SLIDER: &str = "label:l:a labelFontSize:lfs:a labelWidth:lw:a type:t:a value:v:a minValue:min:a maxValue:max:a valueAlignment:va:a valueFontSize:vfs:a valueWidth:vw:a sliderLength:sl:a sliderIncrement:si:a decimalPrecision:dp:a internalPadding:ip:a showFrame:sf:b dragCommand:dc:a pressCommand:pc:a releaseCommand:rc:a";
    const HUD_SLIDER_BUTTON: &str = "sliderLabel:sl:a sliderLabelFontSize:slf:a sliderLabelWidth:slw:a type:t:a value:v:a minValue:min:a maxValue:max:a valueAlignment:va:a valueFontSize:vfs:a valueWidth:vw:a sliderLength:sln:a sliderIncrement:si:a decimalPrecision:dp:a internalPadding:ip:a sliderDragCommand:sdc:a sliderPressCommand:spc:a sliderReleaseCommand:src:a buttonLabel:bl:a buttonLabelFontSize:blf:a buttonWidth:bw:a buttonShape:bsh:a buttonPressCommand:bpc:a buttonReleaseCommand:brc:a";
    for (kind, spec) in [("hudButton", HUD_BUTTON), ("hudSlider", HUD_SLIDER), ("hudSliderButton", HUD_SLIDER_BUTTON)] {
        let mut c = Cmd::new(kind, "A control in the heads up display: -section, -block, its label, value and commands. headsUpDisplay -rem removes it.", move |_s, a| hud_control(kind, a)).positional("name").undoable(false);
        c = crate::scriptui::add_flags(c, spec);
        c = crate::scriptui::add_flags(c, HUD_COMMON);
        c = crate::scriptui::add_flags(c, "exists:ex:b query:q:b edit:e:b queried::a");
        reg(s, c);
    }

    reg(
        s,
        Cmd::new("headsUpDisplay", "Text in the views: -s section -b block -l label -c command; -rem removes; -ex tests; -lh lists.", |_s, a| {
            let name = a.pos_str(0).map(str::to_string);
            with_ui(|u| {
                if a.bool("listHeadsUpDisplays") {
                    return Ok(json!(u.huds.keys().cloned().collect::<Vec<_>>()));
                }
                if let Some(rm) = a.get("remove") {
                    if let Some(n) = rm.as_str().map(str::to_string).or(name.clone()) {
                        u.huds.remove(&n);
                    }
                    return Ok(Value::Null);
                }
                let Some(n) = name else { return Ok(Value::Null) };
                if a.bool("exists") {
                    return Ok(json!(u.huds.contains_key(&n) || u.hud_on.contains(&n)));
                }
                if a.has("visible") && !a.has("section") && !u.huds.contains_key(&n) {
                    if a.bool("visible") {
                        u.hud_on.insert(n);
                    } else {
                        u.hud_on.remove(&n);
                    }
                    return Ok(Value::Null);
                }
                let e = u.huds.entry(n).or_insert(json!({}));
                for (k, ty) in [("section", 0), ("block", 0), ("label", 1), ("command", 1), ("visible", 2)] {
                    if a.has(k) {
                        e[k] = match ty {
                            0 => json!(a.int(k, 0)),
                            1 => json!(a.string(k, "")),
                            _ => json!(a.bool(k)),
                        };
                    }
                }
                Ok(Value::Null)
            })
        })
        .positional("name")
        .undoable(false)
        .flag(f("section", "s", ArgType::Int, "Screen section, 0 to 9."))
        .flag(f("block", "b", ArgType::Int, "Row in the section."))
        .flag(f("label", "l", ArgType::String, "Label text."))
        .flag(f("command", "c", ArgType::String, "MEL giving the value."))
        .flag(f("visible", "vis", ArgType::Bool, "Show it."))
        .flag(f("remove", "rem", ArgType::Any, "Remove it."))
        .flag(f("exists", "ex", ArgType::Bool, "Whether it exists."))
        .flag(f("listHeadsUpDisplays", "lh", ArgType::Bool, "List them.")),
    );

    reg(
        s,
        Cmd::new("lookThru", "Make the active view look through a camera. With no camera, returns the one the view looks through.", |_s, a| {
            if let Some(c) = a.pos_str(0) {
                crate::core::push_ui_request(format!("lookThru|{c}"));
            } else if let Some(n) = a.get("objects").and_then(Value::as_str) {
                crate::core::push_ui_request(format!("lookThru|{n}"));
            }
            Ok(Value::Null)
        })
        .positional("camera")
        .undoable(false)
        .flag(f("objects", "o", ArgType::String, "The camera.")),
    );

    reg(
        s,
        Cmd::new("isolateSelect", "Isolate Select on the active view: -state on or off, -addSelected, -removeSelected, -update.", |_s, a| {
            let mut p = Map::new();
            for k in ["state", "addSelected", "removeSelected", "update"] {
                if let Some(v) = a.get(k) {
                    p.insert(k.to_string(), v.clone());
                }
            }
            crate::core::push_ui_request(format!("isolate|{}", Value::Object(p)));
            Ok(Value::Null)
        })
        .positional("panel")
        .undoable(false)
        .flag(f("state", "s", ArgType::Bool, "On or off."))
        .flag(f("addSelected", "as", ArgType::Bool, "Add the selection."))
        .flag(f("removeSelected", "rs", ArgType::Bool, "Take the selection out."))
        .flag(f("update", "u", ArgType::Bool, "Show the selection.")),
    );

    reg(
        s,
        Cmd::new("workspace", "The project: -o opens a folder as the project, -q -rd or -q -fn gives its root, -cr creates the standard folders.", |_s, a| {
            let root = with_ui(|u| u.workspace.clone());
            if let Some(d) = a.get("openWorkspace").and_then(Value::as_str).map(str::to_string).or_else(|| a.pos_str(0).filter(|_| a.bool("openWorkspace")).map(str::to_string)) {
                with_ui(|u| u.workspace = d.clone());
                return Ok(json!(d));
            }
            if let Some(d) = a.get("create").and_then(Value::as_str) {
                for sub in ["scenes", "scripts", "images", "sourceimages", "data"] {
                    if let Err(e) = std::fs::create_dir_all(PathBuf::from(d).join(sub)) {
                        fail!("workspace: {d}: {e}");
                    }
                }
                return Ok(json!(d));
            }
            let dir = if root.is_empty() { std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default() } else { root };
            Ok(json!(dir))
        })
        .positional("[folder]")
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool, "Query."))
        .flag(f("rootDirectory", "rd", ArgType::Bool, "The project's folder."))
        .flag(f("fullName", "fn", ArgType::Bool, "The project's folder."))
        .flag(f("openWorkspace", "o", ArgType::Any, "Open this folder as the project."))
        .flag(f("create", "cr", ArgType::String, "Create the project's folders here.")),
    );

    reg(
        s,
        Cmd::new("toggle", "Switch a display toggle on the selection: -localAxis, -rotatePivot, -scalePivot, -selectHandle, -template, with -state to set it.", |s, a| {
            let g = &mut s.graph;
            let targets: Vec<NodeId> = {
                let names = a.named();
                if names.is_empty() {
                    g.selection().to_vec()
                } else {
                    names.iter().filter_map(|n| g.find(n).ok().flatten()).collect()
                }
            };
            for flag in ["localAxis", "rotatePivot", "scalePivot", "selectHandle", "boundingBox"] {
                if !a.bool(flag) {
                    continue;
                }
                with_ui(|u| {
                    let e = u.node_flags.entry(flag.to_string()).or_default();
                    for &id in &targets {
                        let on = if a.has("state") { a.bool("state") } else { !e.contains(&id) };
                        if on {
                            e.insert(id);
                        } else {
                            e.remove(&id);
                        }
                    }
                });
            }
            if a.bool("template") {
                for &id in &targets {
                    let tmp = aztek_scene::transform::DagNode::TEMPLATE;
                    let cur = g.node(id).is_some_and(|n| n.ty().dag && n.bool(tmp));
                    let on = if a.has("state") { a.bool("state") } else { !cur };
                    let _ = g.set_value(aztek_scene::plug::PlugRef::new(id, tmp as i32), aztek_scene::plug::PlugValue::Bool(on));
                }
            }
            Ok(Value::Null)
        })
        .positional("objects")
        .flag(f("localAxis", "la", ArgType::Bool, "The local axes."))
        .flag(f("rotatePivot", "rp", ArgType::Bool, "The rotate pivot."))
        .flag(f("scalePivot", "sp", ArgType::Bool, "The scale pivot."))
        .flag(f("selectHandle", "sh", ArgType::Bool, "The selection handle."))
        .flag(f("boundingBox", "bb", ArgType::Bool, "Draw as a box."))
        .flag(f("template", "te", ArgType::Bool, "Template display."))
        .flag(f("state", "st", ArgType::Bool, "On or off rather than a switch.")),
    );

    reg(
        s,
        Cmd::new("polyOptions", "Polygon display in the views: -db borders, -dce crease edges, -dn with -f or -pt normals, -sn their size, -bc backface culling.", |_s, a| {
            let mut p = Map::new();
            let on = |k: &str| a.get(k).and_then(Value::as_bool);
            if let Some(v) = on("displayBorder") {
                p.insert("borderEdges".into(), json!(v));
            }
            if let Some(v) = on("displayCreaseEdge") {
                p.insert("creaseEdges".into(), json!(v));
            }
            if let Some(v) = on("displayNormal") {
                p.insert(if a.bool("point") { "vertexNormals" } else { "faceNormals" }.into(), json!(v));
            }
            if let Some(v) = on("backCulling") {
                p.insert("backface".into(), json!(v));
            }
            if a.has("sizeNormal") {
                p.insert("normalSize".into(), json!(a.float("sizeNormal", 0.4)));
            }
            crate::core::push_ui_request(format!("polyOptions|{}", Value::Object(p)));
            Ok(Value::Null)
        })
        .undoable(false)
        .flag(f("displayBorder", "db", ArgType::Bool, "Border edges thick."))
        .flag(f("displayCreaseEdge", "dce", ArgType::Bool, "Crease edges."))
        .flag(f("displayNormal", "dn", ArgType::Bool, "Normals."))
        .flag(f("facet", "f", ArgType::Bool, "With -dn: face normals."))
        .flag(f("point", "pt", ArgType::Bool, "With -dn: vertex normals."))
        .flag(f("sizeNormal", "sn", ArgType::Float, "Length of the normals."))
        .flag(f("backCulling", "bc", ArgType::Bool, "Backface culling.")),
    );

    reg(
        s,
        Cmd::new("uiPref", "An interface setting the views read: uiPref name value. No value returns it.", |_s, a| {
            let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("uiPref: give a name") };
            let list = a.get("args").and_then(Value::as_array).cloned().unwrap_or_else(|| a.positional.clone());
            with_ui(|u| {
                if let Some(v) = list.get(1) {
                    u.prefs.insert(name, v.clone());
                    Ok(Value::Null)
                } else {
                    Ok(u.prefs.get(&name).cloned().unwrap_or(Value::Null))
                }
            })
        })
        .positional("name value")
        .undoable(false),
    );

    reg(
        s,
        Cmd::new("cleanUpScene", "Optimize Scene Size essentials: removes transforms with nothing under them, deepest first. Returns what went.", |s, _a| {
            let mut gone = Vec::new();
            loop {
                let g = &s.graph;
                let found: Vec<String> = g
                    .nodes()
                    .into_iter()
                    .filter(|&id| g.node(id).is_some_and(|n| n.ty().name == "transform") && g.children(id).is_empty())
                    .map(|id| g.path(id))
                    .collect();
                if found.is_empty() {
                    break;
                }
                for p in found {
                    s.run("delete", json!({"args": [p.clone()]}))?;
                    gone.push(p);
                }
            }
            Ok(json!(gone))
        })
        .positional("[what]")
        .flag(f("emptyTransforms", "et", ArgType::Bool, "Remove transforms with nothing under them.")),
    );
}
