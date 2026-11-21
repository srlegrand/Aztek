//! aztekpy takes Python's interpreter options before -c or -m, as ensurepip passes them when it runs pip.

/// -W sets the warning filter; the others are taken and the command still runs. A bad option is status 2.
#[test]
fn aztekpy_takes_pythons_interpreter_options() {
    let tmp = std::env::temp_dir().join(format!("aztekpy_opts_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let out = tmp.join("out.txt").to_string_lossy().replace('\\', "/");
    let src = format!(
        "import sys, warnings\n\
         open({out:?}, 'w').write(repr((warnings.filters[0][0], warnings.filters[0][2].__name__, sys.argv)))\n\
         sys.exit(3)"
    );
    let run = |argv: &[&str]| {
        let args: Vec<std::ffi::CString> = argv.iter().map(|a| std::ffi::CString::new(*a).unwrap()).collect();
        let ptrs: Vec<*const std::ffi::c_char> = args.iter().map(|a| a.as_ptr()).collect();
        aztek_host::aztek_python_main(ptrs.len() as i32, ptrs.as_ptr())
    };
    assert_eq!(run(&["aztekpy", "-W", "ignore::DeprecationWarning", "-u", "-Es", "-X", "utf8", "-c", &src, "a"]), 3);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "('ignore', 'DeprecationWarning', ['-c', 'a'])");
    assert_eq!(run(&["aztekpy", "-Ic", &src]), 3);
    assert_eq!(run(&["aztekpy", "-W"]), 2);
    assert_eq!(run(&["aztekpy", "-Z", "-c", "pass"]), 2);
    let _ = std::fs::remove_dir_all(&tmp);
}
