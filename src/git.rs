use fntools::value::Apply;

pub fn cur_commit() -> String {
    let dir = std::env::var("CG_REPO");
    let dir = dir.as_deref().unwrap_or(".");

    std::process::Command::new("git")
        .current_dir(dir)
        .args(&["add", "-N", dir])
        .status()
        .expect("Failed to run `git add -N <path>`")
        .apply(|status| assert!(status.success()));

    let commit = std::process::Command::new("git")
        .current_dir(dir)
        .args(&["log", "-1", "--pretty=format:%h"])
        .output()
        .expect("Failed to read last commit")
        .stdout
        .apply(String::from_utf8)
        .unwrap();

    let changes = std::process::Command::new("git")
        .current_dir(dir)
        .args(&["diff", "--quiet", "--exit-code"])
        .status()
        .expect("Failed to read last commit")
        .code()
        .unwrap()
        == 1
        || std::process::Command::new("git")
            .current_dir(dir)
            .args(&["diff", "staged", "--quiet", "--exit-code"])
            .status()
            .expect("Failed to read last commit")
            .code()
            .unwrap()
            == 1;

    kiam::when! {
        changes => commit + " + local changes",
        _ => commit,
    }
}
