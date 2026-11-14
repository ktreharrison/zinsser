//! `zinsser --version` and `-V` print the package version.

use std::process::Command;

#[test]
fn version_flags_print_the_package_version() {
    for flag in ["--version", "-V"] {
        let out = Command::new(env!("CARGO_BIN_EXE_zinsser"))
            .arg(flag)
            .output()
            .unwrap();
        assert!(out.status.success(), "{flag} failed: {out:?}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            format!("zinsser {}", env!("CARGO_PKG_VERSION")),
            "{flag}"
        );
    }
}
