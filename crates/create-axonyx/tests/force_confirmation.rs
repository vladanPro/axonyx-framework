use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn declining_force_preserves_existing_project() {
    let root = std::env::temp_dir().join(format!(
        "axonyx-create-force-confirm-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    let target = root.join("existing-site");
    fs::create_dir_all(&target).expect("target should exist");
    fs::write(target.join("important.txt"), "keep me").expect("sentinel should write");

    let mut child = Command::new(env!("CARGO_BIN_EXE_create-axonyx"))
        .arg(&target)
        .arg("--force")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("generator should start");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(b"n\n")
        .expect("decline should write");
    let output = child.wait_with_output().expect("generator should finish");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Canceled."));
    assert_eq!(
        fs::read_to_string(target.join("important.txt")).expect("sentinel should survive"),
        "keep me"
    );
    fs::remove_dir_all(root).expect("temp dir should clean up");
}

#[test]
fn force_replaces_project_only_after_real_scaffold_succeeds() {
    let root = std::env::temp_dir().join(format!(
        "axonyx-create-force-success-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos()
    ));
    let target = root.join("existing-site");
    fs::create_dir_all(&target).expect("target should exist");
    fs::write(target.join("important.txt"), "old content").expect("sentinel should write");

    let output = Command::new(env!("CARGO_BIN_EXE_create-axonyx"))
        .arg(&target)
        .args(["--force", "--yes", "--git", "--template", "minimal"])
        .output()
        .expect("generator should run");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!target.join("important.txt").exists());
    assert!(target.join("Cargo.toml").exists());
    assert!(target.join("app/page.asx").exists());
    assert!(target.join(".git").exists());
    assert_eq!(fs::read_dir(&root).expect("root should list").count(), 1);
    fs::remove_dir_all(root).expect("temp dir should clean up");
}
