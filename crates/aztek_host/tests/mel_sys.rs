//! MEL's file, system, date and small helper functions.

use aztek_host::Core;
use serde_json::{json, Value};

fn num(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().flat_map(num).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap()],
        _ => Vec::new(),
    }
}

#[test]
fn files_are_written_and_read_back() {
    let dir = std::env::temp_dir().join(format!("aztek_mel_sys_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("out.txt").to_string_lossy().replace('\\', "/");
    let mut c = Core::new();
    c.mel(&format!("global int $f; $f = `fopen \"{file}\" \"w\"`;"));
    assert_ne!(c.mel("global int $f; $f;"), json!(0));
    c.mel("global int $f; fprint $f \"first line\\n\"; fprint $f 12; fprint $f \" words here\\n\"; fprint $f 2.5; fclose $f;");
    assert_eq!(std::fs::read_to_string(dir.join("out.txt")).unwrap(), "first line\n12 words here\n2.5");
    c.mel(&format!("global int $f; $f = fopen(\"{file}\", \"a\"); fprint($f, \"\\nmore\"); fclose($f);"));
    c.mel(&format!("global int $f; $f = fopen(\"{file}\", \"r\");"));
    assert_eq!(c.mel("global int $f; fgetline $f;"), json!("first line\n"));
    assert_eq!(c.mel("global int $f; feof $f;"), json!(0));
    assert_eq!(c.mel("global int $f; fgetword $f;"), json!("12"));
    assert_eq!(c.mel("global int $f; fgetword $f;"), json!("words"));
    assert_eq!(c.mel("global int $f; fgetline $f;"), json!(" here\n"));
    assert_eq!(c.mel("global int $f; fgetline $f;"), json!("2.5\n"));
    assert_eq!(c.mel("global int $f; fgetline $f;"), json!("more"));
    assert_eq!(c.mel("global int $f; feof $f;"), json!(1));
    assert_eq!(c.mel("global int $f; fgetline $f;"), json!(""));
    c.mel("global int $f; frewind $f;");
    assert_eq!(c.mel("global int $f; fgetword($f, \"\\n\");"), json!("first line"));
    c.mel("global int $f; fclose $f;");
    // A file that can't be opened is 0, not an error.
    assert_eq!(c.mel(&format!("fopen(\"{}/none/there.txt\", \"r\");", dir.to_string_lossy().replace('\\', "/"))), json!(0));
    // Binary strings come back.
    c.mel(&format!("global int $f; $f = fopen(\"{file}\", \"w\"); fwrite $f \"abc\"; fwrite $f 7; fclose $f; $f = fopen(\"{file}\", \"r\");"));
    assert_eq!(c.mel("global int $f; string $s; fread($f, $s);"), json!("abc"));
    assert_eq!(c.mel("global int $f; int $i; fread($f, $i);"), json!(7));
    c.mel("global int $f; fclose $f;");

    assert_eq!(c.mel(&format!("filetest -f \"{file}\";")), json!(1));
    assert_eq!(c.mel(&format!("filetest -d \"{file}\";")), json!(0));
    assert_eq!(c.mel(&format!("filetest -s \"{file}\";")), json!(1));
    assert_eq!(c.mel(&format!("filetest -d \"{}\";", dir.to_string_lossy().replace('\\', "/"))), json!(1));
    assert_eq!(c.mel("filetest -e \"/no/such/file/anywhere\";"), json!(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_shell_dates_and_helpers() {
    let mut c = Core::new();
    let said = c.mel("system(\"echo hello\");");
    assert_eq!(said.as_str().unwrap().trim(), "hello");
    c.mel("global int $p; $p = popen(\"echo one two\", \"r\");");
    assert_eq!(c.mel("global int $p; fgetword $p;"), json!("one"));
    c.mel("global int $p; pclose $p;");

    let d = c.mel("date;");
    let d = d.as_str().unwrap();
    assert!(d.len() == 19 && &d[4..5] == "/" && &d[10..11] == " " && &d[13..14] == ":", "{d}");
    let d = c.mel("date -format \"YYYY-MM-DD hh.mm.ss YY\";");
    let d = d.as_str().unwrap();
    assert!(d.len() == 22 && &d[4..5] == "-" && &d[13..14] == "." && d[..2] == *"20", "{d}");
    assert_eq!(c.mel("size(`date -time`);"), json!(8));
    assert_eq!(c.mel("float $t = `timerX`; float $e = `timerX -startTime $t`; ($e >= 0 && $e < 5);"), json!(1));

    assert_eq!(c.mel("fileExtension(\"/a/b.tar.gz\");"), json!("gz"));
    assert_eq!(c.mel("fileExtension(\"/a.b/c\");"), json!(""));
    assert_eq!(c.mel("basenameEx(\"/a/b/scene.v2.ma\");"), json!("scene.v2"));
    assert_eq!(c.mel("format -stringArg \"cube\" -stringArg \"3\" \"^2s of ^1s, ^1s\";"), json!("3 of cube, cube"));
    assert_eq!(c.mel("string $a[] = {\"a\", \"b\", \"c\"}; stringArrayRemoveAtIndex(1, $a); $a;"), json!(["a", "c"]));
    assert_eq!(c.mel("stringAddPrefix({\"a\", \"b\"}, \"L_\");"), json!(["L_a", "L_b"]));
    assert_eq!(c.mel("stringRemovePrefix(\"L_arm\", \"L_\");"), json!("arm"));
    assert_eq!(c.mel("attributeExists(\"translateX\", \"persp\");"), json!(1));
    assert_eq!(c.mel("attributeExists(\"notThere\", \"persp\");"), json!(0));
    assert_eq!(c.mel("attributeExists(\"tx\", \"noSuchNode\");"), json!(0));
    assert_eq!(c.mel("exists fopen;"), json!(1));
    assert_eq!(c.mel("whatIs \"system\";"), json!("Command"));
    c.mel("rehash; trace \"a trace line\";");

    // hermite meets its ends, rot turns X to Y about Z, noise is zero at whole numbers and bounded, the random ones are bounded.
    assert_eq!(num(&c.mel("hermite(1.0, 3.0, 0.0, 0.0, 0.0);")), vec![1.0]);
    assert_eq!(num(&c.mel("hermite(1.0, 3.0, 0.0, 0.0, 1.0);")), vec![3.0]);
    assert_eq!(num(&c.mel("hermite(1.0, 3.0, 0.0, 0.0, 0.5);")), vec![2.0]);
    let r = num(&c.mel("rot(<<1, 0, 0>>, <<0, 0, 1>>, 1.5707963267948966);"));
    assert!(r[0].abs() < 1e-9 && (r[1] - 1.0).abs() < 1e-9 && r[2].abs() < 1e-9, "{r:?}");
    assert_eq!(num(&c.mel("noise(3.0);")), vec![0.0]);
    for i in 0..50 {
        let v = num(&c.mel(&format!("noise(<<{}, 0.37, 1.9>>);", i as f64 * 0.173)))[0];
        assert!((-1.0..=1.0).contains(&v), "{v}");
        let s = num(&c.mel("sphrand(2.0);"));
        assert!(s.iter().map(|x| x * x).sum::<f64>() <= 4.0 + 1e-9, "{s:?}");
    }
    assert_eq!(num(&c.mel("gauss(<<1, 1, 1>>);")).len(), 3);
    assert_eq!(num(&c.mel("gauss(0.0);")), vec![0.0]);
}
