//! aztekpy's -I, Python's isolated mode, in a process of its own: it changes the current folder and PYTHONPATH.

/// -I keeps the current folder off the path for -m, as Python's isolated mode does, and PYTHONPATH's folders with it,
/// an empty entry (the current folder) included.
#[test]
fn aztekpy_isolated_mode_leaves_the_current_folder_off_the_path() {
    let tmp = std::env::temp_dir().join(format!("aztekpy_iso_{}", std::process::id()));
    let pkgs = tmp.join("site-packages");
    let extra = tmp.join("from_pythonpath");
    std::fs::create_dir_all(&pkgs).unwrap();
    std::fs::create_dir_all(&extra).unwrap();
    std::env::set_var("AZTEK_PYTHON_PACKAGES", &pkgs);
    std::env::set_var("PYTHONPATH", format!("{};", extra.display()));
    let out = tmp.join("out.txt").to_string_lossy().replace('\\', "/");
    let extra = extra.to_string_lossy().replace('\\', "/");
    let probe = format!(
        "import os, sys\n\
         paths = [os.path.normcase(os.path.abspath(p)) for p in sys.path if p]\n\
         seen = [os.path.normcase(os.path.abspath(p)) in paths for p in (os.getcwd(), {extra:?})]\n\
         open({out:?}, 'w').write(repr(seen + ['' in sys.path]))\n"
    );
    std::fs::write(pkgs.join("aztek_iso_probe.py"), probe).unwrap();
    std::env::set_current_dir(&tmp).unwrap();
    let args: Vec<std::ffi::CString> =
        ["aztekpy", "-I", "-m", "aztek_iso_probe"].iter().map(|a| std::ffi::CString::new(*a).unwrap()).collect();
    let ptrs: Vec<*const std::ffi::c_char> = args.iter().map(|a| a.as_ptr()).collect();
    assert_eq!(aztek_host::aztek_python_main(ptrs.len() as i32, ptrs.as_ptr()), 0);
    // The current folder, PYTHONPATH's folder, and "".
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "[False, False, False]");
}
