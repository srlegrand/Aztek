//! The reference's math nodes and their DL twins (math_nodes.rs): each computes as the reference's page says, the
//! DL twins' values are distances, and they go through a scene file by their short names.

use std::ffi::{CStr, CString};

use aztek_host::*;
use serde_json::Value;

fn log() -> Vec<(i64, String)> {
    let p = aztek_take_log();
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let v: Value = serde_json::from_str(&s).unwrap();
    v.as_array().unwrap().iter().map(|l| (l["kind"].as_i64().unwrap(), l["text"].as_str().unwrap().to_string())).collect()
}

fn expect(src: &str, want: &[&str]) {
    log();
    let c = CString::new(src).unwrap();
    aztek_python(c.as_ptr());
    let l = log();
    let errors: Vec<&String> = l.iter().filter(|(k, _)| *k == 2).map(|(_, t)| t).collect();
    assert!(errors.is_empty(), "Python failed:\n{}\nrunning:\n{src}", errors.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    let got: Vec<String> = l.into_iter().filter(|(k, _)| *k == 3).map(|(_, t)| t).collect();
    assert_eq!(got, want, "running:\n{src}");
}

#[test]
fn math_nodes() {
    aztek_init();
    expect("import maya.cmds as cmds\ncmds.newScene()", &[]);
    expect(
        r#"
def node(t, **kw):
    n = cmds.createNode(t)
    for k, v in kw.items():
        if isinstance(v, (list, tuple)) and k not in ("input", "matrix", "inMatrix"):
            cmds.setAttr(n + "." + k, *v)
        elif isinstance(v, list) and len(v) == 16:
            cmds.setAttr(n + "." + k, v, type="matrix")
        else:
            cmds.setAttr(n + "." + k, v)
    return n
def arr(t, vals):
    n = cmds.createNode(t)
    for i, x in enumerate(vals):
        cmds.setAttr("%s.input[%d]" % (n, i), x)
    return cmds.getAttr(n + ".output")
r = lambda x: round(x, 4) if isinstance(x, float) else x
g = lambda n, a="output": cmds.getAttr(n + "." + a)
out = []
out.append(("abs", g(node("absolute", input=-2.5)), g(node("absoluteDL", input=-2.5))))
out.append(("sum", arr("sum", [1, 2, 3.5]), arr("sumDL", [1, 2])))
out.append(("mul", arr("multiply", [2, 3]), "avg", arr("average", [1, 2, 6]), "max", arr("max", [1, 7, 3]), "min", arr("minDL", [4, -2])))
out.append(("div", g(node("divide", input1=3)), g(node("divide", input1=3, input2=0))))
out.append(("pow", g(node("power", input=3)), "mod", g(node("modulo", input=-1, modulus=3)), g(node("modulo", input=7.5, modulus=2))))
out.append(("log", r(g(node("log", input=8))), "lerp", g(node("lerp", input1=2, input2=4, weight=0.25)), "ilerp", g(node("inverseLerp", input1=2, input2=4, interpolation=3))))
out.append(("eq", g(node("equal", input1=1, input2=1.05, epsilon=0.1)), g(node("equalDL", input1=1, input2=2)), g(node("greaterThan", input1=2, input2=1)), g(node("lessThan", input1=2, input2=1))))
out.append(("bool", g(node("and", input1=1, input2=0)), g(node("or", input1=1, input2=0)), g(node("not", input=0))))
out.append(("round", g(node("round", input=2.5)), g(node("round", input=-2.5)), g(node("ceil", input=1.2)), g(node("floor", input=-1.2)), g(node("truncate", input=-1.7))))
out.append(("clamp", g(node("clampRange", input=5)), "ss", g(node("smoothStep", input=0.5)), g(node("smoothStep", input=2))))
out.append(("trig", r(g(node("sin", input=90))), r(g(node("cos", input=60))), r(g(node("acos", input=0.5))), r(g(node("atan2", input1=1, input2=1))), r(g(node("pi")))))
out.append(("cross", g(node("crossProduct", input1=(1, 0, 0), input2=(0, 1, 0))), "dot", g(node("dotProduct", input1=(1, 2, 3), input2=(4, 5, 6))), "len", g(node("length", input=(3, 4, 0)))))
m = [2, 0, 0, 0, 0, 3, 0, 0, 0, 0, 4, 0, 5, 6, 7, 1]
out.append(("axis", g(node("axisFromMatrix", input=m, axis=4)), "row", g(node("rowFromMatrix", matrix=m, input=3), "outputX"), g(node("rowFromMatrix", matrix=m, input=3), "outputW")))
out.append(("col", g(node("columnFromMatrix", matrix=m, input=0), "outputW"), "t", g(node("translationFromMatrix", input=m)), "s", g(node("scaleFromMatrix", input=m)), "det", g(node("determinant", input=m))))
out.append(("pt", g(node("multiplyPointByMatrix", matrix=m, input=(1, 1, 1))), "vec", g(node("multiplyVectorByMatrix", matrix=m, input=(1, 1, 1)))))
out.append(("rotv", [tuple(r(x) for x in t) for t in g(node("rotateVector", input=(1, 0, 0), rotate=(0, 0, 90)))]))
rm = cmds.createNode("transform"); cmds.setAttr(rm + ".rotate", 10, 20, 30)
rf = cmds.createNode("rotationFromMatrix"); cmds.connectAttr(rm + ".matrix", rf + ".input")
out.append(("rfm", [tuple(r(x) for x in t) for t in g(rf)]))
ab = cmds.createNode("angleBetween"); out.append(("ab", r(g(ab, "angle")), g(ab, "axis"), g(ab, "vector1")))
out.append(("class", cmds.getClassification("sumDL"), len(cmds.listNodeTypes("math")), len(cmds.listNodeTypes("mathLinear"))))
cmds.currentUnit(linear="m")
n = node("sumDL"); cmds.setAttr(n + ".input[0]", 2); cmds.setAttr(n + ".input[1]", 3)
u = cmds.createNode("sum"); cmds.setAttr(u + ".input[0]", 2)
out.append(("meters", g(n), cmds.getAttr(n + ".input[0]"), g(u), cmds.getAttr(n + ".output", type=True)))
cmds.currentUnit(linear="cm")
for o in out: print(o)
"#,
        &[
            r#"('abs', 2.5, 2.5)"#,
            r#"('sum', 6.5, 3.0)"#,
            r#"('mul', 6.0, 'avg', 3.0, 'max', 7.0, 'min', -2.0)"#,
            r#"('div', 3.0, 0.0)"#,
            r#"('pow', 9.0, 'mod', 2.0, 1.5)"#,
            r#"('log', 3.0, 'lerp', 2.5, 'ilerp', 0.5)"#,
            r#"('eq', True, False, True, False)"#,
            r#"('bool', False, True, True)"#,
            r#"('round', 3.0, -2.0, 2.0, -2.0, -1.0)"#,
            r#"('clamp', 1.0, 'ss', 0.5, 1.0)"#,
            r#"('trig', 1.0, 0.5, 60.0, 45.0, 180.0)"#,
            r#"('cross', [(0.0, 0.0, 1.0)], 'dot', 32.0, 'len', 5.0)"#,
            r#"('axis', [(0.0, -3.0, 0.0)], 'row', 5.0, 1.0)"#,
            r#"('col', 5.0, 't', [(5.0, 6.0, 7.0)], 's', [(2.0, 3.0, 4.0)], 'det', 24.0)"#,
            r#"('pt', [(7.0, 9.0, 11.0)], 'vec', [(2.0, 3.0, 4.0)])"#,
            r#"('rotv', [(0.0, 1.0, 0.0)])"#,
            r#"('rfm', [(10.0, 20.0, 30.0)])"#,
            r#"('ab', 90.0, [(1.0, 0.0, 0.0)], [(0.0, 1.0, 0.0)])"#,
            r#"('class', ['mathLinear/operation:drawdb/shader/operation/sum'], 52, 48)"#,
            r#"('meters', 5.0, 2.0, 2.0, 'doubleLinear')"#,
        ],
    );
    let path = std::env::temp_dir().join(format!("aztek_math_{}.ma", std::process::id()));
    let p = path.to_str().unwrap().replace('\\', "/");
    expect(
        &format!(
            r#"
cmds.file(new=True, force=True)
cmds.currentUnit(linear="m")
a = cmds.createNode("transform", n="a"); b = cmds.createNode("transform", n="b")
cmds.createNode("sumDL", n="s"); cmds.createNode("rotateVector", n="rv"); cmds.createNode("clampRangeDL", n="c")
cmds.setAttr("c.maximum", 0.5)
cmds.connectAttr("a.tx", "s.input[0]"); cmds.setAttr("s.input[1]", 1.5); cmds.connectAttr("s.output", "c.input"); cmds.connectAttr("c.output", "b.ty")
cmds.setAttr("rv.rotate", 0, 0, 90); cmds.setAttr("rv.input", 1, 0, 0); cmds.setAttr("rv.rotateOrder", 2)
cmds.setAttr("a.tx", 2)
print(cmds.getAttr("b.ty"))
cmds.file(rename="{p}"); cmds.file(save=True, type="mayaAscii", force=True)
lines = [l.strip() for l in open("{p}")]
print([l for l in lines[lines.index('createNode sumDL -n "s";'):][:7]])
cmds.file("{p}", open=True, force=True)
print(cmds.getAttr("b.ty"), cmds.getAttr("c.maximum"), cmds.getAttr("rv.rotate"), cmds.getAttr("rv.ro"))
cmds.currentUnit(linear="cm")
"#
        ),
        &[
            "0.5",
            r#"['createNode sumDL -n "s";', 'setAttr ".i[1]" 1.5;', 'createNode rotateVector -n "rv";', 'setAttr ".i" -type "double3" 1 0 0 ;', 'setAttr ".r" -type "double3" 0 0 90 ;', 'setAttr ".ro" 2;', 'createNode clampRangeDL -n "c";']"#,
            "0.5 0.5 [(0.0, 0.0, 90.0)] 2",
        ],
    );
}
