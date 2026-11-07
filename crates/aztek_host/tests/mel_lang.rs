//! The MEL language, written from the reference's documented behaviour.

use aztek_host::Core;
use serde_json::{json, Value};

/// Runs a source on a fresh Core, expecting no error, and gives the last statement's value.
fn val(src: &str) -> Value {
    let mut c = Core::new();
    let v = c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    v
}

fn lines(c: &mut Core, kind: i64) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == kind).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn printed(src: &str) -> Vec<String> {
    let mut c = Core::new();
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    lines(&mut c, 3)
}

fn failure(src: &str) -> String {
    let mut c = Core::new();
    c.mel_checked(src).expect_err(src)
}

#[test]
fn comments_and_literals() {
    assert_eq!(val("// a comment\n/* and\n another */ 1 + /* inline */ 2; // done"), json!(3));
    assert_eq!(val(r#""a\tb\n\"c\"\\";"#), json!("a\tb\n\"c\"\\"));
    assert_eq!(val("1.5e2;"), json!(150.0));
    assert_eq!(val(".5;"), json!(0.5));
    assert_eq!(val("0x10;"), json!(16));
    assert_eq!(val("true + on + yes + false + off + no;"), json!(3));
    assert_eq!(val(r#"string $s = "a;b"; $s"#), json!("a;b"));
}

#[test]
fn declarations() {
    assert_eq!(val("int $a = 1, $b = 2; $a + $b;"), json!(3));
    assert_eq!(val("int $a; $a;"), json!(0));
    assert_eq!(val("float $a; $a;"), json!(0.0));
    assert_eq!(val("string $a; $a;"), json!(""));
    assert_eq!(val("vector $a; $a;"), json!([0.0, 0.0, 0.0]));
    assert_eq!(val("float $f[3]; size($f);"), json!(3));
    assert_eq!(val("string $s[]; size($s);"), json!(0));
    // Without a declaration the first assignment fixes the type.
    assert_eq!(val("$x = 5; $x = 2.7; $x;"), json!(2));
    assert_eq!(val("$x = 1.5; $x = 2; $x;"), json!(2.0));
    assert_eq!(val("$x = {1, 2}; size($x);"), json!(2));
    assert!(failure("print $nope;").contains("undeclared"));
    assert!(failure("int $a; string $a;").contains("different type"));
}

#[test]
fn coercions() {
    assert_eq!(val("int $i = 3.9; $i;"), json!(3));
    assert_eq!(val("int $i = -3.9; $i;"), json!(-3));
    assert_eq!(val(r#"int $i = "12abc"; $i;"#), json!(12));
    assert_eq!(val(r#"int $i = "abc"; $i;"#), json!(0));
    assert_eq!(val(r#"float $f = "1.5x"; $f;"#), json!(1.5));
    assert_eq!(val("string $s = 3; $s;"), json!("3"));
    assert_eq!(val("string $s = 1.5; $s;"), json!("1.5"));
    assert_eq!(val("string $s = 3.0; $s;"), json!("3"));
    assert_eq!(val("string $s = <<1, 2, 3>>; $s;"), json!("1 2 3"));
    assert_eq!(val(r#"vector $v = "1 2 3"; $v;"#), json!([1.0, 2.0, 3.0]));
    assert_eq!(val("vector $v = 2; $v;"), json!([2.0, 2.0, 2.0]));
    assert_eq!(val("(int)3.7;"), json!(3));
    assert_eq!(val("(string)3;"), json!("3"));
    assert_eq!(val("float(3);"), json!(3.0));
    assert_eq!(val(r#"int("7");"#), json!(7));
    assert_eq!(val(r#"string(7) + "x";"#), json!("7x"));
    assert_eq!(val("float $a[] = {1, 2}; $a[1];"), json!(2.0));
    assert!(failure("int $a[] = {1, 2}; string $s = $a;").contains("Cannot convert"));
    assert!(failure(r#"string $a[] = {"x", "y"}; string $s = "a" + $a;"#).contains("string[]"));
}

#[test]
fn operators() {
    assert_eq!(val("1 + 2 * 3;"), json!(7));
    assert_eq!(val("(1 + 2) * 3;"), json!(9));
    assert_eq!(val("5 / 2;"), json!(2));
    assert_eq!(val("5 / 2.0;"), json!(2.5));
    assert_eq!(val("7 % 3;"), json!(1));
    assert_eq!(val("-7 % 3;"), json!(-1));
    assert_eq!(val("5.5 % 2;"), json!(1.5));
    assert_eq!(val("-(2 + 3);"), json!(-5));
    assert_eq!(val("2 > 1;"), json!(1));
    assert_eq!(val("2 <= 1;"), json!(0));
    assert_eq!(val("2 == 2.0;"), json!(1));
    assert_eq!(val("!0;"), json!(1));
    assert_eq!(val("!5;"), json!(0));
    assert_eq!(val("1 && 0;"), json!(0));
    assert_eq!(val("1 || 0;"), json!(1));
    assert_eq!(val("1 ? 2 : 3;"), json!(2));
    assert_eq!(val("0 ? 2 : 0 ? 3 : 4;"), json!(4));
    assert_eq!(val(r#""a" + 1;"#), json!("a1"));
    assert_eq!(val(r#"1 + "a";"#), json!("1a"));
    assert_eq!(val(r#""a" + 1.5;"#), json!("a1.5"));
    assert_eq!(val(r#""a" == "a";"#), json!(1));
    assert_eq!(val(r#""a" != "a";"#), json!(0));
    assert_eq!(val(r#""a" < "b";"#), json!(1));
    assert_eq!(val("int $a = 1; $a += 2; $a *= 3; $a -= 1; $a /= 2; $a;"), json!(4));
    assert_eq!(val(r#"string $s = "a"; $s += "b"; $s;"#), json!("ab"));
    assert_eq!(val("int $a = 1; $a++; $a;"), json!(2));
    assert_eq!(val("int $a = 1; $a++;"), json!(1));
    assert_eq!(val("int $a = 1; ++$a;"), json!(2));
    assert_eq!(val("int $a = 1; $a--; $a;"), json!(0));
    assert_eq!(val("float $a = 1.5; $a++; $a;"), json!(2.5));
    assert_eq!(val("int $a; int $b; $a = $b = 3; $a + $b;"), json!(6));
    assert_eq!(val("int $i = 2147483647; $i + 1;"), json!(-2147483648));
    // Short-circuit: the right side doesn't run.
    assert_eq!(val("int $a = 0; 0 && ($a = 1); 1 || ($a = 2); $a;"), json!(0));
    assert!(failure("1 / 0;").contains("divide by zero"));
    assert!(failure("1 % 0;").contains("divide by zero"));
    assert!(failure("1.0 / 0;").contains("divide by zero"));
}

#[test]
fn vectors_and_matrices() {
    assert_eq!(val("<<1, 2, 3>> + <<1, 1, 1>>;"), json!([2.0, 3.0, 4.0]));
    assert_eq!(val("<<1, 2, 3>> - <<1, 1, 1>>;"), json!([0.0, 1.0, 2.0]));
    assert_eq!(val("<<1, 2, 3>> * <<4, 5, 6>>;"), json!(32.0));
    assert_eq!(val("<<1, 0, 0>> ^ <<0, 1, 0>>;"), json!([0.0, 0.0, 1.0]));
    assert_eq!(val("<<1, 2, 3>> * 2;"), json!([2.0, 4.0, 6.0]));
    assert_eq!(val("2 * <<1, 2, 3>>;"), json!([2.0, 4.0, 6.0]));
    assert_eq!(val("<<2, 4, 6>> / 2;"), json!([1.0, 2.0, 3.0]));
    assert_eq!(val("-<<1, 2, 3>>;"), json!([-1.0, -2.0, -3.0]));
    assert_eq!(val("vector $v = <<1, 2, 3>>; $v.x + $v.y + $v.z;"), json!(6.0));
    assert_eq!(val("vector $v[]; $v[1] = <<1, 2, 3>>; $v[1].y;"), json!(2.0));
    assert_eq!(val("<<1, 2, 3>> == <<1, 2, 3>>;"), json!(1));
    assert_eq!(val("matrix $m[2][2] = <<1, 2; 3, 4>>; $m[1][0];"), json!(3.0));
    assert_eq!(val("matrix $m[2][2]; $m[0][1] = 5; $m[0][1];"), json!(5.0));
    assert_eq!(val("matrix $a[2][2] = <<1, 2; 3, 4>>; matrix $b[2][2] = <<0, 1; 1, 0>>; matrix $c[2][2] = $a * $b; $c[0][0];"), json!(2.0));
    assert!(failure("matrix $m[2][2]; $m[2][0] = 1;").contains("bounds"));
    assert!(failure("matrix $m[2][2] = <<1, 2, 3; 4, 5, 6>>;").contains("matrix"));
}

#[test]
fn arrays() {
    assert_eq!(val("int $a[]; $a[3] = 7; size($a);"), json!(4));
    assert_eq!(val("int $a[]; $a[3] = 7; $a[1];"), json!(0));
    assert_eq!(val("int $a[] = {1, 2, 3}; $a[1] = 9; $a;"), json!([1, 9, 3]));
    assert_eq!(val(r#"string $a[]; $a[size($a)] = "x"; $a[size($a)] = "y"; $a;"#), json!(["x", "y"]));
    assert_eq!(val("int $a[] = {1}; $a[5];"), json!(0));
    assert_eq!(val("float $a[] = {1.5, 2}; $a[0] + $a[1];"), json!(3.5));
    assert_eq!(val(r#"$x = {1, "a", 2.5}; $x;"#), json!(["1", "a", "2.5"]));
    assert_eq!(val("$x = {1, 2.5}; $x;"), json!([1.0, 2.5]));
    assert_eq!(val("int $a[] = {1, 2}; int $b[] = $a; $b[0] = 9; $a[0];"), json!(1));
    assert!(failure("int $a[]; $a[-1] = 1;").contains("bounds"));
    assert!(failure("int $a[]; $a[-1];").contains("bounds"));
    assert!(failure("int $a[]; $a[2000000000] = 1;").contains("bounds"));
}

#[test]
fn globals_persist() {
    let mut c = Core::new();
    c.mel("global int $g = 5;");
    assert_eq!(c.mel("global int $g; $g;"), json!(5));
    // A global's initialiser runs once.
    assert_eq!(c.mel("global int $g = 9; $g;"), json!(5));
    c.mel("global proc bump() { global int $g; $g++; }");
    c.mel("bump; bump();");
    assert_eq!(c.mel("global int $g; $g;"), json!(7));
    // A procedure sees only what it declares global.
    c.mel("global proc reads() { print $g; }");
    assert!(c.mel_checked("reads;").unwrap_err().contains("undeclared"));
    // A different type is refused.
    assert!(c.mel_checked("global string $g;").unwrap_err().contains("different type"));
    // env lists them.
    assert_eq!(c.mel("stringArrayContains(\"$g\", `env`);"), json!(1));
}

#[test]
fn top_level_variables_persist() {
    let mut c = Core::new();
    c.mel("int $a = 3; string $l[]; $l[size($l)] = \"x\";");
    assert_eq!(c.mel("$a + 1;"), json!(4));
    // Declared again without a value, a variable keeps it: the Script Editor's way.
    assert_eq!(c.mel("string $l[]; $l[size($l)] = \"y\"; size($l);"), json!(2));
    assert_eq!(c.mel("int $a = 8; $a;"), json!(8));
}

#[test]
fn control_flow() {
    assert_eq!(val("int $a = 0; if (1) $a = 1; else $a = 2; $a;"), json!(1));
    assert_eq!(val("int $a = 0; if (0) { $a = 1; } else if (0) { $a = 2; } else { $a = 3; } $a;"), json!(3));
    assert_eq!(val("int $s = 0; for ($i = 0; $i < 5; $i++) $s += $i; $s;"), json!(10));
    assert_eq!(val("int $s = 0; int $i; for ($i = 0; $i < 5; ++$i) { $s += $i; } $i;"), json!(5));
    assert_eq!(val("int $s = 0; for ($i = 0, $j = 10; $i < $j; $i++, $j--) $s++; $s;"), json!(5));
    assert_eq!(val("int $s = 0; for (int $i = 0; $i < 3; $i++) $s++; $s;"), json!(3));
    assert_eq!(val("int $s = 0; for ($i = 0; ; $i++) { if ($i == 4) break; $s++; } $s;"), json!(4));
    assert_eq!(val("int $s = 0; for ($i = 0; $i < 5; $i++) { if ($i == 2) continue; $s += $i; } $s;"), json!(8));
    assert_eq!(val("int $s = 0; for ($x in {1, 2, 3}) $s += $x; $s;"), json!(6));
    assert_eq!(val(r#"string $o = ""; string $a[] = {"p", "q"}; string $e; for ($e in $a) { $o += $e; } $o;"#), json!("pq"));
    assert_eq!(val(r#"string $o = ""; for (string $e in {"p", "q"}) $o += $e; $o;"#), json!("pq"));
    assert_eq!(val("int $a[] = {1, 2, 3}; int $s = 0; for ($x in $a) { if ($x == 2) break; $s += $x; } $s;"), json!(1));
    assert_eq!(val("int $i = 0; while ($i < 3) $i++; $i;"), json!(3));
    assert_eq!(val("int $i = 0; while (1) { if (++$i > 2) break; } $i;"), json!(3));
    assert_eq!(val("int $i = 5; do { $i++; } while ($i < 3); $i;"), json!(6));
    assert_eq!(val("int $i = 0; do $i++; while ($i < 3); $i;"), json!(3));
    let sw = |x: &str| val(&format!(r#"string $o = ""; switch ({x}) {{ case 1: $o += "one"; break; case 2: $o += "two"; case 3: $o += "three"; break; default: $o += "other"; break; }} $o;"#));
    assert_eq!(sw("1"), json!("one"));
    assert_eq!(sw("2"), json!("twothree"));
    assert_eq!(sw("3"), json!("three"));
    assert_eq!(sw("9"), json!("other"));
    assert_eq!(val(r#"int $r = 0; switch ("b") { case "a": $r = 1; break; case "b": $r = 2; break; } $r;"#), json!(2));
    assert_eq!(val("int $r = 0; switch (1) { default: $r = 9; } $r;"), json!(9));
    // continue in a switch belongs to the loop around it.
    assert_eq!(val("int $s = 0; for ($i = 0; $i < 3; $i++) { switch ($i) { case 1: continue; } $s++; } $s;"), json!(2));
    // Blocks scope their declarations.
    assert!(failure("{ int $inner = 1; } $inner;").contains("undeclared"));
    assert_eq!(val("int $a = 1; { int $a = 2; } $a;"), json!(1));
    // A fresh variable each time round.
    assert_eq!(val("int $last = 0; for ($i = 0; $i < 3; $i++) { int $n; $n++; $last = $n; } $last;"), json!(1));
}

#[test]
fn procedures() {
    assert_eq!(val("proc int twice(int $x) { return $x * 2; } twice(4);"), json!(8));
    assert_eq!(val("global proc float half(float $x) { return $x / 2; } half(3);"), json!(1.5));
    assert_eq!(val(r#"global proc string hi(string $n, int $k) { return $n + $k; } hi("a", 1);"#), json!("a1"));
    // Command syntax, with words.
    assert_eq!(val(r#"global proc string hi(string $n, int $k) { return $n + $k; } string $r = `hi abc 3`; $r;"#), json!("abc3"));
    assert_eq!(val(r#"int $g = 0; proc store(int $a, string $b) { print ($b + $a + "\n"); } store 1 "a"; store(2, "b");"#), Value::Null);
    assert_eq!(printed(r#"proc store(int $a, string $b) { print ($b + $a + "\n"); } store 1 "a"; store(2, "b");"#), ["a1", "b2"]);
    // Recursion.
    assert_eq!(val("proc int fact(int $n) { if ($n < 2) return 1; return $n * fact($n - 1); } fact(6);"), json!(720));
    assert_eq!(val("global proc int fib(int $n) { return $n < 2 ? $n : fib($n - 1) + fib($n - 2); } fib(15);"), json!(610));
    // Array arguments and returns.
    assert_eq!(val("proc int[] up(int $n) { int $o[]; for ($i = 0; $i < $n; $i++) $o[$i] = $i; return $o; } up(3);"), json!([0, 1, 2]));
    assert_eq!(val(r#"proc string[] pair() { return {"a", "b"}; } string $r[] = pair(); $r[1];"#), json!("b"));
    assert_eq!(val("proc int total(int $a[]) { int $s = 0; for ($x in $a) $s += $x; return $s; } total({1, 2, 3});"), json!(6));
    // Arrays pass by reference.
    assert_eq!(val("proc fill(int $a[]) { $a[0] = 42; } int $b[] = {1}; fill($b); $b[0];"), json!(42));
    assert_eq!(val("proc set1(int $a) { $a = 42; } int $b = 1; set1($b); $b;"), json!(1));
    // A missing return gives the type's zero.
    assert_eq!(val("proc int nothing() { } nothing();"), json!(0));
    // Locals don't leak; defined after use still resolves in one source.
    assert!(failure("proc f() { int $local = 1; } f(); $local;").contains("undeclared"));
    assert_eq!(val("int $r = later(); global proc int later() { return 3; } $r;"), json!(3));
    assert!(failure("proc f(int $a) { } f();").contains("Wrong number"));
    assert!(failure("proc f(int $a) { } f(1, 2);").contains("Wrong number"));
    assert!(failure("{ proc f() { } }").contains("Syntax"));
}

#[test]
fn runaway_recursion_is_an_error() {
    assert!(failure("proc f() { f(); } f();").contains("nested too deeply"));
    assert!(failure(r#"global proc g() { eval("g"); } g;"#).contains("nested too deeply"));
}

#[test]
fn deep_sources_dont_overflow() {
    let parens = format!("{}1{};", "(".repeat(100000), ")".repeat(100000));
    assert!(failure(&parens).contains("deeply"));
    let nots = format!("{}1;", "!".repeat(100000));
    assert!(failure(&nots).contains("deeply"));
    let sum = format!("1{};", "+1".repeat(100000));
    assert!(failure(&sum).contains("too long"));
    assert_eq!(val(&format!("{}1{};", "(".repeat(50), ")".repeat(50))), json!(1));
}

#[test]
fn command_and_function_syntax() {
    let mut c = Core::new();
    c.mel(r#"polyCube -w 2 -n "box";"#);
    assert_eq!(c.mel("objExists box;"), json!(true));
    assert_eq!(c.mel("getAttr box.tx;"), json!(0.0));
    c.mel("string $r[] = `polyCube -w 3`; string $name = $r[0];");
    assert_eq!(c.mel("$name;"), json!("pCube1"));
    assert_eq!(c.mel("size($r);"), json!(2));
    // Variables and parenthesised expressions as arguments and flag values.
    c.mel(r#"float $w = 5; string $n = "ball"; polySphere -r $w -n $n;"#);
    assert_eq!(c.mel("polySphere -q -r ball;"), json!(5.0));
    c.mel(r#"string $suffix = "tx"; setAttr ("box." + $suffix) 4;"#);
    assert_eq!(c.mel(r#"float $x = `getAttr ("box." + $suffix)`; $x;"#), json!(4.0));
    assert_eq!(c.mel(r#"getAttr("box.tx");"#), json!(4.0));
    c.mel("setAttr box.ty -1;");
    assert_eq!(c.mel(r#"getAttr ("box.ty");"#), json!(-1.0));
    c.mel(r#"setAttr ("box.tz") -2;"#);
    assert_eq!(c.mel("getAttr box.tz;"), json!(-2.0));
    // A call an operator carries on is an expression; a negative number after one is an argument.
    assert_eq!(c.mel("int $k[] = {1, 2}; size($k) + 1;"), json!(3));
    assert_eq!(c.mel("abs(-3)*2;"), json!(6));
    assert_eq!(c.mel("abs(-3) - 1;"), json!(2));
    assert_eq!(c.mel("abs(-3) > 2 ? 10 : 20;"), json!(10));
    assert_eq!(c.mel(r#"eval("size($k) + 2");"#), json!(4));
    assert_eq!(c.mel("int $sum = `abs(-3) + 1`; $sum;"), json!(4));
    // -type names the data; the values follow.
    c.mel("setAttr box.translate -type double3 7 8 9;");
    assert_eq!(c.mel("getAttr box.translate;"), json!([7.0, 8.0, 9.0]));
    c.mel("setAttr box.translate 4 -1 -2;");
    // Function syntax: "-sl" is a flag.
    c.mel("select -r box;");
    assert_eq!(c.mel(r#"ls("-sl");"#), json!(["box"]));
    assert_eq!(c.mel(r#"string $s[] = ls("-sl"); $s[0];"#), json!("box"));
    assert_eq!(c.mel(r#"size(`ls -sl`);"#), json!(1));
    // Arrays spread out into arguments.
    c.mel(r#"string $both[] = {"box", "ball"}; select -r $both;"#);
    assert_eq!(c.mel("size(`ls -sl`);"), json!(2));
    c.mel("vector $p = <<1, 2, 3>>; move $p ball;");
    assert_eq!(c.mel("getAttr ball.translate;"), json!([1.0, 2.0, 3.0]));
    // Nested captures.
    c.mel("select -r box;");
    assert_eq!(c.mel("string $t = `nodeType `ls -sl``; $t;"), json!("transform"));
    assert_eq!(c.mel(r#"string $t = `nodeType (`ls -sl`)`; $t;"#), json!("transform"));
    // Commands split over lines.
    c.mel("polyCube\n    -w 2\n    -n \"tall\"\n;");
    assert_eq!(c.mel("objExists tall;"), json!(true));
    // A quoted "-x" is an argument, not a flag.
    assert!(c.mel_checked(r#"objExists "-sl";"#).is_ok());
    let errs = lines(&mut c, 2);
    assert!(errs.is_empty(), "{errs:?}");
    // Results take MEL's types.
    assert_eq!(c.mel("float $at[] = `getAttr ball.translate`; $at[2];"), json!(3.0));
    assert_eq!(c.mel("int $on = `objExists ball`; $on;"), json!(1));
    assert!(c.mel_checked("string $everything = `ls`;").unwrap_err().contains("Cannot convert"));
}

#[test]
fn query_flags_take_values_only_where_the_reference_does() {
    let mut c = Core::new();
    let src = "polyCube -n box; joint -n joint1 -p 1 2 3;";
    c.mel(src);
    // A switch, then a query flag, then the object.
    assert_eq!(c.mel("xform -q -ws -t joint1;"), json!([1.0, 2.0, 3.0]));
    assert_eq!(c.mel("xform -ws -q -t joint1;"), json!([1.0, 2.0, 3.0]));
    assert_eq!(c.mel("xform -q -ws 1 -t joint1;"), json!([1.0, 2.0, 3.0]));
    assert_eq!(c.mel("xform -q -t -ws joint1;"), json!([1.0, 2.0, 3.0]));
    // Function syntax, the same.
    assert_eq!(c.mel(r#"xform("-q", "-ws", "-t", "joint1");"#), json!([1.0, 2.0, 3.0]));
    // Flags with a value in query mode.
    c.mel("currentTime 10; setKeyframe -at tx -v 1 box; setKeyframe -at ty -v 2 box; currentTime 20; setKeyframe -at tx -v 3 box;");
    assert_eq!(c.mel("keyframe -q -at tx -tc box;"), json!([10.0, 20.0]));
    assert_eq!(c.mel("keyframe -at tx -q -tc box;"), json!([10.0, 20.0]));
    assert_eq!(c.mel("keyframe -q -kc box;"), json!(3));
    assert_eq!(c.mel("keyframe -at ty -q -kc box;"), json!(1));
    assert_eq!(c.mel("keyframe -q -at tx -t 20 -vc box;"), json!([3.0]));
    assert_eq!(lines(&mut c, 2), Vec::<String>::new());
}

fn skinned(c: &mut Core) {
    let src = "select -cl; polyCube -n box; select -cl; joint -n joint1 -p 0 0 0; joint -n joint2 -p 0 1 0; joint -n joint3 -p 0 2 0; select -cl;";
    c.mel_checked(src).unwrap();
    c.mel_checked("skinCluster -tsb -n sk joint1 joint2 box;").unwrap();
}

#[test]
fn multi_use_flags() {
    let mut c = Core::new();
    skinned(&mut c);
    // Each use takes its pair; the words after are positional.
    c.mel_checked("skinPercent -tv joint1 0.25 -tv joint2 0.75 sk box.vtx[0];").unwrap();
    assert_eq!(c.mel("skinPercent -t joint1 -q -v sk box.vtx[0];"), json!(0.25));
    assert_eq!(c.mel("skinPercent -q -t joint2 sk box.vtx[0];"), json!(0.75));
    assert_eq!(c.mel("skinPercent -q -v sk box.vtx[0];"), json!([0.25, 0.75]));
    // Values from variables.
    c.mel_checked(r#"string $j = "joint1"; float $w = 0.5; skinPercent -tv $j $w -tv joint2 0.5 sk ("box.vtx[" + 1 + "]");"#).unwrap();
    assert_eq!(c.mel("skinPercent -q -t joint1 sk box.vtx[1];"), json!(0.5));
    // -ai and -ri, repeated.
    c.mel_checked("skinCluster -e -ai joint3 sk;").unwrap();
    assert_eq!(c.mel("skinCluster -q -inf sk;"), json!(["joint1", "joint2", "joint3"]));
    c.mel_checked("skinCluster -e -ri joint3 -ri joint2 sk;").unwrap();
    assert_eq!(c.mel("skinCluster -q -inf sk;"), json!(["joint1"]));
    // A use short of values is an error, not a crash.
    assert!(c.mel_checked("skinPercent -tv joint1 sk box.vtx[0] -tv joint2;").is_err());
    assert!(c.mel_checked("skinPercent -tv joint1;").is_err());
    // Three values a use.
    let made = c.mel_checked("polyCreateFacet -p 0 0 0 -p 1 0 0 -p 1 1 0;").unwrap();
    let info = c.s.run("meshInfo", json!({"args": [made[0]]})).unwrap();
    assert_eq!((&info["faces"], &info["points"]), (&json!(1), &json!(3)), "{info}");
}

#[test]
fn string_functions() {
    assert_eq!(val(r#"size("hello");"#), json!(5));
    assert_eq!(val(r#"size("");"#), json!(0));
    assert_eq!(val(r#"substring("hello", 2, 4);"#), json!("ell"));
    assert_eq!(val(r#"substring("hello", 4, 99);"#), json!("lo"));
    assert_eq!(val(r#"substring("hello", 4, 2);"#), json!(""));
    assert_eq!(val(r#"string $b[]; int $n = tokenize("a b  c", $b); $n;"#), json!(3));
    assert_eq!(val(r#"string $b[]; tokenize("a b  c", $b); $b;"#), json!(["a", "b", "c"]));
    assert_eq!(val(r#"string $b[]; tokenize("a,b;c", ",;", $b); $b;"#), json!(["a", "b", "c"]));
    assert_eq!(val(r#"string $b[]; tokenize "a.b" "." $b; $b[1];"#), json!("b"));
    assert_eq!(val(r#"match("a*b", "xaaaby");"#), json!("aaab"));
    assert_eq!(val(r#"match("^[0-9]+", "123abc");"#), json!("123"));
    assert_eq!(val(r#"match("[0-9]+$", "abc123");"#), json!("123"));
    assert_eq!(val(r#"match("z", "abc");"#), json!(""));
    assert_eq!(val(r#"match("(ab)+", "xababy");"#), json!("abab"));
    assert_eq!(val(r#"match("a|b", "xb");"#), json!("b"));
    assert_eq!(val(r#"match(".*\\.", "a.b.c");"#), json!("a.b."));
    assert!(failure(r#"match("(", "a");"#).contains("parenthesis"));
    assert_eq!(val(r#"gmatch("hello", "h*o");"#), json!(1));
    assert_eq!(val(r#"gmatch("hello", "h?llo");"#), json!(1));
    assert_eq!(val(r#"gmatch("hello", "[gh]ello");"#), json!(1));
    assert_eq!(val(r#"gmatch("hello", "x*");"#), json!(0));
    assert_eq!(val(r#"gmatch("hello", "*l*l*");"#), json!(1));
    assert_eq!(val(r#"substitute("l+", "hello", "L");"#), json!("heLo"));
    assert_eq!(val(r#"substitute("x", "hello", "L");"#), json!("hello"));
    assert_eq!(val(r#"substituteAllString("a.b.c", ".", "_");"#), json!("a_b_c"));
    assert_eq!(val(r#"strip("  a b \n");"#), json!("a b"));
    assert_eq!(val(r#"toupper("aB");"#), json!("AB"));
    assert_eq!(val(r#"tolower("aB");"#), json!("ab"));
    assert_eq!(val(r#"startsWith("hello", "he");"#), json!(1));
    assert_eq!(val(r#"endsWith("hello", "he");"#), json!(0));
    assert_eq!(val(r#"stringArrayToString({"a", "b"}, ", ");"#), json!("a, b"));
    assert_eq!(val(r#"stringToStringArray("a b", " ");"#), json!(["a", "b"]));
    assert_eq!(val(r#"stringArrayContains("a", {"a", "b"});"#), json!(1));
    assert_eq!(val(r#"stringArrayContains("z", {"a", "b"});"#), json!(0));
    assert_eq!(val(r#"stringArrayRemove({"a"}, {"a", "b", "a"});"#), json!(["b"]));
    assert_eq!(val(r#"stringArrayRemoveDuplicates({"a", "b", "a"});"#), json!(["a", "b"]));
    assert_eq!(val(r#"stringArrayCount("a", {"a", "b", "a"});"#), json!(2));
    assert_eq!(val(r#"startString("hello", 2);"#), json!("he"));
    assert_eq!(val(r#"endString("hello", 2);"#), json!("lo"));
    assert_eq!(val(r#"endString("hi", 9);"#), json!("hi"));
    assert_eq!(val(r#"capitalizeString("hello");"#), json!("Hello"));
    assert_eq!(val(r#"capitalizeString("");"#), json!(""));
    assert_eq!(val(r#"shortNameOf("|a|b|c");"#), json!("c"));
    assert_eq!(val(r#"rootOf("|a|b|c");"#), json!("a"));
    assert_eq!(val(r#"rootOf("a.tx");"#), json!("a"));
    assert_eq!(val(r#"polyCube -n box; group -n grp box; longNameOf("box");"#), json!("|grp|box"));
    assert_eq!(val(r#"longNameOf("nothing");"#), json!(""));
    // Multi-byte text counts by character and doesn't split one.
    assert_eq!(val(r#"size("éa");"#), json!(2));
    assert_eq!(val(r#"substring("éab", 1, 2);"#), json!("éa"));
    assert_eq!(val(r#"endString("aé", 1);"#), json!("é"));
}

#[test]
fn math_functions() {
    let near = |src: &str, want: f64| {
        let v = val(src);
        let f = v.as_f64().unwrap_or_else(|| panic!("{src}: {v}"));
        assert!((f - want).abs() < 1e-9, "{src}: {f}");
    };
    assert_eq!(val("abs(-3);"), json!(3));
    assert_eq!(val("abs(-1.5);"), json!(1.5));
    assert_eq!(val("abs -3;"), json!(3));
    near("sin(0);", 0.0);
    near("cos(0);", 1.0);
    near("tan(0);", 0.0);
    near("asin(1);", std::f64::consts::FRAC_PI_2);
    near("acos(1);", 0.0);
    near("atan(1);", std::f64::consts::FRAC_PI_4);
    near("atan2(1, 1);", std::f64::consts::FRAC_PI_4);
    near("sqrt(16);", 4.0);
    near("pow(2, 10);", 1024.0);
    near("exp(0);", 1.0);
    near("log(1);", 0.0);
    near("floor(1.7);", 1.0);
    near("floor(-1.2);", -2.0);
    near("ceil(1.2);", 2.0);
    near("round(1.5);", 2.0);
    near("trunc(-1.7);", -1.0);
    assert_eq!(val("min(1, 2);"), json!(1));
    assert_eq!(val("max(1, 2);"), json!(2));
    near("min(1.5, 2);", 1.5);
    near("max(1.5, 2);", 2.0);
    near("clamp(0, 1, 5);", 1.0);
    near("clamp(0, 1, -5);", 0.0);
    near("clamp(0, 1, 0.5);", 0.5);
    assert_eq!(val("sign(-4);"), json!(-1));
    near("sign(2.5);", 1.0);
    near("deg_to_rad(180);", std::f64::consts::PI);
    near("rad_to_deg(3.14159265358979);", 180.0);
    near("hypot(3, 4);", 5.0);
    near("mag(<<3, 4, 0>>);", 5.0);
    assert_eq!(val("cross(<<1, 0, 0>>, <<0, 1, 0>>);"), json!([0.0, 0.0, 1.0]));
    near("dot(<<1, 2, 3>>, <<4, 5, 6>>);", 32.0);
    assert_eq!(val("unit(<<0, 3, 0>>);"), json!([0.0, 1.0, 0.0]));
    near("linstep(0, 10, 5);", 0.5);
    near("linstep(0, 10, 20);", 1.0);
    near("smoothstep(0, 10, 5);", 0.5);
    near("smoothstep(0, 10, -1);", 0.0);
    // The same seed gives the same numbers, within the range asked.
    let a = val("seed(7); rand(1);");
    assert_eq!(a, val("seed(7); rand(1);"));
    assert_ne!(a, val("seed(8); rand(1);"));
    let r = val("rand(5, 6);").as_f64().unwrap();
    assert!((5.0..6.0).contains(&r));
    assert!(failure("sqrt();").contains("Wrong number"));
    assert!(failure("sqrt(1, 2);").contains("Wrong number"));
    // Out of range input isn't an error, and doesn't crash printing.
    assert_eq!(printed(r#"print (sqrt(-1) + "\n");"#), ["nan"]);
}

#[test]
fn array_functions() {
    assert_eq!(val("sort({3, 1, 2});"), json!([1, 2, 3]));
    assert_eq!(val("sort({2.5, 1.5});"), json!([1.5, 2.5]));
    assert_eq!(val(r#"sort({"b", "a"});"#), json!(["a", "b"]));
    assert_eq!(val("int $a[] = {1, 2}; clear($a); size($a);"), json!(0));
    assert_eq!(val("int $a[] = {1, 2}; clear $a; size($a);"), json!(0));
    assert_eq!(val(r#"string $a[] = {"x"}; appendStringArray($a, {"y", "z", "w"}, 2); $a;"#), json!(["x", "y", "z"]));
    assert_eq!(val("intArrayFind(2, 0, {1, 2, 2});"), json!(1));
    assert_eq!(val("intArrayFind(2, 2, {1, 2, 2});"), json!(2));
    assert_eq!(val("intArrayFind(9, 0, {1});"), json!(-1));
    assert_eq!(val("floatArrayFind(1.5, 0, {1.0, 1.5});"), json!(1));
    assert_eq!(val(r#"stringArrayFind("b", 0, {"a", "b"});"#), json!(1));
    assert_eq!(val(r#"string $a[] = {"a", "c"}; stringArrayInsertAtIndex(1, $a, "b"); $a;"#), json!(["a", "b", "c"]));
}

#[test]
fn errors_stop_the_run() {
    let mut c = Core::new();
    assert_eq!(c.mel(r#"polyCube; error "boom"; polySphere;"#), Value::Null);
    let log = c.take_log();
    let log = log.as_array().unwrap();
    let errs: Vec<&str> = log.iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap()).collect();
    assert_eq!(errs, ["// Error: boom //"]);
    assert_eq!(c.mel("objExists pCube1;"), json!(true));
    assert_eq!(c.mel("objExists pSphere1;"), json!(false));
    // An error inside a procedure ends everything above it.
    let mut c = Core::new();
    c.mel(r#"proc f() { error "x"; print "after\n"; } f(); print "later\n";"#);
    assert!(lines(&mut c, 3).is_empty());
    // The next run is fine, and the variables stand.
    let mut c = Core::new();
    c.mel("proc f() { int $here = 1; error \"x\"; } f();");
    assert_eq!(c.mel("1 + 1;"), json!(2));
    assert!(c.mel_checked("$here;").is_err());
    // A command's own failure, and an unknown name.
    let mut c = Core::new();
    assert!(c.mel_checked("polyCube -nosuchflag;").unwrap_err().contains("invalid flag"));
    assert!(c.mel_checked("nosuchcommand 1 2;").unwrap_err().contains("Cannot find procedure"));
    assert!(c.mel_checked("nosuch(1);").unwrap_err().contains("Cannot find procedure"));
    // A syntax error means nothing of the source runs.
    let mut c = Core::new();
    let e = c.mel_checked("polyCube;\nint $x = ;\npolySphere;").unwrap_err();
    assert_eq!(e, "line 2: Syntax error");
    assert_eq!(c.mel("objExists pCube1;"), json!(false));
    for bad in ["if (1", "for (;;", "int $;", "\"open", "/* open", "{", "}", "<<1,", "`ls", "proc", "switch (1) { 3 }", "$a[", "1 +", "1 = 2;", "else;", "$", "@"] {
        assert!(Core::new().mel_checked(bad).is_err(), "{bad}");
    }
}

#[test]
fn catch_and_warnings() {
    let mut c = Core::new();
    assert_eq!(c.mel(r#"catch(`error "x"`);"#), json!(1));
    assert_eq!(lines(&mut c, 2), ["// Error: x //"]);
    assert_eq!(c.mel(r#"catchQuiet(`error "x"`);"#), json!(1));
    assert!(lines(&mut c, 2).is_empty());
    assert_eq!(c.mel("catch(1 + 1);"), json!(0));
    assert_eq!(c.mel(r#"catchQuiet(eval("nosuch"));"#), json!(1));
    assert_eq!(c.mel("catchQuiet(`nosuch`);"), json!(1));
    assert_eq!(c.mel("catchQuiet(1 / 0);"), json!(1));
    // Command syntax, and after a catch the run goes on.
    assert_eq!(c.mel(r#"int $bad = catchQuiet(`polyCube -nosuch`); $bad + 1;"#), json!(2));
    c.mel(r#"catchQuiet `error "x"`; print "on\n";"#);
    assert_eq!(lines(&mut c, 3), ["on"]);
    // In an if, the usual way to write it.
    assert_eq!(c.mel(r#"int $r = 0; if (catchQuiet(`error "x"`)) $r = 5; $r;"#), json!(5));
    // A catch doesn't take a return in the expression it runs for an error.
    assert_eq!(c.mel(r#"proc int f() { catchQuiet(`error "x"`); return 3; } f();"#), json!(3));
    c.take_log();
    c.mel(r#"warning "careful";"#);
    assert_eq!(lines(&mut c, 4), ["// Warning: careful //"]);
    c.mel(r#"warning -noContext "careful";"#);
    assert_eq!(lines(&mut c, 4), ["// Warning: careful //"]);
    assert!(c.mel_checked(r#"error -noContext "stop";"#).unwrap_err().contains("stop"));
}

#[test]
fn eval_and_source() {
    assert_eq!(val(r#"eval("1 + 2");"#), json!(3));
    assert_eq!(val(r#"int $a = 2; eval("$a * 5");"#), json!(10));
    assert_eq!(val(r#"eval("polyCube -n box"); objExists("box");"#), json!(true));
    assert_eq!(val(r#"string $r[] = eval("polyCube -n box"); $r[0];"#), json!("box"));
    assert_eq!(val(r#"eval "polyCube -n" "box"; objExists box;"#), json!(true));
    assert_eq!(val(r#"eval("global proc int fromEval() { return 9; }"); fromEval();"#), json!(9));
    assert!(failure(r#"eval("int $x = ;");"#).contains("Syntax"));
    // eval runs at the top level: a procedure's locals aren't there, but the error comes back to it.
    assert!(failure(r#"proc f() { int $l = 1; eval("$l"); } f();"#).contains("undeclared"));
    assert_eq!(val(r#"proc int f() { return catchQuiet(eval("error \"x\"")); } f();"#), json!(1));
    let mut c = Core::new();
    assert_eq!(c.mel(r#"evalDeferred("polyCube -n later"); objExists later;"#), json!(false));
    assert_eq!(c.mel("objExists later;"), json!(true));

    let dir = std::env::temp_dir().join(format!("aztek_mel_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("lib.mel");
    std::fs::write(&file, "proc int helper() { return 4; }\nglobal proc int fromFile() { return helper() + 1; }\nint $fileLocal = 1;\nglobal int $fromFileGlobal = 11;\n").unwrap();
    let path = file.to_str().unwrap().replace('\\', "/");
    let mut c = Core::new();
    c.mel_checked(&format!(r#"source "{path}";"#)).unwrap();
    assert_eq!(c.mel("fromFile();"), json!(5));
    // The file's local procedure and variables stay with it.
    assert!(c.mel_checked("helper();").unwrap_err().contains("Cannot find procedure"));
    assert!(c.mel_checked("$fileLocal;").is_err());
    assert_eq!(c.mel("global int $fromFileGlobal; $fromFileGlobal;"), json!(11));
    assert!(c.mel(r#"whatIs "fromFile";"#).as_str().unwrap().starts_with("Mel procedure found in:"));
    assert!(c.mel_checked(r#"source "no/such/file.mel";"#).unwrap_err().contains("no/such/file.mel"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn introspection() {
    let mut c = Core::new();
    assert_eq!(c.mel(r#"whatIs "polyCube";"#), json!("Command"));
    assert_eq!(c.mel(r#"whatIs "nothingAtAll";"#), json!("Unknown"));
    assert_eq!(c.mel(r#"whatIs "GraphEditor";"#), json!("Run Time Command"));
    c.mel("global proc gp() { }");
    assert_eq!(c.mel(r#"whatIs "gp";"#), json!("Mel procedure entered interactively."));
    c.mel("int $n; string $l[];");
    assert_eq!(c.mel(r#"whatIs "$n";"#), json!("int variable"));
    assert_eq!(c.mel(r#"whatIs "$l";"#), json!("string[] variable"));
    assert_eq!(c.mel(r#"whatIs "$none";"#), json!("Unknown"));
    assert_eq!(c.mel(r#"exists "polyCube";"#), json!(1));
    assert_eq!(c.mel(r#"exists "polyExtrudeFacet";"#), json!(1));
    assert_eq!(c.mel(r#"exists "gp";"#), json!(1));
    assert_eq!(c.mel(r#"exists "size";"#), json!(1));
    assert_eq!(c.mel(r#"exists "nothingAtAll";"#), json!(0));
    assert_eq!(c.mel(r#"exists nothingAtAll;"#), json!(0));
    assert!(c.mel("about -v;").as_str().is_some());
    assert_eq!(c.mel(r#"about("-batch");"#), json!(0));
    assert_eq!(c.mel("about -linux;"), json!(cfg!(target_os = "linux") as i32));
    assert!(c.mel_checked("about -nosuch;").is_err());
    c.mel("polyCube -n box;");
    assert_eq!(c.mel("objExists box;"), json!(true));
    assert_eq!(c.mel("nodeType box;"), json!("transform"));
    assert_eq!(c.mel(r#"nodeType("polyCube1");"#), json!("polyCube"));
    c.mel(r#"putenv("AZTEK_MEL_TEST", "yes");"#);
    assert_eq!(c.mel(r#"getenv("AZTEK_MEL_TEST");"#), json!("yes"));
    assert_eq!(c.mel(r#"getenv("AZTEK_MEL_NOT_SET");"#), json!(""));
    assert!(c.mel_checked(r#"putenv("", "x");"#).is_err());
    assert_eq!(c.mel("currentTime 12; currentTime -q;"), json!(12));
}

#[test]
fn echo_and_results() {
    let mut c = Core::new();
    c.mel("polyCube -w 2; int $a = 1 + 1; print \"hi\\n\"; int $b;\nfor ($i = 0; $i < 2; $i++) polySphere;");
    let log = c.take_log();
    let log: Vec<(i64, &str)> = log.as_array().unwrap().iter().map(|l| (l["kind"].as_i64().unwrap(), l["text"].as_str().unwrap())).collect();
    assert_eq!(
        log,
        [
            (0, "polyCube -w 2;"),
            (1, "// Result: pCube1 polyCube1 //"),
            (0, "int $a = 1 + 1;"),
            (1, "// Result: 2 //"),
            (0, "print \"hi\\n\";"),
            (3, "hi"),
            (0, "int $b;"),
            (0, "for ($i = 0; $i < 2; $i++) polySphere;"),
        ]
    );
    // print: no newline, no new line; arrays one a line; floats as the reference prints them.
    assert_eq!(printed(r#"print "a"; print "b\n"; print "c";"#), ["a", "b", "c"]);
    assert_eq!(printed(r#"{ print "a"; print "b\n"; print "c"; }"#), ["ab", "c"]);
    assert_eq!(printed(r#"print {"x", "y"};"#), ["x", "y"]);
    assert_eq!(printed("print 1.5;"), ["1.5"]);
    assert_eq!(printed("print (1.0 / 3);"), ["0.333333"]);
    assert_eq!(printed("print 2.0;"), ["2"]);
    assert_eq!(printed("print 1e-7;"), ["1e-07"]);
    assert_eq!(printed("print <<1, 2.5, 3>>;"), ["1 2.5 3"]);
    assert_eq!(printed(r#"print ("a" + 1 + "\n");"#), ["a1"]);
    assert_eq!(printed("print abc;"), ["abc"]);
    assert_eq!(printed("string $l[] = {\"m\"}; print $l[0];"), ["m"]);
    assert_eq!(printed("vector $v = <<4, 5, 6>>; print $v.y;"), ["5"]);
    // A string result isn't quoted.
    let mut c = Core::new();
    c.mel(r#""a b";"#);
    assert_eq!(lines(&mut c, 1), ["// Result: a b //"]);
}

#[test]
fn typical_script() {
    let src = r#"
        // Builds a row of cubes, named, spaced and keyed, then reports.
        global int $gBuilt = 0;

        global proc string[] buildRow(string $prefix, int $count, float $gap)
        {
            global int $gBuilt;
            string $made[];
            for ($i = 0; $i < $count; $i++) {
                string $name = $prefix + "_" + ($i + 1);
                string $r[] = `polyCube -w 1 -n $name`;
                move -r ($i * $gap) 0 0 $r[0];
                setKeyframe -at tx -t 1 $r[0];
                $made[size($made)] = $r[0];
                $gBuilt++;
            }
            return $made;
        }

        proc int countMatching(string $items[], string $pattern)
        {
            int $n = 0;
            for ($each in $items)
                if (gmatch($each, $pattern)) $n++;
            return $n;
        }

        string $row[] = buildRow("box", 3, 2.5);
        print (stringArrayToString($row, ",") + "\n");
        float $x = `getAttr ($row[2] + ".tx")`;
        print ("last at " + $x + "\n");
        print ("matching: " + countMatching($row, "box_*") + "\n");
        global int $gBuilt;
        print ("built: " + $gBuilt + "\n");
        string $buf[];
        tokenize($row[1], "_", $buf);
        print ($buf[1] + "\n");
        switch (`nodeType $row[0]`) {
            case "transform": print "a transform\n"; break;
            default: print "something else\n";
        }
    "#;
    let mut c = Core::new();
    c.mel_checked(src).unwrap();
    assert_eq!(lines(&mut c, 3), ["box_1,box_2,box_3", "last at 5", "matching: 3", "built: 3", "2", "a transform"]);
}

#[cfg(feature = "python")]
#[test]
fn python_from_mel() {
    let mut c = Core::new();
    assert_eq!(c.mel(r#"python("1 + 1");"#), json!(2));
    assert_eq!(c.mel(r#"string $r[] = python("['a', 'b']"); $r[1];"#), json!("b"));
    // Statements give nothing back, and keep their names for later.
    assert_eq!(c.mel(r#"python("aztek_test_x = 5"); python("aztek_test_x * 2");"#), json!(10));
    // Python's commands act on this Core, and its mel.eval shares the variables.
    c.mel(r#"python("import maya.cmds as cmds; cmds.polyCube(n='fromPy')");"#);
    assert_eq!(c.mel("objExists fromPy;"), json!(true));
    c.mel("global int $shared = 21;");
    assert_eq!(c.mel(r#"python("import maya.mel as mel"); python("mel.eval('global int $shared; $shared * 2')");"#), json!(42));
    // print goes to the history.
    c.take_log();
    c.mel(r#"python("print('from python')");"#);
    assert_eq!(lines(&mut c, 3), ["from python"]);
    // An exception is a MEL error, catchable.
    assert!(c.mel_checked(r#"python("1 / 0");"#).unwrap_err().contains("ZeroDivisionError"));
    assert_eq!(c.mel(r#"catchQuiet(python("raise ValueError('x')"));"#), json!(1));
    // A MEL error raises in Python.
    assert_eq!(c.mel(r#"python("import maya.mel as mel\ntry:\n    mel.eval('error \"no\"')\n    aztek_test_y = 0\nexcept RuntimeError:\n    aztek_test_y = 1"); python("aztek_test_y");"#), json!(1));
}

#[cfg(not(feature = "python"))]
#[test]
fn python_absent() {
    assert!(failure(r#"python("1");"#).contains("no Python"));
}
