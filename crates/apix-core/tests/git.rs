//! `apix_core::git` against a real repository made with the installed `git`.

use std::path::{Path, PathBuf};
use std::process::Command;

use apix_core::git::{ChangeKind, Repo};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Apix Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        status.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn kinds(changes: &[apix_core::git::Change]) -> Vec<(String, ChangeKind)> {
    changes
        .iter()
        .map(|c| (apix_core::git::slashes(&c.path), c.kind))
        .collect()
}

#[test]
fn changes_since_a_branch_point() {
    let dir = std::env::temp_dir().join(format!("apix-git-test-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    let api = dir.join("api");
    write(&api.join("apix.toml"), "name = \"api\"\n");
    write(&api.join("users/get.apix"), "name = \"get\"\nurl = \"a\"\n");
    write(
        &api.join("users/list.apix"),
        "name = \"list\"\nurl = \"a\"\n",
    );
    write(&dir.join("other.txt"), "outside\n");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "first"]);

    let repo = Repo::at(&api).expect("a repository");
    assert_eq!(repo.branch().as_deref(), Some("main"));
    assert!(repo.working_changes().unwrap().is_empty());

    git(&dir, &["checkout", "-q", "-b", "feature"]);
    write(&api.join("users/get.apix"), "name = \"get\"\nurl = \"b\"\n");
    git(&dir, &["commit", "-q", "-am", "change get"]);
    // On main meanwhile: not part of the branch's changes.
    git(&dir, &["checkout", "-q", "main"]);
    write(
        &api.join("users/list.apix"),
        "name = \"list\"\nurl = \"c\"\n",
    );
    git(&dir, &["commit", "-q", "-am", "change list on main"]);
    git(&dir, &["checkout", "-q", "feature"]);
    // Not committed, a new file, and a change outside the collection.
    std::fs::remove_file(api.join("users/list.apix")).unwrap();
    write(&api.join("users/new.apix"), "name = \"new\"\nurl = \"a\"\n");
    write(&dir.join("other.txt"), "changed\n");

    let since = repo.changes_since("main").unwrap();
    assert!(since.warning.is_none());
    assert_eq!(
        kinds(&since.changes),
        vec![
            ("users/get.apix".to_owned(), ChangeKind::Modified),
            ("users/list.apix".to_owned(), ChangeKind::Deleted),
            ("users/new.apix".to_owned(), ChangeKind::Added),
        ]
    );
    // Only what is not committed.
    assert_eq!(
        kinds(&repo.working_changes().unwrap()),
        vec![
            ("users/list.apix".to_owned(), ChangeKind::Deleted),
            ("users/new.apix".to_owned(), ChangeKind::Added),
        ]
    );
    // Between two commits: main's own change shows too, untracked files do not.
    assert_eq!(
        kinds(&repo.changes_between("main", Some("feature")).unwrap()),
        vec![
            ("users/get.apix".to_owned(), ChangeKind::Modified),
            ("users/list.apix".to_owned(), ChangeKind::Modified),
        ]
    );
    let old = repo
        .show("main", &PathBuf::from("users/get.apix"))
        .unwrap()
        .unwrap();
    assert_eq!(
        String::from_utf8(old).unwrap(),
        "name = \"get\"\nurl = \"a\"\n"
    );
    assert!(
        repo.show("main", &PathBuf::from("users/new.apix"))
            .unwrap()
            .is_none()
    );
    assert!(repo.changes_since("no-such-branch").is_err());
    assert!(Repo::at(&std::env::temp_dir().join("apix-surely-not-a-repo-dir")).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn branches_commits_and_a_remote() {
    let dir = std::env::temp_dir().join(format!("apix-git-ops-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let work = dir.join("work");
    let remote = dir.join("remote.git");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "-q", "--bare"]);
    git(&work, &["init", "-q"]);
    git(&work, &["config", "user.name", "Apix Test"]);
    git(&work, &["config", "user.email", "test@example.com"]);
    git(&work, &["config", "commit.gpgsign", "false"]);
    git(
        &work,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    let api = work.join("api");
    write(&api.join("apix.toml"), "name = \"api\"\n");
    write(&work.join("notes.txt"), "outside\n");

    let repo = Repo::at(&api).unwrap();
    assert!(repo.log(5).unwrap().is_empty(), "no commits yet");
    assert!(repo.commit_folder("  ").is_err());
    let hash = repo.commit_folder("Add the collection").unwrap();
    assert!(!hash.is_empty());
    // Only the folder was committed; the file beside it is still new.
    let outside = Repo::at(&work).unwrap().working_changes().unwrap();
    assert_eq!(
        kinds(&outside),
        vec![("notes.txt".to_owned(), ChangeKind::Added)]
    );
    let log = repo.log(5).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].subject, "Add the collection");
    assert!(log[0].time > 1_700_000_000);

    // The first push sets the upstream; pull then has nothing to bring.
    repo.push().unwrap();
    assert!(repo.pull().is_ok());

    repo.create_branch("feature/x").unwrap();
    assert_eq!(repo.branch().as_deref(), Some("feature/x"));
    let mut branches = repo.branches().unwrap();
    branches.sort();
    assert_eq!(branches.len(), 2);
    assert!(branches.contains(&"feature/x".to_owned()));
    let first = branches.iter().find(|b| *b != "feature/x").unwrap().clone();
    repo.switch(&first).unwrap();
    assert_eq!(repo.branch(), Some(first));
    assert!(repo.switch("no-such-branch").is_err());
    std::fs::remove_dir_all(&dir).ok();
}
