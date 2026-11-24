//! cmdScrollFieldExecuter, cmdScrollFieldReporter, cmdShell and colorEditor, with no interface: the model answers, the
//! host runs what the fields run.

use aztek_host::Core;
use serde_json::{json, Value};

fn ok(c: &mut Core, src: &str) -> Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn window(c: &mut Core) {
    ok(c, "window w; columnLayout col;");
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("aztek_u21_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn executer_creates_queries_and_edits_its_text() {
    let mut c = Core::new();
    window(&mut c);
    let path = ok(&mut c, "cmdScrollFieldExecuter -w 200 -h 100 -sourceType \"mel\" ex;");
    assert_eq!(path, json!("w|col|ex"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -exists ex;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -w ex;"), json!(200));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -st ex;"), json!("mel"));
    // Defaults.
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -acb ex;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -spt ex;"), json!(4));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -tfi ex;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -sts ex;"), json!(false));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -mod ex;"), json!(false));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -fn ex;"), json!(""));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -hf ex;"), json!(false));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -nl ex;"), json!(1));

    ok(&mut c, "cmdScrollFieldExecuter -e -t \"one\\ntwo\\nthree\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("one\ntwo\nthree"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -tl ex;"), json!(13));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -nl ex;"), json!(3));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -mod ex;"), json!(true));
    // The caret goes to the start of a line.
    ok(&mut c, "cmdScrollFieldExecuter -e -cl 2 ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -cl ex;"), json!(2));
    ok(&mut c, "cmdScrollFieldExecuter -e -it \">\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("one\n>two\nthree"));
    // A selection, replaced by inserted text.
    ok(&mut c, "cmdScrollFieldExecuter -e -sl 0 3 ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -hsl ex;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -slt ex;"), json!("one"));
    ok(&mut c, "cmdScrollFieldExecuter -e -it \"1\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("1\n>two\nthree"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -hsl ex;"), json!(false));
    ok(&mut c, "cmdScrollFieldExecuter -e -at \"\\nfour\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -nl ex;"), json!(4));
    // Undo and redo step through the text's changes.
    ok(&mut c, "cmdScrollFieldExecuter -e -ud ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("1\n>two\nthree"));
    ok(&mut c, "cmdScrollFieldExecuter -e -ud ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("one\n>two\nthree"));
    ok(&mut c, "cmdScrollFieldExecuter -e -rd ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("1\n>two\nthree"));
    // Clipboard: copy, cut, paste.
    ok(&mut c, "cmdScrollFieldExecuter -e -sla ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -slt ex;"), json!("1\n>two\nthree"));
    ok(&mut c, "cmdScrollFieldExecuter -e -sl 3 6 -cp ex;");
    ok(&mut c, "cmdScrollFieldExecuter -e -sl 0 1 -ct ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("\n>two\nthree"));
    ok(&mut c, "cmdScrollFieldExecuter -e -sl 0 0 -pst ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("1\n>two\nthree"));
    // Replace all, case as -searchMatchCase says.
    ok(&mut c, "cmdScrollFieldExecuter -e -t \"Ab ab AB\" ex;");
    ok(&mut c, "cmdScrollFieldExecuter -e -smc 1 -rpa \"ab\" \"x\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("Ab x AB"));
    ok(&mut c, "cmdScrollFieldExecuter -e -smc 0 -rpa \"ab\" \"y\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("y x y"));
    ok(&mut c, "cmdScrollFieldExecuter -e -clr ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!(""));
    // Settings read back.
    ok(&mut c, "cmdScrollFieldExecuter -e -sln 0 -sts 1 -spt 2 -tfi 0 -acb 0 -cco 0 -opc 0 -sth 0 ex;");
    for (f, v) in [("sln", json!(false)), ("sts", json!(true)), ("spt", json!(2)), ("tfi", json!(false)), ("acb", json!(false)), ("cco", json!(false)), ("opc", json!(false)), ("sth", json!(false))] {
        assert_eq!(ok(&mut c, &format!("cmdScrollFieldExecuter -q -{f} ex;")), v, "{f}");
    }
    ok(&mut c, "cmdScrollFieldExecuter -e -mod 0 ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -mod ex;"), json!(false));
}

#[test]
fn executer_searches_and_selects() {
    let mut c = Core::new();
    window(&mut c);
    ok(&mut c, "cmdScrollFieldExecuter -t \"cat dog Cat dog\" ex;");
    ok(&mut c, "cmdScrollFieldExecuter -e -sl 0 0 -ss \"dog\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -ss ex;"), json!("dog"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -sas ex;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -slt ex;"), json!("dog"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -sas ex;"), json!(true));
    // The second one: selected from 12.
    ok(&mut c, "cmdScrollFieldExecuter -e -ss \"cat\" ex;");
    // Case doesn't matter unless asked: Cat after the caret, then it wraps.
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -sas ex;"), json!(true));
    ok(&mut c, "cmdScrollFieldExecuter -e -smc 1 -sw 0 ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -sas ex;"), json!(false));
    ok(&mut c, "cmdScrollFieldExecuter -e -sw 1 ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -sas ex;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -slt ex;"), json!("cat"));
    // Upward.
    ok(&mut c, "cmdScrollFieldExecuter -e -sl 15 15 -sd 0 -ss \"dog\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -sas ex;"), json!(true));
    let sel = ok(&mut c, "cmdScrollFieldExecuter -q -slt ex;");
    assert_eq!(sel, json!("dog"));
    ok(&mut c, "cmdScrollFieldExecuter -e -it \"DOG\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("cat dog Cat DOG"));
}

#[test]
fn executer_runs_its_text() {
    let mut c = Core::new();
    window(&mut c);
    ok(&mut c, "cmdScrollFieldExecuter -t \"polyCube -n one;\\npolyCube -n two;\" ex;");
    c.take_log();
    ok(&mut c, "cmdScrollFieldExecuter -e -exa ex;");
    assert_eq!(ok(&mut c, "objExists one;"), json!(true));
    assert_eq!(ok(&mut c, "objExists two;"), json!(true));
    let log = c.take_log();
    assert!(log.as_array().unwrap().iter().any(|l| l["text"].as_str().is_some_and(|t| t.starts_with("polyCube"))), "{log}");
    // With a selection, -execute runs only that.
    ok(&mut c, "delete one two;");
    ok(&mut c, "cmdScrollFieldExecuter -e -sl 0 16 ex;");
    ok(&mut c, "cmdScrollFieldExecuter -e -exc ex;");
    assert_eq!(ok(&mut c, "objExists one;"), json!(true));
    assert_eq!(ok(&mut c, "objExists two;"), json!(false));
    // Without one, all of it; the text stays.
    ok(&mut c, "delete one; cmdScrollFieldExecuter -e -sl 0 0 -exc ex;");
    assert_eq!(ok(&mut c, "objExists two;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -nl ex;"), json!(2));
    // Edits given with -execute come first.
    ok(&mut c, "cmdScrollFieldExecuter -e -t \"polyCube -n three;\" -exc ex;");
    assert_eq!(ok(&mut c, "objExists three;"), json!(true));
    // Ctrl+Enter in the shell is -execute.
    ok(&mut c, "cmdScrollFieldExecuter -e -t \"polyCube -n four;\" ex;");
    c.ui_event(&json!({"path": "w|col|ex", "fire": ["__exec"]}));
    assert_eq!(ok(&mut c, "objExists four;"), json!(true));
}

#[test]
fn python_executer_runs_python() {
    let mut c = Core::new();
    window(&mut c);
    ok(&mut c, "cmdScrollFieldExecuter -st \"python\" -t \"import maya.cmds as cmds\\ncmds.polyCube(n='pyCube')\" pex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -st pex;"), json!("python"));
    ok(&mut c, "cmdScrollFieldExecuter -e -exa pex;");
    assert_eq!(ok(&mut c, "objExists pyCube;"), json!(true));
}

#[test]
fn executer_files_and_stored_contents() {
    let mut c = Core::new();
    window(&mut c);
    let d = scratch("files");
    let src = d.join("in.mel");
    std::fs::write(&src, "print \"loaded\";\n").unwrap();
    let srcs = src.to_string_lossy().replace('\\', "/");
    ok(&mut c, "global int $changed = 0; global proc onMod() { global int $changed; $changed++; }");
    ok(&mut c, "cmdScrollFieldExecuter -mcc \"onMod\" ex;");
    ok(&mut c, &format!("cmdScrollFieldExecuter -e -ldf \"{srcs}\" ex;"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("print \"loaded\";\n"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -fn ex;"), json!(srcs));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -mod ex;"), json!(false));
    // A change marks it modified, once.
    ok(&mut c, "cmdScrollFieldExecuter -e -at \"// more\" ex;");
    ok(&mut c, "cmdScrollFieldExecuter -e -at \"!\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -mod ex;"), json!(true));
    assert_eq!(ok(&mut c, "global int $changed; $changed;"), json!(1));
    // Saved: no longer modified, and the file holds the text.
    let out = d.join("out.mel");
    let outs = out.to_string_lossy().replace('\\', "/");
    ok(&mut c, &format!("cmdScrollFieldExecuter -e -svf \"{outs}\" ex;"));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "print \"loaded\";\n// more!");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -fn ex;"), json!(outs));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -mod ex;"), json!(false));
    assert_eq!(ok(&mut c, "global int $changed; $changed;"), json!(2));
    // Stored contents by full path: returns where it went; loaded back into another field; removed.
    let st = d.join("stored.txt");
    let sts = st.to_string_lossy().replace('\\', "/");
    assert_eq!(ok(&mut c, &format!("cmdScrollFieldExecuter -e -stc \"{sts}\" ex;")), json!(sts));
    ok(&mut c, "cmdScrollFieldExecuter other;");
    ok(&mut c, &format!("cmdScrollFieldExecuter -e -ldc \"{sts}\" other;"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t other;"), json!("print \"loaded\";\n// more!"));
    ok(&mut c, &format!("cmdScrollFieldExecuter -e -rsc \"{sts}\" other;"));
    assert!(!st.exists());
    assert!(c.mel_checked(&format!("cmdScrollFieldExecuter -e -ldc \"{sts}\" other;")).is_err());
    // The file changed on disk: -fileChangedCommand hears of it at idle, given the file.
    ok(&mut c, "global string $seen = \"\"; global proc onFile(string $f) { global string $seen; $seen = $f; }");
    ok(&mut c, "cmdScrollFieldExecuter -e -fcc \"onFile\" ex;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $seen; $seen;"), json!(""));
    std::fs::remove_file(&out).unwrap();
    c.idle();
    assert_eq!(ok(&mut c, "global string $seen; $seen;"), json!(outs));
    // With no shell to ask, the prompting flags do nothing.
    ok(&mut c, "cmdScrollFieldExecuter -e -ld -src -sv \"pre\" -svs ex;");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn executer_filter_key_press() {
    let mut c = Core::new();
    window(&mut c);
    ok(&mut c, "global proc int keys(int $m, string $k) { return ($m == 4 && $k == \"a\"); }");
    ok(&mut c, "cmdScrollFieldExecuter -fkp \"keys\" ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -fkp ex;"), json!("keys"));
    assert_eq!(ok(&mut c, "aztekCmdFieldKey \"w|col|ex\" 4 \"a\";"), json!(1));
    assert_eq!(ok(&mut c, "aztekCmdFieldKey \"w|col|ex\" 1 \"a\";"), json!(0));
}

#[test]
fn shell_events_mark_modified() {
    let mut c = Core::new();
    window(&mut c);
    ok(&mut c, "cmdScrollFieldExecuter ex;");
    c.ui_event(&json!({"path": "w|col|ex", "set": {"text": "typed", "__caret": 5, "__sel": [5, 5]}}));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!("typed"));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -mod ex;"), json!(true));
    ok(&mut c, "cmdScrollFieldExecuter -e -ud ex;");
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -t ex;"), json!(""));
    c.ui_event(&json!({"path": "w|col|ex", "set": {"hasFocus": true}}));
    assert_eq!(ok(&mut c, "cmdScrollFieldExecuter -q -hf ex;"), json!(true));
}

#[test]
fn reporter_shows_the_history() {
    let mut c = Core::new();
    window(&mut c);
    let path = ok(&mut c, "cmdScrollFieldReporter -w 200 -h 100 rep;");
    assert_eq!(path, json!("w|col|rep"));
    assert_eq!(ok(&mut c, "cmdScrollFieldReporter -ex rep;"), json!(true));
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(t.as_str().unwrap().contains("cmdScrollFieldReporter -w 200"), "{t}");
    // Cleared: only what comes after (the clear's own result, and this query's echo).
    ok(&mut c, "cmdScrollFieldReporter -e -clr rep;");
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(!t.as_str().unwrap().contains("-w 200"), "{t}");
    c.mel("print \"hello\\n\";");
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(t.as_str().unwrap().contains("hello"), "{t}");
    // Filters: errors left out.
    ok(&mut c, "cmdScrollFieldReporter -e -se 1 rep;");
    assert_eq!(ok(&mut c, "cmdScrollFieldReporter -q -se rep;"), json!(true));
    let _ = c.mel_checked("error \"bad thing\";");
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(!t.as_str().unwrap().contains("// Error: bad thing"), "{t}");
    ok(&mut c, "cmdScrollFieldReporter -e -se 0 rep;");
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(t.as_str().unwrap().contains("// Error: bad thing"), "{t}");
    // Warnings and results.
    ok(&mut c, "cmdScrollFieldReporter -e -sw 1 -sr 1 -si 1 -sst 1 rep;");
    for f in ["sw", "sr", "si", "sst"] {
        assert_eq!(ok(&mut c, &format!("cmdScrollFieldReporter -q -{f} rep;")), json!(true), "{f}");
    }
    let _ = c.mel("warning \"careful\";");
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(!t.as_str().unwrap().contains("// Warning: careful"), "{t}");
    ok(&mut c, "cmdScrollFieldReporter -e -sw 0 -sr 0 -si 0 -sst 0 rep;");
    // MEL lines hidden by source type.
    ok(&mut c, "cmdScrollFieldReporter -e -clr -fst \"mel\" rep;");
    assert_eq!(ok(&mut c, "cmdScrollFieldReporter -q -fst rep;"), json!("mel"));
    c.mel("polyCube -n box;");
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(!t.as_str().unwrap().contains("// Result"), "{t}");
    ok(&mut c, "cmdScrollFieldReporter -e -fst \"\" rep;");
    let t = ok(&mut c, "cmdScrollFieldReporter -q -t rep;");
    assert!(t.as_str().unwrap().contains("// Result"), "{t}");
    // -text replaces what it shows; later lines still come after it. Run as from Python, so nothing echoes.
    let rep_text = |c: &mut Core| c.exec_silent("cmdScrollFieldReporter", json!({"args": ["rep"], "query": true, "queried": ["text"]})).unwrap();
    let rep_edit = |c: &mut Core, flags: Value| {
        let mut call = flags;
        call["args"] = json!(["rep"]);
        call["edit"] = json!(true);
        c.exec_silent("cmdScrollFieldReporter", call).unwrap();
    };
    rep_edit(&mut c, json!({"text": "start\n"}));
    assert_eq!(rep_text(&mut c), json!("start\n"));
    assert_eq!(c.exec_silent("cmdScrollFieldReporter", json!({"args": ["rep"], "query": true, "queried": ["textLength"]})).unwrap(), json!(6));
    c.mel("print \"more\\n\";");
    let t = rep_text(&mut c);
    assert!(t.as_str().unwrap().starts_with("start\n") && t.as_str().unwrap().contains("more"), "{t}");
    // Select, cut, paste.
    rep_edit(&mut c, json!({"text": "abcdef"}));
    rep_edit(&mut c, json!({"select": [0, 3], "cutSelection": true}));
    assert_eq!(rep_text(&mut c), json!("def"));
    rep_edit(&mut c, json!({"selectAll": true, "pasteSelection": true}));
    assert_eq!(rep_text(&mut c), json!("abc"));
    ok(&mut c, "cmdScrollFieldReporter -e -sl 0 1 -cp -sv \"x\" -svs rep;");
    assert_eq!(ok(&mut c, "cmdScrollFieldReporter -q -hf rep;"), json!(false));
}

#[test]
fn reporter_global_settings() {
    let mut c = Core::new();
    window(&mut c);
    ok(&mut c, "cmdScrollFieldReporter rep;");
    ok(&mut c, "cmdScrollFieldReporter -e -eac 1 rep;");
    assert_eq!(ok(&mut c, "commandEcho -q -state;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldReporter -q -eac rep;"), json!(true));
    ok(&mut c, "cmdScrollFieldReporter -e -eac 0 -ln 0 rep;");
    assert_eq!(ok(&mut c, "commandEcho -q -state;"), json!(false));
    assert_eq!(ok(&mut c, "cmdScrollFieldReporter -q -ln rep;"), json!(false));
    assert_eq!(ok(&mut c, "commandEcho -q -lineNumbers;"), json!(false));
    ok(&mut c, "cmdScrollFieldReporter -e -st 1 rep;");
    assert_eq!(ok(&mut c, "stackTrace -q -state;"), json!(true));
    assert_eq!(ok(&mut c, "cmdScrollFieldReporter -q -st rep;"), json!(true));
    ok(&mut c, "cmdScrollFieldReporter -e -st 0 rep;");
    // The focus script is kept for the shell to call.
    ok(&mut c, "global int $focused = 0; cmdScrollFieldReporter -e -rfc \"$focused = 1\" rep;");
    c.ui_event(&json!({"path": "w|col|rep", "set": {"hasFocus": true}, "fire": ["receiveFocusCommand"]}));
    assert_eq!(ok(&mut c, "global int $focused; $focused;"), json!(1));
}

#[test]
fn shell_runs_lines() {
    let mut c = Core::new();
    window(&mut c);
    let path = ok(&mut c, "cmdShell -nsl 5 -nhl 2 sh;");
    assert_eq!(path, json!("w|col|sh"));
    assert_eq!(ok(&mut c, "cmdShell -ex sh;"), json!(true));
    assert_eq!(ok(&mut c, "cmdShell -q -nsl sh;"), json!(5));
    assert_eq!(ok(&mut c, "cmdShell -q -nhl sh;"), json!(2));
    assert_eq!(ok(&mut c, "cmdShell -q -pr sh;"), json!("% "));
    ok(&mut c, "cmdShell -e -pr \"mel> \" sh;");
    assert_eq!(ok(&mut c, "cmdShell -q -pr sh;"), json!("mel> "));
    ok(&mut c, "global int $runs = 0; cmdShell -e -c \"$runs++\" sh;");
    // A line typed and entered: run, what it gives shown, the prompt again.
    c.ui_event(&json!({"path": "w|col|sh", "fire": ["__shellRun"], "args": ["polyCube -n shellCube;"]}));
    assert_eq!(ok(&mut c, "objExists shellCube;"), json!(true));
    assert_eq!(ok(&mut c, "global int $runs; $runs;"), json!(1));
    let props = aztek_host::scriptui::control_kind_of("w|col|sh");
    assert_eq!(props.as_deref(), Some("cmdShell"));
    // Clamped to the documented range.
    ok(&mut c, "cmdShell -e -nsl 0 -nhl 40000 sh;");
    assert_eq!(ok(&mut c, "cmdShell -q -nsl sh;"), json!(1));
    assert_eq!(ok(&mut c, "cmdShell -q -nhl sh;"), json!(32767));
    ok(&mut c, "cmdShell -e -cl sh;");
    assert_eq!(ok(&mut c, "global int $runs; $runs;"), json!(2));
    ok(&mut c, "cmdShell -e -en 0 sh;");
    assert_eq!(ok(&mut c, "cmdShell -q -en sh;"), json!(false));
}

#[test]
fn shell_text_keeps_saved_lines() {
    let mut c = Core::new();
    window(&mut c);
    ok(&mut c, "cmdShell -nsl 3 sh;");
    for i in 0..4 {
        ok(&mut c, &format!("aztekCmdShellRun \"sh\" \"int $x{i} = {i};\";"));
    }
    let ops = aztek_host::scriptui::take_ops();
    let last = ops.iter().rev().find_map(|o| o["props"]["text"].as_str().filter(|_| o["path"] == "w|col|sh")).map(str::to_string).unwrap();
    assert_eq!(last.split('\n').count(), 3, "{last:?}");
    assert!(last.ends_with("% "), "{last:?}");
}

#[test]
fn color_editor_answers_and_converts() {
    let mut c = Core::new();
    // With no shell it gives back what it was opened with, as OK.
    assert_eq!(ok(&mut c, "colorEditor;"), json!("0 0 0 1"));
    assert_eq!(ok(&mut c, "colorEditor -rgb 1 0.5 0 -a 0.25;"), json!("1 0.5 0 1"));
    assert_eq!(ok(&mut c, "colorEditor -q -r;"), json!(true));
    assert_eq!(ok(&mut c, "colorEditor -q -rgb;"), json!([1.0, 0.5, 0.0]));
    assert_eq!(ok(&mut c, "colorEditor -q -a;"), json!(0.25));
    let hsv = ok(&mut c, "colorEditor -q -hsv;");
    let h: Vec<f64> = hsv.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!((h[0] - 30.0).abs() < 1e-9 && (h[1] - 1.0).abs() < 1e-9 && (h[2] - 1.0).abs() < 1e-9, "{h:?}");
    // Opened on HSV: pure green.
    assert_eq!(ok(&mut c, "colorEditor -hsv 120 1 1 -m -pos 10 20 -p \"MayaWindow\";"), json!("0 1 0 1"));
    // The result string splits as the reference's examples do.
    assert_eq!(ok(&mut c, "string $r = `colorEditor -rgb 0.2 0.4 0.6`; string $b[]; tokenize $r \" \" $b; $b[3];"), json!("1"));
    assert!(c.mel_checked("colorEditor -q -m;").is_err());
}

#[test]
fn every_documented_flag_is_known() {
    let c = Core::new();
    let unit: &[(&str, &str)] = &[
        ("cmdScrollFieldExecuter", "acb ann annotation appendText at autoCloseBraces backgroundColor bgc cco cl clear clr commandCompletion copySelection cp ct currentLine cutSelection defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exa exc execute executeAll exists fcc fileChangedCommand filename filterKeyPress fkp fn fpn fullPathName h hasFocus hasSelection height hf highlightColor hlc hsl insertText io isObscured it ld ldc ldf load loadContents loadFile m manage mcc mod modificationChangedCommand modified nbg nl noBackground npm numberOfLines numberOfPopupMenus objectPathCompletion opc p parent pasteSelection pma po popupMenuArray preventOverride pst q query rd redo removeStoredContents replaceAll rpa rsc sas saveFile saveSelection saveSelectionToShelf sbm sd searchAndSelect searchDown searchMatchCase searchString searchWraps select selectAll selectedText showLineNumbers showTabsAndSpaces showTooltipHelp sl sla sln slt smc source sourceType spacesPerTab spt src ss st statusBarMessage stc sth storeContents sts sv svf svs sw t tabsForIndent text textLength tfi tl ud undo useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("cmdScrollFieldReporter", "ann annotation backgroundColor bgc clear clr copySelection cp ct cutSelection defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e eac ebg echoAllCommands edit ekf en enable enableBackground enableKeyboardFocus ex exists filterSourceType fpn fst fullPathName h hasFocus height hf highlightColor hlc io isObscured lineNumbers ln m manage nbg noBackground npm numberOfPopupMenus p parent pasteSelection pma po popupMenuArray preventOverride pst q query receiveFocusCommand rfc saveSelection saveSelectionToShelf sbm se select selectAll si sl sla sr sst st stackTrace statusBarMessage suppressErrors suppressInfo suppressResults suppressStackTrace suppressWarnings sv svs sw t text textLength tl useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("cmdShell", "ann annotation backgroundColor bgc c cl clear command defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor hlc io isObscured m manage nbg nhl noBackground npm nsl numberOfHistoryLines numberOfPopupMenus numberOfSavedLines p parent pma po popupMenuArray pr preventOverride prompt q query sbm statusBarMessage useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("colorEditor", "a alpha hsv hsvValue m mini p parent pos position q query r result rgb rgbValue uds useDisplaySpace vcp useVpColorPicker"),
    ];
    for (cmd, flags) in unit {
        let def = c.s.commands.find(cmd).unwrap_or_else(|| panic!("{cmd} missing"));
        for f in flags.split_whitespace() {
            assert!(def.flags.iter().any(|d| d.name == f || d.short_name == f), "{cmd} -{f}");
        }
    }
}

#[test]
fn fields_from_python() {
    let mut c = Core::new();
    ok(&mut c, "python(\"import maya.cmds as cmds\\nw = cmds.window()\\ncmds.columnLayout()\\ne = cmds.cmdScrollFieldExecuter(text='a = 1', sourceType='python')\\ncmds.cmdScrollFieldExecuter(e, e=True, insertText='\\\\nb = 2')\");");
    assert_eq!(ok(&mut c, "python(\"cmds.cmdScrollFieldExecuter(e, q=True, text=True)\");"), json!("a = 1\nb = 2"));
    assert_eq!(ok(&mut c, "python(\"cmds.cmdScrollFieldExecuter(e, q=True, numberOfLines=True)\");"), json!(2));
    ok(&mut c, "python(\"cmds.cmdScrollFieldExecuter(e, e=True, executeAll=True)\");");
    assert_eq!(ok(&mut c, "python(\"b\");"), json!(2));
    assert_eq!(ok(&mut c, "python(\"cmds.colorEditor(rgbValue=(0, 0, 1))\");"), json!("0 0 1 1"));
    assert_eq!(ok(&mut c, "python(\"cmds.colorEditor(query=True, hsvValue=True)\");"), json!([240.0, 1.0, 1.0]));
}
