//! Utility nodes: the reference's names, values through the graph, undo, save and load, transform attributes.

use aztek_scene::{scene_file, Session};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}
fn set(s: &mut Session, plug: &str, v: Value) {
    run(s, "setAttr", json!({"args": [plug, v]}));
}
fn node(s: &mut Session, ty: &str) -> String {
    run(s, "createNode", json!({"args": [ty]})).as_str().unwrap().to_string()
}
fn connect(s: &mut Session, a: &str, b: &str) {
    run(s, "connectAttr", json!({"args": [a, b]}));
}
fn close(v: &Value, want: f64) {
    let got = v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"));
    assert!((got - want).abs() < 1e-6, "got {got}, wanted {want}");
}
fn close3(v: &Value, want: [f64; 3]) {
    let a = v.as_array().unwrap_or_else(|| panic!("not a list: {v}"));
    for i in 0..3 {
        assert!((a[i].as_f64().unwrap() - want[i]).abs() < 1e-6, "got {v}, wanted {want:?}");
    }
}

#[test]
fn multiply_divide_all_operations() {
    let mut s = Session::new();
    let n = node(&mut s, "multiplyDivide");
    assert_eq!(n, "multiplyDivide1");
    set(&mut s, "multiplyDivide1.input1", json!([2.0, 3.0, 4.0]));
    set(&mut s, "multiplyDivide1.input2", json!([5.0, 6.0, 2.0]));
    close3(&get(&mut s, "multiplyDivide1.output"), [10.0, 18.0, 8.0]);
    set(&mut s, "multiplyDivide1.operation", json!(2));
    close3(&get(&mut s, "multiplyDivide1.o"), [0.4, 0.5, 2.0]);
    set(&mut s, "multiplyDivide1.op", json!(3));
    close(&get(&mut s, "multiplyDivide1.outputZ"), 16.0);
    set(&mut s, "multiplyDivide1.operation", json!(0));
    close(&get(&mut s, "multiplyDivide1.ox"), 2.0);
}

#[test]
fn scalar_and_vector_connections() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    node(&mut s, "multiplyDivide");
    set(&mut s, "multiplyDivide1.input1", json!([2.0, 3.0, 4.0]));
    set(&mut s, "multiplyDivide1.input2", json!([1.0, 1.0, 1.0]));
    connect(&mut s, "multiplyDivide1.outputX", "pCube1.tx");
    close(&get(&mut s, "pCube1.tx"), 2.0);
    run(&mut s, "connectAttr", json!({"args": ["multiplyDivide1.output", "pCube1.translate"], "force": true}));
    close3(&get(&mut s, "pCube1.translate"), [2.0, 3.0, 4.0]);
    // Whole vector replaced the component connection.
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"], "plugs": true})), json!(["multiplyDivide1.output"]));

    node(&mut s, "multiplyDivide");
    set(&mut s, "pCube1.rotate", json!([10.0, 20.0, 30.0]));
    set(&mut s, "multiplyDivide2.input2", json!([2.0, 2.0, 2.0]));
    connect(&mut s, "pCube1.r", "multiplyDivide2.input1");
    close3(&get(&mut s, "multiplyDivide2.output"), [20.0, 40.0, 60.0]);
}

#[test]
fn attribute_through_a_node_back_to_the_same_object_is_no_cycle() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    node(&mut s, "multDoubleLinear");
    set(&mut s, "multDoubleLinear1.input2", json!(3.0));
    connect(&mut s, "pCube1.tx", "multDoubleLinear1.input1");
    connect(&mut s, "multDoubleLinear1.output", "pCube1.ty");
    set(&mut s, "pCube1.tx", json!(2.0));
    close(&get(&mut s, "pCube1.ty"), 6.0);
    close(&get(&mut s, "pCube1.worldMatrix")[13], 6.0);
    // Feeding a real output back is refused.
    node(&mut s, "decomposeMatrix");
    connect(&mut s, "pCube1.worldMatrix[0]", "decomposeMatrix1.inputMatrix");
    assert!(s.run("connectAttr", json!({"args": ["decomposeMatrix1.outputTranslate", "pCube1.scale"]})).is_err());
}

#[test]
fn animated_channel_read_through_a_utility_in_the_same_object() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    node(&mut s, "addDoubleLinear");
    set(&mut s, "addDoubleLinear1.input2", json!(1.0));
    connect(&mut s, "pCube1.tx", "addDoubleLinear1.input1");
    connect(&mut s, "addDoubleLinear1.output", "pCube1.tz");
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 11, "value": 10.0}));
    run(&mut s, "currentTime", json!({"args": [11]}));
    close(&get(&mut s, "pCube1.tz"), 11.0);
    run(&mut s, "currentTime", json!({"args": [1]}));
    close(&get(&mut s, "pCube1.tz"), 1.0);
}

#[test]
fn plus_minus_average_arrays() {
    let mut s = Session::new();
    node(&mut s, "plusMinusAverage");
    let p = "plusMinusAverage1";
    set(&mut s, &format!("{p}.input1D[0]"), json!(4.0));
    set(&mut s, &format!("{p}.input1D[3]"), json!(2.0));
    close(&get(&mut s, &format!("{p}.output1D")), 6.0);
    set(&mut s, &format!("{p}.operation"), json!(2));
    close(&get(&mut s, &format!("{p}.output1D")), 2.0);
    set(&mut s, &format!("{p}.operation"), json!(3));
    close(&get(&mut s, &format!("{p}.output1D")), 3.0);

    set(&mut s, &format!("{p}.operation"), json!(1));
    set(&mut s, &format!("{p}.input3D[0]"), json!([1.0, 2.0, 3.0]));
    set(&mut s, &format!("{p}.input3D[1].input3Dx"), json!(10.0));
    close3(&get(&mut s, &format!("{p}.output3D")), [11.0, 2.0, 3.0]);
    close(&get(&mut s, &format!("{p}.input3D[1].i3x")), 10.0);

    set(&mut s, &format!("{p}.input2D[0].input2Dx"), json!(1.0));
    set(&mut s, &format!("{p}.input2D[0].input2Dy"), json!(2.0));
    close(&get(&mut s, &format!("{p}.output2Dy")), 2.0);

    // Components of elements connect, and the next free element is found.
    run(&mut s, "polyCube", json!({}));
    set(&mut s, "pCube1.tx", json!(7.0));
    connect(&mut s, "pCube1.tx", &format!("{p}.input3D[2].input3Dy"));
    close3(&get(&mut s, &format!("{p}.output3D")), [11.0, 9.0, 3.0]);
    assert_eq!(run(&mut s, "connectAttr", json!({"args": ["pCube1.ty", format!("{p}.input1D")], "nextAvailable": true})), json!(format!("{p}.input1D[4]")));
}

#[test]
fn condition_clamp_set_range_reverse_blend() {
    let mut s = Session::new();
    node(&mut s, "condition");
    set(&mut s, "condition1.colorIfTrue", json!([1.0, 2.0, 3.0]));
    set(&mut s, "condition1.colorIfFalse", json!([4.0, 5.0, 6.0]));
    close3(&get(&mut s, "condition1.outColor"), [1.0, 2.0, 3.0]);
    set(&mut s, "condition1.firstTerm", json!(2.0));
    set(&mut s, "condition1.secondTerm", json!(1.0));
    close(&get(&mut s, "condition1.outColorR"), 4.0);
    set(&mut s, "condition1.operation", json!(2));
    close(&get(&mut s, "condition1.ocg"), 2.0);
    for (op, want) in [(1, 1.0), (3, 1.0), (4, 4.0), (5, 4.0)] {
        set(&mut s, "condition1.operation", json!(op));
        close(&get(&mut s, "condition1.outColorR"), want);
    }

    node(&mut s, "clamp");
    set(&mut s, "clamp1.max", json!([1.0, 1.0, 1.0]));
    set(&mut s, "clamp1.input", json!([-1.0, 0.5, 3.0]));
    close3(&get(&mut s, "clamp1.output"), [0.0, 0.5, 1.0]);

    node(&mut s, "setRange");
    set(&mut s, "setRange1.oldMax", json!([10.0, 10.0, 10.0]));
    set(&mut s, "setRange1.max", json!([1.0, 2.0, 100.0]));
    set(&mut s, "setRange1.value", json!([5.0, 20.0, -1.0]));
    close3(&get(&mut s, "setRange1.outValue"), [0.5, 2.0, 0.0]);

    node(&mut s, "reverse");
    set(&mut s, "reverse1.inputX", json!(0.25));
    close3(&get(&mut s, "reverse1.output"), [0.75, 1.0, 1.0]);

    node(&mut s, "blendColors");
    close3(&get(&mut s, "blendColors1.output"), [0.5, 0.0, 0.5]);
    set(&mut s, "blendColors1.blender", json!(1.0));
    close3(&get(&mut s, "blendColors1.output"), [1.0, 0.0, 0.0]);

    node(&mut s, "blendTwoAttr");
    set(&mut s, "blendTwoAttr1.input[0]", json!(2.0));
    set(&mut s, "blendTwoAttr1.input[1]", json!(4.0));
    set(&mut s, "blendTwoAttr1.attributesBlender", json!(0.5));
    close(&get(&mut s, "blendTwoAttr1.output"), 3.0);

    node(&mut s, "unitConversion");
    set(&mut s, "unitConversion1.input", json!(2.0));
    set(&mut s, "unitConversion1.conversionFactor", json!(0.5));
    close(&get(&mut s, "unitConversion1.output"), 1.0);
}

#[test]
fn remap_value_ramps() {
    let mut s = Session::new();
    node(&mut s, "remapValue");
    // A new node has its two end points.
    close(&get(&mut s, "remapValue1.value[1].value_Position"), 1.0);
    set(&mut s, "remapValue1.inputValue", json!(0.25));
    close(&get(&mut s, "remapValue1.outValue"), 0.25);
    set(&mut s, "remapValue1.inputMax", json!(2.0));
    set(&mut s, "remapValue1.inputValue", json!(1.0));
    close(&get(&mut s, "remapValue1.outValue"), 0.5);
    set(&mut s, "remapValue1.outputMin", json!(10.0));
    set(&mut s, "remapValue1.outputMax", json!(20.0));
    close(&get(&mut s, "remapValue1.outValue"), 15.0);
    // A point in the middle, the short spellings too.
    set(&mut s, "remapValue1.vl[2].vlp", json!(0.5));
    set(&mut s, "remapValue1.value[2].value_FloatValue", json!(1.0));
    close(&get(&mut s, "remapValue1.outValue"), 20.0);
    set(&mut s, "remapValue1.value[2].value_Interp", json!(0));
    set(&mut s, "remapValue1.inputValue", json!(1.5));
    close(&get(&mut s, "remapValue1.outValue"), 20.0);
    // Smooth from the first point to the middle one.
    set(&mut s, "remapValue1.value[0].value_Interp", json!(2));
    set(&mut s, "remapValue1.inputValue", json!(0.25));
    close(&get(&mut s, "remapValue1.outValue"), 10.0 + 10.0 * 0.15625);
}

#[test]
fn distance_angle_vector_product() {
    let mut s = Session::new();
    node(&mut s, "distanceBetween");
    set(&mut s, "distanceBetween1.point1", json!([1.0, 2.0, 3.0]));
    set(&mut s, "distanceBetween1.point2", json!([4.0, 6.0, 3.0]));
    close(&get(&mut s, "distanceBetween1.distance"), 5.0);

    node(&mut s, "angleBetween");
    set(&mut s, "angleBetween1.vector2", json!([0.0, 1.0, 0.0]));
    close(&get(&mut s, "angleBetween1.angle"), 90.0);
    close3(&get(&mut s, "angleBetween1.axis"), [0.0, 0.0, 1.0]);
    close3(&get(&mut s, "angleBetween1.euler"), [0.0, 0.0, 90.0]);

    node(&mut s, "vectorProduct");
    set(&mut s, "vectorProduct1.input1", json!([1.0, 2.0, 3.0]));
    set(&mut s, "vectorProduct1.input2", json!([4.0, 5.0, 6.0]));
    close(&get(&mut s, "vectorProduct1.outputX"), 32.0);
    set(&mut s, "vectorProduct1.operation", json!(2));
    close3(&get(&mut s, "vectorProduct1.output"), [-3.0, 6.0, -3.0]);
    set(&mut s, "vectorProduct1.normalizeOutput", json!(true));
    let k = 1.0 / 6.0f64.sqrt();
    close3(&get(&mut s, "vectorProduct1.output"), [-k, 2.0 * k, -k]);
}

fn translate_matrix(x: f64, y: f64, z: f64) -> Value {
    json!([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0])
}

#[test]
fn matrix_nodes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    set(&mut s, "a.translate", json!([1.0, 2.0, 3.0]));
    set(&mut s, "a.rotate", json!([30.0, 40.0, 50.0]));
    set(&mut s, "a.scale", json!([2.0, 3.0, 4.0]));
    node(&mut s, "decomposeMatrix");
    connect(&mut s, "a.worldMatrix[0]", "decomposeMatrix1.inputMatrix");
    close3(&get(&mut s, "decomposeMatrix1.outputTranslate"), [1.0, 2.0, 3.0]);
    close3(&get(&mut s, "decomposeMatrix1.outputRotate"), [30.0, 40.0, 50.0]);
    close3(&get(&mut s, "decomposeMatrix1.outputScale"), [2.0, 3.0, 4.0]);

    node(&mut s, "composeMatrix");
    for (from, to) in [("outputTranslate", "inputTranslate"), ("outputRotate", "inputRotate"), ("outputScale", "inputScale")] {
        connect(&mut s, &format!("decomposeMatrix1.{from}"), &format!("composeMatrix1.{to}"));
    }
    let (want, got) = (get(&mut s, "a.worldMatrix"), get(&mut s, "composeMatrix1.outputMatrix"));
    for i in 0..16 {
        close(&got[i], want[i].as_f64().unwrap());
    }

    // matrixIn[0] applies first: translate, then scale by 2 about the origin.
    node(&mut s, "multMatrix");
    set(&mut s, "multMatrix1.matrixIn[0]", translate_matrix(1.0, 0.0, 0.0));
    set(&mut s, "multMatrix1.matrixIn[1]", json!([2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0]));
    close(&get(&mut s, "multMatrix1.matrixSum")[12], 2.0);

    node(&mut s, "inverseMatrix");
    set(&mut s, "inverseMatrix1.inputMatrix", translate_matrix(1.0, 2.0, 3.0));
    close(&get(&mut s, "inverseMatrix1.outputMatrix")[13], -2.0);

    node(&mut s, "transposeMatrix");
    set(&mut s, "transposeMatrix1.inputMatrix", translate_matrix(1.0, 2.0, 3.0));
    close(&get(&mut s, "transposeMatrix1.outputMatrix")[3], 1.0);

    node(&mut s, "addMatrix");
    set(&mut s, "addMatrix1.matrixIn[0]", translate_matrix(1.0, 0.0, 0.0));
    set(&mut s, "addMatrix1.matrixIn[1]", translate_matrix(2.0, 0.0, 0.0));
    close(&get(&mut s, "addMatrix1.matrixSum")[12], 3.0);
    close(&get(&mut s, "addMatrix1.matrixSum")[0], 2.0);

    node(&mut s, "wtAddMatrix");
    set(&mut s, "wtAddMatrix1.wtMatrix[0].matrixIn", translate_matrix(4.0, 0.0, 0.0));
    set(&mut s, "wtAddMatrix1.wtMatrix[0].weightIn", json!(0.5));
    set(&mut s, "wtAddMatrix1.wtMatrix[1].matrixIn", translate_matrix(2.0, 0.0, 0.0));
    set(&mut s, "wtAddMatrix1.wm[1].w", json!(0.5));
    close(&get(&mut s, "wtAddMatrix1.matrixSum")[12], 3.0);
    close(&get(&mut s, "wtAddMatrix1.matrixSum")[0], 1.0);

    node(&mut s, "holdMatrix");
    set(&mut s, "holdMatrix1.inMatrix", translate_matrix(9.0, 0.0, 0.0));
    close(&get(&mut s, "holdMatrix1.outMatrix")[12], 9.0);

    node(&mut s, "fourByFourMatrix");
    set(&mut s, "fourByFourMatrix1.in30", json!(5.0));
    set(&mut s, "fourByFourMatrix1.i31", json!(6.0));
    close(&get(&mut s, "fourByFourMatrix1.output")[12], 5.0);
    close(&get(&mut s, "fourByFourMatrix1.output")[13], 6.0);

    node(&mut s, "pickMatrix");
    set(&mut s, "pickMatrix1.inputMatrix", translate_matrix(1.0, 2.0, 3.0));
    set(&mut s, "pickMatrix1.useTranslate", json!(false));
    close(&get(&mut s, "pickMatrix1.outputMatrix")[12], 0.0);

    node(&mut s, "blendMatrix");
    set(&mut s, "blendMatrix1.inputMatrix", translate_matrix(0.0, 0.0, 0.0));
    set(&mut s, "blendMatrix1.target[0].targetMatrix", translate_matrix(10.0, 0.0, 0.0));
    set(&mut s, "blendMatrix1.target[0].weight", json!(0.25));
    close(&get(&mut s, "blendMatrix1.outputMatrix")[12], 2.5);

    node(&mut s, "aimMatrix");
    set(&mut s, "aimMatrix1.primaryTargetMatrix", translate_matrix(0.0, 5.0, 0.0));
    let m = get(&mut s, "aimMatrix1.outputMatrix");
    close3(&json!([m[0], m[1], m[2]]), [0.0, 1.0, 0.0]);
}

#[test]
fn shear_decomposes_back() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    set(&mut s, "a.shear", json!([0.5, 0.25, 0.125]));
    set(&mut s, "a.scale", json!([2.0, 3.0, 4.0]));
    set(&mut s, "a.rotate", json!([10.0, 0.0, 0.0]));
    node(&mut s, "decomposeMatrix");
    connect(&mut s, "a.matrix", "decomposeMatrix1.inputMatrix");
    close3(&get(&mut s, "decomposeMatrix1.outputShear"), [0.5, 0.25, 0.125]);
    close3(&get(&mut s, "decomposeMatrix1.outputScale"), [2.0, 3.0, 4.0]);
    close3(&get(&mut s, "decomposeMatrix1.outputRotate"), [10.0, 0.0, 0.0]);
}

#[test]
fn quaternions_and_compound_connections() {
    let mut s = Session::new();
    node(&mut s, "eulerToQuat");
    set(&mut s, "eulerToQuat1.inputRotate", json!([0.0, 90.0, 0.0]));
    let k = 0.5f64.sqrt();
    close(&get(&mut s, "eulerToQuat1.outputQuatY"), k);
    close(&get(&mut s, "eulerToQuat1.outputQuatW"), k);
    let all = get(&mut s, "eulerToQuat1.outputQuat");
    assert_eq!(all.as_array().unwrap().len(), 4);

    node(&mut s, "quatToEuler");
    connect(&mut s, "eulerToQuat1.outputQuat", "quatToEuler1.inputQuat");
    close3(&get(&mut s, "quatToEuler1.outputRotate"), [0.0, 90.0, 0.0]);
    assert!(run(&mut s, "isConnected", json!({"args": ["eulerToQuat1.outputQuatW", "quatToEuler1.inputQuatW"]})).as_bool().unwrap());

    node(&mut s, "quatMultiply");
    connect(&mut s, "eulerToQuat1.outputQuat", "quatMultiply1.input1Quat");
    connect(&mut s, "eulerToQuat1.outputQuat", "quatMultiply1.input2Quat");
    close(&get(&mut s, "quatMultiply1.outputQuatY"), 1.0);

    run(&mut s, "polyCube", json!({"name": "a"}));
    set(&mut s, "a.ry", json!(90.0));
    close(&get(&mut s, "a.rotateQuaternionW"), k);
}

#[test]
fn modern_float_and_colour_nodes() {
    let mut s = Session::new();
    node(&mut s, "floatMath");
    set(&mut s, "floatMath1.floatA", json!(6.0));
    set(&mut s, "floatMath1.floatB", json!(2.0));
    for (op, want) in [(0, 8.0), (1, 4.0), (2, 12.0), (3, 3.0), (4, 2.0), (5, 6.0), (6, 36.0)] {
        set(&mut s, "floatMath1.operation", json!(op));
        close(&get(&mut s, "floatMath1.outFloat"), want);
    }
    node(&mut s, "floatLogic");
    set(&mut s, "floatLogic1.floatA", json!(1.0));
    assert_eq!(get(&mut s, "floatLogic1.outBool"), json!(false));
    set(&mut s, "floatLogic1.operation", json!(4));
    assert_eq!(get(&mut s, "floatLogic1.outBool"), json!(true));
    node(&mut s, "floatCondition");
    set(&mut s, "floatCondition1.floatA", json!(3.0));
    close(&get(&mut s, "floatCondition1.outFloat"), 3.0);
    node(&mut s, "floatConstant");
    set(&mut s, "floatConstant1.inFloat", json!(3.5));
    close(&get(&mut s, "floatConstant1.outFloat"), 3.5);
    node(&mut s, "colorConstant");
    set(&mut s, "colorConstant1.inColor", json!([0.1, 0.2, 0.3]));
    close3(&get(&mut s, "colorConstant1.outColor"), [0.1, 0.2, 0.3]);
    node(&mut s, "rgbToHsv");
    set(&mut s, "rgbToHsv1.inRgb", json!([0.0, 1.0, 0.0]));
    close3(&get(&mut s, "rgbToHsv1.outHsv"), [120.0, 1.0, 1.0]);
    node(&mut s, "hsvToRgb");
    set(&mut s, "hsvToRgb1.inHsv", json!([240.0, 1.0, 0.5]));
    close3(&get(&mut s, "hsvToRgb1.outRgb"), [0.0, 0.0, 0.5]);
    node(&mut s, "luminance");
    set(&mut s, "luminance1.value", json!([1.0, 1.0, 1.0]));
    close(&get(&mut s, "luminance1.outValue"), 1.0);
    node(&mut s, "pairBlend");
    set(&mut s, "pairBlend1.inTranslate2", json!([10.0, 0.0, 0.0]));
    close(&get(&mut s, "pairBlend1.outTranslateX"), 10.0);
    set(&mut s, "pairBlend1.weight", json!(0.5));
    close(&get(&mut s, "pairBlend1.outTranslateX"), 5.0);
    node(&mut s, "pointMatrixMult");
    set(&mut s, "pointMatrixMult1.inMatrix", translate_matrix(1.0, 0.0, 0.0));
    set(&mut s, "pointMatrixMult1.inPoint", json!([1.0, 1.0, 1.0]));
    close3(&get(&mut s, "pointMatrixMult1.output"), [2.0, 1.0, 1.0]);
    set(&mut s, "pointMatrixMult1.vectorMultiply", json!(true));
    close3(&get(&mut s, "pointMatrixMult1.output"), [1.0, 1.0, 1.0]);
}

#[test]
fn closest_point_on_mesh_node() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2}));
    node(&mut s, "closestPointOnMesh");
    connect(&mut s, "pCubeShape1.outMesh", "closestPointOnMesh1.inMesh");
    set(&mut s, "closestPointOnMesh1.inPosition", json!([0.0, 5.0, 0.0]));
    close3(&get(&mut s, "closestPointOnMesh1.position"), [0.0, 1.0, 0.0]);
    close3(&get(&mut s, "closestPointOnMesh1.result.normal"), [0.0, 1.0, 0.0]);
}

#[test]
fn shading_node_keying_undo_and_files() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "shadingNode", json!({"args": ["multiplyDivide"], "asUtility": true, "name": "md"})), "md");
    assert_eq!(run(&mut s, "getClassification", json!({"args": ["multiplyDivide"]})), json!(["utility/general"]));
    // Keyable.
    run(&mut s, "setKeyframe", json!({"args": ["md.input1X"], "time": 1, "value": 2.0}));
    run(&mut s, "setKeyframe", json!({"args": ["md.input1X"], "time": 11, "value": 4.0}));
    run(&mut s, "currentTime", json!({"args": [11]}));
    close(&get(&mut s, "md.outputX"), 4.0);
    // Undoable.
    run(&mut s, "undoInfo", json!({"stateWithoutFlush": true}));
    run(&mut s, "polyCube", json!({}));
    connect(&mut s, "md.outputX", "pCube1.tx");
    close(&get(&mut s, "pCube1.tx"), 4.0);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"]})), json!([]));
    run(&mut s, "redo", json!({}));
    close(&get(&mut s, "pCube1.tx"), 4.0);
    // Saved and loaded.
    node(&mut s, "plusMinusAverage");
    set(&mut s, "plusMinusAverage1.input3D[1]", json!([1.0, 2.0, 3.0]));
    connect(&mut s, "plusMinusAverage1.output3Dx", "pCube1.ty");
    let doc = scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    scene_file::deserialize(&mut t.graph, &doc).unwrap();
    close(&get(&mut t, "pCube1.ty"), 1.0);
    close3(&get(&mut t, "plusMinusAverage1.output3D"), [1.0, 2.0, 3.0]);
    close(&get(&mut t, "pCube1.tx"), get(&mut s, "pCube1.tx").as_f64().unwrap());
}

#[test]
fn offset_parent_matrix_inherits_transform_and_inverses() {
    let mut s = Session::new();
    run(&mut s, "group", json!({"empty": true, "name": "p"}));
    run(&mut s, "spaceLocator", json!({"name": "a"}));
    run(&mut s, "parent", json!({"args": ["a", "p"]}));
    set(&mut s, "p.translate", json!([100.0, 0.0, 0.0]));
    set(&mut s, "a.translate", json!([1.0, 0.0, 0.0]));
    set(&mut s, "a.offsetParentMatrix", translate_matrix(0.0, 10.0, 0.0));
    close3(&run(&mut s, "xform", json!({"args": ["a"], "query": true, "worldSpace": true, "translation": true})), [101.0, 10.0, 0.0]);
    // The local matrix leaves the offset out; move -a in world space accounts for it.
    close(&get(&mut s, "a.matrix")[13], 0.0);
    close(&get(&mut s, "a.worldMatrix")[13], 10.0);
    close(&get(&mut s, "a.worldInverseMatrix")[13], -10.0);
    close(&get(&mut s, "a.parentInverseMatrix")[12], -100.0);
    close(&get(&mut s, "a.inverseMatrix")[12], -1.0);
    close(&get(&mut s, "a.xformMatrix")[12], 1.0);

    run(&mut s, "xform", json!({"args": ["a"], "worldSpace": true, "translation": [0.0, 0.0, 0.0]}));
    close3(&get(&mut s, "a.translate"), [-100.0, -10.0, 0.0]);

    // Driven through a connection.
    node(&mut s, "composeMatrix");
    set(&mut s, "composeMatrix1.inputTranslate", json!([0.0, 0.0, 5.0]));
    connect(&mut s, "composeMatrix1.outputMatrix", "a.offsetParentMatrix");
    close(&get(&mut s, "a.worldMatrix")[14], 5.0);

    set(&mut s, "a.inheritsTransform", json!(false));
    close(&get(&mut s, "a.worldMatrix")[12], -100.0);
}

#[test]
fn shear_and_limits_change_the_matrix() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "a"}));
    set(&mut s, "a.shearXY", json!(0.5));
    let m = get(&mut s, "a.matrix");
    close(&m[4], 0.5);
    set(&mut s, "a.shxy", json!(0.0));

    run(&mut s, "transformLimits", json!({"args": ["a"], "translationX": [-1.0, 2.0], "enableTranslationX": [true, true]}));
    set(&mut s, "a.tx", json!(5.0));
    close(&get(&mut s, "a.tx"), 2.0);
    set(&mut s, "a.tx", json!(-5.0));
    close(&get(&mut s, "a.tx"), -1.0);
    assert_eq!(run(&mut s, "transformLimits", json!({"args": ["a"], "query": true, "queried": ["translationX"]})), json!([-1.0, 2.0]));
    assert_eq!(run(&mut s, "transformLimits", json!({"args": ["a"], "query": true, "queried": ["enableTranslationX"]})), json!([true, true]));
    run(&mut s, "transformLimits", json!({"args": ["a"], "enableTranslationX": [false, false]}));
    set(&mut s, "a.tx", json!(5.0));
    close(&get(&mut s, "a.tx"), 5.0);
}

#[test]
fn bounding_box_attributes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"width": 2, "height": 4, "depth": 6}));
    set(&mut s, "pCube1.tx", json!(10.0));
    close3(&get(&mut s, "pCube1.boundingBoxMin"), [9.0, -2.0, -3.0]);
    close3(&get(&mut s, "pCube1.bbmx"), [11.0, 2.0, 3.0]);
    close3(&get(&mut s, "pCube1.boundingBoxSize"), [2.0, 4.0, 6.0]);
    close3(&get(&mut s, "pCube1.center"), [10.0, 0.0, 0.0]);
    close(&get(&mut s, "pCube1.boundingBoxMaxY"), 2.0);
}

#[test]
fn ma_files_keep_utility_nodes_and_expressions() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "createNode", json!({"args": ["multiplyDivide"], "name": "md"}));
    set(&mut s, "md.input2", json!([2.0, 2.0, 2.0]));
    run(&mut s, "setAttr", json!({"args": ["box.tx", 3.0]}));
    connect(&mut s, "box.tx", "md.input1X");
    connect(&mut s, "md.outputX", "box.ty");
    run(&mut s, "createNode", json!({"args": ["plusMinusAverage"], "name": "pma"}));
    set(&mut s, "pma.input3D[1]", json!([1.0, 2.0, 3.0]));
    connect(&mut s, "pma.output3Dx", "box.sz");
    run(&mut s, "createNode", json!({"args": ["remapValue"], "name": "rv"}));
    set(&mut s, "rv.value[2].value_Position", json!(0.5));
    set(&mut s, "rv.value[2].value_FloatValue", json!(0.25));
    set(&mut s, "rv.inputValue", json!(0.5));
    run(&mut s, "expression", json!({"string": "box.rx = box.tx * 10;", "name": "e1"}));
    set(&mut s, "box.offsetParentMatrix", translate_matrix(0.0, 0.0, 7.0));
    set(&mut s, "box.shearXY", json!(0.5));

    let (text, warnings) = aztek_scene::ma_file::write_text(&mut s, "x.ma", None);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut t = Session::new();
    let warnings = aztek_scene::ma_file::read_text(&mut t, &text, false);
    assert!(warnings.is_empty(), "{warnings:?}\n{text}");
    close(&get(&mut t, "box.ty"), 6.0);
    close3(&get(&mut t, "pma.output3D"), [1.0, 2.0, 3.0]);
    close(&get(&mut t, "box.sz"), 1.0);
    close(&get(&mut t, "rv.outValue"), 0.25);
    close(&get(&mut t, "box.rx"), 30.0);
    assert_eq!(run(&mut t, "expression", json!({"args": ["e1"], "query": true, "queried": ["string"]})), "box.rx = box.tx * 10;");
    close(&get(&mut t, "box.worldMatrix")[14], 7.0);
    close(&get(&mut t, "box.shxy"), 0.5);
    // Still live after the trip.
    set(&mut t, "box.tx", json!(1.0));
    close(&get(&mut t, "box.ty"), 2.0);
    close(&get(&mut t, "box.rx"), 10.0);
}

#[test]
fn ma_files_as_the_reference_writes_them() {
    let text = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "box";
createNode multiplyDivide -n "md";
    setAttr ".i2" -type "float3" 2 3 4 ;
    setAttr ".op" 1;
createNode plusMinusAverage -n "pma";
    setAttr -s 2 ".i1";
    setAttr ".i1[0]" 5;
    setAttr ".i1[1]" 6;
createNode expression -n "e1";
    setAttr -k on ".nds";
    setAttr ".ixp" -type "string" ".O[0] = .I[0] * 2;";
createNode condition -n "cn";
    setAttr ".ct" -type "float3" 1 2 3 ;
connectAttr "box.tx" "md.i1x";
connectAttr "md.ox" "box.ty";
connectAttr "box.tx" "e1.in[0]";
connectAttr "e1.out[0]" "box.tz";
connectAttr ":time1.o" "e1.tim";
connectAttr "pma.o1" "cn.ft";
// End of x.ma
"#;
    let mut s = Session::new();
    let warnings = aztek_scene::ma_file::read_text(&mut s, text, false);
    assert!(warnings.is_empty(), "{warnings:?}");
    set(&mut s, "box.tx", json!(5.0));
    close(&get(&mut s, "box.ty"), 10.0);
    close(&get(&mut s, "box.tz"), 10.0);
    close(&get(&mut s, "pma.output1D"), 11.0);
    close(&get(&mut s, "cn.firstTerm"), 11.0);
    close(&get(&mut s, "cn.colorIfTrueG"), 2.0);
}
