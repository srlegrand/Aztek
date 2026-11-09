//! Script hooks and timing: scriptJob, scriptNode, deferred scripts, timer, stackTrace, getLastError.
//!
//! Jobs fire at idle, as in the reference: `Core::idle` calls `scan`, which compares what jobs watch with what it last saw and
//! returns the scripts due. The shell calls it on its poll; headless callers call it themselves. Commands that have no
//! state to compare (undo, redo, file operations) are noted by the core as they run.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::plug::{NodeId, PlugValue};
use aztek_scene::Session;
use serde_json::{json, Value};

use crate::scriptui::callback_value;

type Cmd = CommandDef<Session>;

#[derive(Clone, Debug)]
enum Watch {
    Event(String),
    /// A condition: on true, on false, or either way.
    Condition(String, Option<bool>),
    Attribute(String),
    Connection(String),
    AttrAdded(String),
    AttrDeleted(String),
    NodeDeleted(NodeId),
    NodeName(NodeId),
    UiDeleted(String),
    Time,
    Idle,
}

#[derive(Clone, Debug)]
struct Job {
    id: i64,
    watch: Watch,
    script: Value,
    run_once: bool,
    kill_with_scene: bool,
    protected: bool,
    permanent: bool,
    parent: Option<String>,
    /// The flag and value as a listing shows them.
    listing: String,
    last: Value,
}

#[derive(Default)]
struct Jobs {
    list: Vec<Job>,
    next: i64,
    /// The last value seen of each watched state.
    seen: HashMap<String, Value>,
    /// Events noted since the last scan.
    noted: Vec<String>,
    deferred: Vec<Value>,
    timers: HashMap<String, (Instant, Instant)>,
    stack_trace: bool,
    last_error: String,
    warned: HashSet<String>,
    warnings: Vec<String>,
}

thread_local! {
    static JOBS: RefCell<Jobs> = RefCell::new(Jobs::default());
    /// flushIdleQueue was called: the core runs an idle step when the command returns.
    static FLUSH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn with_jobs<R>(f: impl FnOnce(&mut Jobs) -> R) -> R {
    JOBS.with(|j| f(&mut j.borrow_mut()))
}

/// The reference's scriptJob -listEvents names. The ones with a probe or a note fire; the rest are accepted and stay silent.
pub const EVENTS: &[&str] = &[
    "usdStageChanged", "usdLayerChanged", "usdEditTargetChanged",
    "linearUnitChanged", "timeUnitChanged", "angularUnitChanged", "Undo", "undoSupressed", "Redo", "customEvaluatorChanged", "serialExecutorFallback", "timeChanged", "currentContainerChange", "quitApplication", "idleHigh", "idle", "idleVeryLow", "RecentCommandChanged", "ToolChanged", "PostToolChanged", "ToolDirtyChanged", "ToolSettingsChanged", "tabletModeChanged", "DisplayRGBColorChanged", "animLayerRebuild", "animLayerRefresh", "animLayerAnimationChanged", "animLayerLockChanged", "animLayerBaseLockChanged", "animLayerGhostChanged", "cteEventKeyingTargetForClipChanged", "cteEventKeyingTargetForLayerChanged", "cteEventKeyingTargetForInvalidChanged", "teClipAdded", "teClipModified", "teClipRemoved", "teCompositionAdded", "teCompositionRemoved", "teCompositionActiveChanged", "teCompositionNameChanged", "teMuteChanged", "cameraChange", "cameraDisplayAttributesChange", "SelectionChanged", "PreSelectionChangedTriggered", "LiveListChanged", "ActiveViewChanged", "SelectModeChanged", "SelectTypeChanged", "SelectPreferenceChanged", "DisplayPreferenceChanged", "DagObjectCreated", "transformLockChange", "renderLayerManagerChange", "renderLayerChange", "displayLayerManagerChange", "displayLayerAdded", "displayLayerDeleted", "displayLayerVisibilityChanged", "displayLayerChange", "renderPassChange", "renderPassSetChange", "renderPassSetMembershipChange", "passContributionMapChange", "DisplayColorChanged", "lightLinkingChanged", "lightLinkingChangedNonSG", "UvTileProxyDirtyChangeTrigger", "preferredRendererChanged", "polyTopoSymmetryValidChanged", "SceneSegmentChanged", "PostSceneSegmentChanged", "SequencerActiveShotChanged", "ColorIndexChanged", "deleteAll", "NameChanged", "symmetricModellingOptionsChanged", "softSelectOptionsChanged", "SetModified", "xformConstraintOptionsChanged", "metadataVisualStatusChanged", "undoXformCmd", "redoXformCmd", "freezeOptionsChanged", "linearToleranceChanged", "angularToleranceChanged", "nurbsToPolygonsPrefsChanged", "nurbsCurveRebuildPrefsChanged", "constructionHistoryChanged", "threadCountChanged", "SceneSaved", "NewSceneOpened", "SceneOpened", "SceneImported", "PreFileNewOrOpened", "PreFileNewOrOpenedEvent", "PostSceneRead", "workspaceChanged", "nurbsToSubdivPrefsChanged", "PreRenderLayerChange", "PostRenderLayerChange", "SculptMeshActivated", "playbackRangeChanged", "playbackRangeSliderChanged", "playbackSpeedChanged", "playbackModeChanged", "currentSoundNodeChanged", "glFrameTrigger", "currentTimeChanged", "snapModeChanged", "texWindowEditorImageBaseColorChanged", "texWindowEditorCheckerDensityChanged", "texWindowEditorCheckerDisplayChanged", "texWindowEditorDisplaySolidMapChanged", "texWindowEditorShowup", "texWindowEditorClose", "profilerSelectionChanged", "quitApplication2", "PostToolChanged2", "DragRelease", "ModelPanelSetFocus", "modelEditorChanged", "MenuModeChanged", "gridDisplayChanged", "interactionStyleChanged", "axisAtOriginChanged", "CurveRGBColorChanged", "SelectPriorityChanged", "snapModeChanged2", "activeHandleChanged", "ChannelBoxLabelSelected", "colorMgtOCIORulesEnabledChanged", "colorMgtUserPrefsChanged", "RenderSetupSelectionChanged", "colorMgtEnabledChanged", "colorMgtConfigFileEnabledChanged", "colorMgtConfigFilePathChanged", "colorMgtConfigChanged", "colorMgtWorkingSpaceChanged", "colorMgtPrefsViewTransformChanged", "colorMgtPrefsReloaded", "colorMgtOutputChanged", "colorMgtPlayblastOutputChanged", "colorMgtRefreshed", "selectionPipelineChanged", "currentMayaWindowChanged",
];

/// The reference's scriptJob -listConditions names.
pub const CONDITIONS: &[&str] = &[
    "MayaInitialized", "delete", "busy", "readingFile", "writingFile", "playingBack", "playblasting", "opening", "newing", "saving", "SomethingSelected", "UndoAvailable", "RedoAvailable", "undoAvailable", "redoAvailable", "autoKeyframeState", "autoKeyState", "batchRendering", "focusChange", "hotkeySetChange", "isShared", "isCurrentRenderLayer", "dynamicsPlaybackDisabled", "hasSkinClusters", "selectionMode", "objectMode", "componentMode", "hierarchyMode", "softSelectOn", "symmetryOn", "pivotMode", "lockedMode",
];

/// A state behind an event, as a value to compare.
fn probe_event(s: &mut Session, event: &str) -> Option<Value> {
    Some(match event {
        "SelectionChanged" => s.run("ls", json!({"selection": true})).unwrap_or(Value::Null),
        "timeChanged" | "currentTimeChanged" => json!(aztek_scene::anim::frame(&s.graph)),
        "ToolChanged" | "PostToolChanged" => json!(crate::ctx_cmds::current()),
        "SelectModeChanged" => json!(format!("{:?}", aztek_scene::components::mode(&s.graph))),
        "playbackRangeChanged" | "playbackRangeSliderChanged" => {
            let q = |s: &mut Session, f: &str| s.run("playbackOptions", json!({"query": true, "queried": [f]})).unwrap_or(Value::Null);
            json!([q(s, "minTime"), q(s, "maxTime")])
        }
        "DagObjectCreated" => json!(s.graph.nodes().len()),
        _ => return None,
    })
}

pub(crate) fn probe_condition(s: &mut Session, name: &str) -> bool {
    match name {
        "SomethingSelected" => s.run("ls", json!({"selection": true})).ok().and_then(|v| v.as_array().map(|a| !a.is_empty())).unwrap_or(false),
        "UndoAvailable" | "undoAvailable" => s.graph.undo_stack().can_undo(),
        "RedoAvailable" | "redoAvailable" => s.graph.undo_stack().can_redo(),
        "playingBack" => s.run("play", json!({"query": true, "queried": ["state"]})).ok().and_then(|v| v.as_bool()).unwrap_or(false),
        "autoKeyframeState" | "autoKeyState" => s.run("autoKeyframe", json!({"query": true})).ok().and_then(|v| v.as_bool()).unwrap_or(false),
        "MayaInitialized" => true,
        "objectMode" => aztek_scene::components::mode(&s.graph) == aztek_scene::components::ComponentMode::Object,
        "componentMode" => aztek_scene::components::mode(&s.graph) != aztek_scene::components::ComponentMode::Object,
        _ => crate::host_general::condition_state(name).unwrap_or(false),
    }
}

fn job_state(s: &mut Session, w: &Watch) -> Value {
    match w {
        Watch::Condition(n, _) => json!(probe_condition(s, n)),
        Watch::Attribute(p) => s.run("getAttr", json!({"args": [p]})).unwrap_or(Value::Null),
        Watch::Connection(p) => s.run("listConnections", json!({"args": [p], "plugs": true})).unwrap_or(Value::Null),
        Watch::AttrAdded(p) | Watch::AttrDeleted(p) => json!(s.graph.plug(p).is_ok()),
        Watch::NodeDeleted(id) => json!(s.graph.node(*id).is_some()),
        Watch::NodeName(id) => json!(s.graph.node(*id).map(|_| s.graph.path(*id))),
        Watch::Time => json!(aztek_scene::anim::frame(&s.graph)),
        _ => Value::Null,
    }
}

/// What the core saw a command do that no state shows.
pub fn note_command(name: &str, call: &Value) -> bool {
    let on = |k: &str| call.get(k).is_some_and(|v| v.as_bool().unwrap_or(v.as_i64().is_some_and(|i| i != 0)));
    let mut opened = false;
    let mut note = |e: &str| with_jobs(|j| j.noted.push(e.to_string()));
    match name {
        "undo" => note("Undo"),
        "redo" => note("Redo"),
        "newScene" => {
            note("deleteAll");
            note("NewSceneOpened");
        }
        "file" if !on("query") && !on("q") => {
            if on("new") {
                note("deleteAll");
                note("NewSceneOpened");
            } else if on("open") || on("o") {
                opened = true;
                note("deleteAll");
                note("PostSceneRead");
                note("SceneOpened");
            } else if on("save") || on("s") {
                note("SceneSaved");
            } else if on("import") || on("i") {
                note("SceneImported");
            }
        }
        _ => {}
    }
    opened
}

/// Whether a command is about to replace the scene.
pub fn replaces_scene(name: &str, call: &Value) -> bool {
    let on = |k: &str| call.get(k).is_some_and(|v| v.as_bool().unwrap_or(v.as_i64().is_some_and(|i| i != 0)));
    name == "newScene" || (name == "file" && !on("query") && !on("q") && (on("new") || on("open") || on("o")))
}

/// A scene with time-changed scripts needs the time watched, jobs or no jobs.
pub fn watch_time_scripts(s: &mut Session) {
    if !scene_scripts(s, &[7], true).is_empty() {
        watch_time(s);
    }
}

fn watch_time(s: &mut Session) {
    let t = json!(aztek_scene::anim::frame(&s.graph));
    with_jobs(|j| {
        j.seen.entry("timeChanged".to_string()).or_insert(t);
    });
}

pub fn note_event(e: &str) {
    with_jobs(|j| j.noted.push(e.to_string()));
}

pub fn set_last_error(e: &str) {
    with_jobs(|j| j.last_error = e.to_string());
}

pub fn deferred_pending() -> bool {
    with_jobs(|j| !j.deferred.is_empty())
}

pub fn take_deferred() -> Vec<Value> {
    with_jobs(|j| std::mem::take(&mut j.deferred))
}

pub fn take_flush() -> bool {
    FLUSH.with(|f| f.replace(false))
}

/// One warning per name, per session: contexts with no tool yet.
pub fn warn_once(key: &str, msg: &str) {
    let first = with_jobs(|j| j.warned.insert(key.to_string()));
    if first {
        warn(msg);
    }
}

/// A warning for the history; the core takes it when the command returns.
pub fn warn(msg: &str) {
    with_jobs(|j| j.warnings.push(msg.to_string()));
}

pub fn take_warnings() -> Vec<String> {
    with_jobs(|j| std::mem::take(&mut j.warnings))
}

/// What is due: compares, fires, retires. Returns the scripts to run in order.
pub fn scan(s: &mut Session) -> Vec<Value> {
    let gone = crate::scriptui::take_gone();
    let (mut jobs, noted, seen_keys) = with_jobs(|j| {
        let keys: Vec<String> = j.seen.keys().cloned().collect();
        (std::mem::take(&mut j.list), std::mem::take(&mut j.noted), keys)
    });
    if jobs.is_empty() && seen_keys.is_empty() && noted.is_empty() {
        return Vec::new();
    }
    let mut fired_events: HashSet<String> = noted.iter().cloned().collect();
    // Scene change retires what belongs to the scene before anything else hears of it.
    if noted.iter().any(|e| e == "NewSceneOpened" || e == "SceneOpened") {
        jobs.retain(|x| !x.kill_with_scene || x.permanent);
    }
    // Events with a state behind them.
    let mut seen_new = Vec::new();
    for k in seen_keys {
        let Some(now) = probe_event(s, &k) else { continue };
        let was = with_jobs(|j| j.seen.get(&k).cloned()).unwrap_or(Value::Null);
        if now != was {
            let more = k != "DagObjectCreated" || now.as_u64() > was.as_u64();
            if more {
                fired_events.insert(k.clone());
            }
            seen_new.push((k, now));
        }
    }
    with_jobs(|j| {
        for (k, v) in seen_new {
            j.seen.insert(k, v);
        }
    });
    let mut out = Vec::new();
    let mut keep = Vec::with_capacity(jobs.len());
    for mut job in jobs {
        let now = job_state(s, &job.watch);
        let (fire, dead) = match &job.watch {
            Watch::Idle => (true, false),
            Watch::Event(e) => (fired_events.contains(e) || e == "idle" || e == "idleHigh" || e == "idleVeryLow", false),
            Watch::Condition(_, want) => {
                let changed = now != job.last;
                (changed && want.is_none_or(|w| now == json!(w)), false)
            }
            Watch::Attribute(_) => (now != job.last && !now.is_null(), now.is_null()),
            Watch::Connection(_) | Watch::Time | Watch::NodeName(_) => (now != job.last && !now.is_null(), false),
            Watch::AttrAdded(_) => (now == json!(true) && job.last == json!(false), false),
            Watch::AttrDeleted(_) => (now == json!(false) && job.last == json!(true), false),
            Watch::NodeDeleted(_) => (now == json!(false), now == json!(false)),
            Watch::UiDeleted(p) => (gone.iter().any(|g| g == p), gone.iter().any(|g| g == p)),
        };
        job.last = now;
        if fire {
            out.push(job.script.clone());
        }
        // Tied to an interface element that went, it goes too.
        let orphan = job.parent.as_ref().is_some_and(|p| gone.iter().any(|g| g == p || g.ends_with(&format!("|{p}"))));
        if !(dead || orphan || (fire && job.run_once)) {
            keep.push(job);
        }
    }
    // A job started meanwhile by something scan ran goes after the older ones.
    with_jobs(|j| {
        keep.append(&mut j.list);
        j.list = keep;
    });
    // Time-changed script nodes run with the rest.
    if fired_events.contains("timeChanged") {
        out.extend(scene_scripts(s, &[7], true));
    }
    out
}

/// Script nodes of the given script types: their before or after scripts, as callbacks.
pub fn scene_scripts(s: &mut Session, script_types: &[i64], before: bool) -> Vec<Value> {
    let mut out = Vec::new();
    let g = &mut s.graph;
    let ids: Vec<NodeId> = g.nodes().into_iter().filter(|&i| g.node(i).is_some_and(|n| n.ty().name == aztek_scene::script_node::TYPE)).collect();
    for id in ids {
        let path = g.path(id);
        let int = |g: &mut aztek_scene::graph::Graph, a: &str| -> i64 {
            g.plug(&format!("{path}.{a}")).ok().and_then(|p| g.value(&p).ok()).map_or(0, |v| match v {
                PlugValue::Int(i) => i,
                PlugValue::Bool(b) => b as i64,
                PlugValue::Float(f) => f as i64,
                _ => 0,
            })
        };
        let st = int(g, "scriptType");
        if !script_types.contains(&st) {
            continue;
        }
        let python = int(g, "sourceType") == 1;
        let attr = if before { "before" } else { "after" };
        let src = g.plug(&format!("{path}.{attr}")).ok().and_then(|p| g.value(&p).ok()).and_then(|v| match v {
            PlugValue::String(t) => Some(t),
            _ => None,
        });
        if let Some(src) = src.filter(|t| !t.trim().is_empty()) {
            out.push(if python { json!({ "__pysrc__": src }) } else { json!(src) });
        }
    }
    out
}

fn script_job(s: &mut Session, a: &Args) -> CmdResult {
    if let Some(id) = a.get("kill").and_then(Value::as_i64) {
        let force = a.bool("force");
        return with_jobs(|j| {
            let Some(p) = j.list.iter().position(|x| x.id == id) else { fail!("scriptJob: Job {id} does not exist.") };
            if (j.list[p].protected || j.list[p].permanent) && !force {
                fail!("scriptJob: Job {id} is protected; use -force.");
            }
            j.list.remove(p);
            Ok(Value::Null)
        });
    }
    if a.bool("killAll") {
        let force = a.bool("force");
        with_jobs(|j| j.list.retain(|x| (x.protected || x.permanent) && !force));
        return Ok(Value::Null);
    }
    if let Some(id) = a.get("exists").and_then(Value::as_i64) {
        return Ok(json!(with_jobs(|j| j.list.iter().any(|x| x.id == id))));
    }
    if a.bool("listJobs") || a.bool("list") {
        return Ok(with_jobs(|j| json!(j.list.iter().map(|x| format!("{}: {}", x.id, x.listing)).collect::<Vec<_>>())));
    }
    if a.bool("listEvents") {
        return Ok(json!(EVENTS));
    }
    if a.bool("listConditions") {
        return Ok(json!(CONDITIONS));
    }
    let pair = |flag: &str, n: usize| -> Option<Vec<Value>> {
        let v = a.get(flag)?;
        let list: Vec<Value> = v.as_array().cloned().unwrap_or_else(|| vec![v.clone()]);
        (list.len() >= n).then_some(list)
    };
    let text = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_string);
    let mut made: Option<(Watch, Value, String)> = None;
    let mut put = |w: Watch, script: Value, listing: String| {
        made = Some((w, callback_value(&script), listing));
    };
    for (flag, short) in [("event", "e"), ("conditionTrue", "ct"), ("conditionFalse", "cf"), ("conditionChange", "cc"), ("attributeChange", "ac"), ("attributeDeleted", "ad"), ("attributeAdded", "aa"), ("connectionChange", "con"), ("nodeDeleted", "nd"), ("nodeNameChanged", "nnc"), ("uiDeleted", "uid")] {
        let Some(l) = pair(flag, 2) else {
            if a.has(flag) {
                fail!("scriptJob: -{short} takes a name and a script.");
            }
            continue;
        };
        let what = text(&l[0]);
        let listing = format!("-{flag} \"{what}\" \"{}\"", text(&l[1]));
        let w = match flag {
            "event" => {
                if !EVENTS.contains(&what.as_str()) {
                    fail!("scriptJob: Invalid event name '{what}'.");
                }
                Watch::Event(what)
            }
            "conditionTrue" | "conditionFalse" | "conditionChange" => {
                if !CONDITIONS.contains(&what.as_str()) {
                    fail!("scriptJob: Invalid condition name '{what}'.");
                }
                Watch::Condition(what, match flag {
                    "conditionTrue" => Some(true),
                    "conditionFalse" => Some(false),
                    _ => None,
                })
            }
            "attributeChange" | "attributeDeleted" | "attributeAdded" | "connectionChange" => {
                if flag != "attributeAdded" && s.graph.plug(&what).is_err() {
                    fail!("scriptJob: No object matches name: {what}");
                }
                match flag {
                    "attributeChange" => Watch::Attribute(what),
                    "attributeDeleted" => Watch::AttrDeleted(what),
                    "attributeAdded" => Watch::AttrAdded(what),
                    _ => Watch::Connection(what),
                }
            }
            "nodeDeleted" | "nodeNameChanged" => {
                let Ok(Some(id)) = s.graph.find(&what) else { fail!("scriptJob: No object matches name: {what}") };
                if flag == "nodeDeleted" {
                    Watch::NodeDeleted(id)
                } else {
                    Watch::NodeName(id)
                }
            }
            _ => Watch::UiDeleted(with_ui_path(&what)),
        };
        put(w, l[1].clone(), listing);
    }
    if let Some(sc) = a.get("timeChange") {
        put(Watch::Time, sc.clone(), format!("-timeChange \"{}\"", text(sc)));
    }
    if let Some(sc) = a.get("idleEvent") {
        put(Watch::Idle, sc.clone(), format!("-idleEvent \"{}\"", text(sc)));
    }
    let Some((watch, script, listing)) = made else { fail!("scriptJob: give what to watch: -event, -conditionTrue, -attributeChange and so on.") };
    let parent = a.get("parent").map(text).map(|p| with_ui_path(&p));
    if let Some(p) = &parent {
        if !crate::scriptui::with_model(|m| m.find(p).is_some()) {
            fail!("scriptJob: Object '{p}' not found.");
        }
        if a.bool("replacePrevious") {
            with_jobs(|j| j.list.retain(|x| x.parent.as_ref() != Some(p)));
        }
    }
    let last = job_state(s, &watch);
    // Events with a state start from what is there now.
    if let Watch::Event(e) = &watch {
        if let Some(v) = probe_event(s, e) {
            with_jobs(|j| {
                j.seen.insert(e.clone(), v);
            });
        }
    }
    let id = with_jobs(|j| {
        j.next += 1;
        let id = j.next;
        j.list.push(Job { id, watch, script, run_once: a.bool("runOnce"), kill_with_scene: a.bool("killWithScene"), protected: a.bool("protected"), permanent: a.bool("permanent"), parent, listing, last });
        id
    });
    Ok(json!(id))
}

/// A control's full path when it exists, else the name as given.
fn with_ui_path(n: &str) -> String {
    crate::scriptui::with_model(|m| m.find(n).and_then(|i| m.controls.get(&i)).map(|c| c.path.clone())).unwrap_or_else(|| n.to_string())
}

fn script_node(s: &mut Session, a: &Args) -> CmdResult {
    let get_str = |s: &mut Session, path: &str, attr: &str| -> CmdResult {
        let p = s.graph.plug(&format!("{path}.{attr}")).map_err(|e| CommandError::new(e.to_string()))?;
        Ok(match s.graph.value(&p).map_err(|e| CommandError::new(e.to_string()))? {
            PlugValue::String(t) => json!(t),
            PlugValue::Int(i) => json!(i),
            _ => Value::Null,
        })
    };
    let set_attr = |s: &mut Session, path: &str, attr: &str, v: PlugValue| -> CmdResult<()> {
        let p = s.graph.plug(&format!("{path}.{attr}")).map_err(|e| CommandError::new(e.to_string()))?;
        s.graph.set_value(p, v).map_err(|e| CommandError::new(e.to_string()))
    };
    let target = a.pos_str(0).map(str::to_string);
    let path = if a.bool("query") || a.bool("edit") || a.bool("executeBefore") || a.bool("executeAfter") {
        let Some(t) = target else { fail!("scriptNode: name a script node.") };
        let Ok(Some(id)) = s.graph.find(&t) else { fail!("No object matches name: {t}") };
        if s.graph.node(id).is_none_or(|n| n.ty().name != aztek_scene::script_node::TYPE) {
            fail!("scriptNode: {t} is not a script node.");
        }
        s.graph.path(id)
    } else {
        let name = a.string("name", target.as_deref().unwrap_or("script1"));
        let made = s.run("createNode", json!({"args": [aztek_scene::script_node::TYPE], "name": name}))?;
        made.as_str().unwrap_or_default().to_string()
    };
    if a.bool("query") {
        let asked = a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).unwrap_or("").to_string();
        return match asked.as_str() {
            "beforeScript" => get_str(s, &path, "before"),
            "afterScript" => get_str(s, &path, "after"),
            "scriptType" => get_str(s, &path, "scriptType"),
            "sourceType" => Ok(json!(if get_str(s, &path, "sourceType")? == json!(1) { "python" } else { "mel" })),
            _ => fail!("scriptNode: name what to ask about."),
        };
    }
    if let Some(v) = a.get("beforeScript") {
        set_attr(s, &path, "before", PlugValue::String(v.as_str().unwrap_or("").to_string()))?;
    }
    if let Some(v) = a.get("afterScript") {
        set_attr(s, &path, "after", PlugValue::String(v.as_str().unwrap_or("").to_string()))?;
    }
    if a.has("scriptType") {
        set_attr(s, &path, "scriptType", PlugValue::Int(a.int("scriptType", 0)))?;
        if a.int("scriptType", 0) == 7 {
            watch_time(s);
        }
    }
    if let Some(v) = a.get("sourceType") {
        let python = v.as_str() == Some("python") || v.as_i64() == Some(1);
        set_attr(s, &path, "sourceType", PlugValue::Int(python as i64))?;
    }
    let python = get_str(s, &path, "sourceType")? == json!(1);
    for (flag, attr) in [("executeBefore", "before"), ("executeAfter", "after")] {
        if a.bool(flag) {
            if let Some(t) = get_str(s, &path, attr)?.as_str().filter(|t| !t.trim().is_empty()) {
                crate::scriptui::run_soon(if python { json!({ "__pysrc__": t }) } else { json!(t) }, Vec::new());
            }
        }
    }
    Ok(json!(path))
}

fn f(name: &str, short: &str, ty: ArgType) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, "")
}

pub fn register(s: &mut Session) {
    let mut job = Cmd::new(
        "scriptJob",
        "Run a script when something happens: -e event script, -ct -cf -cc condition script, -ac attribute script, -nd node script, -uid control script, -tc script, -ie script. -ro once, -kws with the scene, -p with a control, -k -ka kill, -lj list. Returns the job number.",
        script_job,
    )
    .undoable(false);
    for (n, sh, t) in [("event", "e", 2), ("conditionTrue", "ct", 2), ("conditionFalse", "cf", 2), ("conditionChange", "cc", 2), ("attributeChange", "ac", 2), ("attributeDeleted", "ad", 2), ("attributeAdded", "aa", 2), ("connectionChange", "con", 2), ("nodeDeleted", "nd", 2), ("nodeNameChanged", "nnc", 2), ("uiDeleted", "uid", 2)] {
        job = job.flag(f(n, sh, ArgType::Any).multi(t));
    }
    for (n, sh) in [("timeChange", "tc"), ("idleEvent", "ie"), ("kill", "k"), ("exists", "ex"), ("parent", "p")] {
        job = job.flag(f(n, sh, ArgType::Any));
    }
    for (n, sh) in [("killAll", "ka"), ("force", "f"), ("listJobs", "lj"), ("list", "l"), ("listEvents", "le"), ("listConditions", "lc"), ("runOnce", "ro"), ("killWithScene", "kws"), ("protected", "pro"), ("permanent", "per"), ("replacePrevious", "rp"), ("compressUndo", "cu"), ("disregardIndex", "di"), ("allChildren", "alc")] {
        job = job.flag(f(n, sh, ArgType::Bool));
    }
    s.register(job);

    s.register(
        Cmd::new("scriptNode", "A node holding scripts saved with the scene: -bs before, -as after, -st 0 demand, 1 open and close, 2 GUI open and close, 7 time changed, -stp mel or python, -eb -ea run them.", script_node)
            .positional("name")
            .undoable(false)
            .flag(f("query", "q", ArgType::Bool))
            .flag(f("edit", "e", ArgType::Bool))
            .flag(f("queried", "", ArgType::Any))
            .flag(f("name", "n", ArgType::Any))
            .flag(f("beforeScript", "bs", ArgType::Any))
            .flag(f("afterScript", "as", ArgType::Any))
            .flag(f("scriptType", "st", ArgType::Any))
            .flag(f("sourceType", "stp", ArgType::Any))
            .flag(f("executeBefore", "eb", ArgType::Bool))
            .flag(f("executeAfter", "ea", ArgType::Bool))
            .flag(f("ignoreReferenceEdit", "ire", ArgType::Bool)),
    );

    s.register(
        Cmd::new("deferScript", "evalDeferred for Python: the callable or source runs at the next idle. -lp last, -en next.", |_s, a| {
            let Some(script) = a.positional.first() else {
                if a.bool("list") {
                    return Ok(with_jobs(|j| json!(j.deferred.iter().map(|d| d.to_string()).collect::<Vec<_>>())));
                }
                fail!("evalDeferred: give a script.")
            };
            let cb = callback_value(script);
            with_jobs(|j| {
                if a.bool("evaluateNext") {
                    j.deferred.insert(0, cb);
                } else {
                    j.deferred.push(cb);
                }
            });
            Ok(Value::Null)
        })
        .positional("script")
        .undoable(false)
        .flag(f("lowestPriority", "low", ArgType::Bool))
        .flag(f("lowPriority", "lp", ArgType::Bool))
        .flag(f("evaluateNext", "en", ArgType::Bool))
        .flag(f("list", "ls", ArgType::Bool)),
    );

    s.register(
        Cmd::new("flushIdleQueue", "Runs what waits for idle now: scriptJobs and deferred scripts.", |_s, _a| {
            FLUSH.with(|f| f.set(true));
            Ok(Value::Null)
        })
        .undoable(false),
    );

    s.register(
        Cmd::new("timer", "A stopwatch: -s starts, -lap gives seconds so far, -e ends and gives seconds. -n names one.", |_s, a| {
            let name = a.string("name", "");
            with_jobs(|j| {
                if a.bool("startTimer") {
                    let now = Instant::now();
                    j.timers.insert(name, (now, now));
                    return Ok(Value::Null);
                }
                let Some(&(start, last)) = j.timers.get(&name) else { fail!("timer: No timer named \"{name}\" is running.") };
                if a.bool("lapTime") {
                    let now = Instant::now();
                    j.timers.insert(name, (start, now));
                    return Ok(json!((now - last).as_secs_f64()));
                }
                if a.bool("endTimer") {
                    j.timers.remove(&name);
                    return Ok(json!(start.elapsed().as_secs_f64()));
                }
                Ok(json!(start.elapsed().as_secs_f64()))
            })
        })
        .undoable(false)
        .flag(f("startTimer", "s", ArgType::Bool))
        .flag(f("endTimer", "e", ArgType::Bool))
        .flag(f("lapTime", "lap", ArgType::Bool))
        .flag(f("name", "n", ArgType::Any)),
    );

    s.register(
        Cmd::new("stackTrace", "Whether errors show the procedure stack: -state on or off, -q reads it. -dump writes the stack to the history.", |_s, a| {
            with_jobs(|j| {
                if a.has("state") && !a.bool("query") {
                    j.stack_trace = a.bool("state");
                }
                Ok(json!(j.stack_trace))
            })
        })
        .undoable(false)
        .flag(f("state", "st", ArgType::Bool))
        .flag(f("query", "q", ArgType::Bool))
        .flag(f("dump", "d", ArgType::Bool)),
    );

    s.register(Cmd::new("getLastError", "The text of the last error reported.", |_s, _a| Ok(with_jobs(|j| json!(j.last_error)))).undoable(false));

    s.register(
        Cmd::new("isTrue", "Whether a named condition holds, as scriptJob -conditionTrue reads them.", |s, a| {
            let Some(n) = a.pos_str(0) else { fail!("isTrue: name a condition.") };
            Ok(json!(probe_condition(s, n)))
        })
        .positional("condition")
        .undoable(false),
    );
}
