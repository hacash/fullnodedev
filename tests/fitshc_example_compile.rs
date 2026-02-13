#[cfg(feature = "vm")]
mod fitshc_example_compile {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn compile_example_fitsh_via_fitshc() {
        // Use the repository's example as the canonical source, but compile in a temp dir
        // so tests don't modify tracked files.
        let repo_example = PathBuf::from("vm/doc/example.fitsh");
        let src = fs::read_to_string(&repo_example)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", repo_example.display(), e));

        let mut dir = std::env::temp_dir();
        dir.push("hacash_fitshc_tests");
        let _ = fs::create_dir_all(&dir);

        let tmp_fitsh = dir.join("example.fitsh");
        fs::write(&tmp_fitsh, src)
            .unwrap_or_else(|e| panic!("failed to write {}: {}", tmp_fitsh.display(), e));

        // fitshc writes outputs next to the input file.
        let expected_map = dir.join("example.contractmap.json");
        let expected_deploy = dir.join("example.deploy.json");
        let _ = fs::remove_file(&expected_map);
        let _ = fs::remove_file(&expected_deploy);

        let exe = PathBuf::from(env!("CARGO_BIN_EXE_fitshc"));
        let out = Command::new(&exe)
            .arg(&tmp_fitsh)
            .output()
            .unwrap_or_else(|e| panic!("failed to run {}: {}", exe.display(), e));

        if !out.status.success() {
            panic!(
                "fitshc failed (status={})\n---- stdout ----\n{}\n---- stderr ----\n{}\n",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr),
            );
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("Compile success!"), "stdout was: {}", stdout);
        assert!(expected_map.exists(), "missing {}", expected_map.display());
        assert!(expected_deploy.exists(), "missing {}", expected_deploy.display());
    }
}
