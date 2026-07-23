use super::{mkrepo, revs};
use crate::{
    config::read_config,
    messages::{
        AbandonRevisions, CheckoutRevision, CopyChanges, CreateRevision, DescribeRevision,
        DuplicateRevisions, GitFetch, GitPush, InsertRevision, MoveChanges, MoveSource,
        MutationResult, RevResult, StoreRef, TreePath,
    },
    worker::{Mutation, WorkerCallbacks, WorkerSession, queries},
};
use anyhow::{Context, Result, bail};
use assert_matches::assert_matches;
use jj_lib::{
    git::{GitProgress, GitSidebandLineTerminator, GitSubprocessCallback},
    object_id::ObjectId,
    op_store::RefTarget,
    ref_name::RefNameBuf,
    workspace::Workspace,
};
use pollster::FutureExt;
use std::{
    ffi::OsStr,
    fs, io,
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tempfile::tempdir;

#[derive(Clone, Default)]
struct CountingCallbacks {
    callbacks: Arc<AtomicUsize>,
    events: Arc<AtomicUsize>,
}

impl WorkerCallbacks for CountingCallbacks {
    fn git_subprocess_callback(&self) -> Box<dyn GitSubprocessCallback> {
        self.callbacks.fetch_add(1, Ordering::Relaxed);
        Box::new(CountingGitCallback {
            events: self.events.clone(),
        })
    }
}

struct CountingGitCallback {
    events: Arc<AtomicUsize>,
}

impl GitSubprocessCallback for CountingGitCallback {
    fn needs_progress(&self) -> bool {
        true
    }

    fn progress(&mut self, _progress: &GitProgress) -> io::Result<()> {
        self.events.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn local_sideband(
        &mut self,
        _message: &[u8],
        _term: Option<GitSidebandLineTerminator>,
    ) -> io::Result<()> {
        self.events.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn remote_sideband(
        &mut self,
        _message: &[u8],
        _term: Option<GitSidebandLineTerminator>,
    ) -> io::Result<()> {
        self.events.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn run_git<I, S>(cwd: Option<&Path>, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("run git subprocess")?;
    if !output.status.success() {
        bail!(
            "git subprocess failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[test]
fn abandon_revisions() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let page = queries::query_log(&ws, "all()", 100)?;
    assert_eq!(12, page.rows.len());

    AbandonRevisions {
        ids: vec![revs::resolve_conflict().commit],
    }
    .execute_unboxed(&mut ws)?;

    let page = queries::query_log(&ws, "all()", 100)?;
    assert_eq!(11, page.rows.len());

    Ok(())
}

#[test]
fn checkout_revision() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let head_rev = queries::query_revision(&ws, revs::working_copy())?;
    let conflict_rev = queries::query_revision(&ws, revs::conflict_bookmark())?;
    assert_matches!(head_rev, RevResult::Detail { header, .. } if header.is_working_copy);
    assert_matches!(conflict_rev, RevResult::Detail { header, .. } if !header.is_working_copy);

    let result = CheckoutRevision {
        id: revs::conflict_bookmark(),
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::UpdatedSelection { .. });

    let head_rev = queries::query_revision(&ws, revs::working_copy())?;
    let conflict_rev = queries::query_revision(&ws, revs::conflict_bookmark())?;
    assert_matches!(head_rev, RevResult::NotFound { .. });
    assert_matches!(conflict_rev, RevResult::Detail { header, .. } if header.is_working_copy);

    Ok(())
}

#[test]
fn copy_changes() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let from_rev = queries::query_revision(&ws, revs::resolve_conflict())?;
    let to_rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(from_rev, RevResult::Detail { changes, .. } if changes.len() == 1);
    assert_matches!(to_rev, RevResult::Detail { changes, .. } if changes.is_empty());

    let result = CopyChanges {
        from_id: revs::resolve_conflict().commit,
        to_id: revs::working_copy(),
        paths: vec![TreePath {
            repo_path: "b.txt".to_owned(),
            relative_path: "".into(),
        }],
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::Updated { .. });

    let from_rev = queries::query_revision(&ws, revs::resolve_conflict())?;
    let to_rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(from_rev, RevResult::Detail { changes, .. } if changes.len() == 1);
    assert_matches!(to_rev, RevResult::Detail { changes, .. } if changes.len() == 1);

    Ok(())
}

#[test]
fn create_revision_single_parent() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let parent_rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(parent_rev, RevResult::Detail { header, .. } if header.is_working_copy);

    let result = CreateRevision {
        parent_ids: vec![revs::working_copy()],
    }
    .execute_unboxed(&mut ws)?;

    match result {
        MutationResult::UpdatedSelection { new_selection, .. } => {
            let parent_rev = queries::query_revision(&ws, revs::working_copy())?;
            let child_rev = queries::query_revision(&ws, new_selection.id)?;
            assert!(
                matches!(parent_rev, RevResult::Detail { header, .. } if !header.is_working_copy)
            );
            assert!(
                matches!(child_rev, RevResult::Detail { header, .. } if header.is_working_copy)
            );
        }
        _ => panic!("CreateRevision failed"),
    }

    Ok(())
}

#[test]
fn create_revision_multi_parent() -> Result<()> {
    let repo: tempfile::TempDir = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let parent_rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(parent_rev, RevResult::Detail { header, .. } if header.is_working_copy);

    let result = CreateRevision {
        parent_ids: vec![revs::working_copy(), revs::conflict_bookmark()],
    }
    .execute_unboxed(&mut ws)?;

    match result {
        MutationResult::UpdatedSelection { new_selection, .. } => {
            let child_rev = queries::query_revision(&ws, new_selection.id)?;
            assert_matches!(child_rev, RevResult::Detail { parents, .. } if parents.len() == 2);
        }
        _ => panic!("CreateRevision failed"),
    }

    Ok(())
}

#[test]
fn describe_revision() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(rev, RevResult::Detail { header, .. } if header.description.lines[0].is_empty());

    let result = DescribeRevision {
        id: revs::working_copy(),
        new_description: "wip".to_owned(),
        reset_author: false,
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::Updated { .. });

    let rev = queries::query_revision(&ws, revs::working_copy())?;
    assert!(
        matches!(rev, RevResult::Detail { header, .. } if header.description.lines[0] == "wip")
    );

    Ok(())
}

#[test]
fn describe_revision_with_snapshot() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let rev = queries::query_revision(&ws, revs::working_copy())?;
    assert!(
        matches!(rev, RevResult::Detail { header, changes, .. } if header.description.lines[0].is_empty() && changes.is_empty())
    );

    fs::write(repo.path().join("new.txt"), []).unwrap(); // changes the WC commit

    DescribeRevision {
        id: revs::working_copy(),
        new_description: "wip".to_owned(),
        reset_author: false,
    }
    .execute_unboxed(&mut ws)?;

    let rev = queries::query_revision(&ws, revs::working_copy())?;
    assert!(
        matches!(rev, RevResult::Detail { header, changes, .. } if header.description.lines[0] == "wip" && !changes.is_empty())
    );

    Ok(())
}

#[test]
fn duplicate_revisions() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(rev, RevResult::Detail { header, .. } if header.description.lines[0].is_empty());

    let result = DuplicateRevisions {
        ids: vec![revs::main_bookmark()],
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::UpdatedSelection { .. });

    let page = queries::query_log(&ws, "description(substring:\"unsynced\")", 3)?;
    assert_eq!(2, page.rows.len());

    Ok(())
}

#[test]
fn fetches_and_pushes_with_git_subprocesses() -> Result<()> {
    let root = tempdir()?;
    let bare = root.path().join("remote.git");
    let seed = root.path().join("seed");
    let checkout = root.path().join("checkout");
    let bare_arg = bare.to_string_lossy().into_owned();
    let checkout_arg = checkout.to_string_lossy().into_owned();

    run_git(None, ["init", "--bare", bare_arg.as_str()])?;
    fs::create_dir(&seed)?;
    run_git(Some(&seed), ["init"])?;
    run_git(Some(&seed), ["config", "user.name", "vizjj test"])?;
    run_git(Some(&seed), ["config", "user.email", "vizjj@example.com"])?;
    fs::write(seed.join("shared.txt"), "initial\n")?;
    run_git(Some(&seed), ["add", "shared.txt"])?;
    run_git(Some(&seed), ["commit", "-m", "initial commit"])?;
    run_git(Some(&seed), ["branch", "-M", "main"])?;
    run_git(Some(&seed), ["remote", "add", "origin", bare_arg.as_str()])?;
    run_git(Some(&seed), ["push", "-u", "origin", "main"])?;
    run_git(
        None,
        [
            "--git-dir",
            bare_arg.as_str(),
            "symbolic-ref",
            "HEAD",
            "refs/heads/main",
        ],
    )?;
    run_git(None, ["clone", bare_arg.as_str(), checkout_arg.as_str()])?;

    let (settings, _, _) = read_config(None, None)?;
    Workspace::init_external_git(&settings, &checkout, &checkout.join(".git")).block_on()?;

    fs::write(seed.join("shared.txt"), "from remote\n")?;
    run_git(Some(&seed), ["add", "shared.txt"])?;
    run_git(Some(&seed), ["commit", "-m", "remote commit"])?;
    run_git(Some(&seed), ["push", "origin", "main"])?;
    let fetched_head = run_git(Some(&seed), ["rev-parse", "HEAD"])?;

    let callbacks = CountingCallbacks::default();
    let mut session = WorkerSession::new(callbacks.clone(), None);
    let mut ws = session.load_directory(&checkout)?;
    ws.import_and_snapshot(true)?;
    let result = GitFetch::AllBookmarks {
        remote_name: "origin".to_owned(),
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::Updated { .. });
    assert_eq!(1, queries::query_log(&ws, &fetched_head, 1)?.rows.len());

    run_git(Some(&checkout), ["reset", "--hard", "origin/main"])?;
    fs::write(checkout.join("local.txt"), "pushed by vizjj\n")?;
    run_git(Some(&checkout), ["add", "local.txt"])?;
    run_git(
        Some(&checkout),
        [
            "-c",
            "user.name=vizjj test",
            "-c",
            "user.email=vizjj@example.com",
            "commit",
            "-m",
            "local commit",
        ],
    )?;
    let local_head = run_git(Some(&checkout), ["rev-parse", "HEAD"])?;
    ws.import_and_snapshot(true)?;
    let imported_head = ws
        .view()
        .git_head()
        .as_normal()
        .context("missing imported Git HEAD")?
        .clone();
    assert_eq!(local_head, imported_head.hex());
    let mut tx = ws.start_transaction()?;
    tx.repo_mut()
        .set_local_bookmark_target(&RefNameBuf::from("main"), RefTarget::normal(imported_head));
    ws.finish_transaction(tx, "move main to imported git commit".to_owned())?;

    let result = GitPush::RemoteBookmark {
        remote_name: "origin".to_owned(),
        branch_ref: StoreRef::LocalBookmark {
            branch_name: "main".to_owned(),
            has_conflict: false,
            is_synced: false,
            tracking_remotes: vec!["origin".to_owned()],
            available_remotes: 1,
            potential_remotes: 1,
        },
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::Updated { .. });

    let pushed_head = run_git(
        None,
        [
            "--git-dir",
            bare_arg.as_str(),
            "rev-parse",
            "refs/heads/main",
        ],
    )?;
    assert_eq!(local_head, pushed_head);
    assert_eq!(2, callbacks.callbacks.load(Ordering::Relaxed));
    assert!(callbacks.events.load(Ordering::Relaxed) > 0);

    Ok(())
}

#[test]
fn insert_revision() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let page = queries::query_log(&ws, "main::@", 4)?;
    assert_eq!(2, page.rows.len());

    InsertRevision {
        after_id: revs::main_bookmark(),
        before_id: revs::working_copy(),
        id: revs::resolve_conflict(),
    }
    .execute_unboxed(&mut ws)?;

    let page = queries::query_log(&ws, "main::@", 4)?;
    assert_eq!(3, page.rows.len());

    Ok(())
}

#[test]
fn move_changes_all_paths() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let parent_rev = queries::query_revision(&ws, revs::conflict_bookmark())?;
    assert_matches!(parent_rev, RevResult::Detail { header, .. } if header.has_conflict);

    let result = MoveChanges {
        from_id: revs::resolve_conflict(),
        to_id: revs::conflict_bookmark().commit,
        paths: vec![],
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::Updated { .. });

    let parent_rev = queries::query_revision(&ws, revs::conflict_bookmark())?;
    assert_matches!(parent_rev, RevResult::Detail { header, .. } if !header.has_conflict);

    Ok(())
}

#[test]
fn move_changes_single_path() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let from_rev = queries::query_revision(&ws, revs::main_bookmark())?;
    let to_rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(from_rev, RevResult::Detail { changes, .. } if changes.len() == 2);
    assert_matches!(to_rev, RevResult::Detail { changes, .. } if changes.is_empty());

    let result = MoveChanges {
        from_id: revs::main_bookmark(),
        to_id: revs::working_copy().commit,
        paths: vec![TreePath {
            repo_path: "c.txt".to_owned(),
            relative_path: "".into(),
        }],
    }
    .execute_unboxed(&mut ws)?;
    assert_matches!(result, MutationResult::Updated { .. });

    let from_rev = queries::query_revision(&ws, revs::main_bookmark())?;
    let to_rev = queries::query_revision(&ws, revs::working_copy())?;
    assert_matches!(from_rev, RevResult::Detail { changes, .. } if changes.len() == 1);
    assert_matches!(to_rev, RevResult::Detail { changes, .. } if changes.len() == 1);

    Ok(())
}

#[test]
fn move_source() -> Result<()> {
    let repo = mkrepo();

    let mut session = WorkerSession::default();
    let mut ws = session.load_directory(repo.path())?;

    let page = queries::query_log(&ws, "@+", 1)?;
    assert_eq!(0, page.rows.len());

    MoveSource {
        id: revs::resolve_conflict(),
        parent_ids: vec![revs::working_copy().commit],
    }
    .execute_unboxed(&mut ws)?;

    let page = queries::query_log(&ws, "@+", 2)?;
    assert_eq!(1, page.rows.len());

    Ok(())
}

// XXX missing tests for:
// - branch/ref mutations
// - git interop
