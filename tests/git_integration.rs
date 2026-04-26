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

#[test]
fn test_changed_files_between_commits() {
    setup_test_repo();
    let repo = SourceRepository::from_path(test_repo_path()).expect("failed to open test repo");

    let oids: Vec<_> = repo
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("revwalk failed");

    let second_commit_changes = repo
        .commit_diff(Some(&oids[0]), &oids[1])
        .expect("failed to diff first and second commit")
        .files()
        .to_vec();
    assert_eq!(second_commit_changes, vec![PathBuf::from("src/lib.rs")]);

    let third_commit_changes = repo
        .commit_diff(Some(&oids[1]), &oids[2])
        .expect("failed to diff second and third commit")
        .files()
        .to_vec();
    assert_eq!(third_commit_changes, vec![PathBuf::from("hello.rs")]);
}

#[test]
fn test_files_in_commit() {
    setup_test_repo();
    let repo = SourceRepository::from_path(test_repo_path()).expect("failed to open test repo");

    let oids: Vec<_> = repo
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("revwalk failed");

    let first_commit_files = repo
        .commit_diff(None, &oids[0])
        .expect("failed to list files in first commit")
        .files()
        .to_vec();
    assert_eq!(first_commit_files, vec![PathBuf::from("hello.rs")]);

    let second_commit_files = repo
        .commit_diff(None, &oids[1])
        .expect("failed to list files in second commit")
        .files()
        .to_vec();
    assert_eq!(
        second_commit_files,
        vec![PathBuf::from("hello.rs"), PathBuf::from("src/lib.rs")]
    );
}

#[test]
fn test_commit_diff_files_and_pretty_print() {
    setup_test_repo();
    let repo = SourceRepository::from_path(test_repo_path()).expect("failed to open test repo");

    let oids: Vec<_> = repo
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("revwalk failed");

    let first_commit_diff = repo
        .commit_diff(None, &oids[0])
        .expect("failed to diff root and first commit");
    assert_eq!(first_commit_diff.files(), &[PathBuf::from("hello.rs")]);

    let third_commit_diff = repo
        .commit_diff(Some(&oids[1]), &oids[2])
        .expect("failed to diff second and third commit");
    assert_eq!(third_commit_diff.files(), &[PathBuf::from("hello.rs")]);

    let pretty = third_commit_diff.pretty_print();
    assert!(pretty.contains("files changed: 1"));
    assert!(pretty.contains("hello.rs"));
}
