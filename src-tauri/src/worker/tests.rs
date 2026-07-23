use crate::{
    config::read_config,
    messages::{ChangeId, CommitId, RevId},
    worker::WorkerSession,
};
use anyhow::Result;
use jj_lib::workspace::Workspace;
use jj_lib::{backend::TreeValue, repo_path::RepoPath};
use pollster::FutureExt;
use std::{
    fs::{self, File},
    path::PathBuf,
};
use tempfile::{TempDir, tempdir};
use zip::ZipArchive;

mod mutations;
mod queries;
mod session;

fn mkrepo() -> TempDir {
    let repo_dir = tempdir().unwrap();
    let mut archive_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    archive_path.push("resources/test-repo.zip");
    let archive_file = File::open(&archive_path).unwrap();
    let mut archive = ZipArchive::new(archive_file).unwrap();

    archive.extract(repo_dir.path()).unwrap();

    repo_dir
}

#[test]
fn fixture_uses_jj_043_index_layout() {
    let repo = mkrepo();
    let index = repo.path().join(".jj").join("repo").join("index");

    assert!(!index.join("operations").exists());
    assert!(index.join("segments").is_dir());
    assert!(index.join("op_links").is_dir());
    assert!(
        !repo
            .path()
            .join(".jj")
            .join("repo")
            .join("config.toml")
            .exists()
    );
    assert!(!repo.path().join(".jj/repo/config-id").exists());
    assert!(!repo.path().join(".jj/workspace-config.toml").exists());
}

#[test]
fn opens_and_reopens_jj_043_internal_git_repo() -> Result<()> {
    opens_and_reopens_new_repo(false)
}

#[test]
fn opens_and_reopens_jj_043_colocated_git_repo() -> Result<()> {
    opens_and_reopens_new_repo(true)
}

fn opens_and_reopens_new_repo(colocated: bool) -> Result<()> {
    let repo = tempdir()?;
    let (settings, _, _) = read_config(None, None)?;
    if colocated {
        Workspace::init_colocated_git(&settings, repo.path()).block_on()?;
    } else {
        Workspace::init_internal_git(&settings, repo.path()).block_on()?;
    }

    fs::write(repo.path().join("new.txt"), "jj 0.43")?;
    {
        let mut session = WorkerSession::default();
        let mut ws = session.load_directory(repo.path())?;
        assert!(ws.import_and_snapshot(true)?);
    }
    {
        let mut session = WorkerSession::default();
        let ws = session.load_directory(repo.path())?;
        let commit = ws.get_commit(ws.wc_id())?;
        let value = commit
            .tree()
            .path_value(RepoPath::from_internal_string("new.txt")?)
            .block_on()?;
        assert!(value.is_resolved());
    }

    let index = repo.path().join(".jj").join("repo").join("index");
    assert!(!index.join("operations").exists());
    assert!(index.join("segments").is_dir());
    assert!(index.join("op_links").is_dir());
    Ok(())
}

fn mkid(xid: &str, cid: &str) -> RevId {
    RevId {
        change: ChangeId {
            hex: xid.to_owned(),
            prefix: xid.to_owned(),
            rest: "".to_owned(),
        },
        commit: CommitId {
            hex: cid.to_owned(),
            prefix: cid.to_owned(),
            rest: "".to_owned(),
        },
    }
}

mod revs {
    use crate::messages::RevId;

    use super::mkid;

    pub fn working_copy() -> RevId {
        mkid("nnloouly", "56018b94eb61a9acddc58ad7974aa51c3368eadd")
    }

    pub fn main_bookmark() -> RevId {
        mkid("mnkoropy", "87e9c6c03e1b727ff712d962c03b32fffb704bc0")
    }

    pub fn conflict_bookmark() -> RevId {
        mkid("nwrnuwyp", "880abeefdd3ac344e2a0901c5f486d02d34053da")
    }

    pub fn resolve_conflict() -> RevId {
        mkid("rrxroxys", "db297552443bcafc0f0715b7ace7fb4488d7954d")
    }
}

#[test]
fn wc_path_is_visible() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let ws = session.load_directory(repo.path())?;

    let commit = ws.get_commit(ws.wc_id())?;
    let value = commit
        .tree()
        .path_value(RepoPath::from_internal_string("a.txt")?)
        .block_on()?;

    assert!(value.is_resolved());
    assert!(
        value
            .first()
            .as_ref()
            .is_some_and(|x| matches!(x, TreeValue::File { .. }))
    );

    Ok(())
}

#[test]
fn snapshot_updates_wc_if_changed() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;
    let old_wc = ws.wc_id().clone();

    assert!(!ws.import_and_snapshot(true)?);
    assert_eq!(&old_wc, ws.wc_id());

    fs::write(repo.path().join("new.txt"), []).unwrap();

    assert!(ws.import_and_snapshot(true)?);
    assert_ne!(&old_wc, ws.wc_id());

    Ok(())
}

#[test]
fn transaction_updates_wc_if_snapshot() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;
    let old_wc = ws.wc_id().clone();

    fs::write(repo.path().join("new.txt"), []).unwrap();

    let tx = ws.start_transaction()?;
    ws.finish_transaction(tx, "do nothing")?;

    assert_ne!(&old_wc, ws.wc_id());

    Ok(())
}

#[test]
fn transaction_snapshot_path_is_visible() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    fs::write(repo.path().join("new.txt"), []).unwrap();

    let tx = ws.start_transaction()?;
    ws.finish_transaction(tx, "do nothing")?;

    let commit = ws.get_commit(ws.wc_id())?;
    let value = commit
        .tree()
        .path_value(RepoPath::from_internal_string("new.txt")?)
        .block_on()?;

    assert!(value.is_resolved());
    assert!(
        value
            .first()
            .as_ref()
            .is_some_and(|x| matches!(x, TreeValue::File { .. }))
    );

    Ok(())
}
