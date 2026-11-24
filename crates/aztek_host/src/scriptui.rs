//! Script UI: windows, layouts and controls built by scripts with the reference's commands.
//!
//! The model here is the truth. Every control is a record of its flags by long name, so create, edit and query work with
//! no interface at all, and a query right after a create needs nothing from Qt. The shell mirrors the model: commands
//! queue operations (`take_ops`), the shell applies them and reports what the user did (`Core::ui_event`). Dialogs, which
//! have to answer at once, go through a hook the shell registers (`call_hook`); without one they give the answer a batch
//! run would.
//!
//! Callbacks are JSON: a string is MEL, `{"__pycb__": id}` a Python callable kept by bootstrap.py, `{"__pysrc__": text}`
//! Python source. A string given to a control from Python is Python, as in the reference.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

type Cmd = CommandDef<Session>;

pub type Id = u64;

#[derive(Clone, Debug)]
pub struct Control {
    pub kind: String,
    pub name: String,
    pub path: String,
    pub parent: Option<Id>,
    pub children: Vec<Id>,
    pub props: Map<String, Value>,
}

#[derive(Default)]
pub(crate) struct Model {
    pub controls: BTreeMap<Id, Control>,
    next: Id,
    counters: HashMap<String, u64>,
    pub parent: Option<Id>,
    pub menu: Option<Id>,
    pub collection: Option<Id>,
    templates: HashMap<String, HashMap<String, Map<String, Value>>>,
    template_stack: Vec<String>,
    wait_cursor: bool,
    prompt_text: String,
    built_in: bool,
    /// windowPref: each window's saved size and place, by its name.
    window_prefs: BTreeMap<String, Map<String, Value>>,
    /// windowPref -enableAll false: windows ignore the saved values.
    window_prefs_off: bool,
    /// workspaceControlState: each workspace control's saved size and place, by its name.
    ws_states: BTreeMap<String, Map<String, Value>>,
    dirty_tables: Vec<Id>,
}

thread_local! {
    static UI: RefCell<Model> = RefCell::new(Model::default());
    static OPS: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    /// Callbacks a command asks for: run as soon as the command has returned and the core is free.
    static IMMEDIATE: RefCell<Vec<(Value, Vec<Value>)>> = const { RefCell::new(Vec::new()) };
    /// Control paths that went, for scriptJob -uiDeleted and -parent.
    static GONE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The command running was called from Python: its callback strings are Python.
    static FROM_PYTHON: Cell<bool> = const { Cell::new(false) };
    static HOOK: Cell<Option<UiHook>> = const { Cell::new(None) };
    static ANSWER: RefCell<Option<String>> = const { RefCell::new(None) };
    static HOOK_DEPTH: Cell<i32> = const { Cell::new(0) };
}

/// The shell's answer point: the kind of request and a JSON payload. It answers with `set_hook_answer`.
pub type UiHook = extern "C" fn(kind: *const std::ffi::c_char, json: *const std::ffi::c_char);

pub fn set_hook(h: Option<UiHook>) {
    HOOK.with(|c| c.set(h));
}

pub fn set_hook_answer(s: String) {
    ANSWER.with(|a| *a.borrow_mut() = Some(s));
}

/// True while the shell is inside a hook call, its event loop running on top of a suspended command.
pub fn in_hook() -> bool {
    HOOK_DEPTH.with(Cell::get) > 0
}

/// Asks the shell; None with no shell. The answer comes back parsed.
pub fn call_hook(kind: &str, payload: &Value) -> Option<Value> {
    let hook = HOOK.with(Cell::get)?;
    let k = std::ffi::CString::new(kind).ok()?;
    let p = std::ffi::CString::new(payload.to_string().replace('\0', " ")).ok()?;
    ANSWER.with(|a| *a.borrow_mut() = None);
    HOOK_DEPTH.with(|d| d.set(d.get() + 1));
    hook(k.as_ptr(), p.as_ptr());
    HOOK_DEPTH.with(|d| d.set(d.get() - 1));
    let text = ANSWER.with(|a| a.borrow_mut().take())?;
    Some(serde_json::from_str(&text).unwrap_or(Value::Null))
}

pub(crate) fn with_model<R>(f: impl FnOnce(&mut Model) -> R) -> R {
    UI.with(|m| {
        let mut m = m.borrow_mut();
        ensure_built_in(&mut m);
        f(&mut m)
    })
}

/// The application's own window and progress bar, which scripts hang menus on and report through.
fn ensure_built_in(m: &mut Model) {
    if m.built_in {
        return;
    }
    m.built_in = true;
    // The application's own editors, under the names the reference gives them, so scripts can set them up.
    for (kind, name) in [
        ("mainWindow", MAIN_WINDOW),
        ("progressBar", MAIN_PROGRESS),
        ("timeControl", TIME_SLIDER),
        ("outlinerEditor", "outlinerPanel1"),
        ("animCurveEditor", "graphEditor1GraphEd"),
        ("outlinerEditor", "graphEditor1OutlineEd"),
        ("dopeSheetEditor", "dopeSheetPanel1DopeSheetEd"),
        ("outlinerEditor", "dopeSheetPanel1OutlineEd"),
        ("nodeEditor", "nodeEditorPanel1NodeEditorEd"),
        ("channelBox", crate::channel_box::MAIN),
    ] {
        m.next += 1;
        m.controls.insert(m.next, Control { kind: kind.into(), name: name.into(), path: name.into(), parent: None, children: Vec::new(), props: Map::new() });
    }
}

/// The names scripts know these by, and read from `$gMainWindow` and `$gMainProgressBar`.
pub const MAIN_WINDOW: &str = "MayaWindow";
pub const MAIN_PROGRESS: &str = "MainProgressBar";
/// The time slider: the one timeControl, which scripts read from `$gPlayBackSlider`.
pub const TIME_SLIDER: &str = "timeControl1";

pub fn set_from_python(on: bool) -> bool {
    FROM_PYTHON.with(|f| f.replace(on))
}

fn from_python() -> bool {
    FROM_PYTHON.with(Cell::get)
}

pub fn take_ops() -> Vec<Value> {
    OPS.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

fn op(v: Value) {
    OPS.with(|o| o.borrow_mut().push(v));
}

pub fn take_immediate() -> Vec<(Value, Vec<Value>)> {
    IMMEDIATE.with(|i| {
        if i.borrow().is_empty() {
            Vec::new()
        } else {
            std::mem::take(&mut *i.borrow_mut())
        }
    })
}

pub(crate) fn run_soon(cb: Value, args: Vec<Value>) {
    IMMEDIATE.with(|i| i.borrow_mut().push((cb, args)));
}

pub fn take_gone() -> Vec<String> {
    GONE.with(|g| std::mem::take(&mut *g.borrow_mut()))
}

/// attrNavigationControlGrp's flags that are scripts, though their names don't say so.
const NAV_SCRIPTS: &[&str] = &["connectAttrToDropped", "connectNodeToDropped", "ignore", "unignore", "delete", "relatedNodes", "createNew", "connectToExisting", "disconnect", "defaultTraversal"];
/// What the shell fires when a node is dropped on an attrNavigationControlGrp, and from its right-click menu.
const NAV_DROP: &str = "__navNodeDrop";

/// A flag whose value is a script.
pub fn is_callback(flag: &str) -> bool {
    flag == "command" || flag.contains("Command") || flag.ends_with("Callback") || flag.ends_with("Cmd") || flag == "uiScript"
}

/// A callback as given, in the form it is stored: a string from Python is Python source.
pub fn callback_value(v: &Value) -> Value {
    match v {
        Value::String(s) if from_python() => json!({ "__pysrc__": s }),
        o => o.clone(),
    }
}

fn no_list(v: Vec<String>) -> Value {
    // The reference's list queries give Python None rather than an empty list.
    if v.is_empty() && from_python() {
        Value::Null
    } else {
        json!(v)
    }
}

/// A flag's value as flat values: the single value, a list, or a list of tuples.
fn flat(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        o => vec![o.clone()],
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => (*b as i32).to_string(),
        o => o.to_string(),
    }
}

fn number(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64)).or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0)
}

fn truth(v: &Value) -> bool {
    number(v) != 0.0 || v.as_str().is_some_and(|s| matches!(s, "true" | "on" | "yes"))
}

// ---------------------------------------------------------------------------------------------------------------------
// Finding

impl Model {
    pub fn find(&self, name: &str) -> Option<Id> {
        let n = name.trim_start_matches('|');
        // The menu inside an optionMenuGrp, as the reference names it.
        let n = n.strip_suffix("|OptionMenu").unwrap_or(n);
        if n.is_empty() {
            return None;
        }
        let tail = format!("|{n}");
        self.controls.iter().rev().find(|(_, c)| c.path == n || c.path.ends_with(&tail)).map(|(i, _)| *i)
    }

    fn get(&self, id: Id) -> Option<&Control> {
        self.controls.get(&id)
    }

    fn cat(&self, id: Id) -> Cat {
        self.get(id).map_or(Cat::Control, |c| cat_of(&c.kind))
    }

    /// The nearest window above, itself included.
    fn window_of(&self, mut id: Id) -> Option<Id> {
        loop {
            let c = self.get(id)?;
            match c.parent {
                Some(p) => id = p,
                None => return Some(id),
            }
        }
    }

    fn fresh_name(&mut self, kind: &str, parent: Option<Id>) -> String {
        loop {
            let e = self.counters.entry(kind.to_string()).or_insert(0);
            *e += 1;
            let n = format!("{kind}{e}");
            if !self.name_taken(&n, parent) {
                return n;
            }
        }
    }

    fn name_taken(&self, name: &str, parent: Option<Id>) -> bool {
        match parent {
            Some(p) => self.get(p).is_some_and(|c| c.children.iter().any(|k| self.get(*k).is_some_and(|x| x.name == name))),
            None => self.controls.values().any(|c| c.parent.is_none() && c.name == name),
        }
    }

    /// Keeps a window's size and place under its name, for the next window of that name.
    fn record_window_pref(&mut self, c: &Control) {
        if c.kind != "window" || c.name == MAIN_WINDOW {
            return;
        }
        let e = self.window_prefs.entry(c.name.clone()).or_default();
        for (k, d) in [("width", Some(546)), ("height", Some(350)), ("topEdge", None), ("leftEdge", None)] {
            match (c.props.get(k), d) {
                (Some(v), _) => {
                    e.insert(k.into(), json!(number(v) as i64));
                }
                (None, Some(d)) if !e.contains_key(k) => {
                    e.insert(k.into(), json!(d));
                }
                _ => {}
            }
        }
        if let Some(v) = c.props.get("maximized") {
            e.insert("maximized".into(), json!(truth(v)));
        }
    }

    fn remove(&mut self, id: Id, gone: &mut Vec<Id>) {
        let kids = self.get(id).map(|c| c.children.clone()).unwrap_or_default();
        for k in kids {
            self.remove(k, gone);
        }
        gone.push(id);
    }

    /// Deletes a control and all under it. Current parents inside it fall back.
    fn delete(&mut self, id: Id) -> Vec<Value> {
        // A shown window leaves its size and place as its windowPref.
        if let Some(c) = self.get(id).filter(|c| c.kind == "window" && c.props.get("visible").is_some_and(truth)).cloned() {
            self.record_window_pref(&c);
        }
        let mut gone = Vec::new();
        self.remove(id, &mut gone);
        let mut close = Vec::new();
        if let Some(c) = self.get(id) {
            op(json!({"op": "delete", "path": c.path}));
            if let Some(p) = c.parent {
                if let Some(pc) = self.controls.get_mut(&p) {
                    pc.children.retain(|k| *k != id);
                }
            }
        }
        for g in gone {
            if let Some(c) = self.controls.remove(&g) {
                GONE.with(|l| l.borrow_mut().push(c.path.clone()));
                crate::cmd_fields::forget(&c.path);
                if let Some(cb) = c.props.get("closeCommand") {
                    close.push(cb.clone());
                }
            }
            if self.parent == Some(g) {
                self.parent = None;
            }
            if self.menu == Some(g) {
                self.menu = None;
            }
            if self.collection == Some(g) {
                self.collection = None;
            }
        }
        close
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Kinds and their flags

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cat {
    Window,
    Layout,
    Control,
    Menu,
    MenuItem,
    Collection,
}

pub fn cat_of(kind: &str) -> Cat {
    match kind {
        "window" | "workspaceControl" | "dockControl" | "mainWindow" | "progressWindow" => Cat::Window,
        "menu" | "popupMenu" | "optionMenu" | "optionMenuGrp" => Cat::Menu,
        "menuItem" => Cat::MenuItem,
        "radioCollection" | "radioMenuItemCollection" | "toolCollection" | "iconTextRadioCollection" => Cat::Collection,
        k if k.ends_with("Layout") || k == "scriptedPanel" => Cat::Layout,
        _ => Cat::Control,
    }
}

pub(crate) const COMMON: &str = "annotation:ann:a backgroundColor:bgc:a defineTemplate:dt:a docTag:dtg:a dragCallback:dgc:a dropCallback:dpc:a enable:en:b enableBackground:ebg:b enableKeyboardFocus:ekf:b exists:ex:b fullPathName:fpn:b height:h:a highlightColor:hlc:a isObscured:io:b manage:m:b noBackground:nbg:b numberOfPopupMenus:npm:b parent:p:a popupMenuArray:pma:b preventOverride:po:b statusBarMessage:sbm:a useTemplate:ut:a visible:vis:b visibleChangeCommand:vcc:a width:w:a query:q:b edit:e:b queried::a";
const LAYOUT: &str = "childArray:ca:b numberOfChildren:nch:b";
/// What soundControl and timeControl share: the sound they draw and how, scrubbing, and the mouse scripts.
pub(crate) const SOUND_FLAGS: &str = "sound:s:a soundSource:ss:a displaySound:ds:b waveform:wf:a minTime:min:a maxTime:max:a pressCommand:pc:a releaseCommand:rc:a repeatChunkSize:rcs:a repeatOnHold:roh:b resample:r:b beginScrub:bs:b endScrub:es:b";
/// timeControl's own, beside SOUND_FLAGS; the range, key display and snapping flags are the scene's.
pub(crate) const TIME_CONTROL_FLAGS: &str = "animCurveNames:acn:b animLayerFilterOptions:alf:a currentFrameColor:cfc:a foregroundColor:fgc:a globalTime:gt:b mainListConnection:mlc:a tickSpan:tsp:a forceRefresh:fr:b forceRedraw:fd:b";
const WINDOW: &str = "title:t:a widthHeight:wh:a sizeable:s:b minimizeButton:mnb:b maximizeButton:mxb:b resizeToFitChildren:rtf:b menuBar:mb:b menuBarVisible:mbv:b titleBar:tb:b titleBarMenu:tbm:b toolbox:tlb:b topEdge:te:a leftEdge:le:a topLeftCorner:tlc:a retain:ret:b closeCommand:cc:a minimizeCommand:mnc:a restoreCommand:rc:a iconName:in:a iconify:i:b interactivePlacement:ip:b frontWindow:fw:b numberOfWindows:nw:b mainWindow:mw:b nestedDockingEnabled:nde:b dockCorner:dc:a2 dockStation:ds:b dockingLayout:dl:a state:st:a menuArray:ma:b mainMenuBar:mm:b menuBarCornerWidget:mcw:a2 menuBarResize:mbr:b menuIndex:mi:a2 numberOfMenus:nm:b";
const BUTTON_ICON: &str = "style:st:a image:i:a image1:i1:a image2:i2:a image3:i3:a label:l:a command:c:a doubleClickCommand:dcc:a sourceType:stp:a imageOverlayLabel:iol:a flat:fla:b disabledImage:di:a align:al:a marginWidth:mw:a marginHeight:mh:a font:fn:a labelOffset:lo:a rotation:rot:a selectionImage:si:a highlightImage:hi:a overlayLabelColor:olc:a overlayLabelBackColor:olb:a scaleIcon:sic:b commandRepeatable:rpt:b ltVersion:lt:a version:ver:a useAlpha:ua:b flexibleWidthType:fwt:a flexibleWidthValue:fwv:a flipX:fx:b flipY:fy:b handleNodeDropCallback:hnd:a labelEditingCallback:lec:a";
/// What iconTextCheckBox, iconTextRadioButton and iconTextStaticLabel share: the picture, its label and how both are drawn.
const ICON_LOOK: &str = "style:st:a image:i:a image1:i1:a image2:i2:a image3:i3:a label:l:a font:fn:a align:al:a disabledImage:di:a flipX:fx:b flipY:fy:b imageOverlayLabel:iol:a labelOffset:lo:a marginHeight:mh:a marginWidth:mw:a overlayLabelBackColor:olb:a overlayLabelColor:olc:a rotation:rot:a runTimeCommand:rtc:a useAlpha:ua:b version:ver:a";
const SHELF_BUTTON: &str = "style:st:a image:i:a image1:i1:a image2:i2:a image3:i3:a label:l:a command:c:a doubleClickCommand:dcc:a sourceType:stp:a imageOverlayLabel:iol:a flat:fla:b disabledImage:di:a align:al:a marginWidth:mw:a marginHeight:mh:a font:fn:a labelOffset:lo:a rotation:rot:a selectionImage:si:a highlightImage:hi:a overlayLabelColor:olc:a overlayLabelBackColor:olb:a scaleIcon:sic:b commandRepeatable:rpt:b ltVersion:lt:a version:ver:a useAlpha:ua:b flexibleWidthType:fwt:a flexibleWidthValue:fwv:a enableCommandRepeat:ecr:b flipX:fx:b flipY:fy:b handleNodeDropCallback:hnd:a labelEditingCallback:lec:a menuItem:mi:a2 menuItemPython:mip:a1 menuItemWithOptionBox:mio:a3 noDefaultPopup:ndp:b";
const SLIDER: &str = "value:v:a minValue:min:a maxValue:max:a step:s:a horizontal:hr:b changeCommand:cc:a dragCommand:dc:a";
pub(crate) const SLIDER_GRP: &str = "label:l:a field:f:b value:v:a minValue:min:a maxValue:max:a fieldMinValue:fmn:a fieldMaxValue:fmx:a sliderStep:ss:a step:s:a fieldStep:fs:a extraLabel:el:a changeCommand:cc:a dragCommand:dc:a";
const FIELD: &str = "value:v:a minValue:min:a maxValue:max:a step:s:a changeCommand:cc:a dragCommand:dc:a enterCommand:ec:a receiveFocusCommand:rfc:a editable:ed:b";
pub(crate) const FIELD_GRP: &str = "label:l:a numberOfFields:nf:a value:v:a value1:v1:a value2:v2:a value3:v3:a value4:v4:a extraLabel:el:a changeCommand:cc:a changeCommand1:cc1:a changeCommand2:cc2:a changeCommand3:cc3:a changeCommand4:cc4:a dragCommand:dc:a dragCommand1:dc1:a dragCommand2:dc2:a dragCommand3:dc3:a dragCommand4:dc4:a enable1:en1:b enable2:en2:b enable3:en3:b enable4:en4:b";
/// colorSliderGrp's flags; colorSliderButtonGrp adds a button and a symbol button to them.
const COLOR_GRP: &str = "label:l:a rgbValue:rgb:a hsvValue:hsv:a alphaValue:alp:a forceDragRefresh:fdr:b changeCommand:cc:a dragCommand:dc:a useDisplaySpace:uds:b useVpColorPicker:vcp:b";
const MENU: &str = "label:l:a tearOff:to:b allowOptionBoxes:aob:b deleteAllItems:dai:b itemArray:ia:b numberOfItems:ni:b postMenuCommand:pmc:a postMenuCommandOnce:pmo:b familyImage:fi:a mnemonic:mn:a ltVersion:lt:a version:ver:a scrollable:srb:b helpMenu:hm:b";

/// An iconTextScrollList's rows as the shell draws them, in pixels, and its width when it is given none.
const LIST_ROW: f64 = 18.0;
const LIST_WIDTH: f64 = 100.0;

/// (command, flags). `long:short:type`, type b a switch, a any value, aN N values per use.
const KINDS: &[(&str, &str)] = &[
    ("window", WINDOW),
    ("columnLayout", "adjustableColumn:adj:b columnAlign:cal:a columnAttach:cat:a2 columnOffset:co:a2 columnWidth:cw:a rowSpacing:rs:a generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("rowLayout", "numberOfColumns:nc:a adjustableColumn:adj:a adjustableColumn1:ad1:a rowAttach:rat:a3 columnAlign:cal:a2 columnAttach:cat:a3 columnWidth:cw:a2 columnOffset:co:a2 generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("rowColumnLayout", "numberOfColumns:nc:a numberOfRows:nr:a numberOfRowsColumns:nrc:a adjustableColumn:adj:a adjustableRow:ar:a columnAlign:cal:a2 columnAttach:cat:a3 columnOffset:co:a3 columnSpacing:cs:a2 columnWidth:cw:a2 rowAlign:ral:a2 rowAttach:rat:a3 rowHeight:rh:a2 rowOffset:ro:a3 rowSpacing:rs:a2 generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("gridLayout", "numberOfColumns:nc:a numberOfRows:nr:a numberOfRowsColumns:nrc:a cellWidth:cw:a cellHeight:ch:a cellWidthHeight:cwh:a autoGrow:ag:b allowEmptyCells:aec:b columnsResizable:cr:b position:pos:a2 gridOrder:go:a generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("flowLayout", "vertical:v:b horizontal:hr:b wrap:wr:b columnSpacing:cs:a generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("formLayout", "numberOfDivisions:nd:a attachForm:af:a3 attachControl:ac:a4 attachPosition:ap:a4 attachNone:an:a2 attachOppositeForm:aof:a3 attachOppositeControl:aoc:a4 generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("frameLayout", "label:l:a collapsable:cll:b collapse:cl:b marginHeight:mh:a marginWidth:mw:a borderVisible:bv:b borderStyle:bs:a labelVisible:lv:b labelIndent:li:a labelWidth:lw:a labelAlign:la:a font:fn:a collapseCommand:cc:a expandCommand:ec:a preCollapseCommand:pcc:a preExpandCommand:pec:a backgroundShade:bgs:b generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("scrollLayout", "horizontalScrollBarThickness:hst:a verticalScrollBarThickness:vst:a childResizable:cr:b resizeCommand:rc:a scrollAreaHeight:sah:b scrollAreaWidth:saw:b scrollAreaValue:sav:b scrollByPixel:sbp:a2 scrollPage:sp:a minChildWidth:mcw:a verticalScrollBarAlwaysVisible:vsb:b panEnabled:pe:b borderVisible:bv:b generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("tabLayout", "tabLabel:tl:a2 tabLabelIndex:tli:a2 selectTabIndex:sti:a selectTab:st:a changeCommand:cc:a selectCommand:sc:a preSelectCommand:psc:a doubleClickCommand:dcc:a tabsVisible:tv:b innerMarginWidth:imw:a innerMarginHeight:imh:a scrollable:scr:b childResizable:cr:b horizontalScrollBarThickness:hst:a verticalScrollBarThickness:vst:a moveTab:mt:a2 newTabCommand:ntc:a showNewTab:snt:b tabPosition:tp:a tabTooltip:tt:a2 tabsClosable:tc:b closeTabCommand:ctc:a minChildWidth:mcw:a image:i:a imageVisible:iv:b borderStyle:bs:a closeTab:ct:a generalSpacing:gsp:a margins:mar:a margins4:mr4:a4 postMenuCommand:pmc:a scrollableTabs:stb:b tabIcon:ti:a2 tabIconIndex:tii:a2 tabTooltipIndex:tti:a2"),
    ("paneLayout", "configuration:cn:a paneSize:ps:a3 separatorThickness:st:a separatorMovedCommand:smc:a setPane:sp:a2 activePane:ap:a activePaneIndex:api:a numberOfVisiblePanes:nvp:b staticWidthPane:swp:a staticHeightPane:shp:a paneUnderPointer:pup:b pane1:p1:b pane2:p2:b pane3:p3:b pane4:p4:b activeFrameThickness:aft:a generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("shelfLayout", "cellWidth:cw:a cellHeight:ch:a cellWidthHeight:cwh:a spacing:spa:a alignment:aln:a position:pos:a2 style:st:a version:ver:a horizontal:hr:b generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("menuBarLayout", "menuBarVisible:mbv:b menuArray:ma:b menuIndex:mi:a2 numberOfMenus:nm:b generalSpacing:gsp:a margins:mar:a margins4:mr4:a4"),
    ("button", "label:l:a align:al:a command:c:a recomputeSize:rs:b actOnPress:aop:b actionIsSubstitute:ais:b"),
    ("iconTextButton", BUTTON_ICON),
    ("shelfButton", SHELF_BUTTON),
    ("iconTextCheckBox", "value:v:b onCommand:onc:a offCommand:ofc:a changeCommand:cc:a flat:fla:b highlightImage:hi:a selectionImage:si:a selectionHighlightImage:shi:a"),
    ("iconTextRadioButton", "select:sl:b collection:cl:a onCommand:onc:a offCommand:ofc:a changeCommand:cc:a flat:fla:b highlightImage:hi:a selectionImage:si:a selectionHighlightImage:shi:a"),
    ("iconTextStaticLabel", ""),
    ("iconTextScrollList", "allowMultiSelection:ams:b append:a:a1 changeCommand:cc:a deselectAll:da:b doubleClickCommand:dcc:a dragFeedbackVisible:dfv:b dropRectCallback:drc:a editIndexed:ei:a editable:ed:b itemAt:ia:a2 itemTextColor:itc:a4 numberOfIcons:nic:b numberOfRows:nr:b removeAll:ra:b selectCommand:sc:a selectIndexedItem:sii:a1 selectItem:si:a1 visualRectAt:vra:a2"),
    ("symbolButton", "image:i:a command:c:a"),
    ("symbolCheckBox", "image:i:a onImage:oni:a offImage:ofi:a disableOnImage:dni:a disableOffImage:dfi:a innerMargin:im:b version:ver:a value:v:b onCommand:onc:a offCommand:ofc:a changeCommand:cc:a"),
    ("text", "label:l:a align:al:a font:fn:a recomputeSize:rs:b wordWrap:ww:b hyperlink:hl:b dropRectCallback:drc:a"),
    ("textField", "text:tx:a placeholderText:pht:a editable:ed:b changeCommand:cc:a textChangedCommand:tcc:a enterCommand:ec:a alwaysInvokeEnterCommandOnReturn:aie:b receiveFocusCommand:rfc:a insertText:it:a insertionPosition:ip:a fileName:fi:a font:fn:a searchField:sf:b"),
    ("textFieldGrp", "label:l:a text:tx:a placeholderText:pht:a editable:ed:b changeCommand:cc:a textChangedCommand:tcc:a forceChangeCommand:fcc:b fileName:fi:a insertText:it:a insertionPosition:ip:a"),
    ("textFieldButtonGrp", "label:l:a text:tx:a placeholderText:pht:a editable:ed:b changeCommand:cc:a textChangedCommand:tcc:a forceChangeCommand:fcc:b fileName:fi:a insertText:it:a insertionPosition:ip:a buttonLabel:bl:a buttonCommand:bc:a enableButton:eb:b"),
    ("scrollField", "text:tx:a editable:ed:b wordWrap:ww:b changeCommand:cc:a enterCommand:ec:a keyPressCommand:kpc:a font:fn:a numberOfLines:nl:a insertText:it:a insertionPosition:ip:a clear:cl:b selection:sl:b"),
    ("intField", FIELD),
    ("floatField", "precision:pre:a showTrailingZeros:tze:b"),
    ("intFieldGrp", FIELD_GRP),
    ("floatFieldGrp", "precision:pre:a showTrailingZeros:tze:b step:s:a"),
    ("intSlider", SLIDER),
    ("floatSlider", SLIDER),
    ("intScrollBar", "value:v:a minValue:min:a maxValue:max:a step:s:a largeStep:ls:a horizontal:hr:b changeCommand:cc:a dragCommand:dc:a"),
    ("floatScrollBar", "value:v:a minValue:min:a maxValue:max:a step:s:a largeStep:ls:a horizontal:hr:b changeCommand:cc:a dragCommand:dc:a"),
    ("intSliderGrp", SLIDER_GRP),
    ("floatSliderGrp", "precision:pre:a showTrailingZeros:tze:b"),
    ("checkBox", "label:l:a value:v:b changeCommand:cc:a onCommand:onc:a offCommand:ofc:a align:al:a editable:ed:b recomputeSize:rs:b"),
    ("checkBoxGrp", "label:l:a numberOfCheckBoxes:ncb:a label1:l1:a label2:l2:a label3:l3:a label4:l4:a labelArray2:la2:a2 labelArray3:la3:a3 labelArray4:la4:a4 value1:v1:b value2:v2:b value3:v3:b value4:v4:b valueArray2:va2:a2 valueArray3:va3:a3 valueArray4:va4:a4 changeCommand:cc:a changeCommand1:cc1:a changeCommand2:cc2:a changeCommand3:cc3:a changeCommand4:cc4:a onCommand:onc:a onCommand1:on1:a onCommand2:on2:a onCommand3:on3:a onCommand4:on4:a offCommand:ofc:a offCommand1:of1:a offCommand2:of2:a offCommand3:of3:a offCommand4:of4:a enable1:en1:b enable2:en2:b enable3:en3:b enable4:en4:b editable:ed:b vertical:vr:b"),
    ("radioButton", "label:l:a select:sl:b collection:cl:a changeCommand:cc:a onCommand:onc:a offCommand:ofc:a align:al:a editable:ed:b data:da:a recomputeSize:rs:b"),
    ("radioButtonGrp", "label:l:a numberOfRadioButtons:nrb:a label1:l1:a label2:l2:a label3:l3:a label4:l4:a labelArray2:la2:a2 labelArray3:la3:a3 labelArray4:la4:a4 select:sl:a shareCollection:scl:a changeCommand:cc:a changeCommand1:cc1:a changeCommand2:cc2:a changeCommand3:cc3:a changeCommand4:cc4:a onCommand:onc:a onCommand1:on1:a onCommand2:on2:a onCommand3:on3:a onCommand4:on4:a offCommand:ofc:a offCommand1:of1:a offCommand2:of2:a offCommand3:of3:a offCommand4:of4:a enable1:en1:b enable2:en2:b enable3:en3:b enable4:en4:b data1:da1:a data2:da2:a data3:da3:a data4:da4:a editable:ed:b vertical:vr:b"),
    ("optionMenu", "label:l:a value:v:a select:sl:a numberOfItems:ni:b itemListLong:ill:b itemListShort:ils:b changeCommand:cc:a alwaysCallChangeCommand:acc:b deleteAllItems:dai:b beforeShowPopup:bsp:a postMenuCommand:pmc:a postMenuCommandOnce:pmo:b maxVisibleItems:mvi:a"),
    ("optionMenuGrp", "label:l:a extraLabel:el:a value:v:a select:sl:a numberOfItems:ni:b itemListLong:ill:b itemListShort:ils:b changeCommand:cc:a alwaysCallChangeCommand:acc:b deleteAllItems:dai:b"),
    ("menuItem", "label:l:a command:c:a subMenu:sm:b tearOff:to:b checkBox:cb:b radioButton:rb:b divider:d:b dividerLabel:dl:a optionBox:ob:b optionBoxIcon:obi:a boldFont:bld:b italicized:itl:b image:i:a imageOverlayLabel:iol:a insertAfter:ia:a collection:cl:a data:da:a dragMenuCommand:dmc:a dragDoubleClickCommand:ddc:a echoCommand:ec:b radialPosition:rp:a mnemonic:mn:a keyEquivalent:ke:a altModifier:alt:b ctrlModifier:ctl:b shiftModifier:sht:b commandModifier:cmd:b sourceType:stp:a isOptionBox:iob:b isCheckBox:icb:b isRadioButton:irb:b familyImage:fi:a version:ver:a ltVersion:lt:a allowOptionBoxes:aob:b postMenuCommand:pmc:a postMenuCommandOnce:pmo:b enableCommandRepeat:ecr:b header:hdr:b longDivider:ld:b runTimeCommand:rtc:a"),
    ("menu", MENU),
    ("popupMenu", "button:b:a ctrlModifier:ctl:b altModifier:alt:b shiftModifier:sh:b markingMenu:mm:b allowOptionBoxes:aob:b deleteAllItems:dai:b itemArray:ia:b numberOfItems:ni:b postMenuCommand:pmc:a postMenuCommandOnce:pmo:b"),
    ("separator", "style:st:a horizontal:hr:b"),
    ("textScrollList", "append:a:a1 appendPosition:ap:a2 removeItem:ri:a1 removeIndexedItem:rii:a1 removeAll:ra:b selectItem:si:a1 selectIndexedItem:sii:a1 deselectAll:da:b deselectItem:di:a1 deselectIndexedItem:dii:a1 allowMultiSelection:ams:b numberOfRows:nr:a allItems:ai:b numberOfItems:ni:b numberOfSelectedItems:nsi:b showIndexedItem:shi:a selectCommand:sc:a doubleClickCommand:dcc:a deleteKeyCommand:dkc:a lineFont:lf:a2 font:fn:a allowAutomaticSelection:aas:b uniqueTag:utg:a1 selectUniqueTagItem:sut:a1 allItemsUniqueTags:ait:b emptyLabel:el:a enableAll:ea:b enableItem:ei:a2 enableIndexedItem:eii:a2 enableUniqueTagItem:eut:a2"),
    ("colorSliderGrp", COLOR_GRP),
    ("colorSliderButtonGrp", "buttonLabel:bl:a buttonCommand:bc:a symbolButtonCommand:sbc:a symbolButtonDisplay:sbd:b image:i:a"),
    ("colorInputWidgetGrp", "label:l:a rgbValue:rgb:a hsvValue:hsv:a alphaValue:alp:a forceDragRefresh:fdr:b changeCommand:cc:a"),
    ("commandLine", "command:c:a enterCommand:ec:a holdFocus:hf:b numberOfHistoryLines:nhl:a sourceType:st:a inputAnnotation:ian:a outputAnnotation:oan:a"),
    ("colorIndexSliderGrp", "label:l:a value:v:a minValue:min:a maxValue:max:a invisible:inv:a forceDragRefresh:fdr:b extraLabel:el:a changeCommand:cc:a dragCommand:dc:a"),
    ("progressBar", "progress:pr:a minValue:min:a maxValue:max:a step:s:a status:st:a isInterruptable:ii:b isCancelled:ic:b isMainProgressBar:imp:b beginProgress:bp:b endProgress:ep:b"),
    ("image", "image:i:a"),
    ("picture", "image:i:a tile:tl:b"),
    ("canvas", "rgbValue:rgb:a hsvValue:hsv:a pressCommand:pc:a"),
    ("workspaceControl", "label:l:a uiScript:ui:a floating:fl:b initialWidth:iw:a initialHeight:ih:a minimumWidth:mw:a minimumHeight:mh:a retain:rt:b widthProperty:wp:a heightProperty:hp:a dockToMainWindow:dtm:a2 dockToControl:dtc:a2 dockToPanel:dtp:a3 tabToControl:ttc:a2 collapse:clp:b restore:rs:b closeCommand:cc:a requiredPlugin:rp:a requiredControl:rc:a loadImmediately:li:b checksPlugins:cp:b actLikeMayaUIElement:alm:b duplicatable:dup:b horizontal:hr:b stateString:ss:a uiScriptCommand:uic:a close:cl:b initCallback:ic:a layoutDirectionCallback:ldc:a resizeHeight:rsh:a resizeWidth:rsw:a tabPosition:tp:a2 raise:r:b maximumWidth:mxw:a maximumHeight:mxh:a"),
    ("dockControl", "area:a:a content:con:a allowedArea:aa:a label:l:a floating:fl:b retain:ret:b raise:r:b moveable:mov:b sizeable:s:b fixedWidth:fw:b fixedHeight:fh:b enablePopupOption:epo:b dockStation:ds:a splitLayout:sl:a state:st:a closeCommand:cc:a floatChangeCommand:fcc:a"),
    ("floatSliderButtonGrp", "precision:pre:a showTrailingZeros:tze:b buttonLabel:bl:a buttonCommand:bc:a symbolButtonCommand:sbc:a symbolButtonDisplay:sbd:b image:i:a"),
    ("shelfTabLayout", "tabLabel:tl:a2 tabLabelIndex:tli:a2 selectTabIndex:sti:a selectTab:st:a changeCommand:cc:a selectCommand:sc:a preSelectCommand:psc:a doubleClickCommand:dcc:a tabsVisible:tv:b innerMarginWidth:imw:a innerMarginHeight:imh:a scrollable:scr:b childResizable:cr:b horizontalScrollBarThickness:hst:a verticalScrollBarThickness:vst:a moveTab:mt:a2 newTabCommand:ntc:a showNewTab:snt:b tabPosition:tp:a tabTooltip:tt:a2 tabsClosable:tc:b closeTabCommand:ctc:a minChildWidth:mcw:a image:i:a imageVisible:iv:b borderStyle:bs:a closeTab:ct:a generalSpacing:gsp:a margins:mar:a margins4:mr4:a4 postMenuCommand:pmc:a scrollableTabs:stb:b tabIcon:ti:a2 tabIconIndex:tii:a2 tabTooltipIndex:tti:a2"),
    ("scriptTable", "rows:r:a columns:c:a label:l:a2 columnWidth:cw:a2 rowHeight:rh:a getCellCmd:gcc:a cellChangedCmd:ccc:a afterCellChangedCmd:acc:a selectionChangedCmd:scc:a selectedRow:sr:b selectedRows:srs:a selectedColumns:scs:a selectedCells:sc:a clearTable:ct:b clearRow:cr:a deleteRow:dr:a insertRow:ir:a excludingHeaders:eh:b multiEditEnabled:mee:b underPointerRow:upr:b underPointerColumn:upc:b useDoubleClickEdit:udc:b sortEnabled:se:b cellIndex:ci:a cellValue:cv:a cellBackgroundColorCommand:cbc:a cellForegroundColorCommand:cfc:a columnFilter:cf:a2 editable:ed:b rowsRemovedCmd:rrc:a rowsToBeRemovedCmd:rtc:a selectionBehavior:sb:a selectionMode:sm:a"),
    ("switchTable", "label1:l1:a label2:l2:a selectedRow:sr:b switchNode:sn:a underPointerRow:upr:b"),
    ("layerButton", "label:l:a name:n:a changeCommand:cc:a color:cl:a command:c:a current:cr:b doubleClickCommand:dcc:a hideOnPlaybackCommand:hpc:a identification:id:a labelWidth:lw:a layerHideOnPlayback:hp:b layerState:ls:a layerVisible:lv:b renameCommand:rc:a select:s:b transparent:t:b typeCommand:tc:a visibleCommand:vc:a"),
    ("toolButton", "tool:t:a1 allowMultipleTools:amt:b collection:cl:a select:sl:b changeCommand:cc:a onCommand:onc:a offCommand:ofc:a doubleClickCommand:dcc:a image1:i1:a image2:i2:a image3:i3:a imageOverlayLabel:iol:a style:st:a popupIndicatorVisible:piv:b stateSelfManaged:ssm:b toolImage1:ti1:a2 toolImage2:ti2:a2 toolImage3:ti3:a2 toolArray:ta:b toolCount:tc:b version:ver:a ltVersion:lt:a"),
    ("outlinerEditor", "alwaysToggleSelect:ats:b animLayerFilterOptions:alf:a attrAlphaOrder:aao:a attrFilter:af:a autoExpand:xpd:b autoExpandLayers:ael:b autoSelectNewObjects:aso:b containersIgnoreFilters:cif:b directSelect:ds:b displayMode:dm:a doNotSelectNewObjects:dns:b dropIsParent:dip:b editAttrName:ean:b expandAllItems:eai:b expandAllSelectedItems:eas:b expandAttribute:xta:b expandConnections:xc:b expandObjects:eo:b feedbackItemName:fbn:b feedbackRowNumber:fbr:b getCurrentSetOfItem:gcs:a highlightActive:ha:b highlightSecondary:hs:b ignoreDagHierarchy:hir:b ignoreHiddenAttribute:iha:b ignoreOutlinerColor:ioc:b isChildSelected:ics:a isSet:isSet:a isSetMember:ism:a lineHeight:lh:a longNames:ln:b niceNames:nn:b object:obj:a organizeByClip:obc:b organizeByLayer:obl:b parentObject:po:b pinPlug:pin:a refresh:rfs:b removeFromCurrentSet:rcs:a renameItem:rni:a renameSelectedItem:rsi:b renderFilterActive:rfa:b renderFilterIndex:rfi:a renderFilterVisible:rfv:b selectCommand:sec:a setDefaultFilter:sdf:b setFilter:sf:a setsIgnoreFilters:sif:b showAnimCurvesOnly:aco:b showAnimLayerWeight:salw:b showAssets:a:b showAssignedMaterials:am:b showAttrValues:av:b showAttributes:sat:b showCompounds:cmp:b showConnected:sc:b showContainedOnly:sco:b showContainerContents:scc:b showDagOnly:dag:b showLeafs:laf:b showMuteInfo:smi:b showNamespace:sn:b showNumericAttrsOnly:num:b showParentContainers:spc:b showPinIcons:spi:b showPublishedAsConnected:spu:b showReferenceMembers:srm:b showReferenceNodes:rn:b showSelected:ssd:b showSetMembers:ssm:b showShapes:shp:b showTextureNodesOnly:tno:b showTimeEditor:ste:b showUVAttrsOnly:uv:b showUnitlessCurves:su:b showUpstreamCurves:suc:b sortOrder:so:a transmitFilters:tf:b control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("nodeEditor", "addNewNodes:ann:b addNode:an:a1 additiveGraphingMode:agm:b allAttributes:ala:b allNodes:aln:b autoSizeNodes:asn:b beginCreateNode:bcn:b consistentNameSize:cns:b createNodeCommand:cnc:a customAttributeListEdit:cal:a cycleHUD:ch:b defaultPinnedState:dps:b deleteSelected:dsl:b downstream:ds:b dotFormat:dot:a duplicateSelected:dup:b errorCallback:ecb:a extendToShapes:ets:b feedbackConnection:fbc:b feedbackNode:fbn:b feedbackPlug:fbp:b feedbackTabIndex:fbi:b feedbackType:fbt:b filterCreateNodeTypes:fcn:a focusCommand:fc:a frameAll:fa:b frameModelSelection:fms:b frameSelected:fs:b getNodeList:gnl:b graphSelectedConnections:gsc:b graphSelection:gs:b gridSnap:gsn:b gridVisibility:gv:b hasWatchpoint:hw:b hudMessage:hm:a ignoreAssets:ia:b island:isl:b keyPressCommand:kp:a keyReleaseCommand:kr:a layout:lay:b layoutCommand:lc:a nodeSwatchSize:nss:a nodeTitleMode:ntm:a nodeViewMode:nvm:a panView:pv:a pinSelectedNodes:psn:b popupMenuScript:pms:a redockSelectedNodes:rsn:b removeDownstream:rd:b removeNode:rem:a1 removeUnselected:run:b removeUpstream:ru:b rootNode:rn:a1 rootsFromSelection:rfs:b scaleView:sv:a selectAll:sa:b selectConnectionNodes:scn:b selectDownstream:sdn:b selectFeedbackConnection:sfc:b selectNode:sln:a1 selectUpstream:su:b setWatchpoint:sw:b settingsChangedCallback:scc:a shaderNetworks:sn:b showAllNodeAttributes:saa:a showNamespace:sns:b showSGShapes:ssg:b showShapes:ss:b showTransforms:st:b showUnitConversions:suc:b syncedSelection:ssl:b toggleAttrFilter:taf:b toggleSelectedPins:tsp:b toggleSwatchSize:tss:b toolTipCommand:ttc:a traversalDepthLimit:tdl:a updateNodeAdded:una:b upstream:ups:b useAssets:ua:b control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("dopeSheetEditor", "autoFit:af:a autoFitTime:aft:a displayActiveKeyTangents:dat:a displayActiveKeys:dak:a displayInfinities:di:a displayKeys:dk:a displayTangents:dtn:a displayValues:dv:a dropCommand:dc:a hierarchyBelow:hb:b lookAt:la:a outliner:o:a selectionWindow:sel:a showScene:sc:b showSummary:ss:b showTicks:stk:b snapTime:st:a snapValue:sv:a control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("animCurveEditor", "areCurvesSelected:acs:b autoFit:af:a autoFitTime:aft:a classicMode:cm:b clipTime:ct:a constrainDrag:cd:a curvesShown:cs:b denormalizeCurvesCommand:dcc:a displayActiveKeyTangents:dat:a displayActiveKeys:dak:a displayInfinities:di:a displayKeys:dk:a displayNormalized:dn:b displayTangents:dtn:a displayValues:dv:a keyingTime:kt:a lookAt:la:a menu:m:a normalizeCurvesCommand:ncc:a outliner:o:a preSelectionHighlight:psh:b renormalizeCurves:rnc:b resultSamples:rs:a resultScreenSamples:rss:a resultUpdate:ru:a showActiveCurveNames:acn:b showBufferCurves:sb:a showCurveNames:scn:b showResults:sr:a showUpstreamCurves:suc:b simpleKeyView:skv:a smoothness:s:a snapTime:st:a snapValue:sv:a stackedCurves:sc:b stackedCurvesMax:scx:a stackedCurvesMin:scm:a stackedCurvesSpace:scs:a valueLinesToggle:vlt:a control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("blendShapeEditor", "targetControlList:tcl:b targetList:tl:b verticalSliders:vs:b control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("preloadRefEd", "selectCommand:sc:a selectFileNode:sf:b control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("poseEditor", "targetControlList:tcl:b targetList:tl:b verticalSliders:vs:b control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("componentEditor", "floatField:ff:a floatSlider:fs:a hidePathName:hpn:b hideZeroColumns:hzc:b lockInput:lck:b newTab:nt:a3 operationCount:oc:b operationLabels:ol:b operationType:ot:a precision:pre:a removeTab:rt:a selected:sl:b setOperationLabel:sol:a2 showObjects:so:a showSelected:ss:b sortAlpha:sa:b control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("spreadSheetEditor", "attrRegExp:are:a execute:exe:a fixedAttrList:fal:a1 header:hdr:b keyableOnly:ko:b longNames:ln:b niceNames:nn:b precision:pre:a selectedAttr:sla:b showShapes:ssh:b control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("keyframeOutliner", "animCurve:ac:a display:dsp:a"),
    ("keyframeStats", "animEditor:ae:a precision:pre:a autoUnitWidth:auw:a timeAnnotation:tan:a valueAnnotation:van:a"),
    ("textureWindow", "capture:cpt:a changeCommand:cc:a checkerColor1:cc1:a checkerColor2:cc2:a checkerDensity:cd:a clearImage:ci:b cmEnabled:cme:b displayAxes:dax:b displayCheckered:dc:b displayDistortion:dcd:b displayDivisionLines:ddl:b displayGridLines:dgl:b displayImage:di:a displayLabels:dl:b displayPreselection:dpr:b displaySolidMap:dsm:b displayStyle:dst:a displayUVShaded:dus:b displayUVWireframe:duw:b distortionAlpha:dal:a distortionPerObject:dpo:b divisions:d:a doubleBuffer:dbf:b drawAxis:da:b drawSubregions:dsr:b exposure:exp:a frameAll:fa:b frameSelected:fs:b frontFacingColor:ffc:a gamma:ga:a gridLinesColor:glc:a gridNumbersColor:gnc:a imageBaseColor:ibc:a imageDim:imd:b imageDisplay:id:b imageNames:imn:b imageNumber:in:a imagePixelSnap:ip:b imageRatio:imr:b imageRatioValue:irv:a imageSize:is:b imageTileRange:itr:a imageUnfiltered:iuf:b internalFaces:if:b labelPosition:lp:a loadImage:li:a maxResolution:mrs:a multiColorAlpha:mca:a nbImages:nim:b nextView:nv:b numUvSets:nuv:b numberOfImages:ni:a numberOfTextures:nt:a previousView:pv:b realSize:rs:b refresh:rf:b relativeHandleSize:rhs:b removeAllImages:rai:b removeImage:ri:b rendererString:rst:a reset:r:b saveImage:si:b selectInternalFaces:sif:b selectRelated:sr:b setUvSet:suv:a singleBuffer:sbf:b size:s:a solidMap3dView:sm3:b solidMapColorSeed:smc:a solidMapPerShell:smp:b spacing:sp:a style:st:a subdivisionLinesColor:slc:a textureBorder:tb:b textureBorderColor:tbc:a textureBorderWidth:tbw:a textureNames:tn:b textureNumber:tx:a tileLabels:tlb:b tileLinesColor:tlc:a toggle:tgl:b toggleExposure:txp:b toggleGamma:tgm:b useFaceGroup:ufg:b usedPercentageHUD:upp:b uvSets:uvs:b viewPortImage:vpi:b viewTransformName:vtn:a wireframeComponentColor:wcc:a wireframeObjectColor:woc:a writeImage:wi:a control:ctl:b defineTemplate:dt:a docTag:dtg:a filter:f:a forceMainConnection:fmc:a highlightConnection:hlc:a lockMainConnection:lck:b mainListConnection:mlc:a panel:pnl:a selectionConnection:slc:a stateString:sts:b unParent:up:b unlockMainConnection:ulk:b updateMainConnection:upd:b useTemplate:ut:a"),
    ("timePort", "globalTime:gt:b snap:sn:b"),
    ("soundControl", SOUND_FLAGS),
    ("soundPopup", ""),
    ("rangeControl", "minRange:min:a maxRange:max:a changedCommand:cc:a widthHeight:wh:a2"),
    ("timeRangeControl", "minRange:min:a maxRange:max:a changedCommand:cc:a"),
    ("channelBox", crate::channel_box::FLAGS),
    ("cmdScrollFieldExecuter", crate::cmd_fields::EXECUTER),
    ("cmdScrollFieldReporter", crate::cmd_fields::REPORTER),
    ("cmdShell", crate::cmd_fields::SHELL),
    ("floatSlider2", crate::more_controls::SLIDER2_FLAGS),
    ("falloffCurve", crate::more_controls::CURVE_FLAGS),
    ("falloffCurveAttr", crate::more_controls::CURVE_ATTR_FLAGS),
    ("componentBox", crate::more_controls::COMPONENT_BOX_FLAGS),
    ("gradientControl", crate::ramp_controls::ATTR_FLAGS),
    ("gradientControlNoAttr", crate::ramp_controls::NO_ATTR_FLAGS),
    ("helpLine", ""),
];

/// Editors can be made with no layout to go in, and put in one later with -e -parent.
fn is_editor(kind: &str) -> bool {
    matches!(kind, "preloadRefEd" | "outlinerEditor" | "nodeEditor" | "dopeSheetEditor" | "animCurveEditor" | "blendShapeEditor" | "poseEditor" | "componentEditor" | "spreadSheetEditor" | "keyframeOutliner" | "textureWindow")
}

/// The widget a command makes, when it is another command's with more on it.
fn display_of(cmd: &str) -> &str {
    match cmd {
        "shelfTabLayout" => "tabLayout",
        "timeRangeControl" => "rangeControl",
        c => c,
    }
}

pub(crate) fn parse_flag(tok: &str) -> Option<FlagDef> {
    let mut it = tok.split(':');
    let (long, short, ty) = (it.next()?, it.next()?, it.next()?);
    // b a switch, l a list of words (a string array), a anything, aN N values and used more than once.
    let kind = match ty {
        "b" => ArgType::Bool,
        "l" => ArgType::List,
        _ => ArgType::Any,
    };
    let mut d = FlagDef::new(long, short, kind, Value::Null, "");
    if let Some(n) = ty.strip_prefix('a').and_then(|n| n.parse::<u8>().ok()) {
        d = d.multi(n);
    }
    Some(d)
}

pub(crate) fn add_flags(mut c: Cmd, spec: &str) -> Cmd {
    for t in spec.split_whitespace() {
        if let Some(d) = parse_flag(t) {
            if !c.flags.iter().any(|f| f.name == d.name) {
                c = c.flag(d);
            }
        }
    }
    c
}

/// The -cw2 -cl3 -ad4 family of the group controls and rowLayout.
pub(crate) fn add_column_flags(mut c: Cmd) -> Cmd {
    for n in 1..=6u8 {
        if c.flags.iter().any(|f| f.name == format!("columnWidth{n}")) {
            continue;
        }
        for (long, short, multi) in [("columnWidth", "cw", n), ("columnAlign", "cl", n), ("columnAttach", "ct", n), ("columnOffset", "co", n)] {
            c = c.flag(FlagDef::new(&format!("{long}{n}"), &format!("{short}{n}"), ArgType::Any, Value::Null, "").multi(multi));
        }
        if n > 1 {
            c = c.flag(FlagDef::new(&format!("adjustableColumn{n}"), &format!("ad{n}"), ArgType::Any, Value::Null, ""));
        }
    }
    c
}

fn is_group(kind: &str) -> bool {
    kind.ends_with("Grp")
}

/// The groups that hold a colour with -rgbValue, -hsvValue and -alphaValue.
fn is_color_group(kind: &str) -> bool {
    matches!(kind, "colorSliderGrp" | "colorSliderButtonGrp" | "colorInputWidgetGrp")
}

/// Hue in degrees (0 to 360), saturation and value 0 to 1, as the reference's -hsvValue.
pub(crate) fn rgb_to_hsv([r, g, b]: [f64; 3]) -> [f64; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max <= 0.0 { 0.0 } else { d / max };
    [h, s, max]
}

pub(crate) fn hsv_to_rgb([h, s, v]: [f64; 3]) -> [f64; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match h as i64 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

// ---------------------------------------------------------------------------------------------------------------------
// Applying flags

const META: &[&str] = &["query", "edit", "queried", "exists", "parent", "unParent", "defineTemplate", "useTemplate", "fullPathName", "isObscured", "numberOfPopupMenus", "popupMenuArray", "childArray", "numberOfChildren"];

fn is_int_kind(kind: &str) -> bool {
    matches!(kind, "intField" | "intSlider" | "intScrollBar" | "intSliderGrp" | "intFieldGrp" | "colorIndexSliderGrp")
}

fn is_float_kind(kind: &str) -> bool {
    matches!(kind, "floatField" | "floatSlider" | "floatScrollBar" | "floatSliderGrp" | "floatFieldGrp" | "floatSliderButtonGrp")
}

fn numeric(kind: &str, v: &Value) -> Value {
    if is_int_kind(kind) {
        json!(number(v) as i64)
    } else {
        json!(number(v))
    }
}

/// Puts flags on a control; returns the properties the shell needs to hear about.
fn apply(m: &mut Model, id: Id, flags: &Map<String, Value>) -> Result<Map<String, Value>, String> {
    let mut changed = Map::new();
    let kind = m.get(id).map(|c| c.kind.clone()).unwrap_or_default();
    let nav = m.get(id).and_then(|c| c.props.get("__cmd")).and_then(Value::as_str) == Some("attrNavigationControlGrp");
    for (key, raw) in flags {
        if META.contains(&key.as_str()) || raw.is_null() {
            continue;
        }
        // -alphaValue counts only beside -rgbValue or -hsvValue in the same call.
        if key == "alphaValue" && is_color_group(&kind) && !flags.get("rgbValue").is_some_and(|v| !v.is_null()) && !flags.get("hsvValue").is_some_and(|v| !v.is_null()) {
            continue;
        }
        if is_callback(key) || (nav && NAV_SCRIPTS.contains(&key.as_str())) {
            // -sourceType beside a callback says which language its string is; commandLine's is the line's language.
            let lang = if kind == "commandLine" { None } else { flags.get("sourceType").and_then(Value::as_str) };
            let cb = match (lang, raw) {
                (Some("python"), Value::String(s)) => json!({ "__pysrc__": s }),
                (Some("mel"), Value::String(_)) => raw.clone(),
                _ => callback_value(raw),
            };
            if let Some(c) = m.controls.get_mut(&id) {
                c.props.insert(key.clone(), cb);
            }
            // The shell has no use for scripts, but has to know a control takes part in drags.
            match key.as_str() {
                "dragCallback" => {
                    changed.insert("hasDrag".to_string(), json!(true));
                }
                "dropCallback" => {
                    changed.insert("hasDrop".to_string(), json!(true));
                }
                // A node dropped on it (shelfButton), and a rectangle to draw as a drag hovers (text).
                "handleNodeDropCallback" => {
                    changed.insert("hasNodeDrop".to_string(), json!(true));
                }
                "dropRectCallback" => {
                    changed.insert("hasDropRect".to_string(), json!(true));
                }
                "labelEditingCallback" => {
                    changed.insert("hasLabelEdit".to_string(), json!(true));
                }
                // attrNavigationControlGrp: a node dropped on it goes to these.
                "connectAttrToDropped" | "connectNodeToDropped" if nav => {
                    changed.insert("hasNodeDrop".to_string(), json!(true));
                    changed.insert("nodeDropFlag".to_string(), json!(NAV_DROP));
                }
                _ => {}
            }
            continue;
        }
        apply_one(m, id, &kind, key, raw, &mut changed)?;
    }
    if kind == "iconTextRadioCollection" {
        if let Some(raw) = flags.get("select").filter(|v| !v.is_null()) {
            let quiet = flags.get("disableCommands").is_some_and(truth);
            collection_pick(m, id, &text(raw), quiet)?;
        }
    }
    // attrNavigationControlGrp's right-click menu: the keyable items unless -noKeyableMenu, Ignore with -ignore.
    if nav && (flags.contains_key("ignore") || flags.contains_key("noKeyableMenu") || !m.get(id).is_some_and(|c| c.props.contains_key("__navMenu"))) {
        if let Some(c) = m.controls.get_mut(&id) {
            c.props.insert("__navMenu".into(), json!(true));
            let keyable = !c.props.get("noKeyableMenu").is_some_and(truth);
            changed.insert("navMenu".to_string(), json!({"keyable": keyable, "ignore": c.props.contains_key("ignore")}));
        }
    }
    // A shelf button's popup menu: -menuItem label command, -menuItemWithOptionBox label command optionCommand, in the
    // order given, -menuItemPython marking which are Python.
    if kind == "shelfButton" && ["menuItem", "menuItemWithOptionBox", "menuItemPython"].iter().any(|k| flags.contains_key(*k)) {
        shelf_menu(m, id, &mut changed);
    }
    Ok(changed)
}

/// Builds a shelf button's popup entries from its -menuItem, -menuItemWithOptionBox and -menuItemPython: the labels for
/// the shell, each entry's command (and option box command) kept as callbacks to fire by number.
fn shelf_menu(m: &mut Model, id: Id, changed: &mut Map<String, Value>) {
    let prop = |m: &Model, k: &str| m.get(id).and_then(|c| c.props.get(k)).cloned().unwrap_or(Value::Null);
    let plain = flat(&prop(m, "menuItem"));
    let boxed = flat(&prop(m, "menuItemWithOptionBox"));
    let python: Vec<i64> = flat(&prop(m, "menuItemPython")).iter().map(|v| number(v) as i64).collect();
    let mut entries: Vec<(String, String, Option<String>)> = plain.chunks(2).filter(|c| c.len() == 2).map(|c| (text(&c[0]), text(&c[1]), None)).collect();
    entries.extend(boxed.chunks(3).filter(|c| c.len() == 3).map(|c| (text(&c[0]), text(&c[1]), Some(text(&c[2])))));
    let mut labels = Vec::new();
    for (i, (label, cmd, opt)) in entries.iter().enumerate() {
        let py = python.contains(&(i as i64));
        let cb = |src: &str| if py { json!({ "__pysrc__": src }) } else { json!(src) };
        if let Some(c) = m.controls.get_mut(&id) {
            c.props.insert(format!("menuItemCommand{}", i + 1), cb(cmd));
            if let Some(o) = opt {
                c.props.insert(format!("menuItemOptionCommand{}", i + 1), cb(o));
            }
        }
        labels.push(json!({"label": label, "optionBox": opt.is_some()}));
    }
    set(m, id, "menuEntries", Value::Array(labels), changed);
}

fn set(m: &mut Model, id: Id, key: &str, v: Value, changed: &mut Map<String, Value>) {
    if let Some(c) = m.controls.get_mut(&id) {
        // An editor's main connection is locked by one flag and unlocked by another: both answer -q -lockMainConnection.
        if key == "unlockMainConnection" && v.as_bool() == Some(true) {
            c.props.insert("lockMainConnection".into(), json!(false));
        }
        c.props.insert(key.to_string(), v.clone());
    }
    changed.insert(key.to_string(), v);
}

fn apply_one(m: &mut Model, id: Id, kind: &str, key: &str, raw: &Value, changed: &mut Map<String, Value>) -> Result<(), String> {
    let vals = flat(raw);
    match (kind, key) {
        // The colour groups hold one colour: either form sets it, and both answer -q.
        (k, "rgbValue" | "hsvValue") if is_color_group(k) => {
            if vals.len() < 3 {
                return Err(format!("{kind}: -{key} takes three values."));
            }
            let three = [number(&vals[0]), number(&vals[1]), number(&vals[2])];
            let (rgb, hsv) = if key == "rgbValue" { (three, rgb_to_hsv(three)) } else { (hsv_to_rgb(three), three) };
            set(m, id, "rgbValue", json!(rgb), changed);
            set(m, id, "hsvValue", json!(hsv), changed);
        }
        (k, "alphaValue") if is_color_group(k) => set(m, id, key, json!(number(raw).clamp(0.0, 1.0)), changed),
        ("commandLine", "numberOfHistoryLines") => set(m, id, key, json!((number(raw) as i64).max(0)), changed),
        ("commandLine", "sourceType") => {
            let t = text(raw);
            if t != "mel" && t != "python" {
                return Err(format!("commandLine: -sourceType is \"mel\" or \"python\", not \"{t}\"."));
            }
            set(m, id, key, json!(t), changed);
        }
        (_, "widthHeight") => {
            if vals.len() >= 2 {
                set(m, id, "width", json!(number(&vals[0]) as i64), changed);
                set(m, id, "height", json!(number(&vals[1]) as i64), changed);
            }
        }
        (_, "width" | "height") => set(m, id, key, json!(number(raw) as i64), changed),
        // A window's place: kept as its two edges, sent to the shell as one corner.
        ("window", "topLeftCorner" | "topEdge" | "leftEdge") => {
            if key == "topLeftCorner" {
                if vals.len() >= 2 {
                    set(m, id, "topEdge", json!(number(&vals[0]) as i64), changed);
                    set(m, id, "leftEdge", json!(number(&vals[1]) as i64), changed);
                }
            } else {
                set(m, id, key, json!(number(raw) as i64), changed);
            }
            let props = m.get(id).map(|c| c.props.clone()).unwrap_or_default();
            let edge = |k: &str| props.get(k).map(|v| number(v) as i64).unwrap_or(0);
            changed.insert("topLeftCorner".into(), json!([edge("topEdge"), edge("leftEdge")]));
        }
        ("dockControl", "state") => {
            // What -q -state gave: hex of the dock's settings.
            let t = text(raw);
            let bytes: Option<Vec<u8>> = (0..t.len() / 2).map(|i| u8::from_str_radix(t.get(i * 2..i * 2 + 2)?, 16).ok()).collect();
            let st: Value = bytes.and_then(|b| serde_json::from_slice(&b).ok()).ok_or_else(|| "dockControl: -state takes what -q -state gave.".to_string())?;
            for k in ["area", "floating", "width", "height", "visible", "raise"] {
                if let Some(v) = st.get(k).filter(|v| !v.is_null()) {
                    set(m, id, k, v.clone(), changed);
                }
            }
        }
        ("dockControl", "splitLayout") => {
            let t = text(raw);
            if t != "horizontal" && t != "vertical" {
                return Err(format!("dockControl: -splitLayout is horizontal or vertical, not {t}."));
            }
            set(m, id, key, json!(t), changed);
        }
        ("flowLayout", "horizontal") => {
            // The obsolete -vertical is the other way of saying it.
            set(m, id, "vertical", json!(!truth(raw)), changed);
            set(m, id, key, json!(truth(raw)), changed);
        }
        ("textScrollList" | "iconTextScrollList", _) => list_flag(m, id, key, raw, changed)?,
        // The icon controls' overlay colours, margins and picture turn.
        ("iconTextCheckBox" | "iconTextRadioButton" | "iconTextStaticLabel", "overlayLabelColor" | "overlayLabelBackColor") => {
            let n = if key == "overlayLabelColor" { 3 } else { 4 };
            if vals.len() != n {
                return Err(format!("{kind}: -{key} takes {n} values."));
            }
            set(m, id, key, json!(vals.iter().map(|v| number(v).clamp(0.0, 1.0)).collect::<Vec<_>>()), changed);
        }
        ("iconTextCheckBox" | "iconTextRadioButton" | "iconTextStaticLabel", "marginWidth" | "marginHeight") => set(m, id, key, json!((number(raw) as i64).max(0)), changed),
        ("iconTextCheckBox" | "iconTextRadioButton" | "iconTextStaticLabel", "labelOffset") => set(m, id, key, json!(number(raw) as i64), changed),
        ("iconTextCheckBox" | "iconTextRadioButton" | "iconTextStaticLabel", "rotation") => set(m, id, key, json!(number(raw)), changed),
        // -menuIndex menu index: the menu moves to that place (from 0) among the bar's menus.
        ("window" | "menuBarLayout", "menuIndex") => {
            for ch in vals.chunks(2) {
                let [which, at] = ch else { return Err(format!("{kind}: -menuIndex takes a menu and an index.")) };
                let want = text(which);
                let kids = m.get(id).map(|c| c.children.clone()).unwrap_or_default();
                let menus: Vec<Id> = kids.iter().copied().filter(|k| m.get(*k).is_some_and(|x| x.kind == "menu")).collect();
                let Some(&mv) = menus.iter().find(|k| m.get(**k).is_some_and(|x| x.name == want || x.path == want)) else { return Err(format!("{kind}: no menu {want} on the bar.")) };
                let mut order: Vec<Id> = menus.iter().copied().filter(|k| *k != mv).collect();
                let at = (number(at).max(0.0) as usize).min(order.len());
                order.insert(at, mv);
                // The menus take the places menus had among the children, in the new order.
                let mut next = order.iter();
                let new_kids: Vec<Id> = kids.iter().map(|k| if menus.contains(k) { *next.next().unwrap() } else { *k }).collect();
                if let Some(c) = m.controls.get_mut(&id) {
                    c.children = new_kids;
                }
                let paths: Vec<String> = order.iter().filter_map(|k| m.get(*k)).map(|x| x.path.clone()).collect();
                set(m, id, "menuOrder", json!(paths), changed);
            }
        }
        // -menuBarCornerWidget widget corner: a control shown in a corner of the menu bar.
        ("window", "menuBarCornerWidget") => {
            let mut corners = m.get(id).and_then(|c| c.props.get("cornerWidgets")).and_then(Value::as_object).cloned().unwrap_or_default();
            for ch in vals.chunks(2) {
                let [w, corner] = ch else { return Err("window: -menuBarCornerWidget takes a control and a corner.".into()) };
                let corner = text(corner);
                if !matches!(corner.as_str(), "topLeft" | "topRight" | "bottomLeft" | "bottomRight") {
                    return Err(format!("window: the corner is topLeft, topRight, bottomLeft or bottomRight, not {corner}."));
                }
                let name = text(w);
                let path = m.find(&name).and_then(|i| m.get(i)).map(|c| c.path.clone()).ok_or_else(|| format!("Object '{name}' not found."))?;
                corners.insert(corner, json!(path));
            }
            set(m, id, "cornerWidgets", Value::Object(corners), changed);
        }
        ("workspaceControl", "resizeWidth") => set(m, id, "width", json!(number(raw) as i64), changed),
        ("workspaceControl", "resizeHeight") => set(m, id, "height", json!(number(raw) as i64), changed),
        // -tabPosition north|east|west, and whether every control of its tab group takes it.
        ("workspaceControl", "tabPosition") => {
            let pos = vals.first().map(text).unwrap_or_default();
            if !matches!(pos.as_str(), "north" | "east" | "west") {
                return Err(format!("workspaceControl: -tabPosition is north, east or west, not {pos}."));
            }
            set(m, id, "tabPosition", json!(pos), changed);
            set(m, id, "tabPositionAll", json!(vals.get(1).is_some_and(truth)), changed);
        }
        // A change of orientation tells -layoutDirectionCallback: 1 horizontal, 0 vertical.
        ("workspaceControl", "horizontal") => {
            let on = truth(raw);
            let (made, was, cb) = m.get(id).map_or((false, false, None), |c| (c.props.contains_key("visible"), c.props.get("horizontal").is_some_and(truth), c.props.get("layoutDirectionCallback").cloned()));
            set(m, id, key, json!(on), changed);
            if made && was != on {
                if let Some(Value::String(p)) = cb {
                    run_soon(json!(format!("{p} {}", on as i32)), Vec::new());
                }
            }
        }
        ("formLayout", "attachForm" | "attachControl" | "attachPosition" | "attachNone" | "attachOppositeForm" | "attachOppositeControl") => form_flag(m, id, key, &vals, changed)?,
        ("tabLayout", "tabLabel") => {
            for ch in vals.chunks(2) {
                if let [c, l] = ch {
                    let Some(child) = m.find(&text(c)) else { return Err(format!("Object '{}' not found.", text(c))) };
                    let path = m.get(child).unwrap().path.clone();
                    let labels = tab_labels(m, id);
                    let mut labels = labels;
                    labels.insert(path, json!(text(l)));
                    set(m, id, "tabLabels", Value::Object(labels), changed);
                }
            }
        }
        ("tabLayout", "tabLabelIndex") => {
            for ch in vals.chunks(2) {
                if let [i, l] = ch {
                    let kids = m.get(id).map(|c| c.children.clone()).unwrap_or_default();
                    let Some(&k) = kids.get((number(i) as usize).saturating_sub(1)) else { return Err("tabLabelIndex: no such tab.".into()) };
                    let path = m.get(k).unwrap().path.clone();
                    let mut labels = tab_labels(m, id);
                    labels.insert(path, json!(text(l)));
                    set(m, id, "tabLabels", Value::Object(labels), changed);
                }
            }
        }
        ("tabLayout", "tabTooltip" | "tabIcon") => {
            let store = if key == "tabTooltip" { "tabTooltips" } else { "tabIcons" };
            for ch in vals.chunks(2) {
                if let [c, l] = ch {
                    let Some(child) = m.find(&text(c)) else { return Err(format!("Object '{}' not found.", text(c))) };
                    let path = m.get(child).unwrap().path.clone();
                    let mut map = m.get(id).and_then(|c| c.props.get(store)).and_then(Value::as_object).cloned().unwrap_or_default();
                    map.insert(path, json!(text(l)));
                    set(m, id, store, Value::Object(map), changed);
                }
            }
        }
        ("tabLayout", "tabTooltipIndex" | "tabIconIndex") => {
            let store = if key == "tabTooltipIndex" { "tabTooltips" } else { "tabIcons" };
            for ch in vals.chunks(2) {
                if let [i, l] = ch {
                    let kids = m.get(id).map(|c| c.children.clone()).unwrap_or_default();
                    let Some(&k) = kids.get((number(i) as usize).saturating_sub(1)) else { return Err(format!("{key}: no such tab.")) };
                    let path = m.get(k).unwrap().path.clone();
                    let mut map = m.get(id).and_then(|c| c.props.get(store)).and_then(Value::as_object).cloned().unwrap_or_default();
                    map.insert(path, json!(text(l)));
                    set(m, id, store, Value::Object(map), changed);
                }
            }
        }
        ("tabLayout", "closeTab") => {
            // By 1-based index, as the other index flags: the tab and what is in it go.
            let kids = m.get(id).map(|c| c.children.clone()).unwrap_or_default();
            let i = number(raw) as usize;
            let Some(&k) = kids.get(i.saturating_sub(1)).filter(|_| i >= 1) else { return Err("closeTab: no such tab.".into()) };
            for cb in m.delete(k) {
                run_soon(cb, Vec::new());
            }
            let n = m.get(id).map(|c| c.children.len()).unwrap_or(0);
            let sel = m.get(id).and_then(|c| c.props.get("selectTabIndex")).map(number).unwrap_or(1.0) as usize;
            if sel > n {
                set(m, id, "selectTabIndex", json!(n.max(1)), changed);
            }
        }
        ("tabLayout", "borderStyle") => {
            let t = text(raw);
            if !matches!(t.as_str(), "none" | "top" | "notop" | "full") {
                return Err(format!("borderStyle: none, top, notop or full, not {t}."));
            }
            set(m, id, key, json!(t), changed);
        }
        ("shelfLayout", "horizontal") => set(m, id, key, json!(truth(raw)), changed),
        ("tabLayout", "selectTab") => {
            let name = text(raw);
            let kids = m.get(id).map(|c| c.children.clone()).unwrap_or_default();
            let Some(pos) = kids.iter().position(|k| m.get(*k).is_some_and(|c| c.name == name || c.path == name)) else { return Err(format!("Object '{name}' not found.")) };
            set(m, id, "selectTabIndex", json!(pos + 1), changed);
        }
        ("tabLayout", "selectTabIndex") => set(m, id, key, json!(number(raw) as i64), changed),
        (_, "value" | "minValue" | "maxValue" | "step" | "fieldMinValue" | "fieldMaxValue" | "sliderStep" | "fieldStep") if is_int_kind(kind) || is_float_kind(kind) => {
            if key == "value" && kind.ends_with("FieldGrp") {
                for (i, v) in vals.iter().take(4).enumerate() {
                    set(m, id, &format!("value{}", i + 1), numeric(kind, v), changed);
                }
            } else {
                set(m, id, key, numeric(kind, raw), changed);
            }
        }
        (_, "value1" | "value2" | "value3" | "value4") if kind.ends_with("FieldGrp") => set(m, id, key, numeric(kind, raw), changed),
        (_, "precision" | "numberOfFields" | "numberOfCheckBoxes" | "numberOfRadioButtons" | "numberOfColumns" | "numberOfRows" | "numberOfDivisions") => set(m, id, key, json!(number(raw) as i64), changed),
        ("checkBoxGrp", "valueArray2" | "valueArray3" | "valueArray4") => {
            for (i, v) in vals.iter().take(4).enumerate() {
                set(m, id, &format!("value{}", i + 1), json!(truth(v)), changed);
            }
        }
        ("checkBoxGrp" | "radioButtonGrp", "labelArray2" | "labelArray3" | "labelArray4") => {
            for (i, v) in vals.iter().take(4).enumerate() {
                set(m, id, &format!("label{}", i + 1), json!(text(v)), changed);
            }
        }
        ("radioButtonGrp", "select") => {
            let n = number(raw) as i64;
            set(m, id, key, json!(n), changed);
            if n > 0 {
                share_exclusive(m, id);
            }
        }
        ("optionMenu" | "optionMenuGrp", "value") => {
            let want = text(raw);
            let items = menu_items(m, id);
            let Some(i) = items.iter().position(|(_, l)| *l == want) else { return Err(format!("Can't find menu item with label \"{want}\".")) };
            set(m, id, "select", json!(i + 1), changed);
        }
        ("optionMenu" | "optionMenuGrp", "select") => set(m, id, key, json!(number(raw) as i64), changed),
        ("optionMenu" | "optionMenuGrp" | "menu" | "popupMenu", "deleteAllItems") => {
            if truth(raw) {
                let kids = m.get(id).map(|c| c.children.clone()).unwrap_or_default();
                for k in kids {
                    m.delete(k);
                }
                changed.remove("select");
            }
        }
        ("radioButton" | "iconTextRadioButton" | "toolButton", "select") => {
            let on = truth(raw);
            set(m, id, key, json!(on), changed);
            if on {
                radio_exclusive(m, id);
            }
        }
        // -tool adds tools to the button; one already on it becomes the current one.
        ("toolButton", "tool") => {
            let mut tools: Vec<String> = m.get(id).and_then(|c| c.props.get("tool")).map(flat).unwrap_or_default().iter().map(text).collect();
            let mut cur = m.get(id).and_then(|c| c.props.get("currentTool")).map(text);
            for v in &vals {
                let t = text(v);
                if tools.contains(&t) {
                    cur = Some(t);
                } else {
                    tools.push(t.clone());
                    if cur.is_none() {
                        cur = Some(t);
                    }
                }
            }
            set(m, id, "tool", json!(tools), changed);
            set(m, id, "currentTool", json!(cur.unwrap_or_default()), changed);
        }
        // Pairs of tool and image: the image the button shows while that tool is its current one.
        ("toolButton", "toolImage1" | "toolImage2" | "toolImage3") => {
            let mut pairs: Vec<Value> = m.get(id).and_then(|c| c.props.get(key)).map(flat).unwrap_or_default();
            for ch in vals.chunks(2) {
                let [t, img] = ch else { return Err(format!("toolButton: -{key} takes a tool and an image.")) };
                match pairs.chunks(2).position(|p| text(&p[0]) == text(t)) {
                    Some(i) => pairs[i * 2 + 1] = json!(text(img)),
                    None => pairs.extend([json!(text(t)), json!(text(img))]),
                }
            }
            set(m, id, key, Value::Array(pairs), changed);
        }
        // Every item of the collection unselected.
        ("toolCollection" | "radioCollection" | "iconTextRadioCollection", "clear") => {
            if truth(raw) {
                for k in collection_members(m, id) {
                    if let Some(c) = m.controls.get_mut(&k) {
                        c.props.insert("select".into(), json!(false));
                        op(json!({"op": "edit", "path": c.path, "props": {"select": false}}));
                    }
                }
            }
        }
        // Picked in apply, which also sees -disableCommands.
        ("iconTextRadioCollection", "select" | "disableCommands") => {}
        ("radioCollection" | "toolCollection", "select") => {
            let name = text(raw);
            let kids = collection_members(m, id);
            if name == "NONE" {
                for k in kids {
                    if let Some(c) = m.controls.get_mut(&k) {
                        c.props.insert("select".into(), json!(false));
                        op(json!({"op": "edit", "path": c.path, "props": {"select": false}}));
                    }
                }
            } else {
                let Some(b) = kids.iter().copied().find(|k| m.get(*k).is_some_and(|c| c.name == name || c.path == name)) else { return Err(format!("Object '{name}' not found.")) };
                if let Some(c) = m.controls.get_mut(&b) {
                    c.props.insert("select".into(), json!(true));
                    op(json!({"op": "edit", "path": c.path, "props": {"select": true}}));
                }
                radio_exclusive(m, b);
            }
        }
        ("scriptTable", _) => table_flag(m, id, key, raw, changed),
        ("paneLayout", "paneSize") => {
            let mut sizes = m.get(id).and_then(|c| c.props.get("paneSizes")).and_then(Value::as_object).cloned().unwrap_or_default();
            for ch in vals.chunks(3) {
                if let [pane, w, h] = ch {
                    sizes.insert((number(pane) as i64).to_string(), json!([number(w) as i64, number(h) as i64]));
                }
            }
            set(m, id, "paneSizes", Value::Object(sizes), changed);
        }
        ("scrollField", "clear") => {
            if truth(raw) {
                set(m, id, "text", json!(""), changed);
            }
        }
        ("textField" | "textFieldGrp" | "textFieldButtonGrp" | "scrollField", "insertText") => {
            let cur = m.get(id).and_then(|c| c.props.get("text")).map(text).unwrap_or_default();
            set(m, id, "text", json!(cur + &text(raw)), changed);
        }
        ("textField" | "textFieldGrp" | "textFieldButtonGrp" | "scrollField", "text") => set(m, id, key, json!(text(raw)), changed),
        ("progressBar", "progress" | "minValue" | "maxValue" | "step") => progress_flag(m, id, key, raw, changed),
        // Scrubbing: time changes play the sound under the mouse until -endScrub.
        ("soundControl" | "timeControl", "beginScrub" | "endScrub") => {
            if truth(raw) {
                set(m, id, "scrubbing", json!(key == "beginScrub"), changed);
            }
        }
        // Draw again: the sound's waveform resampled to the width, or the keys read again.
        ("soundControl" | "timeControl", "resample" | "forceRefresh" | "forceRedraw") => {
            if truth(raw) {
                let n = m.get(id).and_then(|c| c.props.get("redraws")).and_then(Value::as_i64).unwrap_or(0) + 1;
                set(m, id, "redraws", json!(n), changed);
            }
        }
        ("soundControl" | "timeControl", "waveform") => {
            let t = text(raw);
            if !matches!(t.as_str(), "top" | "bottom" | "both") {
                return Err(format!("{kind}: -waveform is top, bottom or both, not {t}."));
            }
            set(m, id, key, json!(t), changed);
        }
        ("soundControl" | "timeControl", "soundSource") => {
            // The sound shown: an audio node, or "", "<sequencer>" or "<trax>"; -sound names the node alone.
            let t = text(raw);
            set(m, id, key, json!(t), changed);
            if !t.starts_with('<') {
                set(m, id, "sound", json!(t), changed);
            }
        }
        ("soundControl" | "timeControl", "sound") => {
            let t = text(raw);
            set(m, id, key, json!(t), changed);
            set(m, id, "soundSource", json!(t), changed);
        }
        ("soundControl" | "timeControl", "minTime" | "maxTime" | "repeatChunkSize") => set(m, id, key, json!(number(raw)), changed),
        ("timeControl", "tickSpan") => set(m, id, key, json!((number(raw) as i64).max(1)), changed),
        ("timeControl", "currentFrameColor") => {
            if vals.len() != 4 {
                return Err("timeControl: -currentFrameColor takes red, green, blue and alpha.".into());
            }
            set(m, id, key, json!(vals.iter().map(number).collect::<Vec<_>>()), changed);
        }
        ("timeControl", "foregroundColor") => {
            if vals.len() != 3 {
                return Err("timeControl: -foregroundColor takes red, green and blue.".into());
            }
            set(m, id, key, json!(vals.iter().map(number).collect::<Vec<_>>()), changed);
        }
        ("timeControl", "animLayerFilterOptions") => {
            let t = text(raw);
            if !matches!(t.as_str(), "allAffecting" | "active" | "animLayerEditor") {
                return Err(format!("timeControl: -animLayerFilterOptions is allAffecting, active or animLayerEditor, not {t}."));
            }
            set(m, id, key, json!(t), changed);
        }
        ("progressBar", "beginProgress") => {
            if truth(raw) {
                set(m, id, "progress", json!(0), changed);
                set(m, id, "active", json!(true), changed);
                set(m, id, "isCancelled", json!(false), changed);
            }
        }
        ("progressBar", "endProgress") => {
            if truth(raw) {
                set(m, id, "active", json!(false), changed);
            }
        }
        _ => {
            // Flags by value: a number where a switch is given and so on stay as they came.
            let v = match raw {
                Value::Array(a) if a.len() == 1 && !matches!(key, "columnWidth" | "backgroundColor") => a[0].clone(),
                o => o.clone(),
            };
            set(m, id, key, v, changed);
        }
    }
    Ok(())
}

/// A scriptTable's cells live with it, as rows of strings; the core fills them from -getCellCmd.
fn table_cells(m: &Model, id: Id) -> Vec<Vec<String>> {
    m.get(id).and_then(|c| c.props.get("cells")).and_then(Value::as_array).map(|rows| rows.iter().map(|r| r.as_array().map(|x| x.iter().map(text).collect()).unwrap_or_default()).collect()).unwrap_or_default()
}

fn table_flag(m: &mut Model, id: Id, key: &str, raw: &Value, changed: &mut Map<String, Value>) {
    let get = |m: &Model, k: &str| m.get(id).and_then(|c| c.props.get(k)).map(number).unwrap_or(0.0) as i64;
    let mut cells = table_cells(m, id);
    let row = (number(raw) as usize).max(1) - 1;
    match key {
        "rows" | "columns" | "rowHeight" => set(m, id, key, json!(number(raw) as i64), changed),
        "cells" => {
            set(m, id, key, raw.clone(), changed);
            filter_rows(m, id, changed);
        }
        "columnFilter" => {
            // Each column's filter, kept by column: 0 filters on every column.
            let v = flat(raw);
            let (col, pat) = (v.first().map(number).unwrap_or(0.0) as i64, v.get(1).map(text).unwrap_or_default());
            let mut filters = m.get(id).and_then(|c| c.props.get("columnFilters")).and_then(Value::as_object).cloned().unwrap_or_default();
            if pat.is_empty() {
                filters.remove(&col.to_string());
            } else {
                filters.insert(col.to_string(), json!(pat));
            }
            set(m, id, "columnFilters", Value::Object(filters), changed);
            set(m, id, key, raw.clone(), changed);
            filter_rows(m, id, changed);
        }
        "clearTable" => {
            if truth(raw) {
                set(m, id, "cells", json!(Vec::<Vec<String>>::new()), changed);
                set(m, id, "rows", json!(0), changed);
                set(m, id, "selectedCells", json!([]), changed);
            }
        }
        "clearRow" => {
            if let Some(r) = cells.get_mut(row) {
                r.iter_mut().for_each(String::clear);
            }
            set(m, id, "cells", json!(cells), changed);
        }
        "insertRow" => {
            let cols = get(m, "columns").max(0) as usize;
            cells.insert(row.min(cells.len()), vec![String::new(); cols]);
            let n = get(m, "rows") + 1;
            set(m, id, "rows", json!(n), changed);
            set(m, id, "cells", json!(cells), changed);
        }
        "deleteRow" => {
            if row < cells.len() {
                cells.remove(row);
            }
            let n = (get(m, "rows") - 1).max(0);
            set(m, id, "rows", json!(n), changed);
            set(m, id, "cells", json!(cells), changed);
        }
        "cellValue" => {
            let at = m.get(id).and_then(|c| c.props.get("cellIndex")).map(flat).unwrap_or_default();
            if let [r, c, ..] = at.as_slice() {
                let (r, c) = ((number(r) as usize).max(1) - 1, (number(c) as usize).max(1) - 1);
                if r < cells.len() && c < cells[r].len() {
                    cells[r][c] = text(raw);
                    set(m, id, "cells", json!(cells), changed);
                }
            }
        }
        _ => {
            let v = match raw {
                Value::Array(a) if a.len() == 1 => a[0].clone(),
                o => o.clone(),
            };
            set(m, id, key, v, changed);
        }
    }
}

/// The reference's column filter match: case does not count, `*` any run, `?` any one character, `[...]` one of a set.
pub(crate) fn wildcard_match(pat: &str, s: &str) -> bool {
    let p: Vec<char> = pat.to_lowercase().chars().collect();
    let t: Vec<char> = s.to_lowercase().chars().collect();
    fn at(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') => (0..=t.len()).any(|k| at(&p[1..], &t[k..])),
            Some('?') => !t.is_empty() && at(&p[1..], &t[1..]),
            Some('[') => {
                let Some(end) = p.iter().position(|&c| c == ']') else { return !t.is_empty() && t[0] == '[' && at(&p[1..], &t[1..]) };
                let set = &p[1..end];
                let Some(&c) = t.first() else { return false };
                let mut hit = false;
                let mut i = 0;
                while i < set.len() {
                    if i + 2 < set.len() && set[i + 1] == '-' {
                        hit |= set[i] <= c && c <= set[i + 2];
                        i += 3;
                    } else {
                        hit |= set[i] == c;
                        i += 1;
                    }
                }
                hit && at(&p[end + 1..], &t[1..])
            }
            Some(&c) => t.first() == Some(&c) && at(&p[1..], &t[1..]),
        }
    }
    at(&p, &t)
}

/// The rows the column filters hide, 1-based, for the shell.
fn filter_rows(m: &mut Model, id: Id, changed: &mut Map<String, Value>) {
    let cells = table_cells(m, id);
    let filters: Vec<(i64, String)> = m
        .get(id)
        .and_then(|c| c.props.get("columnFilters"))
        .and_then(Value::as_object)
        .map(|o| o.iter().map(|(k, v)| (k.parse().unwrap_or(0), text(v))).collect())
        .unwrap_or_default();
    let mut hidden = Vec::new();
    for (r, row) in cells.iter().enumerate() {
        let shows = filters.iter().all(|(col, pat)| {
            if *col <= 0 {
                row.iter().any(|x| wildcard_match(pat, x))
            } else {
                row.get(*col as usize - 1).is_some_and(|x| wildcard_match(pat, x))
            }
        });
        if !shows {
            hidden.push(json!(r + 1));
        }
    }
    set(m, id, "hiddenRows", Value::Array(hidden), changed);
}

/// Cells from the core's -getCellCmd calls, or one the user changed, as an edit the shell hears about.
pub(crate) fn table_set_cells(path: &str, cells: Vec<Vec<String>>) {
    let _ = model_edit(path, &Map::from_iter([("cells".to_string(), json!(cells))]));
}

/// The tables whose cells have to be asked for: path, script, rows, columns.
pub(crate) fn take_dirty_tables() -> Vec<(String, Value, usize, usize)> {
    with_model(|m| {
        let dirty = std::mem::take(&mut m.dirty_tables);
        dirty
            .into_iter()
            .filter_map(|id| {
                let c = m.get(id)?;
                let cb = c.props.get("getCellCmd")?.clone();
                let n = |k: &str| c.props.get(k).map(number).unwrap_or(0.0).max(0.0) as usize;
                Some((c.path.clone(), cb, n("rows"), n("columns")))
            })
            .collect()
    })
}

/// A flag edit made from inside the core: as a script's `-e`, heard by the shell.
pub(crate) fn model_edit(path: &str, flags: &Map<String, Value>) -> Result<(), String> {
    with_model(|m| {
        let Some(id) = m.find(path) else { return Err(format!("Object '{path}' not found.")) };
        let changed = apply(m, id, flags)?;
        if !changed.is_empty() {
            let c = m.get(id).unwrap();
            op(json!({"op": "edit", "path": c.path, "props": changed}));
        }
        Ok(())
    })
}

/// The kind of widget behind a control, for tests and the shell.
pub fn control_kind_of(path: &str) -> Option<String> {
    control_kind(path)
}

/// What the model holds for a control: its kind, and one flag's stored value.
pub(crate) fn control_kind(path: &str) -> Option<String> {
    with_model(|m| m.find(path).and_then(|i| m.get(i)).map(|c| c.kind.clone()))
}

pub(crate) fn control_full_path(path: &str) -> Option<String> {
    with_model(|m| m.find(path).and_then(|i| m.get(i)).map(|c| c.path.clone()))
}

/// A command's -defineTemplate: the flags go into the template as that command's defaults.
pub(crate) fn define_template(template: &str, cmd: &str, flags: &Map<String, Value>) {
    with_model(|m| {
        let e = m.templates.entry(template.to_string()).or_default().entry(cmd.to_string()).or_default();
        for (k, v) in flags {
            if !META.contains(&k.as_str()) && k != "defineTemplate" && !v.is_null() {
                e.insert(k.clone(), v.clone());
            }
        }
    })
}

/// The defaults a command takes from the templates: -useTemplate's, or the current one's.
pub(crate) fn template_defaults(cmd: &str, use_template: Option<&str>) -> Map<String, Value> {
    with_model(|m| {
        let mut out = Map::new();
        let names: Vec<String> = use_template.map(str::to_string).into_iter().chain(m.template_stack.last().cloned()).collect();
        for t in names {
            if let Some(d) = m.templates.get(&t).and_then(|x| x.get(cmd)) {
                for (k, v) in d {
                    out.entry(k.clone()).or_insert(v.clone());
                }
            }
        }
        out
    })
}

/// The menu items go in now (setParent -menu), by its full path.
pub(crate) fn current_menu() -> Option<String> {
    with_model(|m| m.menu.and_then(|i| m.get(i)).map(|c| c.path.clone()))
}

pub(crate) fn control_prop(path: &str, key: &str) -> Option<Value> {
    with_model(|m| m.find(path).and_then(|i| m.get(i)).and_then(|c| c.props.get(key).cloned()))
}

pub(crate) fn control_props(path: &str) -> Option<Map<String, Value>> {
    with_model(|m| m.find(path).and_then(|i| m.get(i)).map(|c| c.props.clone()))
}

pub(crate) fn delete_control(path: &str) {
    with_model(|m| {
        if let Some(i) = m.find(path) {
            m.delete(i);
        }
    });
}

/// The generic query or -exists of a command, for commands that wrap the generic one.
pub(crate) fn command_for(cmd: &str, a: &Args) -> CmdResult {
    control_command(cmd, a)
}

/// Every control of a kind, as (path, flags).
pub(crate) fn controls_of(kind: &str) -> Vec<(String, Map<String, Value>)> {
    with_model(|m| m.controls.values().filter(|c| c.kind == kind).map(|c| (c.path.clone(), c.props.clone())).collect())
}

/// As `controls_of`, or None while the model is in use further up the stack (a line printed from inside a command).
pub(crate) fn try_controls_of(kind: &str) -> Option<Vec<(String, Map<String, Value>)>> {
    UI.with(|m| {
        let mut m = m.try_borrow_mut().ok()?;
        ensure_built_in(&mut m);
        Some(m.controls.values().filter(|c| c.kind == kind).map(|c| (c.path.clone(), c.props.clone())).collect())
    })
}

/// As `model_edit`, doing nothing while the model is in use further up the stack.
pub(crate) fn try_model_edit(path: &str, flags: &Map<String, Value>) {
    UI.with(|m| {
        let Ok(mut m) = m.try_borrow_mut() else { return };
        let Some(id) = m.find(path) else { return };
        if let Ok(changed) = apply(&mut m, id, flags) {
            if !changed.is_empty() {
                let p = m.get(id).unwrap().path.clone();
                op(json!({"op": "edit", "path": p, "props": changed}));
            }
        }
    })
}

fn tab_labels(m: &Model, id: Id) -> Map<String, Value> {
    m.get(id).and_then(|c| c.props.get("tabLabels")).and_then(Value::as_object).cloned().unwrap_or_default()
}

fn progress_flag(m: &mut Model, id: Id, key: &str, raw: &Value, changed: &mut Map<String, Value>) {
    let get = |m: &Model, k: &str, d: f64| m.get(id).and_then(|c| c.props.get(k)).map(number).unwrap_or(d);
    let (lo, hi) = (get(m, "minValue", 0.0), get(m, "maxValue", 100.0));
    let v = number(raw);
    match key {
        "step" => {
            let cur = get(m, "progress", lo) + v;
            set(m, id, "progress", json!(cur.clamp(lo, hi.max(lo)) as i64), changed);
        }
        "progress" => set(m, id, "progress", json!(v.clamp(lo, hi.max(lo)) as i64), changed),
        _ => set(m, id, key, json!(v as i64), changed),
    }
}

fn list_flag(m: &mut Model, id: Id, key: &str, raw: &Value, changed: &mut Map<String, Value>) -> Result<(), String> {
    let vals = flat(raw);
    let mut items: Vec<String> = m.get(id).and_then(|c| c.props.get("items")).and_then(Value::as_array).map(|a| a.iter().map(text).collect()).unwrap_or_default();
    let mut tags: Vec<String> = m.get(id).and_then(|c| c.props.get("tags")).and_then(Value::as_array).map(|a| a.iter().map(text).collect()).unwrap_or_default();
    let mut sel: Vec<i64> = m.get(id).and_then(|c| c.props.get("selected")).and_then(Value::as_array).map(|a| a.iter().map(|v| number(v) as i64).collect()).unwrap_or_default();
    // Rows turned off by -enableItem and the like, from 0.
    let mut off: Vec<i64> = m.get(id).and_then(|c| c.props.get("disabled")).and_then(Value::as_array).map(|a| a.iter().map(|v| number(v) as i64).collect()).unwrap_or_default();
    let multi = m.get(id).and_then(|c| c.props.get("allowMultiSelection")).is_some_and(truth);
    let pick = |sel: &mut Vec<i64>, i: i64| {
        if !multi {
            sel.clear();
        }
        if !sel.contains(&i) {
            sel.push(i);
        }
    };
    let remove = |items: &mut Vec<String>, tags: &mut Vec<String>, sel: &mut Vec<i64>, off: &mut Vec<i64>, i: usize| {
        items.remove(i);
        if i < tags.len() {
            tags.remove(i);
        }
        for rows in [&mut *sel, &mut *off] {
            rows.retain(|s| *s != i as i64);
            for s in rows.iter_mut() {
                if *s > i as i64 {
                    *s -= 1;
                }
            }
        }
    };
    let able = |off: &mut Vec<i64>, i: i64, on: bool| {
        off.retain(|x| *x != i);
        if !on {
            off.push(i);
            off.sort_unstable();
        }
    };
    let mut touched = true;
    match key {
        // From the shell: what the user picked, as row numbers from 0.
        "selected" => sel = vals.iter().map(|v| number(v) as i64).collect(),
        "append" => items.extend(vals.iter().map(text)),
        "appendPosition" => {
            for ch in vals.chunks(2) {
                if let [p, t] = ch {
                    let at = ((number(p) as usize).max(1) - 1).min(items.len());
                    items.insert(at, text(t));
                    if at < tags.len() {
                        tags.insert(at, String::new());
                    }
                    for s in sel.iter_mut().chain(off.iter_mut()) {
                        if *s >= at as i64 {
                            *s += 1;
                        }
                    }
                }
            }
        }
        "removeItem" => {
            for v in &vals {
                while let Some(i) = items.iter().position(|t| *t == text(v)) {
                    remove(&mut items, &mut tags, &mut sel, &mut off, i);
                }
            }
        }
        "removeIndexedItem" => {
            let mut idx: Vec<usize> = vals.iter().map(|v| number(v) as usize).collect();
            idx.sort_unstable();
            for i in idx.into_iter().rev() {
                if i >= 1 && i <= items.len() {
                    remove(&mut items, &mut tags, &mut sel, &mut off, i - 1);
                }
            }
        }
        "removeAll" => {
            if truth(raw) {
                items.clear();
                tags.clear();
                sel.clear();
                off.clear();
                set(m, id, "itemColors", json!([]), changed);
            }
        }
        // iconTextScrollList -itemTextColor index r g b: the text colour of an item (from 1).
        "itemTextColor" => {
            let mut colors: Vec<Value> = m.get(id).and_then(|c| c.props.get("itemColors")).and_then(Value::as_array).cloned().unwrap_or_default();
            for ch in vals.chunks(4) {
                let [i, r, g, b] = ch else { return Err("iconTextScrollList: -itemTextColor takes an index and three colour values.".into()) };
                let i = number(i) as i64 - 1;
                if i < 0 || i as usize >= items.len() {
                    return Err(format!("iconTextScrollList: no item {}.", i + 1));
                }
                colors.retain(|c| c[0].as_i64() != Some(i));
                colors.push(json!([i, number(r).clamp(0.0, 1.0), number(g).clamp(0.0, 1.0), number(b).clamp(0.0, 1.0)]));
            }
            set(m, id, "itemColors", Value::Array(colors), changed);
            touched = false;
        }
        // iconTextScrollList -editIndexed: the item (from 1) the shell opens for editing.
        "editIndexed" => {
            let i = number(raw) as i64;
            if i < 1 || i as usize > items.len() {
                return Err(format!("iconTextScrollList: no item {i}."));
            }
            set(m, id, "editIndexed", json!(i), changed);
            touched = false;
        }
        // From the shell: an item's text edited in place, as its row from 0 and the new text.
        "__editedItem" => {
            if let [i, t, ..] = vals.as_slice() {
                let i = number(i) as usize;
                if i < items.len() {
                    items[i] = text(t);
                }
            }
        }
        "selectItem" => {
            for v in &vals {
                let want = text(v);
                if let Some(i) = items.iter().position(|t| *t == want) {
                    pick(&mut sel, i as i64);
                }
            }
        }
        "selectIndexedItem" => {
            for v in &vals {
                let i = number(v) as i64 - 1;
                if i >= 0 && (i as usize) < items.len() {
                    pick(&mut sel, i);
                }
            }
        }
        "deselectAll" => {
            if truth(raw) {
                sel.clear();
            }
        }
        "deselectItem" => {
            for v in &vals {
                let want = text(v);
                if let Some(i) = items.iter().position(|t| *t == want) {
                    sel.retain(|s| *s != i as i64);
                }
            }
        }
        "deselectIndexedItem" => {
            for v in &vals {
                let i = number(v) as i64 - 1;
                sel.retain(|s| *s != i);
            }
        }
        "uniqueTag" => {
            // Tags follow the items last appended.
            let start = items.len().saturating_sub(vals.len());
            tags.resize(items.len(), String::new());
            for (k, v) in vals.iter().enumerate() {
                if start + k < tags.len() {
                    tags[start + k] = text(v);
                }
            }
        }
        "enableAll" => {
            if truth(raw) {
                off.clear();
            }
        }
        // Pairs of an item (its text, 1-based index or unique tag) and whether it is on.
        "enableItem" | "enableIndexedItem" | "enableUniqueTagItem" => {
            for ch in vals.chunks(2) {
                let [which, on] = ch else { return Err(format!("textScrollList: -{key} takes an item and on or off.")) };
                let rows: Vec<i64> = match key {
                    "enableIndexedItem" => {
                        let i = number(which) as i64 - 1;
                        if i < 0 || i as usize >= items.len() {
                            return Err(format!("textScrollList: no item {}.", i + 1));
                        }
                        vec![i]
                    }
                    "enableItem" => items.iter().enumerate().filter(|(_, t)| **t == text(which)).map(|(i, _)| i as i64).collect(),
                    _ => tags.iter().enumerate().filter(|(_, t)| **t == text(which)).map(|(i, _)| i as i64).collect(),
                };
                if rows.is_empty() {
                    return Err(format!("textScrollList: no item {}.", text(which)));
                }
                for i in rows {
                    able(&mut off, i, truth(on));
                }
            }
        }
        "selectUniqueTagItem" => {
            for v in &vals {
                let want = text(v);
                if let Some(i) = tags.iter().position(|t| *t == want) {
                    pick(&mut sel, i as i64);
                }
            }
        }
        _ => {
            touched = false;
            let v = match raw {
                Value::Array(a) if a.len() == 1 => a[0].clone(),
                o => o.clone(),
            };
            set(m, id, key, v, changed);
        }
    }
    if touched {
        set(m, id, "items", json!(items), changed);
        set(m, id, "tags", json!(tags), changed);
        set(m, id, "selected", json!(sel), changed);
        set(m, id, "disabled", json!(off), changed);
    }
    Ok(())
}

fn form_flag(m: &mut Model, id: Id, key: &str, vals: &[Value], changed: &mut Map<String, Value>) -> Result<(), String> {
    let n = match key {
        "attachNone" => 2,
        "attachForm" | "attachOppositeForm" => 3,
        _ => 4,
    };
    let mut list: Vec<Value> = m.get(id).and_then(|c| c.props.get("attachments")).and_then(Value::as_array).cloned().unwrap_or_default();
    for ch in vals.chunks(n) {
        if ch.len() < n {
            return Err(format!("formLayout: -{key} takes {n} values."));
        }
        let name = text(&ch[0]);
        let Some(c) = m.find(&name) else { return Err(format!("Object '{name}' not found.")) };
        let path = m.get(c).unwrap().path.clone();
        let edge = text(&ch[1]);
        if !matches!(edge.as_str(), "top" | "bottom" | "left" | "right") {
            return Err(format!("formLayout: '{edge}' is not an edge."));
        }
        let mut a = json!({"control": path, "edge": edge, "kind": key});
        if n > 2 {
            a["offset"] = json!(number(&ch[2]) as i64);
        }
        if n > 3 {
            match key {
                "attachPosition" => a["position"] = json!(number(&ch[3])),
                _ => {
                    let other = text(&ch[3]);
                    let Some(o) = m.find(&other) else { return Err(format!("Object '{other}' not found.")) };
                    a["other"] = json!(m.get(o).unwrap().path.clone());
                }
            }
        }
        list.retain(|x| !(x["control"] == a["control"] && x["edge"] == a["edge"]));
        list.push(a);
    }
    set(m, id, "attachments", Value::Array(list), changed);
    Ok(())
}

fn menu_items(m: &Model, id: Id) -> Vec<(String, String)> {
    let Some(c) = m.get(id) else { return Vec::new() };
    c.children
        .iter()
        .filter_map(|k| m.get(*k))
        .filter(|k| k.kind == "menuItem" && !k.props.get("divider").is_some_and(truth))
        .map(|k| (k.name.clone(), k.props.get("label").map(text).unwrap_or_else(|| k.name.clone())))
        .collect()
}

/// A tool button's current tool: the one last picked, or the first attached.
pub(crate) fn tool_of(props: &Map<String, Value>) -> Option<String> {
    let tools: Vec<String> = props.get("tool").map(flat).unwrap_or_default().iter().map(text).collect();
    props.get("currentTool").map(text).filter(|t| tools.contains(t)).or_else(|| tools.first().cloned())
}

fn collection_members(m: &Model, coll: Id) -> Vec<Id> {
    let path = m.get(coll).map(|c| c.path.clone()).unwrap_or_default();
    m.controls.iter().filter(|(_, c)| c.props.get("collection").and_then(Value::as_str) == Some(path.as_str())).map(|(i, _)| *i).collect()
}

/// radioButtonGrp -shareCollection: groups sharing one collection have one button on between them, so picking in one
/// clears the others.
fn share_exclusive(m: &mut Model, id: Id) {
    let shared = |m: &Model, i: Id| m.get(i).and_then(|c| c.props.get("shareCollection")).map(text).filter(|s| !s.is_empty()).and_then(|s| m.find(&s));
    // The group the set is named by: the first one, which the others point at.
    let root = |m: &Model, i: Id| shared(m, i).unwrap_or(i);
    let mine = root(m, id);
    let others: Vec<Id> = m.controls.iter().filter(|(k, c)| **k != id && c.kind == "radioButtonGrp").map(|(k, _)| *k).filter(|&k| root(m, k) == mine).collect();
    if others.is_empty() {
        return;
    }
    for k in others {
        if let Some(c) = m.controls.get_mut(&k) {
            if c.props.get("select").is_some_and(|v| number(v) > 0.0) {
                c.props.insert("select".into(), json!(0));
                op(json!({"op": "edit", "path": c.path, "props": {"select": 0}}));
            }
        }
    }
}

/// iconTextRadioCollection -select: the named button on and the one that was on off, each running its -changeCommand
/// (with 1 or 0) and its -onCommand or -offCommand, unless `quiet` (-disableCommands). NONE turns them all off.
fn collection_pick(m: &mut Model, coll: Id, name: &str, quiet: bool) -> Result<(), String> {
    let kids = collection_members(m, coll);
    let target = if name == "NONE" {
        None
    } else {
        Some(kids.iter().copied().find(|k| m.get(*k).is_some_and(|c| c.name == name || c.path == name)).ok_or_else(|| format!("Object '{name}' not found."))?)
    };
    let mut due = Vec::new();
    for k in kids {
        let Some(c) = m.controls.get_mut(&k) else { continue };
        let was = c.props.get("select").is_some_and(truth);
        let now = Some(k) == target;
        if was == now {
            continue;
        }
        c.props.insert("select".into(), json!(now));
        op(json!({"op": "edit", "path": c.path, "props": {"select": now}}));
        if !quiet {
            for flag in ["changeCommand", if now { "onCommand" } else { "offCommand" }] {
                if let Some(cb) = c.props.get(flag) {
                    // The button turned off runs first.
                    due.push((now, cb.clone(), vec![json!(now)]));
                }
            }
        }
    }
    due.sort_by_key(|d| d.0);
    for (_, cb, args) in due {
        run_soon(cb, args);
    }
    Ok(())
}

fn radio_exclusive(m: &mut Model, id: Id) {
    let Some(coll) = m.get(id).and_then(|c| c.props.get("collection")).and_then(Value::as_str).map(str::to_string) else { return };
    let others: Vec<Id> = m.controls.iter().filter(|(k, c)| **k != id && c.props.get("collection").and_then(Value::as_str) == Some(coll.as_str())).map(|(k, _)| *k).collect();
    for k in others {
        if let Some(c) = m.controls.get_mut(&k) {
            if c.props.get("select").is_some_and(truth) {
                c.props.insert("select".into(), json!(false));
                op(json!({"op": "edit", "path": c.path, "props": {"select": false}}));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Query

fn default_value(c: &Control, key: &str) -> Value {
    if c.kind == "channelBox" {
        if let Some(v) = crate::channel_box::default_value(key) {
            return v;
        }
    }
    match (c.kind.as_str(), key) {
        ("window" | "workspaceControl" | "dockControl" | "progressWindow", "visible") => json!(false),
        ("iconTextButton" | "iconTextCheckBox" | "iconTextRadioButton" | "iconTextStaticLabel", "flipX" | "flipY") => json!(false),
        (k, key) if matches!(k, "iconTextCheckBox" | "iconTextRadioButton" | "iconTextStaticLabel") && icon_default(key).is_some() => icon_default(key).unwrap(),
        ("iconTextRadioCollection", "global") => json!(false),
        ("iconTextScrollList", "dragFeedbackVisible") => json!(true),
        ("iconTextScrollList", "editable") => json!(false),
        (_, "enable" | "visible" | "manage" | "editable" | "sizeable" | "minimizeButton" | "maximizeButton" | "titleBar" | "titleBarMenu" | "enableKeyboardFocus" | "enableButton") => json!(true),
        ("floatSliderGrp" | "intSliderGrp" | "colorIndexSliderGrp" | "floatSliderButtonGrp", "minValue") => zero(&c.kind),
        ("floatSliderGrp" | "intSliderGrp" | "floatSliderButtonGrp", "maxValue") => ten(&c.kind),
        ("scriptTable", "rows" | "columns" | "selectionBehavior") => json!(0),
        ("switchTable", "label1") => json!("Shape"),
        ("switchTable", "label2") => json!("Input"),
        ("switchTable", "selectedRow" | "underPointerRow") => json!(0),
        ("switchTable", "switchNode") => json!(""),
        // Click, Ctrl and Shift as a list's: what the table shows when no mode is given.
        ("scriptTable", "selectionMode") => json!(3),
        ("outlinerEditor", "mainListConnection") => json!("worldList"),
        ("timeControl", "mainListConnection") => json!(""),
        ("soundControl" | "timeControl", "waveform") => json!("top"),
        ("soundControl" | "timeControl", "sound" | "soundSource") => json!(""),
        ("soundControl" | "timeControl", "displaySound") => json!(c.kind == "soundControl"),
        ("soundControl" | "timeControl", "repeatChunkSize") => json!(1.0),
        ("soundControl" | "timeControl", "repeatOnHold" | "scrubbing") => json!(false),
        ("soundControl", "minTime") => json!(aztek_scene::anim::range().min),
        ("soundControl", "maxTime") => json!(aztek_scene::anim::range().max),
        ("timeControl", "globalTime") => json!(true),
        ("timeControl", "tickSpan") => json!(1),
        ("timeControl", "animLayerFilterOptions") => json!("allAffecting"),
        ("timeControl", "currentFrameColor") => json!([0.82, 0.82, 0.82, 0.47]),
        ("timeControl", "foregroundColor") => json!([0.59, 0.59, 0.59]),
        ("dopeSheetEditor" | "animCurveEditor", "mainListConnection") => json!("animationList"),
        ("nodeEditor", "mainListConnection") => json!(""),
        (k, "selectionConnection") if is_editor(k) => json!("modelList"),
        (k, "highlightConnection" | "forceMainConnection" | "filter") if is_editor(k) => json!(""),
        (k, "panel") if is_editor(k) => json!(c.name),
        (k, "control") if is_editor(k) => json!(true),
        (k, "lockMainConnection") if is_editor(k) => json!(false),
        ("nodeEditor", "traversalDepthLimit") => json!(-1),
        ("nodeEditor", "nodeTitleMode") => json!("name"),
        ("outlinerEditor", "displayMode") => json!("DAG"),
        ("outlinerEditor", "showShapes" | "showAttributes" | "showDagOnly" | "showConnected" | "showAnimCurvesOnly") => json!(c.kind == "outlinerEditor" && key == "showDagOnly"),
        ("componentEditor" | "componentBox", "precision") => json!(3),
        ("componentBox", "labelWidth" | "maxHeight" | "maxWidth") => json!(0),
        ("floatSlider2", "minimum" | "value1") => json!(0.0),
        ("floatSlider2", "maximum" | "value2") => json!(10.0),
        ("floatSlider2", "polarity") => json!(0),
        ("falloffCurve" | "falloffCurveAttr", "currentKey") => json!(0),
        ("falloffCurve", "readOnly" | "snapToGrid" | "customCurveWidget") => json!(false),
        ("falloffCurveAttr", "readOnly" | "snapToGrid" | "customCurveWidget") => json!(0),
        ("falloffCurve", "optionVar") => json!(""),
        // The file being opened, until a reference is picked in the list.
        ("preloadRefEd", "selectFileNode") => json!("0"),
        ("preloadRefEd", "selectCommand") => json!(""),
        ("keyframeStats", "precision") => json!(3),
        ("keyframeStats", "autoUnitWidth") => json!(0),
        ("keyframeStats", "animEditor" | "timeAnnotation" | "valueAnnotation") => json!(""),
        ("spreadSheetEditor", "precision") => json!(3),
        ("spreadSheetEditor", "keyableOnly" | "longNames") => json!(key == "keyableOnly"),
        ("toolButton", "select" | "stateSelfManaged") => json!(false),
        ("toolButton", "popupIndicatorVisible") => json!(true),
        ("layerButton", "select" | "current" | "transparent") => json!(false),
        ("layerButton", "layerVisible") => json!(true),
        ("layerButton", "layerState") => json!("normal"),
        ("layerButton", "name") => json!(c.name),
        ("colorIndexSliderGrp", "maxValue") => json!(31),
        (k, "alphaValue") if is_color_group(k) => json!(1.0),
        (k, "useDisplaySpace" | "forceDragRefresh") if is_color_group(k) => json!(false),
        ("colorSliderButtonGrp", "symbolButtonDisplay") => json!(false),
        ("colorSliderButtonGrp", "buttonLabel") => json!(""),
        ("commandLine", "numberOfHistoryLines") => json!(50),
        ("commandLine", "sourceType") => json!("mel"),
        ("commandLine", "holdFocus") => json!(false),
        ("commandLine", "inputAnnotation" | "outputAnnotation") => json!(""),
        (k, "minValue") if is_int_kind(k) || is_float_kind(k) => zero(k),
        (k, "maxValue") if k.ends_with("Slider") || k.ends_with("ScrollBar") => hundred(k),
        (k, "value" | "value1" | "value2" | "value3" | "value4" | "step") if is_int_kind(k) || is_float_kind(k) => zero(k),
        ("window" | "workspaceControl" | "dockControl", "title" | "label") => json!(c.name),
        ("dockControl", "area") => json!("left"),
        ("dockControl", "retain" | "enablePopupOption" | "moveable") => json!(true),
        ("workspaceControl", "retain") => json!(true),
        ("window", "mainMenuBar" | "menuBar") | ("workspaceControl", "actLikeMayaUIElement" | "horizontal" | "duplicatable") => json!(false),
        ("workspaceControl", "initCallback" | "layoutDirectionCallback" | "stateString") => json!(""),
        ("dockControl", "fixedWidth" | "fixedHeight" | "raise") => json!(false),
        ("flowLayout", "horizontal") => json!(!c.props.get("vertical").is_some_and(truth)),
        ("flowLayout", "vertical" | "wrap") => json!(false),
        ("tabLayout", "scrollableTabs" | "imageVisible") => json!(true),
        ("tabLayout", "borderStyle") => json!("full"),
        ("shelfLayout", "horizontal") => json!(true),
        ("shelfButton", "enableCommandRepeat") => json!(true),
        ("shelfButton", "flipX" | "flipY" | "noDefaultPopup") => json!(false),
        ("symbolCheckBox", "innerMargin") => json!(false),
        ("frameLayout", "backgroundShade") => json!(false),
        ("floatScrollBar" | "intScrollBar", "largeStep") => json!(if c.kind == "floatScrollBar" { json!(10.0) } else { json!(10) }),
        (k, "label") if !is_group(k) => json!(c.name),
        (k, "label") if is_group(k) => json!(""),
        (_, "text" | "placeholderText" | "annotation" | "docTag" | "image" | "status" | "emptyLabel") => json!(""),
        ("window", "width") => json!(546),
        ("window", "height") => json!(350),
        (_, "width") => json!(100),
        (_, "height") => json!(22),
        ("progressBar", "maxValue") => json!(100),
        ("progressBar", "minValue" | "progress" | "step") => json!(0),
        ("checkBox" | "symbolCheckBox" | "iconTextCheckBox", "value") => json!(false),
        ("checkBoxGrp", "value1" | "value2" | "value3" | "value4") => json!(false),
        ("radioButton" | "iconTextRadioButton", "select") => json!(false),
        ("radioButtonGrp" | "optionMenu" | "optionMenuGrp", "select") => json!(0),
        ("formLayout", "numberOfDivisions") => json!(100),
        (_, "rgbValue" | "hsvValue") => json!([0.0, 0.0, 0.0]),
        (_, "backgroundColor") => json!([0.27, 0.27, 0.27]),
        (_, "collapse" | "collapsable" | "adjustableColumn" | "isCancelled" | "isInterruptable" | "active" | "allowMultiSelection" | "floating") => json!(false),
        _ => Value::Null,
    }
}

/// The documented defaults of iconTextCheckBox, iconTextRadioButton and iconTextStaticLabel.
fn icon_default(key: &str) -> Option<Value> {
    Some(match key {
        "marginWidth" | "marginHeight" => json!(1),
        "labelOffset" => json!(0),
        "overlayLabelColor" => json!([0.8, 0.8, 0.8]),
        "overlayLabelBackColor" => json!([0.0, 0.0, 0.0, 0.5]),
        "useAlpha" => json!(true),
        "flat" => json!(false),
        "align" => json!("center"),
        "style" => json!("textOnly"),
        "rotation" => json!(0.0),
        "image1" | "image2" | "image3" | "disabledImage" | "highlightImage" | "selectionImage" | "selectionHighlightImage" | "imageOverlayLabel" | "version" | "font" => json!(""),
        _ => return None,
    })
}

fn zero(k: &str) -> Value {
    if is_int_kind(k) {
        json!(0)
    } else {
        json!(0.0)
    }
}
fn ten(k: &str) -> Value {
    if is_int_kind(k) {
        json!(10)
    } else {
        json!(10.0)
    }
}
fn hundred(k: &str) -> Value {
    if is_int_kind(k) {
        json!(100)
    } else {
        json!(100.0)
    }
}

fn query(m: &Model, id: Id, key: &str) -> Value {
    let Some(c) = m.get(id) else { return Value::Null };
    let prop = |k: &str| c.props.get(k).cloned().unwrap_or_else(|| default_value(c, k));
    let kids_named = |kids: &[Id]| kids.iter().filter_map(|k| m.get(*k)).map(|k| k.name.clone()).collect::<Vec<_>>();
    match key {
        "exists" => json!(true),
        "fullPathName" => json!(c.path),
        "control" if c.kind == "preloadRefEd" => json!(c.path),
        "stateString" if c.kind == "preloadRefEd" => {
            // What makes another editor like this one, for $editorName.
            let mut out = String::from("preloadRefEd -e");
            for k in ["filter", "mainListConnection", "selectionConnection", "highlightConnection", "docTag"] {
                let v = text(&prop(k));
                if !v.is_empty() {
                    out.push_str(&format!(" -{k} \"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")));
                }
            }
            out.push_str(" $editorName;");
            json!(out)
        }
        "parent" => json!(c.parent.and_then(|p| m.get(p)).map(|p| p.path.clone()).unwrap_or_default()),
        "childArray" => {
            let kids: Vec<Id> = c.children.iter().copied().filter(|k| m.cat(*k) != Cat::Menu || m.get(*k).is_some_and(|x| x.kind == "optionMenu" || x.kind == "optionMenuGrp")).collect();
            no_list(kids_named(&kids))
        }
        "numberOfChildren" => json!(c.children.iter().filter(|k| m.cat(**k) != Cat::Menu || m.get(**k).is_some_and(|x| x.kind == "optionMenu" || x.kind == "optionMenuGrp")).count()),
        "popupMenuArray" | "menuArray" => {
            let want = if key == "popupMenuArray" { "popupMenu" } else { "menu" };
            let v: Vec<Id> = c.children.iter().copied().filter(|k| m.get(*k).is_some_and(|x| x.kind == want)).collect();
            no_list(kids_named(&v))
        }
        "numberOfPopupMenus" => json!(c.children.iter().filter(|k| m.get(**k).is_some_and(|x| x.kind == "popupMenu")).count()),
        "numberOfMenus" => json!(c.children.iter().filter(|k| m.get(**k).is_some_and(|x| x.kind == "menu")).count()),
        // The corner widgets, topLeft, topRight, bottomLeft then bottomRight.
        "menuBarCornerWidget" => {
            let corners = c.props.get("cornerWidgets").and_then(Value::as_object).cloned().unwrap_or_default();
            let v: Vec<String> = ["topLeft", "topRight", "bottomLeft", "bottomRight"].iter().map(|k| corners.get(*k).map(text).map(|p| p.rsplit('|').next().unwrap_or("").to_string()).unwrap_or_default()).collect();
            json!(v)
        }
        "tabPosition" if c.kind == "workspaceControl" => json!(c.props.get("tabPosition").map(text).unwrap_or_else(|| "north".into())),
        "isObscured" => json!(!m.window_of(id).and_then(|w| m.get(w)).is_some_and(|w| w.props.get("visible").is_some_and(truth))),
        "widthHeight" => json!([prop("width"), prop("height")]),
        "state" if c.kind == "dockControl" => {
            // The dock's state as one string to keep in an optionVar and give back to -state: hex of its settings.
            let st = json!({"area": prop("area"), "floating": prop("floating"), "width": prop("width"), "height": prop("height"), "visible": prop("visible"), "raise": prop("raise")});
            json!(st.to_string().bytes().map(|b| format!("{b:02x}")).collect::<String>())
        }
        "horizontal" if c.kind == "flowLayout" => json!(!prop("vertical").as_bool().unwrap_or(false)),
        "topLeftCorner" => json!([prop("topEdge").as_i64().unwrap_or(0), prop("leftEdge").as_i64().unwrap_or(0)]),
        "tabLabel" => {
            let labels = tab_labels(m, id);
            let v: Vec<String> = c.children.iter().filter_map(|k| m.get(*k)).map(|k| labels.get(&k.path).map(text).unwrap_or_else(|| k.name.clone())).collect();
            no_list(v)
        }
        "tabLabelIndex" => query(m, id, "tabLabel"),
        "tabTooltip" | "tabTooltipIndex" | "tabIcon" | "tabIconIndex" if c.kind == "tabLayout" => {
            let store = if key.starts_with("tabTooltip") { "tabTooltips" } else { "tabIcons" };
            let map = c.props.get(store).and_then(Value::as_object).cloned().unwrap_or_default();
            let v: Vec<String> = c.children.iter().filter_map(|k| m.get(*k)).map(|k| map.get(&k.path).map(text).unwrap_or_default()).collect();
            no_list(v)
        }
        "selectTabIndex" => {
            if c.children.is_empty() {
                json!(0)
            } else {
                c.props.get("selectTabIndex").cloned().unwrap_or(json!(1))
            }
        }
        "selectTab" => {
            let i = c.props.get("selectTabIndex").map(number).unwrap_or(1.0) as usize;
            json!(c.children.get(i.saturating_sub(1)).and_then(|k| m.get(*k)).map(|k| k.name.clone()).unwrap_or_default())
        }
        "value" if c.kind.ends_with("FieldGrp") => {
            let n = c.props.get("numberOfFields").map(number).unwrap_or(1.0) as usize;
            Value::Array((1..=n.clamp(1, 4)).map(|i| prop(&format!("value{i}"))).collect())
        }
        "value" | "select" if c.kind == "optionMenu" || c.kind == "optionMenuGrp" => {
            let items = menu_items(m, id);
            let sel = c.props.get("select").map(number).unwrap_or(1.0) as usize;
            let sel = sel.clamp(1, items.len().max(1));
            if key == "select" {
                json!(if items.is_empty() { 0 } else { sel })
            } else {
                json!(items.get(sel - 1).map(|x| x.1.clone()).unwrap_or_default())
            }
        }
        "itemListLong" | "itemListShort" | "itemArray" => {
            let v: Vec<String> = c.children.iter().filter_map(|k| m.get(*k)).filter(|k| k.kind == "menuItem").map(|k| if key == "itemListLong" { k.path.clone() } else { k.name.clone() }).collect();
            no_list(v)
        }
        "numberOfItems" if c.kind != "textScrollList" && c.kind != "iconTextScrollList" => json!(c.children.iter().filter(|k| m.get(**k).is_some_and(|x| x.kind == "menuItem")).count()),
        "select" if c.kind == "radioCollection" || c.kind == "toolCollection" || c.kind == "iconTextRadioCollection" => {
            let kids = collection_members(m, id);
            json!(kids.iter().find_map(|k| m.get(*k).filter(|x| x.props.get("select").is_some_and(truth)).map(|x| x.name.clone())).unwrap_or_else(|| "NONE".to_string()))
        }
        "toolArray" => no_list(c.props.get("tool").map(flat).unwrap_or_default().iter().map(text).collect()),
        "toolCount" => json!(c.props.get("tool").map(flat).map_or(0, |t| t.len())),
        "tool" if c.kind == "toolButton" => json!(tool_of(&c.props).unwrap_or_default()),
        "toolImage1" | "toolImage2" | "toolImage3" if c.kind == "toolButton" => no_list(c.props.get(key).map(flat).unwrap_or_default().iter().map(text).collect()),
        "selectedCells" | "selectedRows" | "selectedColumns" | "selectedRow" if c.kind == "scriptTable" => {
            let at: Vec<i64> = c.props.get("selectedCells").map(flat).unwrap_or_default().iter().map(|v| number(v) as i64).collect();
            let mut rows: Vec<i64> = at.chunks(2).map(|x| x[0]).collect();
            let mut cols: Vec<i64> = at.chunks(2).filter_map(|x| x.get(1).copied()).collect();
            rows.dedup();
            cols.sort_unstable();
            cols.dedup();
            match key {
                "selectedRow" => json!(rows.first().copied().unwrap_or(0)),
                "selectedRows" => json!(rows),
                "selectedColumns" => json!(cols),
                _ => json!(at),
            }
        }
        "cellValue" if c.kind == "scriptTable" => {
            let at = c.props.get("cellIndex").map(flat).unwrap_or_default();
            if let [r, col, ..] = at.as_slice() {
                let (r, col) = ((number(r) as usize).max(1) - 1, (number(col) as usize).max(1) - 1);
                json!(table_cells(m, id).get(r).and_then(|x| x.get(col)).cloned().unwrap_or_default())
            } else {
                json!("")
            }
        }
        "selectedRow" | "underPointerRow" if c.kind == "switchTable" => c.props.get(key).cloned().unwrap_or(json!(0)),
        "underPointerRow" | "underPointerColumn" => json!(0),
        "isOptionBox" => json!(c.props.get("optionBox").is_some_and(truth)),
        "isCheckBox" => json!(c.props.contains_key("checkBox")),
        "isRadioButton" => json!(c.props.contains_key("radioButton")),
        "paneSize" => {
            // Width and height of each pane given, in the panes' order.
            let sizes = c.props.get("paneSizes").and_then(Value::as_object).cloned().unwrap_or_default();
            let mut keys: Vec<i64> = sizes.keys().filter_map(|k| k.parse().ok()).collect();
            keys.sort_unstable();
            Value::Array(keys.iter().flat_map(|k| flat(&sizes[&k.to_string()])).collect())
        }
        "collectionItemArray" => no_list(kids_named(&collection_members(m, id))),
        "numberOfCollectionItems" => json!(collection_members(m, id).len()),
        "allItems" => {
            let v: Vec<String> = c.props.get("items").and_then(Value::as_array).map(|a| a.iter().map(text).collect()).unwrap_or_default();
            no_list(v)
        }
        "allItemsUniqueTags" => no_list(c.props.get("tags").and_then(Value::as_array).map(|a| a.iter().map(text).collect()).unwrap_or_default()),
        "numberOfItems" => json!(c.props.get("items").and_then(Value::as_array).map_or(0, Vec::len)),
        "numberOfSelectedItems" => json!(c.props.get("selected").and_then(Value::as_array).map_or(0, Vec::len)),
        "selectItem" | "selectIndexedItem" | "selectUniqueTagItem" if c.kind == "textScrollList" || c.kind == "iconTextScrollList" => {
            let items: Vec<String> = c.props.get("items").and_then(Value::as_array).map(|a| a.iter().map(text).collect()).unwrap_or_default();
            let tags: Vec<String> = c.props.get("tags").and_then(Value::as_array).map(|a| a.iter().map(text).collect()).unwrap_or_default();
            let sel: Vec<usize> = c.props.get("selected").and_then(Value::as_array).map(|a| a.iter().map(|v| number(v) as usize).collect()).unwrap_or_default();
            match key {
                "selectItem" => no_list(sel.iter().filter_map(|i| items.get(*i).cloned()).collect()),
                "selectUniqueTagItem" => no_list(sel.iter().filter_map(|i| tags.get(*i).cloned()).collect()),
                _ => {
                    if sel.is_empty() && from_python() {
                        Value::Null
                    } else {
                        json!(sel.iter().map(|i| i + 1).collect::<Vec<_>>())
                    }
                }
            }
        }
        "numberOfRows" | "numberOfIcons" | "itemAt" | "visualRectAt" if c.kind == "iconTextScrollList" => {
            let items: Vec<String> = c.props.get("items").and_then(Value::as_array).map(|a| a.iter().map(text).collect()).unwrap_or_default();
            let height = c.props.get("height").map(number).filter(|h| *h > 0.0);
            let width = c.props.get("width").map(number).filter(|w| *w > 0.0).unwrap_or(LIST_WIDTH);
            let at = c.props.get("__at").map(flat).unwrap_or_default();
            let row_at = || -> Option<usize> {
                let [x, y, ..] = at.as_slice() else { return None };
                let (x, y) = (number(x), number(y));
                if x < 0.0 || y < 0.0 || x >= width || height.is_some_and(|h| y >= h) {
                    return None;
                }
                Some((y / LIST_ROW) as usize).filter(|r| *r < items.len())
            };
            match key {
                "numberOfRows" => json!(match height {
                    Some(h) => ((h / LIST_ROW) as usize).max(1),
                    None => items.len(),
                }),
                "numberOfIcons" => json!(items.len()),
                "itemAt" => json!(row_at().map(|r| items[r].clone()).unwrap_or_default()),
                _ => match row_at() {
                    Some(r) => json!([0, (r as f64 * LIST_ROW) as i64, width as i64, LIST_ROW as i64]),
                    None => json!([]),
                },
            }
        }
        "attachForm" | "attachControl" | "attachPosition" | "attachNone" | "attachOppositeForm" | "attachOppositeControl" => {
            let list = c.props.get("attachments").and_then(Value::as_array).cloned().unwrap_or_default();
            Value::Array(list.into_iter().filter(|a| a["kind"] == key).collect())
        }
        _ => prop(key),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The generic command

fn target(m: &Model, a: &Args, cmd: &str) -> Result<Id, String> {
    let Some(n) = a.pos_str(0) else {
        // Query and edit act on the layout under construction, or the last one, when no name is given.
        if cat_of(cmd) == Cat::Layout {
            if let Some(p) = m.parent {
                return Ok(p);
            }
        }
        if cmd == "optionMenu" || cmd == "menu" {
            if let Some(p) = m.menu {
                return Ok(p);
            }
        }
        return Err(format!("{cmd}: name a control."));
    };
    m.find(n).ok_or_else(|| format!("Object '{n}' not found."))
}

fn wants_kind(cmd: &str, c: &Control) -> bool {
    let kind = c.kind.as_str();
    match cmd {
        "control" | "layout" => true,
        "progressBar" => kind == "progressBar",
        "menu" => matches!(kind, "menu" | "popupMenu" | "optionMenu" | "optionMenuGrp" | "menuBarLayout"),
        x => x == kind || c.props.get("__cmd").and_then(Value::as_str) == Some(x),
    }
}

fn run_query(cmd: &str, a: &Args) -> CmdResult {
    with_model(|m| {
        let mut asked: Vec<String> = a.get("queried").and_then(Value::as_array).map(|q| q.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        for k in a.flags.keys() {
            if !matches!(k.as_str(), "query" | "queried") && !asked.contains(k) {
                asked.push(k.clone());
            }
        }
        // The point an iconTextScrollList is asked about.
        for k in ["itemAt", "visualRectAt"] {
            if let Some(at) = a.get(k).filter(|v| !v.is_null() && !v.is_boolean()).cloned() {
                if let Some(c) = a.pos_str(0).and_then(|n| m.find(n)).and_then(|i| m.controls.get_mut(&i)) {
                    c.props.insert("__at".to_string(), at);
                }
            }
        }
        // Which cell is asked about is not itself the question.
        if let Some(cell) = a.get("cellIndex").cloned() {
            asked.retain(|k| k != "cellIndex");
            if let Some(c) = a.pos_str(0).and_then(|n| m.find(n)).and_then(|i| m.controls.get_mut(&i)) {
                c.props.insert("cellIndex".to_string(), cell);
            }
        }
        if asked.iter().any(|k| k == "exists") {
            let ok = a.pos_str(0).and_then(|n| m.find(n)).is_some_and(|i| m.get(i).is_some_and(|c| wants_kind(cmd, c)));
            return Ok(json!(ok));
        }
        let id = target(m, a, cmd).map_err(aztek_ops::CommandError::new)?;
        let Some(first) = asked.first() else { fail!("{cmd}: name what to ask about.") };
        Ok(query(m, id, first))
    })
}

fn run_edit(cmd: &str, a: &Args) -> CmdResult {
    let out = with_model(|m| -> Result<Value, String> {
        let id = target(m, a, cmd)?;
        if m.get(id).is_some_and(|c| is_editor(&c.kind)) {
            if a.bool("unParent") {
                move_editor(m, id, None);
            } else if let Some(p) = a.get("parent").map(text).filter(|p| !p.is_empty()) {
                let to = m.find(&p).ok_or_else(|| format!("Object '{p}' not found."))?;
                move_editor(m, id, Some(to));
            }
        }
        if m.get(id).is_some_and(|c| c.kind == "workspaceControl") {
            if a.bool("close") {
                let path = m.get(id).unwrap().path.clone();
                for cb in close_control(m, id) {
                    run_soon(cb, Vec::new());
                }
                return Ok(json!(path));
            }
            if ["dockToMainWindow", "dockToControl", "dockToPanel", "tabToControl", "floating", "resizeWidth", "resizeHeight", "tabPosition", "horizontal"].iter().any(|k| a.has(k)) {
                crate::user_folder::layout_modified();
            }
        }
        let changed = apply(m, id, &a.flags)?;
        let c = m.get(id).unwrap();
        let (is_table, path) = (c.kind == "scriptTable", c.path.clone());
        if !changed.is_empty() {
            op(json!({"op": "edit", "path": c.path, "props": changed}));
        }
        // New rows, columns or a new script: the cells are asked for again.
        if is_table && (a.has("rows") || a.has("columns") || a.has("getCellCmd") || a.has("cellBackgroundColorCommand") || a.has("cellForegroundColorCommand")) {
            m.dirty_tables.push(id);
        }
        Ok(json!(path))
    });
    out.map_err(aztek_ops::CommandError::new)
}

fn parent_for(m: &Model, cmd: &str, a: &Args) -> Result<Option<Id>, String> {
    if let Some(p) = a.get("parent").map(text) {
        if !p.is_empty() {
            return m.find(&p).map(Some).ok_or_else(|| format!("Object '{p}' not found."));
        }
    }
    match cat_of(cmd) {
        Cat::Window => Ok(None),
        Cat::Collection => Ok(if a.bool("global") { None } else { m.parent }),
        Cat::MenuItem => m.menu.map(Some).ok_or_else(|| "No menu found.".to_string()),
        Cat::Menu if cmd == "menu" => {
            // Menus go on the current menu bar layout, or the bar of the window being built.
            let mut p = m.parent;
            while let Some(i) = p {
                if m.get(i).is_some_and(|c| c.kind == "menuBarLayout") {
                    return Ok(Some(i));
                }
                p = m.get(i).and_then(|c| c.parent);
            }
            m.parent.and_then(|i| m.window_of(i)).map(Some).ok_or_else(|| "No window found.".to_string())
        }
        _ if is_editor(cmd) => Ok(m.parent),
        _ => m.parent.map(Some).ok_or_else(|| "No window found.".to_string()),
    }
}

/// -e -parent or -unParent on an editor: it moves, and the shell makes its widget again in the new place.
fn move_editor(m: &mut Model, id: Id, parent: Option<Id>) {
    let Some(c) = m.get(id).cloned() else { return };
    op(json!({"op": "delete", "path": c.path}));
    if let Some(p) = c.parent.and_then(|p| m.controls.get_mut(&p)) {
        p.children.retain(|k| *k != id);
    }
    let path = match parent.and_then(|p| m.get(p)) {
        Some(p) => format!("{}|{}", p.path, c.name),
        None => c.name.clone(),
    };
    let parent_path = parent.and_then(|p| m.get(p)).map(|p| p.path.clone());
    if let Some(p) = parent.and_then(|p| m.controls.get_mut(&p)) {
        p.children.push(id);
    }
    if let Some(x) = m.controls.get_mut(&id) {
        x.parent = parent;
        x.path = path.clone();
    }
    let mut props = c.props.clone();
    props.insert("name".to_string(), json!(c.name));
    op(json!({"op": "create", "path": path, "kind": c.kind, "parent": parent_path, "props": props}));
}

fn create(cmd: &str, a: &Args) -> CmdResult {
    create_as(display_of(cmd), cmd, a)
}

/// Makes a control of `kind` for the command `base`, which names it and answers -exists for it.
pub(crate) fn create_as(kind: &str, base: &str, a: &Args) -> CmdResult {
    let cmd = kind;
    let r = with_model(|m| -> Result<Value, String> {
        // A template only stores what it was given.
        if let Some(t) = a.get("defineTemplate").map(text) {
            let e = m.templates.entry(t).or_default().entry(base.to_string()).or_default();
            for (k, v) in &a.flags {
                if !META.contains(&k.as_str()) {
                    e.insert(k.clone(), v.clone());
                }
            }
            return Ok(json!(base));
        }
        let parent = parent_for(m, cmd, a)?;
        let name = match a.pos_str(0) {
            Some(n) => {
                let short = n.rsplit('|').next().unwrap_or(n);
                if m.name_taken(short, parent) {
                    return Err(format!("Object's name '{short}' is not unique."));
                }
                short.to_string()
            }
            None => m.fresh_name(base, parent),
        };
        let path = match parent.and_then(|p| m.get(p)) {
            Some(p) => format!("{}|{name}", p.path),
            None => name.clone(),
        };
        m.next += 1;
        let id = m.next;
        let mut made = Map::new();
        if base != kind {
            made.insert("__cmd".to_string(), json!(base));
        }
        m.controls.insert(id, Control { kind: cmd.to_string(), name: name.clone(), path: path.clone(), parent, children: Vec::new(), props: made });
        if let Some(p) = parent {
            m.controls.get_mut(&p).unwrap().children.push(id);
        }
        // What templates give comes first; what the command says wins.
        let mut flags = Map::new();
        let names: Vec<String> = a.get("useTemplate").map(text).into_iter().chain(m.template_stack.last().cloned()).collect();
        for t in names {
            if let Some(d) = m.templates.get(&t).and_then(|x| x.get(base)) {
                for (k, v) in d {
                    flags.entry(k.clone()).or_insert(v.clone());
                }
            }
        }
        for (k, v) in &a.flags {
            flags.insert(k.clone(), v.clone());
        }
        // -runTimeCommand: what the named command has fills in the label, annotation, image, command and version not given.
        if let Some(rtc) = flags.get("runTimeCommand").map(text).filter(|t| !t.is_empty()) {
            let Some(e) = crate::ui_cmds::run_time_entry(&rtc) else {
                m.delete(id);
                return Err(format!("{cmd}: no runTimeCommand named {rtc}."));
            };
            // A check box or radio button has no -command: its -changeCommand takes the command.
            let command_flag = if matches!(cmd, "iconTextCheckBox" | "iconTextRadioButton") { "changeCommand" } else { "command" };
            for (from, to) in [("label", "label"), ("annotation", "annotation"), ("image", "image"), ("version", "version"), ("command", command_flag)] {
                if flags.get(to).is_some_and(|v| !v.is_null()) || (to == "image" && flags.get("image1").is_some_and(|v| !v.is_null())) {
                    continue;
                }
                if cmd == "iconTextStaticLabel" && from == "command" {
                    continue;
                }
                let v = text(&e[from]);
                if v.is_empty() {
                    continue;
                }
                let v = if from == "command" && e["language"].as_str() == Some("python") { json!({ "__pysrc__": v }) } else { json!(v) };
                flags.insert(to.to_string(), v);
            }
        }
        if matches!(cmd, "radioButton" | "iconTextRadioButton" | "toolButton") && !flags.contains_key("collection") {
            if let Some(c) = m.collection.and_then(|c| m.get(c)) {
                flags.insert("collection".into(), json!(c.path));
            }
        }
        if let Some(c) = flags.get("collection").map(text) {
            if let Some(cp) = m.find(&c).and_then(|i| m.get(i)).map(|x| x.path.clone()) {
                flags.insert("collection".into(), json!(cp));
            }
        }
        // Radio items in a menu item collection.
        if cmd == "menuItem" && flags.get("radioButton").is_some() && !flags.contains_key("collection") {
            if let Some(c) = m.collection.and_then(|c| m.get(c)) {
                flags.insert("collection".into(), json!(c.path));
            }
        }
        let mut props = match apply(m, id, &flags) {
            Ok(p) => p,
            Err(e) => {
                m.delete(id);
                return Err(e);
            }
        };
        if matches!(cmd, "radioButton" | "iconTextRadioButton") && flags.get("select").is_some_and(truth) {
            radio_exclusive(m, id);
        }
        if cmd == "scriptTable" {
            m.dirty_tables.push(id);
        }
        // The labels a control shows when it is given none, for the shell.
        props.entry("name".to_string()).or_insert(json!(name));
        let parent_path = parent.and_then(|p| m.get(p)).map(|p| p.path.clone());
        op(json!({"op": "create", "path": path, "kind": cmd, "parent": parent_path, "props": props}));
        // Where the next things go.
        match cat_of(cmd) {
            Cat::Window | Cat::Layout => {
                m.parent = Some(id);
                if cmd == "window" {
                    m.menu = None;
                }
            }
            Cat::Menu => m.menu = Some(id),
            Cat::MenuItem => {
                if flags.get("subMenu").is_some_and(truth) {
                    m.menu = Some(id);
                }
            }
            Cat::Collection => m.collection = Some(id),
            Cat::Control => {}
        }
        if cmd == "window" {
            return Ok(json!(name));
        }
        Ok(json!(path))
    });
    r.map_err(aztek_ops::CommandError::new)
}

fn control_command(cmd: &str, a: &Args) -> CmdResult {
    if a.bool("query") {
        return run_query(cmd, a);
    }
    if a.bool("edit") {
        return run_edit(cmd, a);
    }
    if a.bool("exists") {
        return Ok(json!(a.pos_str(0).is_some_and(|n| with_model(|m| m.find(n).is_some_and(|i| m.get(i).is_some_and(|c| wants_kind(cmd, c)))))));
    }
    create(cmd, a)
}

/// Runs what a window's making asks for: the uiScript of a workspace control, inside it.
fn after_create(cmd: &str, result: &CmdResult, a: &Args) {
    if cmd == "window" && result.is_ok() {
        apply_window_pref(result.as_ref().ok().and_then(Value::as_str).unwrap_or(""), a);
        return;
    }
    if cmd == "dockControl" && result.is_ok() {
        // Docks float here: what shows is the window it holds.
        if let Some(content) = a.get("content").map(text) {
            let path = with_model(|m| m.find(&content).and_then(|i| m.get(i)).map(|c| c.path.clone()));
            if let Some(p) = path {
                op(json!({"op": "show", "path": p}));
            }
        }
        return;
    }
    if cmd != "workspaceControl" || result.is_err() {
        return;
    }
    if let Some(script) = a.get("uiScript") {
        run_soon(callback_value(script), Vec::new());
    }
    let name = result.as_ref().ok().and_then(Value::as_str).unwrap_or("").to_string();
    let docked = a.has("dockToMainWindow") || a.has("dockToControl") || a.has("dockToPanel") || a.has("tabToControl");
    // -initCallback: a MEL proc given the control's name once it is in the layout.
    if let Some(Value::String(p)) = a.get("initCallback") {
        if !p.is_empty() {
            run_soon(json!(format!("{p} \"{name}\"")), Vec::new());
        }
    }
    with_model(|m| {
        let saved = m.ws_states.get(&name).cloned();
        if let Some(i) = m.find(&name) {
            if let Some(c) = m.controls.get_mut(&i) {
                // A saved workspaceControlState sizes and places it, where the call doesn't.
                if let Some(st) = saved {
                    let mut edit = Map::new();
                    for (k, v) in st {
                        if matches!(k.as_str(), "width" | "height" | "topEdge" | "leftEdge" | "maximized") && !a.has(&k) {
                            c.props.insert(k.clone(), v.clone());
                            edit.insert(k, v);
                        }
                    }
                    if let (Some(t), Some(l)) = (c.props.get("topEdge").cloned(), c.props.get("leftEdge").cloned()) {
                        edit.insert("topLeftCorner".into(), json!([t, l]));
                    }
                    if !edit.is_empty() {
                        op(json!({"op": "edit", "path": c.path, "props": edit}));
                    }
                }
                c.props.insert("visible".into(), json!(true));
                // It floats until it is docked.
                if !c.props.contains_key("floating") {
                    c.props.insert("floating".into(), json!(!docked));
                }
            }
        }
    });
    op(json!({"op": "show", "path": name}));
}

// ---------------------------------------------------------------------------------------------------------------------
// What the shell tells us

/// `{"path", "set": {flag: value}, "fire": [flag...], "args": [...], "closed": bool}`. Returns the callbacks to run.
pub fn shell_event(ev: &Value) -> Vec<(Value, Vec<Value>)> {
    let field = crate::cmd_fields::before_shell_event(ev);
    let mut out = shell_event_model(ev);
    out.extend(crate::command_line::shell_event(ev));
    if let Some(b) = field {
        out.extend(crate::cmd_fields::after_shell_event(b, ev));
    }
    out
}

fn shell_event_model(ev: &Value) -> Vec<(Value, Vec<Value>)> {
    let path = ev["path"].as_str().unwrap_or("");
    let args: Vec<Value> = ev["args"].as_array().cloned().unwrap_or_default();
    with_model(|m| {
        let Some(id) = m.find(path) else { return Vec::new() };
        if let Some(s) = ev["set"].as_object() {
            let saved = set_from_python(false);
            // Radio exclusivity, the option menu's index and so on behave as from a script; the shell has the rest.
            let _ = apply(m, id, s);
            set_from_python(saved);
        }
        let mut out = Vec::new();
        // A tool button's press makes its tool current.
        if ev["set"]["select"].as_bool() == Some(true) && m.get(id).is_some_and(|c| c.kind == "toolButton") {
            if let Some(t) = m.get(id).and_then(|c| tool_of(&c.props)) {
                out.push((json!(format!("setToolTo \"{t}\";")), Vec::new()));
            }
        }
        if let Some(f) = ev["fire"].as_array() {
            for flag in f.iter().filter_map(Value::as_str) {
                if flag.starts_with("__nav") {
                    out.extend(nav_event(m, id, flag, &args));
                    continue;
                }
                // -postMenuCommandOnce: the menu's -pmc runs the first time it shows, not again.
                if flag == "postMenuCommand" {
                    let once = m.get(id).and_then(|c| c.props.get("postMenuCommandOnce")).is_some_and(truth);
                    if once {
                        let ran = m.get(id).and_then(|c| c.props.get("_postMenuRan")).is_some_and(truth);
                        if ran {
                            continue;
                        }
                        if let Some(c) = m.controls.get_mut(&id) {
                            c.props.insert("_postMenuRan".into(), json!(true));
                        }
                    }
                }
                if let Some(cb) = m.get(id).and_then(|c| c.props.get(flag)).cloned() {
                    // -handleNodeDropCallback and -labelEditingCallback: a MEL script has the node's name or the new
                    // label added to it as a string.
                    if matches!(flag, "handleNodeDropCallback" | "labelEditingCallback") {
                        if let (Value::String(src), Some(arg)) = (&cb, args.first()) {
                            let quoted = text(arg).replace('\\', "\\\\").replace('"', "\\\"");
                            out.push((json!(format!("{src} \"{quoted}\"")), Vec::new()));
                            continue;
                        }
                    }
                    out.push((cb, args.clone()));
                }
            }
        }
        if ev["closed"].as_bool() == Some(true) {
            out.extend(close_control(m, id).into_iter().map(|cb| (cb, Vec::new())));
        }
        if ev["set"].get("horizontal").is_some() || ev["set"].get("floating").is_some() || ev["set"].get("width").is_some() {
            if m.get(id).is_some_and(|c| c.kind == "workspaceControl") {
                crate::user_folder::layout_modified();
            }
        }
        out
    })
}

/// An attrNavigationControlGrp's drop and menu: a node dropped goes to -connectAttrToDropped (node, attribute) or else
/// -connectNodeToDropped (node, the attribute's node); Set Key keys the attribute, Break Connection cuts what drives it.
fn nav_event(m: &Model, id: Id, flag: &str, args: &[Value]) -> Vec<(Value, Vec<Value>)> {
    let Some(c) = m.get(id) else { return Vec::new() };
    let attr = c.props.get("attribute").map(text).filter(|t| !t.is_empty()).or_else(|| c.props.get("attrNavDecision").and_then(|v| flat(v).first().map(text))).unwrap_or_default();
    let quote = |t: &str| format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\""));
    let call = |cb: &Value, extra: &[String]| -> (Value, Vec<Value>) {
        match cb {
            Value::String(src) => (json!(format!("{src} {}", extra.iter().map(|x| quote(x)).collect::<Vec<_>>().join(" "))), Vec::new()),
            o => (o.clone(), extra.iter().map(|x| json!(x)).collect()),
        }
    };
    match flag {
        NAV_DROP => {
            let mut out = Vec::new();
            for node in args.iter().map(text).filter(|n| !n.is_empty()) {
                if let Some(cb) = c.props.get("connectAttrToDropped") {
                    out.push(call(cb, &[node.clone(), attr.clone()]));
                } else if let Some(cb) = c.props.get("connectNodeToDropped") {
                    let dest = attr.split('.').next().unwrap_or("").to_string();
                    out.push(call(cb, &[node.clone(), dest]));
                }
            }
            out
        }
        "__navIgnore" => c.props.get("ignore").map(|cb| vec![(cb.clone(), vec![json!(attr)])]).unwrap_or_default(),
        "__navSetKey" if !attr.is_empty() => vec![(json!(format!("setKeyframe {};", quote(&attr))), Vec::new())],
        "__navBreak" if !attr.is_empty() => {
            let q = quote(&attr);
            vec![(json!(format!("{{ string $s[] = `listConnections -p 1 -s 1 -d 0 {q}`; if (size($s)) disconnectAttr $s[0] {q}; }}")), Vec::new())]
        }
        _ => Vec::new(),
    }
}

/// A window or dock closed: kept hidden when it is retained (a dock or workspace control unless -retain says not, a
/// window only with -retain), else deleted. A workspace control's size and place are kept as its workspaceControlState.
/// Returns the close commands to run.
fn close_control(m: &mut Model, id: Id) -> Vec<Value> {
    let Some(c) = m.get(id).cloned() else { return Vec::new() };
    if c.kind == "window" {
        m.record_window_pref(&c);
    }
    if c.kind == "workspaceControl" {
        let st = m.ws_states.entry(c.name.clone()).or_default();
        for k in ["width", "height", "topEdge", "leftEdge", "maximized"] {
            if let Some(v) = c.props.get(k) {
                st.insert(k.to_string(), v.clone());
            }
        }
    }
    let retain = c.props.get("retain").map_or(c.kind == "dockControl" || c.kind == "workspaceControl", truth);
    if retain {
        let mut out = Vec::new();
        if let Some(c) = m.controls.get_mut(&id) {
            c.props.insert("visible".into(), json!(false));
            if let Some(cb) = c.props.get("closeCommand") {
                out.push(cb.clone());
            }
            op(json!({"op": "edit", "path": c.path, "props": {"visible": false}}));
        }
        out
    } else {
        m.delete(id)
    }
}

/// A panel placed in a script's layout: a control the shell can build, with the current parent left as it is. No parent
/// and no layout under construction means it lives only as a panel.
pub fn embed_panel(kind: &str, name: &str, parent: Option<&str>, camera: &str) {
    with_model(|m| {
        let p = match parent.filter(|p| !p.is_empty()) {
            Some(n) => m.find(n),
            None => m.parent,
        };
        let Some(p) = p else { return };
        let path = format!("{}|{name}", m.get(p).unwrap().path);
        m.next += 1;
        let id = m.next;
        m.controls.insert(id, Control { kind: kind.to_string(), name: name.to_string(), path: path.clone(), parent: Some(p), children: Vec::new(), props: Map::new() });
        m.controls.get_mut(&p).unwrap().children.push(id);
        let parent_path = m.get(p).unwrap().path.clone();
        op(json!({"op": "create", "path": path, "kind": kind, "parent": parent_path, "props": {"name": name, "camera": camera}}));
    });
}

/// A cancel click, from the shell with a command suspended under it: the model only.
pub fn progress_cancelled() {
    with_model(|m| {
        let found = m.controls.values_mut().filter(|c| c.kind == "progressWindow" || c.path == MAIN_PROGRESS);
        for c in found {
            c.props.insert("isCancelled".into(), json!(true));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// The special commands

fn f(name: &str, short: &str, ty: ArgType) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, "")
}

fn delete_ui(a: &Args) -> CmdResult {
    let names = a.named();
    if names.is_empty() {
        fail!("deleteUI: name something to delete.");
    }
    let only: Option<&[Cat]> = if a.bool("window") {
        Some(&[Cat::Window])
    } else if a.bool("layout") {
        Some(&[Cat::Layout])
    } else if a.bool("control") {
        Some(&[Cat::Control, Cat::Menu])
    } else if a.bool("menu") {
        Some(&[Cat::Menu])
    } else if a.bool("menuItem") {
        Some(&[Cat::MenuItem])
    } else if a.bool("collection") {
        Some(&[Cat::Collection])
    } else {
        None
    };
    if a.bool("uiTemplate") {
        with_model(|m| {
            for n in &names {
                m.templates.remove(n);
            }
        });
        return Ok(Value::Null);
    }
    if a.bool("toolContext") {
        for n in &names {
            crate::ctx_cmds::delete_context(n);
        }
        return Ok(Value::Null);
    }
    if a.bool("panel") || a.bool("editor") {
        for n in &names {
            crate::panel_cmds::delete_panel(n);
        }
        return Ok(Value::Null);
    }
    for n in &names {
        let closes = with_model(|m| -> Result<Vec<Value>, String> {
            let Some(id) = m.find(n) else {
                return Err(format!("Object '{n}' not found."));
            };
            let cat = m.cat(id);
            if let Some(cats) = only {
                if !cats.contains(&cat) {
                    return Err(format!("Object '{n}' not found."));
                }
            }
            if m.get(id).is_some_and(|c| c.kind == "mainWindow" || c.path == MAIN_PROGRESS) {
                return Err(format!("deleteUI: '{n}' belongs to the application."));
            }
            Ok(m.delete(id))
        })
        .map_err(aztek_ops::CommandError::new)?;
        for c in closes {
            run_soon(c, Vec::new());
        }
    }
    Ok(Value::Null)
}

fn set_parent(a: &Args) -> CmdResult {
    let menu = a.bool("menu");
    let out = with_model(|m| -> Result<Value, String> {
        let cur = if menu { m.menu } else { m.parent };
        let path_of = |m: &Model, i: Option<Id>| json!(i.and_then(|x| m.get(x)).map(|c| c.path.clone()).unwrap_or_else(|| "NONE".to_string()));
        if a.bool("query") {
            return Ok(path_of(m, cur));
        }
        let up = |m: &Model, i: Id| m.get(i).and_then(|c| c.parent).or(Some(i));
        let next = if a.bool("topLevel") {
            cur.and_then(|i| m.window_of(i))
        } else if a.bool("upLevel") || a.pos_str(0) == Some("..") {
            match cur {
                // Up from a top-level menu stays on it, as the reference does: scripts that go up once too often
                // (menus of several submenus) still put their next items in that menu.
                Some(i) if menu => m.get(i).and_then(|c| c.parent).filter(|p| matches!(m.cat(*p), Cat::Menu | Cat::MenuItem)).or(Some(i)),
                Some(i) => up(m, i),
                None => None,
            }
        } else if let Some(n) = a.pos_str(0) {
            if n == "|" {
                None
            } else {
                Some(m.find(n).ok_or_else(|| format!("setParent: Object '{n}' not found."))?)
            }
        } else {
            return Ok(path_of(m, cur));
        };
        if menu {
            m.menu = next;
        } else {
            m.parent = next;
        }
        Ok(path_of(m, next))
    });
    out.map_err(aztek_ops::CommandError::new)
}

fn object_type_of(kind: &str) -> String {
    match kind {
        "window" => "floatingWindow".into(),
        "mainWindow" => "floatingWindow".into(),
        "textField" => "field".into(),
        "text" => "staticText".into(),
        "menuItem" => "commandMenuItem".into(),
        k if is_group(k) => "rowGroupLayout".into(),
        k => k.to_string(),
    }
}

fn dialog_answer(a: &Args, kind: &str, keys: &[&str]) -> Option<Value> {
    let mut p = Map::new();
    for k in keys {
        if let Some(v) = a.get(k) {
            p.insert(k.to_string(), v.clone());
        }
    }
    call_hook(kind, &Value::Object(p))
}

/// Batch runs take a dialog's default.
fn dialog_default(a: &Args) -> Value {
    if let Some(d) = a.get("defaultButton") {
        return json!(text(d));
    }
    if let Some(b) = a.get("button").map(flat).and_then(|b| b.first().cloned()) {
        return json!(text(&b));
    }
    json!(a.string("dismissString", "Confirm"))
}

fn progress_window(a: &Args) -> CmdResult {
    const NAME: &str = "progressWindow";
    let r = with_model(|m| -> Result<Value, String> {
        let existing = m.controls.iter().find(|(_, c)| c.kind == NAME).map(|(i, _)| *i);
        if a.bool("query") {
            let asked = a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| a.flags.keys().find(|k| !matches!(k.as_str(), "query" | "queried")).cloned());
            let Some(q) = asked else { return Err("progressWindow: name what to ask about.".into()) };
            let Some(id) = existing else {
                return Ok(match q.as_str() {
                    "isCancelled" | "isInterruptable" | "exists" => json!(false),
                    "maxValue" => json!(100),
                    "status" | "title" => json!(""),
                    _ => json!(0),
                });
            };
            return Ok(if q == "exists" { json!(true) } else { query(m, id, &q) });
        }
        if a.bool("endProgress") {
            if let Some(id) = existing {
                m.delete(id);
            }
            return Ok(Value::Null);
        }
        let id = match existing {
            Some(id) if a.bool("edit") => id,
            // As the reference: false when one is already showing, the call failing quietly.
            Some(_) => return Ok(json!(false)),
            None if a.bool("edit") => return Err("progressWindow: none is showing.".into()),
            None => {
                m.next += 1;
                let id = m.next;
                m.controls.insert(id, Control { kind: NAME.into(), name: NAME.into(), path: NAME.into(), parent: None, children: Vec::new(), props: Map::new() });
                op(json!({"op": "create", "path": NAME, "kind": NAME, "parent": null, "props": {}}));
                id
            }
        };
        let mut changed = Map::new();
        for (k, v) in &a.flags {
            if matches!(k.as_str(), "query" | "edit" | "queried") || v.is_null() {
                continue;
            }
            match k.as_str() {
                "progress" | "minValue" | "maxValue" | "step" => progress_flag(m, id, k, v, &mut changed),
                _ => set(m, id, k, v.clone(), &mut changed),
            }
        }
        if !changed.is_empty() {
            op(json!({"op": "edit", "path": NAME, "props": changed}));
        }
        Ok(json!(true))
    });
    let v = r.map_err(aztek_ops::CommandError::new)?;
    // Long loops never come back to the event loop: let it show its progress now.
    if !a.bool("query") {
        call_hook("pump", &json!({}));
    }
    Ok(v)
}

/// The positions -position and -clear take.
const IN_VIEW_POSITIONS: &[&str] = &["topLeft", "topCenter", "topRight", "midLeft", "midCenter", "midCenterTop", "midCenterBot", "midRight", "botLeft", "botCenter", "botRight"];

/// inViewMessage: one request to the UI per call, carrying what the reference's flags set. The UI keeps one message per
/// position, so a new message replaces the one where it goes.
fn in_view_message(a: &Args) -> CmdResult {
    for k in ["hide", "show", "minimize", "restore"] {
        if a.bool(k) {
            crate::core::push_ui_request(format!("inViewMessage|{}", json!({ "all": k })));
            return Ok(Value::Null);
        }
    }
    if let Some(p) = a.get("clear") {
        let p = text(p);
        if !IN_VIEW_POSITIONS.contains(&p.as_str()) {
            fail!("inViewMessage: -clear takes one of {}", IN_VIEW_POSITIONS.join(", "));
        }
        crate::core::push_ui_request(format!("inViewMessage|{}", json!({ "clear": true, "position": p })));
        return Ok(Value::Null);
    }
    let pos = a.string("position", "topCenter");
    if !IN_VIEW_POSITIONS.contains(&pos.as_str()) {
        fail!("inViewMessage: -position takes one of {}", IN_VIEW_POSITIONS.join(", "));
    }
    // -message is the general text; -assistMessage and -statusMessage take it over and say which kind it is.
    let (kind, msg) = if let Some(m) = a.get("assistMessage") {
        ("assist", text(m))
    } else if let Some(m) = a.get("statusMessage") {
        ("status", text(m))
    } else {
        ("general", a.get("message").map(text).unwrap_or_default())
    };
    let (fade_in, stay, fade_out) = (a.int("fadeInTime", 100), a.int("fadeStayTime", 1000), a.int("fadeOutTime", 400));
    let fade = a.bool("fade");
    // Without -fade the message stays until it is cleared, clicked or dragged away.
    let time = if fade { (fade_in + stay + fade_out) as f64 / 1000.0 } else { 0.0 };
    let back = a.get("backColor").and_then(Value::as_i64).unwrap_or(0) as u32 & 0x00ff_ffff;
    let req = json!({
        "message": msg,
        "kind": kind,
        "position": pos,
        "fade": fade,
        "time": time,
        "fadeInTime": fade_in,
        "fadeStayTime": stay,
        "fadeOutTime": fade_out,
        "alpha": a.float("alpha", 0.8).clamp(0.0, 1.0),
        "textAlpha": a.float("textAlpha", 1.0).clamp(0.0, 1.0),
        "backColor": format!("#{back:06x}"),
        "font": a.string("font", ""),
        "fontSize": a.int("fontSize", 0),
        "frameOffset": a.int("frameOffset", 20),
        "textOffset": a.int("textOffset", 10),
        "clickKill": a.bool("clickKill"),
        "dragKill": a.bool("dragKill"),
        "uvEditor": a.bool("uvEditor"),
    });
    crate::core::push_ui_request(format!("inViewMessage|{req}"));
    Ok(Value::Null)
}

fn workspace_control_state(_s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("workspaceControlState: name a workspace control.") };
    let asked = |k: &str| a.bool(k) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == k));
    with_model(|m| {
        if a.bool("exists") {
            return Ok(json!(m.ws_states.contains_key(&name)));
        }
        if a.bool("remove") {
            m.ws_states.remove(&name);
            return Ok(Value::Null);
        }
        if a.bool("query") {
            let Some(st) = m.ws_states.get(&name) else { fail!("workspaceControlState: no state is kept for {name}.") };
            let int = |k: &str| st.get(k).map(|v| json!(number(v) as i64)).unwrap_or(json!(0));
            let pair = |k: &str| match st.get(k) {
                Some(Value::Array(v)) => json!(v.iter().map(|x| number(x) as i64).collect::<Vec<_>>()),
                _ => json!([0, 0]),
            };
            return Ok(if asked("widthHeight") {
                json!([int("width"), int("height")])
            } else if asked("topLeftCorner") {
                json!([int("topEdge"), int("leftEdge")])
            } else if asked("defaultTopLeftCorner") {
                pair("defaultTopLeftCorner")
            } else if asked("defaultWidthHeight") {
                pair("defaultWidthHeight")
            } else if asked("maximized") {
                json!(st.get("maximized").is_some_and(truth))
            } else if let Some(k) = ["width", "height", "topEdge", "leftEdge"].into_iter().find(|k| asked(k)) {
                int(k)
            } else {
                fail!("workspaceControlState: name what to query.")
            });
        }
        // Made, or changed: the values given.
        let st = m.ws_states.entry(name.clone()).or_default();
        let ints = |v: &Value| -> Vec<i64> { flat(v).iter().map(|x| number(x) as i64).collect() };
        if let Some(v) = a.get("widthHeight").map(ints) {
            if let [w, h, ..] = v.as_slice() {
                st.insert("width".into(), json!(w));
                st.insert("height".into(), json!(h));
            }
        }
        if let Some(v) = a.get("topLeftCorner").map(ints) {
            if let [t, l, ..] = v.as_slice() {
                st.insert("topEdge".into(), json!(t));
                st.insert("leftEdge".into(), json!(l));
            }
        }
        for k in ["width", "height", "topEdge", "leftEdge"] {
            if let Some(v) = a.get(k) {
                st.insert(k.into(), json!(number(v) as i64));
            }
        }
        for k in ["defaultTopLeftCorner", "defaultWidthHeight"] {
            if let Some(v) = a.get(k).map(ints) {
                st.insert(k.into(), json!(v));
            }
        }
        if let Some(v) = a.get("maximized") {
            st.insert("maximized".into(), json!(truth(v)));
        }
        Ok(Value::Null)
    })
}

/// The flags a windowPref line in windowPrefs.mel can carry, as (long, short, values).
const WINDOW_PREF_FLAGS: [(&str, &str, usize); 8] = [
    ("topEdge", "te", 1),
    ("leftEdge", "le", 1),
    ("width", "w", 1),
    ("height", "h", 1),
    ("topLeftCorner", "tlc", 2),
    ("widthHeight", "wh", 2),
    ("maximized", "max", 1),
    ("parentMain", "pm", 1),
];

/// Sets a window preference from the values given: the two-value flags are split into their edges and sizes.
fn set_window_pref(e: &mut Map<String, Value>, key: &str, vals: &[Value]) {
    let int = |v: &Value| json!(number(v) as i64);
    match (key, vals) {
        ("widthHeight", [w, h, ..]) => {
            e.insert("width".into(), int(w));
            e.insert("height".into(), int(h));
        }
        ("topLeftCorner", [t, l, ..]) => {
            e.insert("topEdge".into(), int(t));
            e.insert("leftEdge".into(), int(l));
        }
        ("width" | "height" | "topEdge" | "leftEdge", [v, ..]) => {
            e.insert(key.into(), int(v));
        }
        ("maximized" | "parentMain", [v, ..]) => {
            e.insert(key.into(), json!(truth(v)));
        }
        _ => {}
    }
}

/// The saved window preferences as windowPref statements, one per window, for windowPrefs.mel.
pub(crate) fn window_pref_lines() -> Vec<String> {
    let quote = |t: &str| format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\""));
    with_model(|m| {
        m.window_prefs
            .iter()
            .map(|(name, p)| {
                let mut l = String::from("windowPref");
                for k in ["topEdge", "leftEdge", "width", "height"] {
                    if let Some(v) = p.get(k) {
                        l.push_str(&format!(" -{k} {}", number(v) as i64));
                    }
                }
                for k in ["maximized", "parentMain"] {
                    if let Some(v) = p.get(k) {
                        l.push_str(&format!(" -{k} {}", truth(v) as i32));
                    }
                }
                l.push_str(&format!(" {};", quote(name)));
                l
            })
            .collect()
    })
}

/// Reads windowPref statements (as -saveAll or savePrefs writes them) into the saved preferences.
fn load_window_prefs(text: &str) {
    // Statements end at a semicolon outside quotes; words split on spaces, quoted words kept whole.
    let mut words: Vec<Vec<(String, bool)>> = vec![Vec::new()];
    let mut cur = String::new();
    let (mut quoted, mut was_quoted, mut esc) = (false, false, false);
    let mut in_comment = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
            }
            continue;
        }
        if quoted {
            match (esc, ch) {
                (true, 'n') => {
                    cur.push('\n');
                    esc = false;
                }
                (true, c) => {
                    cur.push(c);
                    esc = false;
                }
                (false, '\\') => esc = true,
                (false, '"') => quoted = false,
                (false, c) => cur.push(c),
            }
            continue;
        }
        match ch {
            '/' if chars.peek() == Some(&'/') => in_comment = true,
            '"' => {
                quoted = true;
                was_quoted = true;
            }
            c if c.is_whitespace() || c == ';' => {
                if !cur.is_empty() || was_quoted {
                    words.last_mut().unwrap().push((std::mem::take(&mut cur), was_quoted));
                    was_quoted = false;
                }
                if c == ';' {
                    words.push(Vec::new());
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || was_quoted {
        words.last_mut().unwrap().push((cur, was_quoted));
    }
    with_model(|m| {
        for st in words {
            if st.first().map(|w| w.0.as_str()) != Some("windowPref") {
                continue;
            }
            let mut e = Map::new();
            let mut name = None;
            let mut i = 1;
            while i < st.len() {
                let (w, q) = &st[i];
                i += 1;
                if !q && w.starts_with('-') {
                    let fl = &w[1..];
                    if let Some(&(long, _, n)) = WINDOW_PREF_FLAGS.iter().find(|x| x.0 == fl || x.1 == fl) {
                        let vals: Vec<Value> = st.iter().skip(i).take(n).map(|x| x.0.parse::<f64>().map(|v| json!(v)).unwrap_or(json!(x.0 == "true" || x.0 == "on"))).collect();
                        i += n;
                        set_window_pref(&mut e, long, &vals);
                    }
                    continue;
                }
                name = Some(w.clone());
            }
            if let Some(n) = name.filter(|n| !n.is_empty()) {
                m.window_prefs.entry(n).or_default().extend(e);
            }
        }
    });
}

/// A saved preference sizes and places a window just made, where the call doesn't.
fn apply_window_pref(name: &str, a: &Args) {
    with_model(|m| {
        if m.window_prefs_off {
            return;
        }
        let Some(pref) = m.window_prefs.get(name).cloned() else { return };
        let Some(id) = m.find(name).filter(|i| m.get(*i).is_some_and(|c| c.kind == "window")) else { return };
        let given = |k: &str| match k {
            "width" | "height" => a.has(k) || a.has("widthHeight"),
            "topEdge" | "leftEdge" => a.has(k) || a.has("topLeftCorner"),
            _ => a.has(k),
        };
        let Some(c) = m.controls.get_mut(&id) else { return };
        let mut edit = Map::new();
        for k in ["width", "height", "topEdge", "leftEdge", "maximized"] {
            if let Some(v) = pref.get(k).filter(|_| !given(k)) {
                c.props.insert(k.into(), v.clone());
                edit.insert(k.into(), v.clone());
            }
        }
        if edit.contains_key("topEdge") || edit.contains_key("leftEdge") {
            let edge = |k: &str| c.props.get(k).map(|v| number(v) as i64).unwrap_or(0);
            edit.insert("topLeftCorner".into(), json!([edge("topEdge"), edge("leftEdge")]));
        }
        if !edit.is_empty() {
            op(json!({"op": "edit", "path": c.path, "props": edit}));
        }
    });
}

/// A main window state file: a name without a folder is in the preferences folder.
fn main_window_state_path(name: &str) -> std::path::PathBuf {
    let p = std::path::PathBuf::from(name);
    if p.is_absolute() {
        p
    } else {
        crate::user_folder::user_pref_dir().join(p)
    }
}

/// -saveMainWindowState: the main window's size and place and each workspace control's place, docking and showing.
fn save_main_window_state(name: &str) -> CmdResult {
    let st = with_model(|m| {
        let main = m.find(MAIN_WINDOW).and_then(|i| m.get(i)).map(|c| {
            let mut o = Map::new();
            for k in ["width", "height", "topEdge", "leftEdge", "maximized"] {
                if let Some(v) = c.props.get(k) {
                    o.insert(k.into(), v.clone());
                }
            }
            o
        });
        let mut wcs: BTreeMap<String, Map<String, Value>> = m.ws_states.clone();
        for c in m.controls.values().filter(|c| c.kind == "workspaceControl") {
            let e = wcs.entry(c.name.clone()).or_default();
            for k in ["width", "height", "topEdge", "leftEdge", "maximized", "floating", "visible", "dockToMainWindow", "dockToControl", "tabToControl"] {
                if let Some(v) = c.props.get(k) {
                    e.insert(k.into(), v.clone());
                }
            }
        }
        json!({"mainWindow": main, "workspaceControls": wcs})
    });
    let path = main_window_state_path(name);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| aztek_ops::CommandError::new(format!("windowPref: {}: {e}", d.display())))?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&st).unwrap_or_default()).map_err(|e| aztek_ops::CommandError::new(format!("windowPref: {}: {e}", path.display())))?;
    Ok(Value::Null)
}

/// -restoreMainWindowState: reads what -saveMainWindowState wrote and puts the main window and workspace controls back.
fn restore_main_window_state(name: &str) -> CmdResult {
    let path = main_window_state_path(name);
    let text = std::fs::read_to_string(&path).map_err(|e| aztek_ops::CommandError::new(format!("windowPref: {}: {e}", path.display())))?;
    let st: Value = serde_json::from_str(&text).map_err(|e| aztek_ops::CommandError::new(format!("windowPref: {} is not a main window state: {e}", path.display())))?;
    with_model(|m| {
        let apply = |m: &mut Model, name: &str, props: &Map<String, Value>| {
            let Some(c) = m.find(name).and_then(|i| m.controls.get_mut(&i)) else { return };
            let mut edit = props.clone();
            edit.retain(|k, _| !matches!(k.as_str(), "dockToMainWindow" | "dockToControl" | "tabToControl"));
            for (k, v) in &edit {
                c.props.insert(k.clone(), v.clone());
            }
            if let (Some(t), Some(l)) = (edit.get("topEdge").cloned(), edit.get("leftEdge").cloned()) {
                edit.insert("topLeftCorner".into(), json!([t, l]));
            }
            if !edit.is_empty() {
                op(json!({"op": "edit", "path": c.path, "props": edit}));
            }
        };
        if let Some(main) = st.get("mainWindow").and_then(Value::as_object) {
            apply(m, MAIN_WINDOW, main);
        }
        if let Some(wcs) = st.get("workspaceControls").and_then(Value::as_object) {
            for (n, v) in wcs {
                let Some(o) = v.as_object() else { continue };
                let kept = m.ws_states.entry(n.clone()).or_default();
                for k in ["width", "height", "topEdge", "leftEdge", "maximized"] {
                    if let Some(x) = o.get(k) {
                        kept.insert(k.into(), x.clone());
                    }
                }
                apply(m, n, o);
            }
        }
    });
    Ok(Value::Null)
}

fn window_pref(_s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).unwrap_or("").to_string();
    let query = a.bool("query");
    let asked = |k: &str| a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == k)) || (query && a.bool(k));
    if let Some(f) = a.get("saveMainWindowState").filter(|_| !query) {
        return save_main_window_state(&text(f));
    }
    if let Some(f) = a.get("restoreMainWindowState").filter(|_| !query) {
        return restore_main_window_state(&text(f));
    }
    let file = || crate::user_folder::user_pref_dir().join("windowPrefs.mel");
    if a.bool("saveAll") {
        crate::sys_prefs::write_window_prefs(&file())?;
        return Ok(Value::Null);
    }
    if a.bool("loadAll") {
        if let Ok(t) = std::fs::read_to_string(file()) {
            load_window_prefs(&t);
        }
        return Ok(Value::Null);
    }
    with_model(|m| {
        if a.bool("exists") {
            return Ok(json!(m.window_prefs.contains_key(&name)));
        }
        if query && asked("enableAll") {
            return Ok(json!(!m.window_prefs_off));
        }
        if !query && a.has("enableAll") {
            m.window_prefs_off = !a.bool("enableAll");
            return Ok(Value::Null);
        }
        if a.bool("removeAll") {
            m.window_prefs.clear();
            return Ok(Value::Null);
        }
        if name.is_empty() {
            fail!("windowPref: name a window.");
        }
        if a.bool("remove") {
            m.window_prefs.remove(&name);
            return Ok(Value::Null);
        }
        if query {
            let Some(e) = m.window_prefs.get(&name) else { fail!("windowPref: no window preference is kept for {name}.") };
            let int = |k: &str| e.get(k).map(|v| number(v) as i64).unwrap_or(0);
            return Ok(if asked("widthHeight") {
                json!([int("width"), int("height")])
            } else if asked("topLeftCorner") {
                json!([int("topEdge"), int("leftEdge")])
            } else if asked("maximized") {
                json!(e.get("maximized").is_some_and(truth))
            } else if asked("parentMain") {
                json!(e.get("parentMain").map_or(true, truth))
            } else if let Some(k) = ["width", "height", "topEdge", "leftEdge"].into_iter().find(|k| asked(k)) {
                json!(int(k))
            } else {
                fail!("windowPref: name what to query.")
            });
        }
        if a.bool("edit") && !m.window_prefs.contains_key(&name) {
            fail!("windowPref: no window preference is kept for {name}.");
        }
        let e = m.window_prefs.entry(name).or_default();
        for &(k, _, _) in &WINDOW_PREF_FLAGS {
            if let Some(v) = a.get(k) {
                set_window_pref(e, k, &flat(v));
            }
        }
        Ok(Value::Null)
    })
}

/// The workspace control a control or panel is in, or "".
pub(crate) fn parent_workspace_control(name: &str) -> String {
    with_model(|m| {
        let mut at = m.find(name);
        while let Some(i) = at {
            let Some(c) = m.get(i) else { break };
            if c.kind == "workspaceControl" {
                return c.name.clone();
            }
            at = c.parent;
        }
        String::new()
    })
}

/// The scene's timeControl flags: the range picked on the slider, its key display and snapping, and the sound it shows.
const TIME_SCENE_FLAGS: &[&str] = &["rangeArray", "range", "rangeVisible", "showKeys", "snap", "showKeysCombined", "animLayerShowWeight", "tickSize", "minTime", "maxTime", "sound", "displaySound"];

/// timeControl: the one time slider. The scene keeps its range, key display and sound; the model keeps it as a control,
/// with its colours, mouse scripts and the rest. Create mode edits it, as there is only ever the one.
fn time_control(s: &mut Session, a: &Args, scene: &aztek_ops::RunFn<Session>) -> CmdResult {
    let name = a.pos_str(0).map(str::to_string);
    if a.bool("exists") && !a.bool("query") {
        return Ok(json!(name.as_deref().is_some_and(|n| with_model(|m| m.find(n).is_some_and(|i| m.get(i).is_some_and(|c| c.kind == "timeControl"))))));
    }
    let id = with_model(|m| m.find(TIME_SLIDER)).ok_or_else(|| aztek_ops::CommandError::new("timeControl: the time slider is missing"))?;
    if a.bool("query") {
        let mut asked: Vec<String> = a.get("queried").and_then(Value::as_array).map(|q| q.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        for k in a.flags.keys() {
            if !matches!(k.as_str(), "query" | "queried") && !asked.contains(k) {
                asked.push(k.clone());
            }
        }
        let Some(first) = asked.first().cloned() else { fail!("timeControl: name what to ask about.") };
        if TIME_SCENE_FLAGS.contains(&first.as_str()) {
            return scene(s, a);
        }
        if first == "animCurveNames" {
            return time_control_curves(s, id, scene);
        }
        if first == "exists" {
            return Ok(json!(true));
        }
        return Ok(with_model(|m| query(m, id, &first)));
    }
    // Edit, or create, which edits the one there is.
    let mut scene_args = Args { positional: a.positional.clone(), flags: Map::new() };
    let mut own = Map::new();
    for (k, v) in &a.flags {
        if TIME_SCENE_FLAGS.contains(&k.as_str()) {
            scene_args.flags.insert(k.clone(), v.clone());
        } else if !matches!(k.as_str(), "edit" | "query" | "queried") {
            own.insert(k.clone(), v.clone());
        }
    }
    // The sound shown is the scene's; the control keeps it too, for -soundSource.
    for k in ["sound", "displaySound"] {
        if let Some(v) = a.get(k) {
            own.insert(k.to_string(), v.clone());
        }
    }
    if let Some(v) = a.get("soundSource").and_then(Value::as_str) {
        if !v.starts_with('<') {
            scene_args.flags.insert("sound".into(), json!(v));
        }
    }
    if !scene_args.flags.is_empty() {
        scene_args.flags.insert("edit".into(), json!(true));
        scene(s, &scene_args)?;
    }
    let path = with_model(|m| -> Result<String, String> {
        let changed = apply(m, id, &own)?;
        let c = m.get(id).unwrap();
        if !changed.is_empty() {
            op(json!({"op": "edit", "path": c.path, "props": changed}));
        }
        Ok(c.path.clone())
    })
    .map_err(aztek_ops::CommandError::new)?;
    Ok(json!(path))
}

/// timeControl -q -animCurveNames: the curves whose keys the slider shows, from its main list connection, or the
/// selection; none when -showKeys is none.
fn time_control_curves(s: &mut Session, id: Id, scene: &aztek_ops::RunFn<Session>) -> CmdResult {
    let shown = scene(s, &Args { positional: Vec::new(), flags: [("query".to_string(), json!(true)), ("queried".to_string(), json!(["showKeys"]))].into_iter().collect() })?;
    if shown.as_str() == Some("none") {
        return Ok(json!([]));
    }
    let conn = with_model(|m| m.get(id).and_then(|c| c.props.get("mainListConnection")).map(text)).unwrap_or_default();
    let objects: Vec<String> = if conn.is_empty() { s.graph.selection().iter().map(|&i| s.graph.path(i)).collect() } else { crate::script_objects::connection_objects(&conn).unwrap_or_default() };
    if objects.is_empty() {
        return Ok(json!([]));
    }
    let r = s.run("keyframe", json!({"args": objects, "query": true, "name": true})).unwrap_or(json!([]));
    Ok(r)
}

fn register_time_control(s: &mut Session) {
    let Some(base) = s.commands.find("timeControl").cloned() else { return };
    let scene = base.run.clone();
    let mut c = Cmd::new(
        "timeControl",
        "The time slider, the one timeControl ($gPlayBackSlider): the range picked on it (-q -ra, -q -rng, -q -rv), its key display and \
         snapping, the sound it shows and how, its colours, and scripts run on mouse down and up. -q -acn lists the curves whose keys it shows.",
        move |s, a| time_control(s, a, &scene),
    )
    .positional("the slider's name ($gPlayBackSlider)")
    .undoable(false);
    for fl in &base.flags {
        // The scene's flags, but with the reference's short names where they differ.
        let mut fl = fl.clone();
        match fl.name.as_str() {
            "snap" => fl.short_name = "sn".into(),
            "forceRedraw" | "beginScrub" | "endScrub" | "minTime" | "maxTime" => continue,
            _ => {}
        }
        if fl.name == "tickSize" {
            fl.ty = ArgType::Int;
        }
        c = c.flag(fl);
    }
    c = add_flags(c, TIME_CONTROL_FLAGS);
    c = add_flags(c, SOUND_FLAGS);
    c = add_flags(c, COMMON);
    s.register(c);
}

pub fn register(s: &mut Session) {
    register_time_control(s);
    s.register(
        Cmd::new(
            "aztekTimeControlState",
            "For the main window's time slider: what timeControl set on it, as one object (colours 0 to 1, the waveform part, \
             whether it drives the scene's time, snapping, tick span and width, shown and enabled).",
            |s, _a| {
                let q = |s: &mut Session, flag: &str| s.run("timeControl", json!({"query": true, "queried": [flag]})).unwrap_or(Value::Null);
                let (snap, tick, ds) = (q(s, "snap"), q(s, "tickSize"), q(s, "displaySound"));
                Ok(with_model(|m| {
                    let Some(id) = m.find(TIME_SLIDER) else { return Value::Null };
                    let mut o = Map::new();
                    for k in ["currentFrameColor", "foregroundColor", "backgroundColor", "waveform", "globalTime", "tickSpan", "enable", "visible", "manage", "repeatOnHold", "repeatChunkSize", "scrubbing", "redraws", "annotation"] {
                        o.insert(k.to_string(), query(m, id, k));
                    }
                    let c = m.get(id).unwrap();
                    o.insert("enableBackground".into(), json!(c.props.contains_key("backgroundColor") && c.props.get("enableBackground").is_none_or(truth) && !c.props.get("noBackground").is_some_and(truth)));
                    o.insert("path".into(), json!(c.path));
                    o.insert("snap".into(), snap);
                    o.insert("tickSize".into(), tick);
                    o.insert("displaySound".into(), ds);
                    Value::Object(o)
                }))
            },
        )
        .undoable(false),
    );
    s.register(
        Cmd::new("pause", "Wait the given number of seconds before the script goes on: for demos and tests, to see what it did.", |_s, a| {
            let secs = a.get("seconds").and_then(Value::as_f64).unwrap_or(0.0);
            if secs > 0.0 {
                std::thread::sleep(std::time::Duration::from_secs_f64(secs.min(86_400.0)));
            }
            Ok(Value::Null)
        })
        .flag(FlagDef::new("seconds", "sec", ArgType::Int, Value::Null, "How many seconds to wait."))
        .undoable(true),
    );
    for (name, spec) in KINDS {
        let cmd: &'static str = name;
        let mut c = Cmd::new(cmd, &format!("Script UI: {cmd}, with create, edit and query as in the reference's command."), move |s, a| {
            if let Some(v) = crate::attr_ui::live_query(s, cmd, a) {
                return Ok(v);
            }
            if cmd == "switchTable" {
                return switch_table(s, a);
            }
            if cmd == "channelBox" {
                return crate::channel_box::command(s, a);
            }
            if crate::cmd_fields::is_field(cmd) {
                return crate::cmd_fields::command(s, cmd, a);
            }
            if crate::more_controls::is_mine(cmd) {
                return crate::more_controls::command(s, cmd, a);
            }
            if crate::ramp_controls::is_mine(cmd) {
                return crate::ramp_controls::command(s, cmd, a);
            }
            let r = control_command(cmd, a);
            if !a.bool("query") && !a.bool("edit") {
                after_create(cmd, &r, a);
            }
            if r.is_ok() && !a.bool("query") {
                crate::attr_ui::after_command(s, cmd, a, &r);
            }
            r
        })
        .positional("name")
        .undoable(false);
        c = add_flags(c, spec);
        // The float kinds add to their int counterparts.
        c = match *name {
            "floatField" => add_flags(c, FIELD),
            "floatFieldGrp" => add_flags(c, FIELD_GRP),
            "floatSliderGrp" | "floatSliderButtonGrp" => add_flags(c, SLIDER_GRP),
            "colorSliderButtonGrp" => add_flags(c, COLOR_GRP),
            "iconTextCheckBox" | "iconTextRadioButton" | "iconTextStaticLabel" => add_flags(c, ICON_LOOK),
            _ => c,
        };
        if is_group(name) || *name == "rowLayout" || *name == "keyframeStats" {
            c = add_column_flags(c);
            c = add_flags(c, "adjustableColumn:adj:a columnWidth:cw:a2 columnAlign:cal:a2 columnAttach:cat:a3 rowAttach:rat:a3");
        }
        if cat_of(name) == Cat::Layout {
            c = add_flags(c, LAYOUT);
        }
        if cat_of(name) == Cat::Window {
            c = add_flags(c, LAYOUT);
        }
        c = add_flags(c, COMMON);
        s.register(c);
    }

    // Any control, or any layout, by name.
    for (name, doc) in [("control", "Edit and query any control."), ("layout", "Edit and query any layout.")] {
        let cmd: &'static str = name;
        let c = Cmd::new(cmd, doc, move |_s, a| {
            if !a.bool("query") && !a.bool("edit") && !a.bool("exists") {
                fail!("{cmd}: use -q or -e.");
            }
            control_command(cmd, a)
        })
        .positional("name")
        .undoable(false);
        let c = if cmd == "layout" { add_flags(c, "generalSpacing:gsp:a margins:mar:a margins4:mr4:a4") } else { c };
        s.register(add_flags(add_flags(c, LAYOUT), COMMON));
    }

    crate::attr_ui::register(s);
    crate::cmd_fields::register(s);
    crate::more_controls::register(s);

    for (name, spec) in [
        ("toolCollection", "clear:cl:b select:sl:a collectionItemArray:cia:b numberOfCollectionItems:nci:b global:gl:b parent:p:a exists:ex:b query:q:b edit:e:b queried::a defineTemplate:dt:a useTemplate:ut:a"),
        ("iconTextRadioCollection", "select:sl:a disableCommands:dcm:b collectionItemArray:cia:b numberOfCollectionItems:nci:b global:gl:b parent:p:a exists:ex:b query:q:b edit:e:b queried::a defineTemplate:dt:a useTemplate:ut:a"),
        ("radioCollection", "select:sl:a collectionItemArray:cia:b numberOfCollectionItems:nci:b global:gl:b parent:p:a exists:ex:b query:q:b edit:e:b queried::a defineTemplate:dt:a useTemplate:ut:a"),
        ("radioMenuItemCollection", "select:sl:a collectionItemArray:cia:b numberOfCollectionItems:nci:b global:gl:b parent:p:a exists:ex:b query:q:b edit:e:b queried::a defineTemplate:dt:a useTemplate:ut:a"),
    ] {
        let cmd: &'static str = name;
        let c = Cmd::new(cmd, "A set of radio items, one selected.", move |_s, a| control_command(cmd, a)).positional("name").undoable(false);
        s.register(add_flags(c, spec));
    }

    s.register(
        Cmd::new("showWindow", "Show a window; with no name, the one being built.", |_s, a| {
            let r = with_model(|m| -> Result<String, String> {
                let id = match a.pos_str(0) {
                    Some(n) => m.find(n).ok_or_else(|| format!("showWindow: Object '{n}' not found."))?,
                    None => m.parent.and_then(|p| m.window_of(p)).ok_or("showWindow: No window found.".to_string())?,
                };
                if m.cat(id) != Cat::Window {
                    return Err("showWindow: not a window.".into());
                }
                let c = m.controls.get_mut(&id).unwrap();
                c.props.insert("visible".into(), json!(true));
                Ok(c.path.clone())
            });
            let p = r.map_err(aztek_ops::CommandError::new)?;
            op(json!({"op": "show", "path": p}));
            Ok(json!(p))
        })
        .positional("[window]")
        .undoable(false),
    );

    let mut del = Cmd::new("deleteUI", "Delete windows, layouts, controls, menus: -window -layout -control -menu -menuItem pick the kind.", |_s, a| delete_ui(a)).positional("names").undoable(false);
    for (n, sh) in [("window", "wnd"), ("layout", "lay"), ("control", "ctl"), ("menu", "m"), ("menuItem", "mi"), ("panel", "pnl"), ("panelConfig", "pc"), ("editor", "ed"), ("collection", "cl"), ("radioMenuItemCollection", "ric"), ("uiTemplate", "uit"), ("exists", "ex"), ("toolContext", "tc")] {
        del = del.flag(f(n, sh, ArgType::Bool));
    }
    s.register(del);

    s.register(
        Cmd::new("setParent", "The parent new controls go under: a name, .. for one level up, -top the window, -menu the menu parent instead. -q reads it.", |_s, a| set_parent(a))
            .positional("[name or ..]")
            .undoable(false)
            .flag(f("query", "q", ArgType::Bool))
            .flag(f("edit", "e", ArgType::Bool))
            .flag(f("menu", "m", ArgType::Bool))
            .flag(f("topLevel", "top", ArgType::Bool))
            .flag(f("upLevel", "u", ArgType::Bool))
            .flag(f("defineTemplate", "dt", ArgType::Any))
            .flag(f("useTemplate", "ut", ArgType::Any)),
    );

    s.register(
        Cmd::new("objectTypeUI", "The type of an interface element by name; -isType tests it, -listAll lists the types.", |_s, a| {
            if a.bool("listAll") {
                let all: Vec<String> = KINDS.iter().map(|(k, _)| object_type_of(k)).collect();
                return Ok(json!(all));
            }
            let Some(n) = a.pos_str(0) else { fail!("objectTypeUI: give a name.") };
            let k = with_model(|m| m.find(n).and_then(|i| m.get(i)).map(|c| c.kind.clone()));
            let Some(k) = k else {
                if crate::panel_cmds::panel_type(n).is_some() {
                    return Ok(json!("modelEditor"));
                }
                fail!("Object '{n}' not found.")
            };
            let t = object_type_of(&k);
            if let Some(want) = a.get("isType").map(text) {
                return Ok(json!(want == t || want == k));
            }
            if a.bool("superClasses") {
                return Ok(json!([t]));
            }
            Ok(json!(t))
        })
        .positional("name")
        .undoable(false)
        .flag(f("isType", "i", ArgType::Any))
        .flag(f("listAll", "la", ArgType::Bool))
        .flag(f("superClasses", "sc", ArgType::Bool)),
    );

    s.register(
        Cmd::new(
            "lsUI",
            "The interface elements by name: -windows, -menus, -menuItems, -controls, -controlLayouts, -collection, -type for \
             some kinds, all with none of those or -dumpWidgets; -long gives full paths, -head and -tail cut the list.",
            |_s, a| {
                let pick = [("windows", Cat::Window), ("menus", Cat::Menu), ("menuItems", Cat::MenuItem), ("controls", Cat::Control), ("controlLayouts", Cat::Layout), ("collection", Cat::Collection)];
                let cats: Vec<Cat> = pick.iter().filter(|(f, _)| a.bool(f)).map(|(_, c)| *c).collect();
                let types: Vec<String> = a.get("type").map(|v| flat(v).iter().map(text).collect()).unwrap_or_default();
                let long = a.bool("long");
                // Panels, editors, contexts and templates aren't kept here: asked for alone, nothing.
                let everything = cats.is_empty() && types.is_empty() && !["panels", "editors", "contexts", "cmdTemplates"].iter().any(|f| a.bool(f));
                let mut out: Vec<String> = with_model(|m| {
                    m.controls
                        .values()
                        .filter(|c| everything || cats.contains(&cat_of(&c.kind)) || types.iter().any(|t| *t == c.kind || *t == object_type_of(&c.kind)))
                        .map(|c| if long { c.path.clone() } else { c.name.clone() })
                        .collect()
                });
                if let Some(n) = a.get("head").map(number) {
                    out.truncate(n.max(0.0) as usize);
                }
                if let Some(n) = a.get("tail").map(number) {
                    let n = n.max(0.0) as usize;
                    if out.len() > n {
                        out.drain(..out.len() - n);
                    }
                }
                if a.bool("numWindows") {
                    return Ok(json!(with_model(|m| m.controls.values().filter(|c| cat_of(&c.kind) == Cat::Window).count())));
                }
                Ok(json!(out))
            },
        )
        .undoable(false)
        .flag(f("windows", "wnd", ArgType::Bool))
        .flag(f("menus", "m", ArgType::Bool))
        .flag(f("menuItems", "mi", ArgType::Bool))
        .flag(f("controls", "ctl", ArgType::Bool))
        .flag(f("controlLayouts", "cl", ArgType::Bool))
        .flag(f("collection", "col", ArgType::Bool))
        .flag(f("panels", "p", ArgType::Bool))
        .flag(f("editors", "ed", ArgType::Bool))
        .flag(f("contexts", "ctx", ArgType::Bool))
        .flag(f("cmdTemplates", "ct", ArgType::Bool))
        .flag(f("type", "typ", ArgType::Any))
        .flag(f("long", "l", ArgType::Bool))
        .flag(f("head", "hd", ArgType::Any))
        .flag(f("tail", "tl", ArgType::Any))
        .flag(f("numWindows", "nw", ArgType::Bool))
        .flag(f("dumpWidgets", "dw", ArgType::Bool)),
    );

    s.register(
        Cmd::new("uiTemplate", "A named set of flag defaults, filled by the -defineTemplate flag of each control command.", |_s, a| {
            let name = a.pos_str(0).map(str::to_string);
            with_model(|m| {
                if a.bool("exists") {
                    return Ok(json!(name.as_ref().is_some_and(|n| m.templates.contains_key(n))));
                }
                let n = match name {
                    Some(n) => n,
                    None => {
                        let e = m.counters.entry("uiTemplate".into()).or_insert(0);
                        *e += 1;
                        format!("uiTemplate{e}")
                    }
                };
                m.templates.entry(n.clone()).or_default();
                Ok(json!(n))
            })
        })
        .positional("[name]")
        .undoable(false)
        .flag(f("exists", "ex", ArgType::Bool))
        .flag(f("defineTemplate", "dt", ArgType::Any))
        .flag(f("useTemplate", "ut", ArgType::Any)),
    );

    s.register(
        Cmd::new("setUITemplate", "Make a template current: -pushTemplate, -popTemplate. Returns the current one.", |_s, a| {
            with_model(|m| {
                if let Some(n) = a.pos_str(0) {
                    if a.bool("pushTemplate") {
                        if n != "NONE" && !m.templates.contains_key(n) {
                            fail!("setUITemplate: Object '{n}' not found.");
                        }
                        m.template_stack.push(n.to_string());
                    }
                }
                if a.bool("popTemplate") {
                    m.template_stack.pop();
                }
                Ok(json!(m.template_stack.last().cloned().unwrap_or_else(|| "NONE".to_string())))
            })
        })
        .positional("[template]")
        .undoable(false)
        .flag(f("pushTemplate", "pst", ArgType::Bool))
        .flag(f("popTemplate", "ppt", ArgType::Bool))
        .flag(f("query", "q", ArgType::Bool))
        .flag(f("edit", "e", ArgType::Bool)),
    );

    s.register(
        Cmd::new("waitCursor", "The wait cursor: -state on or off.", |_s, a| {
            with_model(|m| {
                if a.has("state") && !a.bool("query") {
                    m.wait_cursor = a.bool("state");
                    op(json!({"op": "waitCursor", "on": m.wait_cursor}));
                }
                Ok(json!(m.wait_cursor))
            })
        })
        .undoable(false)
        .flag(f("state", "st", ArgType::Bool))
        .flag(f("query", "q", ArgType::Bool)),
    );

    s.register(
        Cmd::new(
            "windowPref",
            "Saved window sizes and places, by window name. One is kept when a window closes and sizes and places the next \
             window of that name. -loadAll and -saveAll read and write windowPrefs.mel in the preferences folder.",
            window_pref,
        )
        .positional("[window]")
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool))
        .flag(f("edit", "e", ArgType::Bool))
        .flag(f("queried", "", ArgType::Any))
        .flag(f("exists", "ex", ArgType::Bool))
        .flag(f("remove", "r", ArgType::Bool))
        .flag(f("removeAll", "ra", ArgType::Bool))
        .flag(f("loadAll", "la", ArgType::Bool))
        .flag(f("saveAll", "sa", ArgType::Bool))
        .flag(f("widthHeight", "wh", ArgType::Any).multi(2))
        .flag(f("width", "w", ArgType::Any))
        .flag(f("height", "h", ArgType::Any))
        .flag(f("topLeftCorner", "tlc", ArgType::Any).multi(2))
        .flag(f("topEdge", "te", ArgType::Any))
        .flag(f("leftEdge", "le", ArgType::Any))
        .flag(f("maximized", "max", ArgType::Bool))
        .flag(f("parentMain", "pm", ArgType::Bool))
        .flag(f("restoreMainWindowState", "rms", ArgType::String))
        .flag(f("saveMainWindowState", "sms", ArgType::String))
        .flag(f("enableAll", "ea", ArgType::Bool))
    );

    s.register(
        Cmd::new(
            "workspaceControlState",
            "The size and place a workspace control is given when it is made: kept for each name, made when a workspace \
             control closes or by this command, and taken off with -remove. -ex says whether one is kept.",
            workspace_control_state,
        )
        .positional("name")
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool))
        .flag(f("edit", "e", ArgType::Bool))
        .flag(f("queried", "", ArgType::Any))
        .flag(f("exists", "ex", ArgType::Bool))
        .flag(f("remove", "r", ArgType::Bool))
        .flag(f("defaultTopLeftCorner", "dc", ArgType::Any).multi(2))
        .flag(f("defaultWidthHeight", "dwh", ArgType::Any).multi(2))
        .flag(f("height", "h", ArgType::Any))
        .flag(f("width", "w", ArgType::Any))
        .flag(f("leftEdge", "le", ArgType::Any))
        .flag(f("topEdge", "te", ArgType::Any))
        .flag(f("maximized", "max", ArgType::Bool))
        .flag(f("topLeftCorner", "tlc", ArgType::Any).multi(2))
        .flag(f("widthHeight", "wh", ArgType::Any).multi(2))
        .flag(f("heightProperty", "hp", ArgType::Any))
        .flag(f("widthProperty", "wp", ArgType::Any))
        .flag(f("maximumWidthHeight", "mwh", ArgType::Any).multi(2))
        .flag(f("minimumWidthHeight", "nwh", ArgType::Any).multi(2)),
    );

    s.register(
        Cmd::new("confirmDialog", "A modal question with buttons. Returns the button pressed, or -dismissString when closed.", |_s, a| {
            if let Some(v) = dialog_answer(a, "confirmDialog", &["title", "message", "messageAlign", "button", "defaultButton", "cancelButton", "dismissString", "icon", "backgroundColor", "annotation", "detailedMessage"]) {
                return Ok(v);
            }
            Ok(dialog_default(a))
        })
        .undoable(false)
        .flag(f("title", "t", ArgType::Any))
        .flag(f("message", "m", ArgType::Any))
        .flag(f("messageAlign", "ma", ArgType::Any))
        .flag(f("button", "b", ArgType::Any).multi(1))
        .flag(f("defaultButton", "db", ArgType::Any))
        .flag(f("cancelButton", "cb", ArgType::Any))
        .flag(f("dismissString", "ds", ArgType::Any))
        .flag(f("icon", "icn", ArgType::Any))
        .flag(f("backgroundColor", "bgc", ArgType::Any))
        .flag(f("annotation", "ann", ArgType::Any).multi(1))
        .flag(f("detailedMessage", "dm", ArgType::Any))
        .flag(f("parent", "p", ArgType::Any)),
    );

    s.register(
        Cmd::new("promptDialog", "A modal question with a text field. Returns the button pressed; -q -tx gives the text.", |_s, a| {
            if a.bool("query") {
                return Ok(json!(with_model(|m| m.prompt_text.clone())));
            }
            if let Some(v) = dialog_answer(a, "promptDialog", &["title", "message", "messageAlign", "text", "button", "defaultButton", "cancelButton", "dismissString", "style", "scrollableField", "backgroundColor"]) {
                let t = text(&v["text"]);
                with_model(|m| m.prompt_text = t);
                return Ok(v["button"].clone());
            }
            let t = a.string("text", "");
            with_model(|m| m.prompt_text = t);
            Ok(dialog_default(a))
        })
        .undoable(false)
        .flag(f("query", "q", ArgType::Bool))
        .flag(f("title", "t", ArgType::Any))
        .flag(f("message", "m", ArgType::Any))
        .flag(f("messageAlign", "ma", ArgType::Any))
        .flag(f("text", "tx", ArgType::Any))
        .flag(f("button", "b", ArgType::Any).multi(1))
        .flag(f("defaultButton", "db", ArgType::Any))
        .flag(f("cancelButton", "cb", ArgType::Any))
        .flag(f("dismissString", "ds", ArgType::Any))
        .flag(f("style", "st", ArgType::Any))
        .flag(f("scrollableField", "sf", ArgType::Bool))
        .flag(f("backgroundColor", "bgc", ArgType::Any))
        .flag(f("parent", "p", ArgType::Any)),
    );

    s.register(
        Cmd::new("fileDialog2", "The file browser. -fm 0 any file, 1 an existing file, 2 a folder, 3 a folder only, 4 several files. Returns the paths, or nothing when cancelled.", |_s, a| {
            let v = dialog_answer(a, "fileDialog2", &["caption", "fileMode", "fileFilter", "selectFileFilter", "startingDirectory", "okCaption", "cancelCaption", "dialogStyle"]);
            Ok(match v {
                Some(Value::Array(l)) if !l.is_empty() => Value::Array(l),
                _ => Value::Null,
            })
        })
        .undoable(false)
        .flag(f("caption", "cap", ArgType::Any))
        .flag(f("fileMode", "fm", ArgType::Any))
        .flag(f("fileFilter", "ff", ArgType::Any))
        .flag(f("selectFileFilter", "sff", ArgType::Any))
        .flag(f("startingDirectory", "dir", ArgType::Any))
        .flag(f("okCaption", "okc", ArgType::Any))
        .flag(f("cancelCaption", "cc", ArgType::Any))
        .flag(f("dialogStyle", "ds", ArgType::Any))
        .flag(f("hideNameEdit", "hne", ArgType::Bool))
        .flag(f("returnFilter", "rf", ArgType::Bool))
        .flag(f("setProjectBtnEnabled", "spe", ArgType::Bool))
        .flag(f("useManifest", "um", ArgType::Bool))
        .flag(f("fileTypeChanged", "ftc", ArgType::Any))
        .flag(f("optionsUICreate", "ocr", ArgType::Any))
        .flag(f("optionsUIInit", "oin", ArgType::Any))
        .flag(f("optionsUICommit", "ocm", ArgType::Any))
        .flag(f("optionsUICommit2", "oc2", ArgType::Any))
        .flag(f("optionsUICancel", "oca", ArgType::Any))
        .flag(f("optionsUITitle", "oti", ArgType::Any))
        .flag(f("buttonBoxOrientation", "bbo", ArgType::Any))
        .flag(f("selectionChanged", "sc", ArgType::Any)),
    );

    s.register(
        Cmd::new("fileDialog", "The older file browser: -m 0 read, 1 write, -dm the mask, -t the title, -dfn the default name. Returns a path, or an empty string.", |_s, a| {
            let v = dialog_answer(a, "fileDialog", &["title", "mode", "directoryMask", "defaultFileName"]);
            Ok(match v {
                Some(Value::Array(l)) if !l.is_empty() => l[0].clone(),
                _ => json!(""),
            })
        })
        .undoable(false)
        .flag(f("title", "t", ArgType::Any))
        .flag(f("mode", "m", ArgType::Any))
        .flag(f("directoryMask", "dm", ArgType::Any))
        .flag(f("defaultFileName", "dfn", ArgType::Any))
        .flag(f("application", "app", ArgType::Bool)),
    );

    s.register(
        Cmd::new("progressWindow", "A progress window: -t -pr -st -min -max -ii, -e to update, -q -ic asks whether it was cancelled, -ep ends it.", |_s, a| progress_window(a))
            .undoable(false)
            .flag(f("query", "q", ArgType::Bool))
            .flag(f("edit", "e", ArgType::Bool))
            .flag(f("queried", "", ArgType::Any))
            .flag(f("title", "t", ArgType::Any))
            .flag(f("progress", "pr", ArgType::Any))
            .flag(f("status", "st", ArgType::Any))
            .flag(f("minValue", "min", ArgType::Any))
            .flag(f("maxValue", "max", ArgType::Any))
            .flag(f("step", "s", ArgType::Any))
            .flag(f("isInterruptable", "ii", ArgType::Bool))
            .flag(f("isCancelled", "ic", ArgType::Bool))
            .flag(f("endProgress", "ep", ArgType::Bool)),
    );

    s.register(
        Cmd::new("headsUpMessage", "A short message over the view. -time how long it stays.", |_s, a| {
            let Some(m) = a.pos_str(0) else { fail!("headsUpMessage: give the message.") };
            let req = json!({"message": m, "time": a.float("time", 1.0), "position": "midCenter", "horizontalOffset": a.float("horizontalOffset", 0.0), "verticalOffset": a.float("verticalOffset", 0.0)});
            crate::core::push_ui_request(format!("inViewMessage|{req}"));
            Ok(Value::Null)
        })
        .positional("message")
        .undoable(false)
        .flag(f("object", "o", ArgType::Any))
        .flag(f("selection", "s", ArgType::Bool))
        .flag(f("time", "t", ArgType::Float))
        .flag(f("uvTextureEditor", "uve", ArgType::Bool))
        .flag(f("verticalOffset", "vo", ArgType::Int))
        .flag(f("horizontalOffset", "ho", ArgType::Int))
        .flag(f("viewport", "vp", ArgType::Bool)),
    );

    let mut iv = Cmd::new(
        "inViewMessage",
        "A message over the active view: -amg, -msg or -smg the text (HTML allowed), -pos where, -fade to fade after -fit, -fst and -fot \
         milliseconds; -ck and -dk delete it on a click or a drag. -cl position clears the message there; -hd, -sh, -min and -res hide, \
         show, minimize and restore every message.",
        |_s, a| in_view_message(a),
    )
    .undoable(false);
    for (n, sh, ty) in [
        ("message", "msg", ArgType::String),
        ("assistMessage", "amg", ArgType::String),
        ("statusMessage", "smg", ArgType::String),
        ("position", "pos", ArgType::String),
        ("fadeInTime", "fit", ArgType::Int),
        ("fadeOutTime", "fot", ArgType::Int),
        ("fadeStayTime", "fst", ArgType::Int),
        ("alpha", "a", ArgType::Float),
        ("backColor", "bkc", ArgType::Int),
        ("textAlpha", "ta", ArgType::Float),
        ("textOffset", "tof", ArgType::Int),
        ("frameOffset", "fof", ArgType::Int),
        ("font", "ft", ArgType::String),
        ("fontSize", "fts", ArgType::Int),
        ("clear", "cl", ArgType::String),
    ] {
        iv = iv.flag(f(n, sh, ty));
    }
    for (n, sh) in [("fade", "f"), ("clickKill", "ck"), ("dragKill", "dk"), ("hide", "hd"), ("show", "sh"), ("minimize", "min"), ("restore", "res"), ("uvEditor", "uv")] {
        iv = iv.flag(f(n, sh, ArgType::Bool));
    }
    s.register(iv);
}

// ---------------------------------------------------------------------------------------------------------------------
// switchTable

/// The rows a switch table shows for a shading switch: each input's shape and what feeds its value.
fn switch_rows(s: &mut Session, node: &str) -> Vec<Vec<String>> {
    let g = &s.graph;
    let Ok(Some(id)) = g.find(node) else { return Vec::new() };
    let mut rows: std::collections::BTreeMap<i32, [String; 2]> = std::collections::BTreeMap::new();
    for (d, src) in g.connections() {
        if d.node != id {
            continue;
        }
        let name = g.attr(&d).name.clone();
        let Some(child) = name.strip_prefix("input.") else { continue };
        let e = rows.entry(d.element.max(0)).or_default();
        let from = if child == "inShape" { g.path(src.node) } else { format!("{}.{}", g.path(src.node), g.attr(&src).name) };
        if child == "inShape" {
            e[0] = from;
        } else if e[1].is_empty() {
            e[1] = from;
        }
    }
    // Inputs with only a value set show it.
    rows.into_values().map(|[a, b]| vec![a, b]).collect()
}

/// switchTable: a script UI control showing a shading switch's shapes and their inputs, two columns.
fn switch_table(s: &mut Session, a: &Args) -> CmdResult {
    if let Some(n) = a.get("switchNode").map(text).filter(|n| !n.is_empty()) {
        if !a.bool("query") {
            let ok = s.graph.find(&n).ok().flatten().is_some_and(|id| s.graph.node(id).is_some_and(|x| x.ty().name.ends_with("ShadingSwitch")));
            if !ok {
                fail!("switchTable: {n} is not a shading switch node");
            }
        }
    }
    let r = control_command("switchTable", a)?;
    if a.bool("query") {
        return Ok(r);
    }
    // The rows, worked out from the scene whenever the control is made or edited.
    let path = if a.bool("edit") { a.pos_str(0).and_then(control_full_path).unwrap_or_default() } else { r.as_str().unwrap_or("").to_string() };
    if let Some(node) = control_prop(&path, "switchNode").map(|v| text(&v)).filter(|n| !n.is_empty()) {
        let rows = switch_rows(s, &node);
        let _ = model_edit(&path, &Map::from_iter([("cells".to_string(), json!(rows))]));
    }
    Ok(r)
}
