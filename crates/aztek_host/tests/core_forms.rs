//! The core commands as scripts write them: MEL spellings with switches before the values, and the reference's Python forms.

use std::ffi::{CStr, CString};

use aztek_host::*;
use serde_json::{json, Value};

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn t(c: &mut Core, plug: &str) -> Value {
    c.s.run("getAttr", json!({"args": [plug]})).unwrap()
}

#[test]
fn mel_move_rotate_scale_forms() {
    let mut c = Core::new();
    c.mel("polyCube -n parentBox; polyCube -n a; parent a parentBox; setAttr parentBox.tx 100;");
    c.mel("move -r 1 2 3 a;");
    assert_eq!(t(&mut c, "a.translate"), json!([1.0, 2.0, 3.0]));
    // A switch before values that could read as its own.
    c.mel("move -r -os 1 0 0 a;");
    assert_eq!(t(&mut c, "a.translate"), json!([2.0, 2.0, 3.0]));
    c.mel("move -ws 0 1 0 a;");
    assert_eq!(t(&mut c, "a.translate"), json!([-100.0, 1.0, 0.0]));
    c.mel("move -ls 1 1 1 a;");
    assert_eq!(t(&mut c, "a.translate"), json!([1.0, 1.0, 1.0]));
    c.mel("move -x 5 -ls a;");
    assert_eq!(t(&mut c, "a.tx"), json!(5.0));
    c.mel("move -xy 7 8 -ls a;");
    assert_eq!(t(&mut c, "a.translate"), json!([7.0, 8.0, 1.0]));
    c.mel("rotate 0 90 0 a;");
    assert_eq!(t(&mut c, "a.rotate"), json!([0.0, 90.0, 0.0]));
    c.mel("rotate -r -os 0 1 0 a;");
    assert!((t(&mut c, "a.ry").as_f64().unwrap() - 91.0).abs() < 1e-6);
    c.mel("scale 2 2 2 a; scale -r 2 1 1 a;");
    assert_eq!(t(&mut c, "a.scale"), json!([4.0, 2.0, 2.0]));
    // Components go on working, whatever the switches.
    c.mel("move -r -ws 0 1 0 parentBox.vtx[0];");
    assert!(errors(&mut c).is_empty());
    c.mel("makeIdentity -a 1 -t 1 a;");
    assert_eq!(t(&mut c, "a.translate"), json!([0.0, 0.0, 0.0]));
    assert_eq!(t(&mut c, "a.scale"), json!([4.0, 2.0, 2.0]));
    assert!(errors(&mut c).is_empty());
}

#[test]
fn mel_attribute_and_expression_forms() {
    let mut c = Core::new();
    c.mel("polyCube -n a;");
    assert_eq!(c.mel("getAttr -type a.tx;"), json!("doubleLinear"));
    c.mel("setAttr -k off a.tx; setAttr -l on a.ty;");
    assert_eq!(c.mel("getAttr -k a.tx;"), json!(false));
    assert_eq!(c.mel("getAttr -l a.ty;"), json!(true));
    c.mel("setAttr -l off a.ty; setAttr a.translate -type double3 1 2 3;");
    assert_eq!(t(&mut c, "a.translate"), json!([1.0, 2.0, 3.0]));
    c.mel("addAttr -ln power -at double -min 0 -max 10 -dv 2 -k 1 a;");
    assert_eq!(c.mel("addAttr -q -max a.power;"), json!(10.0));
    assert_eq!(c.mel("attributeQuery -n a -ex power;"), json!(true));
    c.mel("createNode multiplyDivide -n md; connectAttr a.t md.input1; connectAttr -f md.outputX a.power;");
    assert_eq!(t(&mut c, "a.power"), json!(1.0));
    assert_eq!(c.mel("listConnections -s 0 -d 1 md;"), json!(["a"]));
    assert_eq!(c.mel("nodeType md;"), json!("multiplyDivide"));
    assert_eq!(c.mel("shadingNode -asUtility reverse -n rv;"), json!("rv"));
    c.mel("expression -s \"a.sx = 2 + 1; a.sy = a.tz;\" -o a -ae 1 -uc all -n e1;");
    assert_eq!(t(&mut c, "a.sx"), json!(3.0));
    assert_eq!(t(&mut c, "a.sy"), json!(3.0));
    assert_eq!(c.mel("expression -q -s e1;"), json!("a.sx = 2 + 1; a.sy = a.tz;"));
    c.mel("expression -e -s \"a.sx = 9;\" e1;");
    assert_eq!(t(&mut c, "a.sx"), json!(9.0));
    assert!(errors(&mut c).is_empty());
    c.mel("expression -s \"a.sz = ;\";");
    assert_eq!(errors(&mut c).len(), 1);
    c.mel("namespace -add rig; namespace -set rig; createNode transform -n t1; namespace -set \":\";");
    assert_eq!(c.mel("objExists rig:t1;"), json!(true));
    assert_eq!(c.mel("namespaceInfo -lon;"), json!(["rig"]));
    assert_eq!(c.mel("about -linux;"), json!(cfg!(target_os = "linux") as i32));
    c.mel("optionVar -iv \"count\" 5;");
    assert_eq!(c.mel("optionVar -q \"count\";"), json!(5));
    assert_eq!(c.mel("currentUnit -q -l;"), json!("cm"));
    c.mel("undoInfo -ock; setAttr a.rx 1; setAttr a.ry 1; undoInfo -cck; undo;");
    assert_eq!(t(&mut c, "a.rx"), json!(0.0));
    assert!(errors(&mut c).is_empty());
}

fn py(src: &str) {
    let c = CString::new(src).unwrap();
    aztek_python(c.as_ptr());
}

fn log() -> Vec<(i64, String)> {
    let p = aztek_take_log();
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let v: Value = serde_json::from_str(&s).unwrap();
    v.as_array().unwrap().iter().map(|l| (l["kind"].as_i64().unwrap(), l["text"].as_str().unwrap().to_string())).collect()
}

/// What print shows for an expression, with no errors on the way.
fn show(expr: &str) -> String {
    log();
    py(&format!("print({expr})"));
    let l = log();
    assert!(l.iter().all(|(k, _)| *k != 2), "{expr}: {l:?}");
    l.last().map(|x| x.1.clone()).unwrap_or_default()
}

fn query_outliner() -> Value {
    let (name, arg) = (CString::new("outliner").unwrap(), CString::new("").unwrap());
    let p = aztek_query(name.as_ptr(), arg.as_ptr());
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    serde_json::from_str(&s).unwrap()
}

#[test]
fn python_forms() {
    aztek_init();
    log();
    py("import maya.cmds as cmds\ncmds.polyCube(n='x')\ncmds.polyCube(n='y')");
    show("1");
    // The form the reference's docs give: values, name, relative.
    py("cmds.move(1, 2, 3, 'x', r=True)");
    assert_eq!(show("cmds.getAttr('x.translate')"), "[(1.0, 2.0, 3.0)]");
    py("cmds.move(1, 0, 0, 'x', r=True, os=True)");
    assert_eq!(show("cmds.getAttr('x.tx')"), "2.0");
    py("cmds.move(5, 'x', x=True, ls=True)");
    assert_eq!(show("cmds.getAttr('x.tx')"), "5.0");
    py("cmds.move(1, 'x', x=True, r=True)");
    assert_eq!(show("cmds.getAttr('x.tx')"), "6.0");
    py("cmds.move(0, 0, 0, 'x', a=True, ws=True)");
    assert_eq!(show("cmds.xform('x', q=True, ws=True, t=True)"), "[0.0, 0.0, 0.0]");
    py("cmds.rotate(0, 45, 0, 'x', r=True, os=True)\ncmds.scale(2, 2, 2, 'x')");
    assert_eq!(show("cmds.xform('x', q=True, ro=True)"), "[0.0, 45.0, 0.0]");
    assert_eq!(show("cmds.xform('x', q=True, s=True)"), "[2.0, 2.0, 2.0]");
    // Compounds, multis, and the reference's None for an empty list.
    assert_eq!(show("cmds.getAttr('x.tx', type=True)"), "doubleLinear");
    py("cmds.createNode('plusMinusAverage', n='pma')");
    py("for i in range(3): cmds.setAttr('pma.input1D[%d]' % i, i + 1.0)");
    assert_eq!(show("cmds.getAttr('pma.input1D')"), "[1.0, 2.0, 3.0]");
    assert_eq!(show("cmds.getAttr('pma.input1D', mi=True)"), "[0, 1, 2]");
    assert_eq!(show("cmds.getAttr('pma.output1D')"), "6.0");
    py("cmds.createNode('eulerToQuat', n='q')");
    assert_eq!(show("cmds.getAttr('q.outputQuat')"), "[(0.0, 0.0, 0.0, 1.0)]");
    assert_eq!(show("cmds.listRelatives('x', p=True)"), "None");
    assert_eq!(show("cmds.listConnections('x')"), "None");
    assert_eq!(show("cmds.listRelatives('x', s=True)"), "['xShape']");
    assert_eq!(show("cmds.addAttr('x', ln='w', at='double', max=4, k=True)"), "['x.w']");
    assert_eq!(show("cmds.addAttr('x.w', q=True, max=True)"), "4.0");
    assert_eq!(show("cmds.attributeQuery('w', n='x', k=True)"), "True");
    // Expressions.
    assert_eq!(show("cmds.expression(s='x.sx = 3;', o='x', ae=1, uc='all', n='ex')"), "ex");
    assert_eq!(show("cmds.getAttr('x.sx')"), "3.0");
    assert_eq!(show("cmds.expression('ex', q=True, s=True)"), "x.sx = 3;");
    py("cmds.expression('ex', e=True, s='x.sx = 4;')");
    assert_eq!(show("cmds.getAttr('x.sx')"), "4.0");
    // Odds and ends.
    py("cmds.optionVar(iv=('count', 5))");
    assert_eq!(show("cmds.optionVar(q='count')"), "5");
    assert_eq!(show("cmds.optionVar(ex='count')"), "True");
    assert_eq!(show("cmds.about(linux=True)"), if cfg!(target_os = "linux") { "True" } else { "False" });
    assert_eq!(show("cmds.undoInfo(q=True, st=True)"), "True");
    assert_eq!(show("cmds.currentUnit(q=True, a=True)"), "deg");
    assert_eq!(show("cmds.namespace(add='ns')"), "ns");
    assert_eq!(show("cmds.namespaceInfo(lon=True)"), "['ns']");
    assert_eq!(show("len(cmds.ls('x', uuid=True)[0])"), "36");
    py("cmds.transformLimits('y', tx=(-1, 1), etx=(True, True))\ncmds.setAttr('y.tx', 5)");
    assert_eq!(show("cmds.getAttr('y.tx')"), "1.0");
    assert_eq!(show("cmds.transformLimits('y', q=True, tx=True)"), "[-1.0, 1.0]");

    // The Outliner leaves out what asks to be hidden.
    let names = |v: &Value| v.as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(names(&query_outliner()).contains(&"y".to_string()));
    py("cmds.setAttr('y.hiddenInOutliner', True)");
    assert!(!names(&query_outliner()).contains(&"y".to_string()));
}

/// DAG Objects Only off: the nodes that aren't in the DAG are listed too, a skin's bind pose among them.
#[test]
fn outliner_lists_other_nodes_when_asked() {
    aztek_init();
    py("import maya.cmds as cmds\ncmds.file(new=True, f=True)\ncmds.polyCube(n='skinned')\ncmds.select(cl=True)\ncmds.joint(n='root')\ncmds.skinCluster('root', 'skinned')");
    let all = |arg: &str| {
        let (name, arg) = (CString::new("outliner").unwrap(), CString::new(arg).unwrap());
        let p = aztek_query(name.as_ptr(), arg.as_ptr());
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        aztek_free(p);
        serde_json::from_str::<Value>(&s).unwrap()
    };
    let types = |v: &Value| v.as_array().unwrap().iter().map(|n| n["type"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let dag_only = types(&all(""));
    assert!(!dag_only.contains(&"dagPose".to_string()) && !dag_only.contains(&"skinCluster".to_string()), "{dag_only:?}");
    let every = types(&all("dg"));
    for t in ["dagPose", "skinCluster", "polyCube", "lambert"] {
        assert!(every.contains(&t.to_string()), "{t} missing from {every:?}");
    }
    assert!(every.contains(&"transform".to_string()));
}
