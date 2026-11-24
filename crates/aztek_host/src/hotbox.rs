//! hotBox: which rows of menus the hotbox shows, how it is drawn, and popping it up and down from a script.
//!
//! The rows are the common menus, the pane's menus and one row per menu set. The older menu sets the reference still
//! names are rows here too: polygons and surfaces show the Modeling set's mesh and curve menus, cloth the dynamics
//! menus (one system), live and custom menu sets have no menus. The shell draws the hotbox from `aztekHotBox -state`,
//! pops it on the `hotBox|show` request and tells the core when it shows (`aztekHotBox -shown x y`) and when something
//! in it was used (`aztekHotBox -used`), so a release soon after with nothing picked runs -noClickCommand.

use std::cell::RefCell;
use std::time::Instant;

use aztek_ops::{fail, Args, CmdResult, CommandDef};
use aztek_scene::Session;
use serde_json::{json, Value};

type Cmd = CommandDef<Session>;

/// Each row: its name in the state, its toggle flag and its only flag.
const ROWS: &[(&str, &str, &str)] = &[
    ("common", "commonToggleMenus", "commonOnlyMenus"),
    ("pane", "PaneToggleMenus", "PaneOnlyMenus"),
    ("modeling", "modelingToggleMenus", "modelingOnlyMenus"),
    ("polygons", "polygonsToggleMenus", "polygonsOnlyMenus"),
    ("surfaces", "surfacesToggleMenus", "surfacesOnlyMenus"),
    ("rigging", "riggingToggleMenus", "riggingOnlyMenus"),
    ("animation", "animationToggleMenus", "animationOnlyMenus"),
    ("dynamics", "dynamicsToggleMenus", "dynamicsOnlyMenus"),
    ("cloth", "clothToggleMenus", "clothOnlyMenus"),
    ("rendering", "renderingToggleMenus", "renderingOnlyMenus"),
    ("live", "liveToggleMenus", "liveOnlyMenus"),
    ("customMenuSets", "customMenuSetsToggleMenus", ""),
];

const FLAGS: &str = "PaneOnlyMenus:po:b PaneToggleMenus:pt:b animationOnlyMenus:ao:b animationToggleMenus:at:b clothOnlyMenus:clo:b \
    clothToggleMenus:clt:b commonOnlyMenus:co:b commonToggleMenus:ct:b customMenuSetsToggleMenus:cst:b displayCenterOnly:dco:b \
    displayHotbox:dh:b displayStyle:ds:b displayZonesOnly:dzo:b dynamicsOnlyMenus:do:b dynamicsToggleMenus:dt:b liveOnlyMenus:lo:b \
    liveToggleMenus:lt:b menuSetOnly:mso:a menuSetToggle:mst:a2 modelingOnlyMenus:mdo:b modelingToggleMenus:mdt:b \
    noClickCommand:ncc:a noClickDelay:ncd:a noClickPosition:ncp:b noKeyPress:nkp:b polygonsOnlyMenus:plo:b \
    polygonsToggleMenus:plt:b position:pos:a release:rl:b renderingOnlyMenus:ro:b renderingToggleMenus:rt:b \
    riggingOnlyMenus:rgo:b riggingToggleMenus:rgt:b rmbPopups:rmb:b showAllToggleMenus:a:b surfacesOnlyMenus:so:b \
    surfacesToggleMenus:st:b transparenyLevel:tr:a updateMenus:um:b query:q:b queried::a";

/// Flags that change how the hotbox is set up; a call with none of them (and no -release) pops it.
const SETTINGS: &[&str] = &["showAllToggleMenus", "menuSetOnly", "menuSetToggle", "displayHotbox", "displayZonesOnly", "displayCenterOnly", "rmbPopups", "transparenyLevel", "updateMenus"];

struct State {
    rows: Vec<bool>,
    style: &'static str,
    rmb: bool,
    transparency: i64,
    shown: bool,
    shown_at: Option<Instant>,
    used: bool,
    no_key_press: bool,
    no_click: Option<Value>,
    no_click_delay: f64,
    no_click_position: bool,
    position: Option<(i64, i64)>,
    pointer: (i64, i64),
    menus_updated: u64,
}

impl Default for State {
    fn default() -> Self {
        State {
            rows: vec![true; ROWS.len()],
            style: "dh",
            rmb: true,
            transparency: 25,
            shown: false,
            shown_at: None,
            used: false,
            no_key_press: false,
            no_click: None,
            no_click_delay: 0.1,
            no_click_position: false,
            position: None,
            pointer: (0, 0),
            menus_updated: 0,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn truth(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => matches!(s.as_str(), "1" | "true" | "on" | "yes"),
        _ => false,
    }
}

fn number(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64)).or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0)
}

fn flat(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        o => vec![o.clone()],
    }
}

/// The row a menu set's name stands for: the reference's menu set names, and Aztek's.
fn menu_set_row(name: &str) -> Option<usize> {
    let n = name.trim().to_lowercase();
    let n = n.strip_suffix("menuset").unwrap_or(&n);
    let row = match n {
        "modeling" | "modelling" => "modeling",
        "polygons" | "polygon" => "polygons",
        "surfaces" => "surfaces",
        "rigging" => "rigging",
        "animation" => "animation",
        "dynamics" | "fx" => "dynamics",
        "cloth" => "cloth",
        "rendering" => "rendering",
        "live" => "live",
        "custom" | "custommenusets" => "customMenuSets",
        _ => return None,
    };
    ROWS.iter().position(|r| r.0 == row)
}

fn query(a: &Args) -> CmdResult {
    let first = a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| a.flags.keys().find(|k| !matches!(k.as_str(), "query" | "queried")).cloned());
    let Some(q) = first else { fail!("hotBox: name what to ask about.") };
    with(|s| {
        if let Some(i) = ROWS.iter().position(|r| r.1 == q) {
            return Ok(json!(s.rows[i]));
        }
        if let Some(i) = ROWS.iter().position(|r| r.2 == q) {
            return Ok(json!(s.rows.iter().enumerate().all(|(k, on)| *on == (k == i))));
        }
        Ok(match q.as_str() {
            "showAllToggleMenus" => json!(s.rows.iter().all(|on| *on)),
            "displayStyle" => json!(s.style),
            "displayHotbox" => json!(s.style == "dh"),
            "displayZonesOnly" => json!(s.style == "dzo"),
            "displayCenterOnly" => json!(s.style == "dco"),
            "rmbPopups" => json!(s.rmb),
            "transparenyLevel" => json!(s.transparency),
            "noKeyPress" => json!(s.shown && s.no_key_press),
            "release" => json!(!s.shown),
            "noClickDelay" => json!(s.no_click_delay),
            "noClickPosition" => json!(s.no_click_position),
            "position" => match s.position {
                Some((x, y)) => json!([x, y]),
                None => json!([s.pointer.0, s.pointer.1]),
            },
            _ => fail!("hotBox: -{q} can't be queried."),
        })
    })
}

fn hot_box(a: &Args) -> CmdResult {
    if a.bool("query") {
        return query(a);
    }
    let given = |k: &str| a.flags.get(k).is_some_and(|v| !v.is_null());
    // Settings first: all rows, then one row alone, then single rows on or off.
    let mut pop = !a.bool("release") && !SETTINGS.iter().any(|k| given(k)) && !ROWS.iter().any(|r| given(r.1) || (!r.2.is_empty() && given(r.2)));
    let mut set_rows = None;
    if given("showAllToggleMenus") {
        set_rows = Some(vec![truth(&a.flags["showAllToggleMenus"]); ROWS.len()]);
    }
    let mut rows = set_rows.unwrap_or_else(|| with(|s| s.rows.clone()));
    for (i, r) in ROWS.iter().enumerate() {
        if !r.2.is_empty() && a.bool(r.2) {
            rows = (0..ROWS.len()).map(|k| k == i).collect();
        }
    }
    if let Some(n) = a.get("menuSetOnly").and_then(Value::as_str) {
        let Some(i) = menu_set_row(n) else { fail!("hotBox: no menu set named {n}.") };
        rows = (0..ROWS.len()).map(|k| k == i).collect();
    }
    for (i, r) in ROWS.iter().enumerate() {
        if given(r.1) {
            rows[i] = truth(&a.flags[r.1]);
        }
    }
    if let Some(v) = a.get("menuSetToggle").filter(|v| !v.is_null()) {
        let vals = flat(v);
        for ch in vals.chunks(2) {
            let [name, on] = ch else { fail!("hotBox: -menuSetToggle takes a menu set and on or off.") };
            let n = name.as_str().unwrap_or("");
            let Some(i) = menu_set_row(n) else { fail!("hotBox: no menu set named {n}.") };
            rows[i] = truth(on);
        }
    }
    let style = if a.bool("displayCenterOnly") {
        Some("dco")
    } else if a.bool("displayZonesOnly") {
        Some("dzo")
    } else if a.bool("displayHotbox") {
        Some("dh")
    } else {
        None
    };
    let transparency = match a.get("transparenyLevel").filter(|v| !v.is_null()) {
        // Only steps of 25: others go to the nearest.
        Some(v) => Some(((number(v).clamp(0.0, 100.0) / 25.0).round() * 25.0) as i64),
        None => None,
    };
    let mut due = None;
    with(|s| {
        s.rows = rows;
        if let Some(st) = style {
            s.style = st;
        }
        if given("rmbPopups") {
            s.rmb = a.bool("rmbPopups");
        }
        if let Some(t) = transparency {
            s.transparency = t;
        }
        if a.bool("updateMenus") {
            s.menus_updated += 1;
            crate::core::push_ui_request("hotBox|update".to_string());
        }
        // Pop-up flags also say it pops when they come alone.
        if ["noClickCommand", "noClickDelay", "noClickPosition", "position", "noKeyPress"].iter().any(|k| given(k)) && !a.bool("release") {
            pop = true;
        }
        if pop {
            s.shown = true;
            s.shown_at = Some(Instant::now());
            s.used = false;
            s.no_key_press = a.bool("noKeyPress");
            s.no_click = a.get("noClickCommand").filter(|v| !v.is_null()).map(crate::scriptui::callback_value);
            s.no_click_delay = a.get("noClickDelay").filter(|v| !v.is_null()).map(number).unwrap_or(0.1).max(0.0);
            s.no_click_position = a.bool("noClickPosition");
            s.position = a.get("position").filter(|v| !v.is_null()).map(|v| {
                let p = flat(v);
                (p.first().map(number).unwrap_or(0.0) as i64, p.get(1).map(number).unwrap_or(0.0) as i64)
            });
            if let Some((x, y)) = s.position {
                s.pointer = (x, y);
            }
            let req = json!({"position": s.position.map(|(x, y)| json!([x, y])), "noKeyPress": s.no_key_press});
            crate::core::push_ui_request(format!("hotBox|show|{req}"));
        }
        if a.bool("release") && s.shown {
            s.shown = false;
            crate::core::push_ui_request("hotBox|hide".to_string());
            let quick = s.shown_at.is_some_and(|t| t.elapsed().as_secs_f64() <= s.no_click_delay);
            if quick && !s.used {
                if let Some(cb) = s.no_click.clone() {
                    let (x, y) = s.pointer;
                    // A MEL script has the pointer's place added to it with -noClickPosition; a Python one is given it.
                    due = Some(match (&cb, s.no_click_position) {
                        (Value::String(src), true) => (json!(format!("{src} {x} {y}")), Vec::new()),
                        (_, true) => (cb, vec![json!(x), json!(y)]),
                        _ => (cb, Vec::new()),
                    });
                }
            }
        }
    });
    if let Some((cb, args)) = due {
        crate::scriptui::run_soon(cb, args);
    }
    Ok(Value::Null)
}

/// What the shell needs to draw the hotbox, and what it tells the core.
fn shell(a: &Args) -> CmdResult {
    if let Some(p) = a.get("shown").filter(|v| !v.is_null()) {
        let p = flat(p);
        with(|s| {
            if !s.shown {
                // Popped by the shell's own key: no script asked for anything on release.
                s.shown_at = Some(Instant::now());
                s.no_click = None;
                s.no_key_press = false;
                s.position = None;
            }
            s.shown = true;
            s.used = false;
            s.pointer = (p.first().map(number).unwrap_or(0.0) as i64, p.get(1).map(number).unwrap_or(0.0) as i64);
        });
    }
    if a.bool("used") {
        with(|s| s.used = true);
    }
    if a.bool("hidden") {
        with(|s| s.shown = false);
    }
    Ok(with(|s| {
        let rows: serde_json::Map<String, Value> = ROWS.iter().zip(&s.rows).map(|(r, on)| (r.0.to_string(), json!(*on))).collect();
        json!({
            "rows": rows,
            "style": s.style,
            "rmbPopups": s.rmb,
            "transparency": s.transparency,
            "shown": s.shown,
            "noKeyPress": s.no_key_press,
            "menusUpdated": s.menus_updated,
        })
    }))
}

pub(crate) fn register(s: &mut Session) {
    let c = Cmd::new(
        "hotBox",
        "The hotbox: with no flags it pops up, -release puts it away (running -noClickCommand when that was quick and nothing \
         was picked, -noClickPosition adding the pointer's place). The row flags show and hide the rows of menus (-*ToggleMenus, \
         -*OnlyMenus, -showAllToggleMenus, -menuSetOnly, -menuSetToggle); -displayHotbox, -displayZonesOnly and -displayCenterOnly \
         how much of it is drawn, -displayStyle which; -transparenyLevel, -rmbPopups and -updateMenus the rest.",
        |_s, a| hot_box(a),
    )
    .undoable(false);
    s.register(crate::scriptui::add_flags(c, FLAGS));
    let c = Cmd::new(
        "aztekHotBox",
        "For the shell's hotbox: its rows, style, transparency and right-click setting as one object; -shown x y when it pops at the \
         pointer, -used when something in it was picked, -hidden when it went away by itself.",
        |_s, a| shell(a),
    )
    .undoable(false);
    s.register(crate::scriptui::add_flags(c, "shown:sh:a used:u:b hidden:hd:b"));
}
