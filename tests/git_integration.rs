use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;

use sourcery::git_handler::SourceRepository;

static SETUP: Once = Once::new();

fn test_repo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_fixtures/test_repo")
}

fn setup_test_repo() {
    SETUP.call_once(|| {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/setup_test_repo.sh");
        let status = Command::new("bash")
            .arg(&script)
            .status()
            .expect("failed to run setup script");
        assert!(status.success(), "setup_test_repo.sh failed");
    });
}

#[test]
fn test_open_local_repo() {
    setup_test_repo();
    let repo = SourceRepository::from_path(test_repo_path()).expect("failed to open test repo");
    assert_eq!(repo.dest_dir, test_repo_path());
}

#[test]
fn test_iterate_commits() {
    setup_test_repo();
    let repo = SourceRepository::from_path(test_repo_path()).expect("failed to open test repo");

    let oids: Vec<_> = repo
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("revwalk failed");

    assert_eq!(oids.len(), 3, "expected 3 commits, got {}", oids.len());
}

#[test]
fn test_checkout_commit() {
    setup_test_repo();
    // Use the pre-made copy so checkout mutations don't interfere with other tests
    let repo_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_fixtures/test_repo_checkout");
    let repo = SourceRepository::from_path(repo_path.clone()).expect("failed to open test repo");

    let oids: Vec<_> = repo
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("revwalk failed");

    // Checkout the first commit — only hello.rs should exist, no src/ directory
    repo.checkout_commit(&oids[0]).expect("checkout failed");
    assert!(repo_path.join("hello.rs").exists());
    assert!(
        !repo_path.join("src/lib.rs").exists(),
        "src/lib.rs should not exist at the first commit"
    );

    // Checkout the second commit — src/lib.rs should now exist
    repo.checkout_commit(&oids[1]).expect("checkout failed");
    assert!(repo_path.join("src/lib.rs").exists());

    // Checkout the third (latest) commit — hello.rs should contain "add"
    repo.checkout_commit(&oids[2]).expect("checkout failed");
    let contents = std::fs::read_to_string(repo_path.join("hello.rs")).unwrap();
    assert!(
        contents.contains("add"),
        "hello.rs at third commit should contain 'add'"
    );
}
