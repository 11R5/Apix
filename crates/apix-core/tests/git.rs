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
