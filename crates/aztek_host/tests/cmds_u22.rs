//! colorInputWidgetGrp, colorSliderButtonGrp, colorSliderGrp, commandLine and commandPort as the reference documents them.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use aztek_host::Core;
use serde_json::{json, Value};

fn ok(c: &mut Core, src: &str) -> Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn close3(v: &Value, want: [f64; 3]) {
    let a: Vec<f64> = v.as_array().unwrap_or_else(|| panic!("{v}")).iter().map(|x| x.as_f64().unwrap()).collect();
    assert_eq!(a.len(), 3, "{v}");
    for i in 0..3 {
        assert!((a[i] - want[i]).abs() < 1e-6, "{v} vs {want:?}");
    }
}

#[test]
fn color_slider_grp_keeps_rgb_and_hsv_together() {
    let mut c = Core::new();
    ok(&mut c, "window w; columnLayout col;");
    ok(&mut c, "colorSliderGrp -label \"Blue\" -rgb 0 0 1 blue;");
    ok(&mut c, "colorSliderGrp -label \"Green\" -hsv 120 1 1 green;");
    close3(&ok(&mut c, "colorSliderGrp -q -hsv blue;"), [240.0, 1.0, 1.0]);
    close3(&ok(&mut c, "colorSliderGrp -q -rgb green;"), [0.0, 1.0, 0.0]);
    ok(&mut c, "colorSliderGrp -e -rgb 0.5 0.25 0.25 blue;");
    close3(&ok(&mut c, "colorSliderGrp -q -hsv blue;"), [0.0, 0.5, 0.5]);
    // -alphaValue counts only beside -rgb or -hsv.
    assert_eq!(ok(&mut c, "colorSliderGrp -q -alp blue;"), json!(1.0));
    ok(&mut c, "colorSliderGrp -e -alp 0.3 blue;");
    assert_eq!(ok(&mut c, "colorSliderGrp -q -alp blue;"), json!(1.0));
    ok(&mut c, "colorSliderGrp -e -rgb 1 1 1 -alp 0.3 blue;");
    assert_eq!(ok(&mut c, "colorSliderGrp -q -alphaValue blue;"), json!(0.3));
    assert_eq!(ok(&mut c, "colorSliderGrp -q -uds blue;"), json!(false));
    ok(&mut c, "colorSliderGrp -e -useDisplaySpace true -useVpColorPicker true blue;");
    assert_eq!(ok(&mut c, "colorSliderGrp -q -useDisplaySpace blue;"), json!(true));
    // A colour picked in the window comes back with both forms.
    c.ui_event(&json!({"path": "w|col|green", "set": {"rgbValue": [1.0, 0.0, 0.0]}, "fire": ["changeCommand"]}));
    close3(&ok(&mut c, "colorSliderGrp -q -hsv green;"), [0.0, 1.0, 1.0]);
}

#[test]
fn color_slider_button_grp_has_its_buttons() {
    let mut c = Core::new();
    ok(&mut c, "window w; columnLayout col;");
    let r = ok(
        &mut c,
        "colorSliderButtonGrp -label \"Label\" -buttonLabel \"Button\" -rgb 1 0 0 -symbolButtonDisplay true -columnWidth 5 30 -image \"navButtonUnconnected.png\" csb;",
    );
    assert_eq!(r, json!("w|col|csb"));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -l csb;"), json!("Label"));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -bl csb;"), json!("Button"));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -sbd csb;"), json!(true));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -i csb;"), json!("navButtonUnconnected.png"));
    close3(&ok(&mut c, "colorSliderButtonGrp -q -hsv csb;"), [0.0, 1.0, 1.0]);
    ok(&mut c, "colorSliderButtonGrp -e -hsv 240 0.5 1 -alp 0.25 -sbd false csb;");
    close3(&ok(&mut c, "colorSliderButtonGrp -q -rgb csb;"), [0.5, 0.5, 1.0]);
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -alp csb;"), json!(0.25));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -sbd csb;"), json!(false));
    ok(&mut c, "colorSliderButtonGrp -e -uds true -vcp false -fdr csb;");
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -uds csb;"), json!(true));
    // The two buttons and the slider run their scripts.
    ok(&mut c, "global int $u22b = 0; global int $u22s = 0; global int $u22c = 0; global int $u22d = 0;");
    ok(&mut c, "colorSliderButtonGrp -e -bc \"$u22b = 1\" -sbc \"$u22s = 1\" -cc \"$u22c = 1\" -dc \"$u22d = 1\" csb;");
    for f in ["buttonCommand", "symbolButtonCommand", "changeCommand", "dragCommand"] {
        c.ui_event(&json!({"path": "w|col|csb", "fire": [f]}));
    }
    assert_eq!(ok(&mut c, "global int $u22b; global int $u22s; global int $u22c; global int $u22d; $u22b + $u22s + $u22c + $u22d;"), json!(4));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -ex csb;"), json!(true));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -en csb;"), json!(true));
    ok(&mut c, "colorSliderButtonGrp -e -en false -ann \"tip\" csb;");
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -en csb;"), json!(false));
    assert_eq!(ok(&mut c, "colorSliderButtonGrp -q -ann csb;"), json!("tip"));
}

#[test]
fn color_input_widget_grp_sets_and_reports() {
    let mut c = Core::new();
    ok(&mut c, "window -title \"Colors\" w; columnLayout col;");
    let r = ok(&mut c, "colorInputWidgetGrp -label \"Color\" -rgb 1 0 0 ciw;");
    assert_eq!(r, json!("w|col|ciw"));
    assert_eq!(ok(&mut c, "colorInputWidgetGrp -q -label ciw;"), json!("Color"));
    close3(&ok(&mut c, "colorInputWidgetGrp -q -rgb ciw;"), [1.0, 0.0, 0.0]);
    close3(&ok(&mut c, "colorInputWidgetGrp -q -hsv ciw;"), [0.0, 1.0, 1.0]);
    ok(&mut c, "colorInputWidgetGrp -e -hsv 60 1 0.5 -alphaValue 0.75 ciw;");
    close3(&ok(&mut c, "colorInputWidgetGrp -q -rgb ciw;"), [0.5, 0.5, 0.0]);
    assert_eq!(ok(&mut c, "colorInputWidgetGrp -q -alp ciw;"), json!(0.75));
    ok(&mut c, "global string $u22col = \"\"; colorInputWidgetGrp -e -cc \"$u22col = `colorInputWidgetGrp -q -l ciw`\" -cw2 60 200 -ad2 2 ciw;");
    c.ui_event(&json!({"path": "w|col|ciw", "set": {"hsvValue": [0.0, 0.0, 1.0]}, "fire": ["changeCommand"]}));
    close3(&ok(&mut c, "colorInputWidgetGrp -q -rgb ciw;"), [1.0, 1.0, 1.0]);
    assert_eq!(ok(&mut c, "global string $u22col; $u22col;"), json!("Color"));
    assert!(c.mel_checked("colorInputWidgetGrp -e -rgb 1 ciw;").is_err());
}

fn output_of(ops: &[Value], path: &str) -> Option<String> {
    ops.iter().rev().find_map(|o| (o["path"] == path).then(|| o["props"]["__output"].as_str().map(str::to_string)).flatten())
}

#[test]
fn command_line_runs_lines_and_keeps_history() {
    let mut c = Core::new();
    ok(&mut c, "window w; formLayout form;");
    let r = ok(&mut c, "commandLine cl;");
    assert_eq!(r, json!("w|form|cl"));
    ok(&mut c, "commandLine -e -h 25 cl;");
    assert_eq!(ok(&mut c, "commandLine -q -h cl;"), json!(25));
    assert_eq!(ok(&mut c, "commandLine -q -nhl cl;"), json!(50));
    assert_eq!(ok(&mut c, "commandLine -q -st cl;"), json!("mel"));
    assert_eq!(ok(&mut c, "commandLine -q -hf cl;"), json!(false));
    ok(&mut c, "commandLine -e -hf true -ian \"in\" -oan \"out\" -nhl 2 cl;");
    assert_eq!(ok(&mut c, "commandLine -q -holdFocus cl;"), json!(true));
    assert_eq!(ok(&mut c, "commandLine -q -inputAnnotation cl;"), json!("in"));
    assert_eq!(ok(&mut c, "commandLine -q -outputAnnotation cl;"), json!("out"));
    assert_eq!(ok(&mut c, "commandLine -q -numberOfHistoryLines cl;"), json!(2));
    assert!(c.mel_checked("commandLine -e -st \"perl\" cl;").is_err());

    // A line entered runs; what it gave shows on the right.
    let _ = aztek_host::scriptui::take_ops();
    c.ui_event(&json!({"path": "w|form|cl", "fire": ["__cmdLineRun"], "args": ["createNode transform -n lineNode"]}));
    assert_eq!(ok(&mut c, "objExists lineNode;"), json!(true));
    let ops = aztek_host::scriptui::take_ops();
    assert_eq!(output_of(&ops, "w|form|cl").as_deref(), Some("// Result: lineNode //"));
    c.ui_event(&json!({"path": "w|form|cl", "fire": ["__cmdLineRun"], "args": ["noSuchCommandAtAll"]}));
    let ops = aztek_host::scriptui::take_ops();
    assert!(output_of(&ops, "w|form|cl").unwrap().contains("Error"));
    c.ui_event(&json!({"path": "w|form|cl", "fire": ["__cmdLineRun"], "args": ["createNode transform -n third"]}));
    let ops = aztek_host::scriptui::take_ops();
    // Two lines of history kept: the oldest went.
    let hist = ops.iter().rev().find_map(|o| o["props"].get("__history").cloned()).unwrap();
    assert_eq!(hist, json!(["noSuchCommandAtAll", "createNode transform -n third"]));

    // -command runs as the text changes, -enterCommand on the keypad's Enter; both stay MEL under -st python.
    ok(&mut c, "global int $u22typed = 0; global int $u22enter = 0;");
    ok(&mut c, "commandLine -e -st \"python\" -c \"$u22typed += 1\" -ec \"$u22enter = 1\" cl;");
    c.ui_event(&json!({"path": "w|form|cl", "set": {"__input": "p"}, "fire": ["command"]}));
    c.ui_event(&json!({"path": "w|form|cl", "fire": ["enterCommand"]}));
    assert_eq!(ok(&mut c, "global int $u22typed; $u22typed;"), json!(1));
    assert_eq!(ok(&mut c, "global int $u22enter; $u22enter;"), json!(1));
    // The line itself is Python now.
    c.ui_event(&json!({"path": "w|form|cl", "fire": ["__cmdLineRun"], "args": ["1 + 2"]}));
    let ops = aztek_host::scriptui::take_ops();
    assert_eq!(output_of(&ops, "w|form|cl").as_deref(), Some("# Result: 3 #"));
}

/// Writes to a port and reads the answer, while the core takes its idle steps.
fn talk(c: &mut Core, addr: &str, writes: Vec<String>, wait: bool) -> Vec<String> {
    let addr = addr.to_string();
    let h = std::thread::spawn(move || {
        let mut s = std::net::TcpStream::connect(&addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut out = Vec::new();
        for w in writes {
            s.write_all(w.as_bytes()).unwrap();
            if !wait {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            let mut buf = vec![0u8; 65536];
            let n = s.read(&mut buf).unwrap_or(0);
            out.push(String::from_utf8_lossy(&buf[..n]).to_string());
        }
        out
    });
    let start = Instant::now();
    while !h.is_finished() && start.elapsed() < Duration::from_secs(15) {
        c.idle();
        std::thread::sleep(Duration::from_millis(5));
    }
    c.idle();
    h.join().unwrap()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn command_port_runs_what_clients_write() {
    let mut c = Core::new();
    let p = free_port();
    let name = format!(":{p}");
    assert_eq!(ok(&mut c, &format!("commandPort -q \"{name}\";")), json!(false));
    ok(&mut c, &format!("commandPort -n \"{name}\";"));
    assert_eq!(ok(&mut c, &format!("commandPort -q \"{name}\";")), json!(true));
    assert!(ok(&mut c, "commandPort -lp;").as_array().unwrap().contains(&json!(name)));
    assert!(c.mel_checked(&format!("commandPort -n \"{name}\";")).is_err(), "a port opens once");
    let addr = format!("127.0.0.1:{p}");
    let got = talk(&mut c, &addr, vec!["createNode transform -n portNode;".into(), "getAttr portNode.translate;".into()], true);
    assert_eq!(got[0], "portNode\n\0");
    assert_eq!(got[1], "0\t0\t0\n\0");
    assert_eq!(ok(&mut c, "objExists portNode;"), json!(true));
    let got = talk(&mut c, &addr, vec!["noSuchCommandAtAll;".into()], true);
    assert!(got[0].contains("Error"), "{got:?}");
    ok(&mut c, &format!("commandPort -cl -n \"{name}\";"));
    assert_eq!(ok(&mut c, &format!("commandPort -q \"{name}\";")), json!(false));
    assert!(std::net::TcpStream::connect(&addr).is_err());
}

#[test]
fn command_port_settings() {
    let mut c = Core::new();
    let mut opened: Vec<String> = Vec::new();
    // -returnNumCommands: how many commands ran.
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -rnc -n \":{p}\";"));
    let got = talk(&mut c, &format!("127.0.0.1:{p}"), vec!["createNode transform -n a1; createNode transform -n a2;".into()], true);
    assert_eq!(got[0], "2\n\0");
    // -prefix: each line given to the command as its one string, the last answer back.
    ok(&mut c, "global proc string u22Echo(string $s) { return (\"got \" + $s); }");
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -pre \"u22Echo\" -n \":{p}\";"));
    let got = talk(&mut c, &format!("127.0.0.1:{p}"), vec!["one\ntwo".into()], true);
    assert_eq!(got[0], "got two\n\0");
    // -noreturn: nothing comes back, the command still runs.
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -nr -n \":{p}\";"));
    talk(&mut c, &format!("127.0.0.1:{p}"), vec!["createNode transform -n quiet;".into()], false);
    for _ in 0..50 {
        c.idle();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(ok(&mut c, "objExists quiet;"), json!(true));
    // -echoOutput: what the commands print comes back too.
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -eo -n \":{p}\";"));
    let got = talk(&mut c, &format!("127.0.0.1:{p}"), vec!["warning \"careful\"; createNode transform -n echoed;".into()], true);
    assert!(got[0].contains("careful") && got[0].contains("echoed"), "{got:?}");
    // -bufferSize: a longer command closes the connection.
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -bs 16 -n \":{p}\";"));
    let got = talk(&mut c, &format!("127.0.0.1:{p}"), vec!["createNode transform -n muchTooLongForTheBuffer;".into()], true);
    assert_eq!(got[0], "");
    assert_eq!(ok(&mut c, "objExists muchTooLongForTheBuffer;"), json!(false));
    // -sourceType python, and -pickleOutput.
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -stp \"python\" -n \":{p}\";"));
    let got = talk(&mut c, &format!("127.0.0.1:{p}"), vec!["6 * 7".into()], true);
    assert_eq!(got[0], "42\n\0");
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -stp \"python\" -po -n \":{p}\";"));
    let got = talk(&mut c, &format!("127.0.0.1:{p}"), vec!["[1, 2]".into()], true);
    assert!(got[0].starts_with("(lp") && got[0].contains("I1\n") && got[0].contains("I2\n"), "{got:?}");
    // -securityWarning: a warning in the history when a client first writes.
    let p = free_port();
    opened.push(format!(":{p}"));
    ok(&mut c, &format!("commandPort -sw -n \":{p}\";"));
    let _ = c.take_log();
    talk(&mut c, &format!("127.0.0.1:{p}"), vec!["createNode transform;".into()], true);
    let log = c.take_log();
    assert!(log.as_array().unwrap().iter().any(|l| l["kind"] == 4 && l["text"].as_str().unwrap().contains("commandPort")), "{log}");
    assert!(c.mel_checked("commandPort -stp \"perl\" -n \":1\";").is_err());
    // Close what this test opened (other tests' ports may be open beside them).
    let mine: Vec<String> = ok(&mut c, "commandPort -lp;").as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).filter(|n| opened.contains(n)).collect();
    assert_eq!(mine.len(), opened.len());
    for n in &mine {
        ok(&mut c, &format!("commandPort -cl -n \"{n}\";"));
    }
    let left = ok(&mut c, "commandPort -lp;");
    assert!(opened.iter().all(|n| !left.as_array().unwrap().contains(&json!(n))));
}

#[cfg(unix)]
#[test]
fn command_port_on_a_unix_socket() {
    let mut c = Core::new();
    let path = std::env::temp_dir().join(format!("aztek_u22_{}", std::process::id()));
    let name = path.to_string_lossy().to_string();
    ok(&mut c, &format!("commandPort -n \"{name}\";"));
    assert!(path.exists());
    let p2 = path.clone();
    let h = std::thread::spawn(move || {
        let mut s = std::os::unix::net::UnixStream::connect(&p2).unwrap();
        s.write_all(b"createNode transform -n sockNode;").unwrap();
        let mut buf = vec![0u8; 1024];
        let n = s.read(&mut buf).unwrap();
        String::from_utf8_lossy(&buf[..n]).to_string()
    });
    let start = Instant::now();
    while !h.is_finished() && start.elapsed() < Duration::from_secs(10) {
        c.idle();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(h.join().unwrap(), "sockNode\n\0");
    ok(&mut c, &format!("commandPort -cl -n \"{name}\";"));
    assert!(!path.exists());
}

#[test]
fn every_flag_of_the_reference_is_there() {
    let c = Core::new();
    for (cmd, flags) in [
        ("colorInputWidgetGrp", "ad2 ad3 ad4 ad5 ad6 adj adjustableColumn adjustableColumn2 adjustableColumn3 adjustableColumn4 adjustableColumn5 adjustableColumn6 alp alphaValue ann annotation backgroundColor bgc cal cat cc changeCommand cl2 cl3 cl4 cl5 cl6 co2 co3 co4 co5 co6 columnAlign columnAlign2 columnAlign3 columnAlign4 columnAlign5 columnAlign6 columnAttach columnAttach2 columnAttach3 columnAttach4 columnAttach5 columnAttach6 columnOffset2 columnOffset3 columnOffset4 columnOffset5 columnOffset6 columnWidth columnWidth1 columnWidth2 columnWidth3 columnWidth4 columnWidth5 columnWidth6 ct2 ct3 ct4 ct5 ct6 cw cw1 cw2 cw3 cw4 cw5 cw6 defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fdr forceDragRefresh fpn fullPathName h height highlightColor hlc hsv hsvValue io isObscured l label m manage nbg noBackground npm numberOfPopupMenus p parent pma po popupMenuArray preventOverride q query rat rgb rgbValue rowAttach sbm statusBarMessage useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("colorSliderButtonGrp", "ad2 ad3 ad4 ad5 ad6 adj adjustableColumn adjustableColumn2 adjustableColumn3 adjustableColumn4 adjustableColumn5 adjustableColumn6 alp alphaValue ann annotation backgroundColor bc bgc bl buttonCommand buttonLabel cal cat cc changeCommand cl2 cl3 cl4 cl5 cl6 co2 co3 co4 co5 co6 columnAlign columnAlign2 columnAlign3 columnAlign4 columnAlign5 columnAlign6 columnAttach columnAttach2 columnAttach3 columnAttach4 columnAttach5 columnAttach6 columnOffset2 columnOffset3 columnOffset4 columnOffset5 columnOffset6 columnWidth columnWidth1 columnWidth2 columnWidth3 columnWidth4 columnWidth5 columnWidth6 ct2 ct3 ct4 ct5 ct6 cw cw1 cw2 cw3 cw4 cw5 cw6 dc defineTemplate dgc docTag dpc dragCallback dragCommand dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fdr forceDragRefresh fpn fullPathName h height highlightColor hlc hsv hsvValue i image io isObscured l label m manage nbg noBackground npm numberOfPopupMenus p parent pma po popupMenuArray preventOverride q query rat rgb rgbValue rowAttach sbc sbd sbm statusBarMessage symbolButtonCommand symbolButtonDisplay uds useDisplaySpace useTemplate useVpColorPicker ut vcc vcp vis visible visibleChangeCommand w width"),
        ("colorSliderGrp", "alp alphaValue uds useDisplaySpace useVpColorPicker vcp"),
        ("commandLine", "ann annotation backgroundColor bgc c command defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e ebg ec edit ekf en enable enableBackground enableKeyboardFocus enterCommand ex exists fpn fullPathName h height hf highlightColor hlc holdFocus ian inputAnnotation io isObscured m manage nbg nhl noBackground npm numberOfHistoryLines numberOfPopupMenus oan outputAnnotation p parent pma po popupMenuArray preventOverride q query sbm sourceType st statusBarMessage useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("commandPort", "bs bufferSize cl close echoOutput eo listPorts lp n name noreturn nr pickleOutput po pre prefix returnNumCommands rnc securityWarning sourceType stp sw"),
    ] {
        let def = c.s.commands.find(cmd).unwrap_or_else(|| panic!("{cmd} missing"));
        for f in flags.split_whitespace() {
            assert!(def.flags.iter().any(|d| d.name == f || d.short_name == f), "{cmd} -{f}");
        }
    }
}
