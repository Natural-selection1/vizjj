use std::fmt::Display;

use anyhow::{Context, Error, Result, anyhow};
use futures_util::TryStreamExt;
use indexmap::IndexMap;
use itertools::Itertools;
use jj_cli::{git_util::load_git_import_options, ui::Ui};
use jj_lib::ref_name::{RefNameBuf, RemoteName, RemoteNameBuf, RemoteRefSymbol};
use jj_lib::{
    backend::CommitId,
    commit::Commit,
    git::{
        self, GitFetchRefExpression, GitPushOptions, GitPushRefTargets, GitSettings,
        GitSubprocessOptions, REMOTE_NAME_FOR_LOCAL_GIT_REPO,
    },
    git_backend::GitBackend,
    matchers::{EverythingMatcher, FilesMatcher, Matcher},
    merge::{Diff, Merge},
    merged_tree::MergedTree,
    object_id::ObjectId as ObjectIdTrait,
    op_store::{RefTarget, RemoteRef, RemoteRefState},
    op_walk,
    refs::{self, LocalAndRemoteRef, RefPushAction},
    repo::Repo,
    repo_path::RepoPath,
    revset::{self, RevsetStreamExt},
    rewrite::{self, RebaseOptions, RebasedCommit},
    str_util::{StringExpression, StringPattern},
};
use pollster::FutureExt;

use crate::messages::{
    AbandonRevisions, BackoutRevisions, CheckoutRevision, CopyChanges, CreateRef, CreateRevision,
    DeleteRef, DescribeRevision, DuplicateRevisions, GitFetch, GitPush, InsertRevision,
    MoveChanges, MoveRef, MoveRevision, MoveSource, MutationResult, RenameBranch, StoreRef,
    TrackBranch, TreePath, UndoOperation, UntrackBranch,
};

use super::Mutation;
use super::gui_util::WorkspaceSession;

type BookmarkPushUpdate = (RefNameBuf, Diff<Option<CommitId>>);
type RemoteBranchUpdates<'a> = Vec<(&'a str, Vec<BookmarkPushUpdate>)>;

macro_rules! precondition {
    ($($args:tt)*) => {
        return Ok(MutationResult::PreconditionError { message: format!($($args)*) })
    }
}

impl Mutation for AbandonRevisions {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let abandoned_ids = self
            .ids
            .into_iter()
            .map(|id| CommitId::try_from_hex(&id.hex).expect("frontend-validated id"))
            .collect_vec();

        if ws.check_immutable(abandoned_ids.clone())? {
            precondition!("Some revisions are immutable");
        }

        for id in &abandoned_ids {
            let commit = tx
                .repo()
                .store()
                .get_commit(id)
                .context("Failed to lookup commit")?;
            tx.repo_mut().record_abandoned_commit(&commit);
        }
        tx.repo_mut().rebase_descendants().block_on()?;

        let transaction_description = if abandoned_ids.len() == 1 {
            format!("abandon commit {}", abandoned_ids[0].hex())
        } else {
            format!(
                "abandon commit {} and {} more",
                abandoned_ids[0].hex(),
                abandoned_ids.len() - 1
            )
        };

        match ws.finish_transaction(tx, transaction_description)? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for BackoutRevisions {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        if self.ids.len() != 1 {
            precondition!("Not implemented for >1 rev");
        }

        let mut tx = ws.start_transaction()?;

        let working_copy = ws.get_commit(ws.wc_id())?;
        let reverted = ws.resolve_multiple_changes(self.ids)?;
        let reverted_parents = reverted[0].parents().block_on()?;

        let old_base_tree = rewrite::merge_commit_trees(tx.repo(), &reverted_parents).block_on()?;
        let new_base_tree = working_copy.tree();
        let old_tree = reverted[0].tree();
        let new_tree = MergedTree::merge(Merge::from_vec(vec![
            (
                new_base_tree,
                format!("{} (backout destination)", working_copy.conflict_label()),
            ),
            (
                old_tree,
                format!("{} (backed out revision)", reverted[0].conflict_label()),
            ),
            (old_base_tree, "parents of backed out revision".to_owned()),
        ]))
        .block_on()?;

        tx.repo_mut()
            .rewrite_commit(&working_copy)
            .set_tree(new_tree)
            .write()
            .block_on()?;

        match ws.finish_transaction(tx, format!("back out commit {}", reverted[0].id().hex()))? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for CheckoutRevision {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let edited = ws.resolve_single_change(&self.id)?;

        if ws.check_immutable(vec![edited.id().clone()])? {
            precondition!("Revision is immutable");
        }

        if edited.id() == ws.wc_id() {
            return Ok(MutationResult::Unchanged);
        }

        tx.repo_mut()
            .edit(ws.name().to_owned(), &edited)
            .block_on()?;

        match ws.finish_transaction(tx, format!("edit commit {}", edited.id().hex()))? {
            Some(new_status) => {
                let new_selection = ws.format_header(&edited, Some(false))?;
                Ok(MutationResult::UpdatedSelection {
                    new_status,
                    new_selection,
                })
            }
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for CreateRevision {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let parents_revset = ws.evaluate_revset_changes(
            &self
                .parent_ids
                .into_iter()
                .map(|id| id.change)
                .collect_vec(),
        )?;

        let parent_commits = ws.resolve_multiple(parents_revset)?;
        let parent_ids = parent_commits
            .iter()
            .map(|commit| commit.id().clone())
            .collect();
        let merged_tree = rewrite::merge_commit_trees(tx.repo(), &parent_commits).block_on()?;

        let new_commit = tx
            .repo_mut()
            .new_commit(parent_ids, merged_tree)
            .write()
            .block_on()?;

        tx.repo_mut()
            .edit(ws.name().to_owned(), &new_commit)
            .block_on()?;

        match ws.finish_transaction(tx, "new empty commit")? {
            Some(new_status) => {
                let new_selection = ws.format_header(&new_commit, Some(false))?;
                Ok(MutationResult::UpdatedSelection {
                    new_status,
                    new_selection,
                })
            }
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for DescribeRevision {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let described = ws.resolve_single_change(&self.id)?;

        if ws.check_immutable(vec![described.id().clone()])? {
            precondition!("Revision {} is immutable", self.id.change.prefix);
        }

        if self.new_description == described.description() && !self.reset_author {
            return Ok(MutationResult::Unchanged);
        }

        let mut commit_builder = tx
            .repo_mut()
            .rewrite_commit(&described)
            .set_description(self.new_description);

        if self.reset_author {
            let new_author = commit_builder.committer().clone();
            commit_builder = commit_builder.set_author(new_author);
        }

        commit_builder.write().block_on()?;

        match ws.finish_transaction(tx, format!("describe commit {}", described.id().hex()))? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for DuplicateRevisions {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let clonees = ws.resolve_multiple_changes(self.ids)?; // in reverse topological order
        let num_clonees = clonees.len();
        let mut clones: IndexMap<Commit, Commit> = IndexMap::new();

        // toposort ensures that parents are duplicated first
        for clonee in clonees.into_iter().rev() {
            let clone_parents: Vec<_> = clonee
                .parents()
                .block_on()?
                .iter()
                .map(|parent| {
                    if let Some(cloned_parent) = clones.get(parent) {
                        cloned_parent
                    } else {
                        parent
                    }
                    .id()
                    .clone()
                })
                .collect();
            let clone = tx
                .repo_mut()
                .rewrite_commit(&clonee)
                .clear_rewrite_source()
                .generate_new_change_id()
                .set_parents(clone_parents)
                .write()
                .block_on()?;
            clones.insert(clonee, clone);
        }

        match ws.finish_transaction(tx, format!("duplicating {num_clonees} commit(s)"))? {
            Some(new_status) => {
                if num_clonees == 1 {
                    let new_commit = clones
                        .get_index(0)
                        .ok_or(anyhow!("single source should have single copy"))?
                        .1;
                    let new_selection = ws.format_header(new_commit, None)?;
                    Ok(MutationResult::UpdatedSelection {
                        new_status,
                        new_selection,
                    })
                } else {
                    Ok(MutationResult::Updated { new_status })
                }
            }
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for InsertRevision {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let target = ws
            .resolve_single_change(&self.id)
            .context("resolve change_id")?;
        let before = ws
            .resolve_single_change(&self.before_id)
            .context("resolve before_id")?;
        let after = ws
            .resolve_single_change(&self.after_id)
            .context("resolve after_id")?;

        if ws.check_immutable(vec![target.id().clone(), before.id().clone()])? {
            precondition!("Some revisions are immutable");
        }

        // rebase the target's children
        let rebased_children = ws.disinherit_children(&mut tx, &target)?;

        // update after, which may have been a descendant of target
        let after_id = rebased_children
            .get(after.id())
            .unwrap_or(after.id())
            .clone();

        // rebase the target (which now has no children), then the new post-target tree atop it
        let rebased_id = target.id().hex();
        let target = rewrite::rebase_commit(tx.repo_mut(), target, vec![after_id]).block_on()?;
        rewrite::rebase_commit(tx.repo_mut(), before, vec![target.id().clone()]).block_on()?;

        match ws.finish_transaction(tx, format!("rebase commit {rebased_id}"))? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for MoveRevision {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let target = ws.resolve_single_change(&self.id)?;
        let parents = ws.resolve_multiple_changes(self.parent_ids)?;

        if ws.check_immutable(vec![target.id().clone()])? {
            precondition!("Revision {} is immutable", self.id.change.prefix);
        }

        // rebase the target's children
        let rebased_children = ws.disinherit_children(&mut tx, &target)?;

        // update parents, which may have been descendants of the target
        let parent_ids: Vec<_> = parents
            .iter()
            .map(|new_parent| {
                rebased_children
                    .get(new_parent.id())
                    .unwrap_or(new_parent.id())
                    .clone()
            })
            .collect();

        // rebase the target itself
        let rebased_id = target.id().hex();
        rewrite::rebase_commit(tx.repo_mut(), target, parent_ids).block_on()?;

        match ws.finish_transaction(tx, format!("rebase commit {rebased_id}"))? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for MoveSource {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let target = ws.resolve_single_change(&self.id)?;
        let parent_ids = ws
            .resolve_multiple_commits(&self.parent_ids)?
            .into_iter()
            .map(|commit| commit.id().clone())
            .collect();

        if ws.check_immutable(vec![target.id().clone()])? {
            precondition!("Revision {} is immutable", self.id.change.prefix);
        }

        // just rebase the target, which will also rebase its descendants
        let rebased_id = target.id().hex();
        rewrite::rebase_commit(tx.repo_mut(), target, parent_ids).block_on()?;

        match ws.finish_transaction(tx, format!("rebase commit {rebased_id}"))? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for MoveChanges {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let from = ws.resolve_single_change(&self.from_id)?;
        let mut to = ws.resolve_single_commit(&self.to_id)?;
        let matcher = build_matcher(&self.paths)?;

        if ws.check_immutable(vec![from.id().clone(), to.id().clone()])? {
            precondition!("Revisions are immutable");
        }

        // construct a split tree and a remainder tree by copying changes from child to parent and from parent to child
        let from_tree = from.tree();
        let from_parents = from.parents().block_on()?;
        let parent_tree = rewrite::merge_commit_trees(tx.repo(), &from_parents).block_on()?;
        let split_tree = rewrite::restore_tree(
            &from_tree,
            &parent_tree,
            from.conflict_label(),
            "parents of moved revision".to_owned(),
            matcher.as_ref(),
        )
        .block_on()?;
        let remainder_tree = rewrite::restore_tree(
            &parent_tree,
            &from_tree,
            "parents of moved revision".to_owned(),
            from.conflict_label(),
            matcher.as_ref(),
        )
        .block_on()?;

        // abandon or rewrite source
        let abandon_source = remainder_tree.tree_ids() == parent_tree.tree_ids();
        if abandon_source {
            tx.repo_mut().record_abandoned_commit(&from);
        } else {
            tx.repo_mut()
                .rewrite_commit(&from)
                .set_tree(remainder_tree)
                .write()
                .block_on()?;
        }

        // rebase descendants of source, which may include destination
        if tx.repo().index().is_ancestor(from.id(), to.id())? {
            let mut rebase_map = std::collections::HashMap::new();
            tx.repo_mut()
                .rebase_descendants_with_options(
                    &revset::RevsetExpression::none(),
                    &RebaseOptions::default(),
                    |old_commit, rebased_commit| {
                        rebase_map.insert(
                            old_commit.id().clone(),
                            match rebased_commit {
                                RebasedCommit::Rewritten(new_commit) => new_commit.id().clone(),
                                RebasedCommit::Abandoned { parent_id } => parent_id,
                            },
                        );
                    },
                )
                .block_on()?;
            let rebased_to_id = rebase_map
                .get(to.id())
                .ok_or_else(|| anyhow!("descendant to_commit not found in rebase map"))?
                .clone();
            to = tx.repo().store().get_commit(&rebased_to_id)?;
        }

        // apply changes to destination
        let to_tree = to.tree();
        let new_to_tree = MergedTree::merge(Merge::from_vec(vec![
            (
                to_tree,
                format!("{} (move destination)", to.conflict_label()),
            ),
            (parent_tree, "parents of moved revision".to_owned()),
            (split_tree, "moved changes".to_owned()),
        ]))
        .block_on()?;
        let description = combine_messages(&from, &to, abandon_source);
        tx.repo_mut()
            .rewrite_commit(&to)
            .set_tree(new_to_tree)
            .set_description(description)
            .write()
            .block_on()?;

        match ws.finish_transaction(
            tx,
            format!("move changes from {} to {}", from.id().hex(), to.id().hex()),
        )? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for CopyChanges {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let from = ws.resolve_single_commit(&self.from_id)?;
        let from_tree = from.tree();
        let to = ws.resolve_single_change(&self.to_id)?;
        let matcher = build_matcher(&self.paths)?;

        if ws.check_immutable(vec![to.id().clone()])? {
            precondition!("Revisions are immutable");
        }

        // construct a restore tree - the destination with some portions overwritten by the source
        let to_tree = to.tree();
        let new_to_tree = rewrite::restore_tree(
            &from_tree,
            &to_tree,
            from.conflict_label(),
            to.conflict_label(),
            matcher.as_ref(),
        )
        .block_on()?;
        if new_to_tree.tree_ids() == to.tree_ids() {
            Ok(MutationResult::Unchanged)
        } else {
            tx.repo_mut()
                .rewrite_commit(&to)
                .set_tree(new_to_tree)
                .write()
                .block_on()?;

            tx.repo_mut().rebase_descendants().block_on()?;

            match ws.finish_transaction(tx, format!("restore into commit {}", to.id().hex()))? {
                Some(new_status) => Ok(MutationResult::Updated { new_status }),
                None => Ok(MutationResult::Unchanged),
            }
        }
    }
}

impl Mutation for TrackBranch {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        match self.r#ref {
            StoreRef::Tag { tag_name } => {
                precondition!("{} is a tag and cannot be tracked", tag_name);
            }
            StoreRef::LocalBookmark { branch_name, .. } => {
                precondition!("{} is a local bookmark and cannot be tracked", branch_name);
            }
            StoreRef::RemoteBookmark {
                branch_name,
                remote_name,
                ..
            } => {
                let mut tx = ws.start_transaction()?;
                let branch_name_ref = RefNameBuf::from(branch_name);
                let remote_name_ref = RemoteNameBuf::from(remote_name);
                let remote_ref_symbol = RemoteRefSymbol {
                    name: &branch_name_ref,
                    remote: &remote_name_ref,
                };

                let remote_ref: &jj_lib::op_store::RemoteRef =
                    ws.view().get_remote_bookmark(remote_ref_symbol);

                if remote_ref.is_tracked() {
                    precondition!(
                        "{:?}@{:?} is already tracked",
                        branch_name_ref.as_str(),
                        remote_name_ref.as_str()
                    );
                }

                tx.repo_mut().track_remote_bookmark(remote_ref_symbol)?;

                match ws.finish_transaction(
                    tx,
                    format!(
                        "track remote bookmark {:?}@{:?}",
                        branch_name_ref.as_str(),
                        remote_name_ref.as_str()
                    ),
                )? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
        }
    }
}

impl Mutation for UntrackBranch {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let mut untracked = Vec::new();
        match self.r#ref {
            StoreRef::Tag { tag_name } => {
                precondition!("{} is a tag and cannot be untracked", tag_name);
            }
            StoreRef::LocalBookmark { branch_name, .. } => {
                // untrack all remotes
                for (remote_ref_symbol, remote_ref) in ws.view().remote_bookmarks_matching(
                    &StringPattern::exact(branch_name).to_matcher(),
                    &StringPattern::all().to_matcher(),
                ) {
                    if remote_ref_symbol.remote != REMOTE_NAME_FOR_LOCAL_GIT_REPO
                        && remote_ref.is_tracked()
                    {
                        tx.repo_mut().untrack_remote_bookmark(remote_ref_symbol);
                        untracked.push(format!(
                            "{}@{}",
                            remote_ref_symbol.name.as_str(),
                            remote_ref_symbol.remote.as_str()
                        ));
                    }
                }
            }
            StoreRef::RemoteBookmark {
                branch_name,
                remote_name,
                ..
            } => {
                let branch_name_ref = RefNameBuf::from(branch_name);
                let remote_name_ref = RemoteNameBuf::from(remote_name);
                let remote_ref_symbol = RemoteRefSymbol {
                    name: &branch_name_ref,
                    remote: &remote_name_ref,
                };
                let remote_ref: &jj_lib::op_store::RemoteRef =
                    ws.view().get_remote_bookmark(remote_ref_symbol);

                if !remote_ref.is_tracked() {
                    precondition!(
                        "{:?}@{:?} is not tracked",
                        branch_name_ref.as_str(),
                        remote_name_ref.as_str()
                    );
                }

                tx.repo_mut().untrack_remote_bookmark(remote_ref_symbol);
                untracked.push(format!(
                    "{}@{}",
                    branch_name_ref.as_str(),
                    remote_name_ref.as_str()
                ));
            }
        }

        match ws.finish_transaction(
            tx,
            format!("untrack remote {}", combine_bookmarks(&untracked)),
        )? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for RenameBranch {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let old_name = self.r#ref.as_branch()?;
        let old_name_ref = RefNameBuf::from(old_name);

        let ref_target = ws.view().get_local_bookmark(&old_name_ref).clone();
        if ref_target.is_absent() {
            precondition!("No such bookmark: {}", old_name_ref.as_str());
        }

        let new_name_ref = RefNameBuf::from(self.new_name);
        if ws.view().get_local_bookmark(&new_name_ref).is_present() {
            precondition!("Bookmark already exists: {}", new_name_ref.as_str());
        }

        let mut tx = ws.start_transaction()?;

        tx.repo_mut()
            .set_local_bookmark_target(&new_name_ref, ref_target);
        tx.repo_mut()
            .set_local_bookmark_target(&old_name_ref, RefTarget::absent());

        match ws.finish_transaction(
            tx,
            format!(
                "rename {} to {}",
                old_name_ref.as_str(),
                new_name_ref.as_str()
            ),
        )? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for CreateRef {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let commit = ws.resolve_single_change(&self.id)?;

        match self.r#ref {
            StoreRef::RemoteBookmark {
                branch_name,
                remote_name,
                ..
            } => {
                precondition!(
                    "{}@{} is a remote bookmark and cannot be created",
                    branch_name,
                    remote_name
                );
            }
            StoreRef::LocalBookmark { branch_name, .. } => {
                let branch_name_ref = RefNameBuf::from(branch_name);
                let existing_branch = ws.view().get_local_bookmark(&branch_name_ref);
                if existing_branch.is_present() {
                    precondition!("{} already exists", branch_name_ref.as_str());
                }

                tx.repo_mut().set_local_bookmark_target(
                    &branch_name_ref,
                    RefTarget::normal(commit.id().clone()),
                );

                match ws.finish_transaction(
                    tx,
                    format!(
                        "create {} pointing to commit {}",
                        branch_name_ref.as_str(),
                        ws.format_commit_id(commit.id()).hex
                    ),
                )? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
            StoreRef::Tag { tag_name, .. } => {
                let tag_name_ref = RefNameBuf::from(tag_name);
                let existing_tag = ws.view().get_local_tag(&tag_name_ref);
                if existing_tag.is_present() {
                    precondition!("{} already exists", tag_name_ref.as_str());
                }

                tx.repo_mut()
                    .set_local_tag_target(&tag_name_ref, RefTarget::normal(commit.id().clone()));

                match ws.finish_transaction(
                    tx,
                    format!(
                        "create {} pointing to commit {}",
                        tag_name_ref.as_str(),
                        ws.format_commit_id(commit.id()).hex
                    ),
                )? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
        }
    }
}

impl Mutation for DeleteRef {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        match self.r#ref {
            StoreRef::RemoteBookmark {
                branch_name,
                remote_name,
                ..
            } => {
                let mut tx = ws.start_transaction()?;

                // forget the bookmark entirely - when target is absent, it's removed from the view
                let remote_ref = RemoteRef {
                    target: RefTarget::absent(),
                    state: RemoteRefState::New,
                };
                let remote_name_ref = RemoteNameBuf::from(remote_name);
                let branch_name_ref = RefNameBuf::from(branch_name);
                let remote_ref_symbol = RemoteRefSymbol {
                    name: &branch_name_ref,
                    remote: &remote_name_ref,
                };

                tx.repo_mut()
                    .set_remote_bookmark(remote_ref_symbol, remote_ref);

                match ws.finish_transaction(
                    tx,
                    format!(
                        "forget {}@{}",
                        branch_name_ref.as_str(),
                        remote_name_ref.as_str()
                    ),
                )? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
            StoreRef::LocalBookmark { branch_name, .. } => {
                let branch_name_ref = RefNameBuf::from(branch_name);
                let mut tx = ws.start_transaction()?;

                tx.repo_mut()
                    .set_local_bookmark_target(&branch_name_ref, RefTarget::absent());

                match ws.finish_transaction(tx, format!("forget {}", branch_name_ref.as_str()))? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
            StoreRef::Tag { tag_name } => {
                let tag_name_ref = RefNameBuf::from(tag_name);
                let mut tx = ws.start_transaction()?;

                tx.repo_mut()
                    .set_local_tag_target(&tag_name_ref, RefTarget::absent());

                match ws.finish_transaction(tx, format!("forget tag {}", tag_name_ref.as_str()))? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
        }
    }
}

// does not currently enforce fast-forwards
impl Mutation for MoveRef {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        let commit = ws.resolve_single_change(&self.to_id)?;

        match self.r#ref {
            StoreRef::RemoteBookmark {
                branch_name,
                remote_name,
                ..
            } => {
                precondition!("Bookmark is remote: {branch_name}@{remote_name}")
            }
            StoreRef::LocalBookmark { branch_name, .. } => {
                let branch_name_ref = RefNameBuf::from(branch_name);
                let old_target = ws.view().get_local_bookmark(&branch_name_ref);
                if old_target.is_absent() {
                    precondition!("No such bookmark: {:?}", branch_name_ref.as_str());
                }

                tx.repo_mut().set_local_bookmark_target(
                    &branch_name_ref,
                    RefTarget::normal(commit.id().clone()),
                );

                match ws.finish_transaction(
                    tx,
                    format!(
                        "point {:?} to commit {}",
                        branch_name_ref,
                        commit.id().hex()
                    ),
                )? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
            StoreRef::Tag { tag_name } => {
                let tag_name_ref = RefNameBuf::from(tag_name);
                let old_target = ws.view().get_local_tag(&tag_name_ref);
                if old_target.is_absent() {
                    precondition!("No such tag: {:?}", tag_name_ref.as_str());
                }

                tx.repo_mut()
                    .set_local_tag_target(&tag_name_ref, RefTarget::normal(commit.id().clone()));

                match ws.finish_transaction(
                    tx,
                    format!(
                        "point {:?} to commit {}",
                        tag_name_ref.as_str(),
                        commit.id().hex()
                    ),
                )? {
                    Some(new_status) => Ok(MutationResult::Updated { new_status }),
                    None => Ok(MutationResult::Unchanged),
                }
            }
        }
    }
}

impl Mutation for GitPush {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        // determine bookmarks to push, recording the old and new commits
        let mut remote_branch_updates: RemoteBranchUpdates<'_> = Vec::new();
        let remote_branch_refs: Vec<_> = match &*self {
            GitPush::AllBookmarks { remote_name } => {
                let remote_name_ref = RemoteNameBuf::from(remote_name);
                let mut branch_updates = Vec::new();
                for (branch_name, targets) in ws.view().local_remote_bookmarks(&remote_name_ref) {
                    if !targets.remote_ref.is_tracked() {
                        continue;
                    }

                    match classify_branch_push(branch_name.as_str(), remote_name, targets) {
                        Err(message) => return Ok(MutationResult::PreconditionError { message }),
                        Ok(None) => (),
                        Ok(Some(update)) => branch_updates.push((branch_name.to_owned(), update)),
                    }
                }
                remote_branch_updates.push((remote_name, branch_updates));

                ws.view()
                    .remote_bookmarks(&remote_name_ref)
                    .map(|(name, remote_ref)| (name.to_owned(), remote_ref))
                    .collect()
            }
            GitPush::AllRemotes { branch_ref } => {
                let branch_name = branch_ref.as_branch()?;
                let branch_name_ref = RefNameBuf::from(branch_name);

                let mut remote_branch_refs = Vec::new();
                for (remote_name, group) in ws
                    .view()
                    .all_remote_bookmarks()
                    .filter_map(|(remote_ref_symbol, remote_ref)| {
                        if remote_ref.is_tracked() && remote_ref_symbol.name == branch_name_ref {
                            Some((remote_ref_symbol.remote, remote_ref))
                        } else {
                            None
                        }
                    })
                    .chunk_by(|(remote_name, _)| *remote_name)
                    .into_iter()
                {
                    let mut branch_updates = Vec::new();
                    for (_, remote_ref) in group {
                        let targets = LocalAndRemoteRef {
                            local_target: ws.view().get_local_bookmark(&branch_name_ref),
                            remote_ref,
                        };
                        match classify_branch_push(branch_name, remote_name.as_str(), targets) {
                            Err(message) => {
                                return Ok(MutationResult::PreconditionError { message });
                            }
                            Ok(None) => (),
                            Ok(Some(update)) => {
                                branch_updates.push((RefNameBuf::from(branch_name), update))
                            }
                        }
                        remote_branch_refs.push((RefNameBuf::from(branch_name), remote_ref));
                    }
                    remote_branch_updates.push((remote_name.as_str(), branch_updates));
                }

                remote_branch_refs
            }
            GitPush::RemoteBookmark {
                remote_name,
                branch_ref,
            } => {
                let branch_name = branch_ref.as_branch()?;
                let branch_name_ref = RefNameBuf::from(branch_name);
                let local_target = ws.view().get_local_bookmark(&branch_name_ref);
                let remote_name_ref = RemoteNameBuf::from(remote_name);
                let remote_ref_symbol = RemoteRefSymbol {
                    name: &branch_name_ref,
                    remote: &remote_name_ref,
                };
                let remote_ref = ws.view().get_remote_bookmark(remote_ref_symbol);

                match classify_branch_push(
                    branch_name,
                    remote_name,
                    LocalAndRemoteRef {
                        local_target,
                        remote_ref,
                    },
                ) {
                    Err(message) => return Ok(MutationResult::PreconditionError { message }),
                    Ok(None) => (),
                    Ok(Some(update)) => {
                        remote_branch_updates
                            .push((remote_name, vec![(RefNameBuf::from(branch_name), update)]));
                    }
                }

                vec![(
                    RefNameBuf::from(branch_name),
                    ws.view().get_remote_bookmark(remote_ref_symbol),
                )]
            }
        };

        // check for conflicts
        let mut new_heads = vec![];
        for (_, branch_updates) in &mut remote_branch_updates {
            for (_, update) in branch_updates {
                if let Some(new_target) = &update.after {
                    new_heads.push(new_target.clone());
                }
            }
        }

        let mut old_heads = remote_branch_refs
            .into_iter()
            .flat_map(|(_, old_head)| old_head.target.added_ids())
            .cloned()
            .collect_vec();
        if old_heads.is_empty() {
            old_heads.push(ws.repo().store().root_commit_id().clone());
        }

        let commits: Vec<_> = revset::walk_revs(ws.repo(), &new_heads, &old_heads)?
            .stream()
            .commits(ws.repo().store())
            .try_collect()
            .block_on()?;
        for commit in commits {
            let mut reasons = vec![];
            if commit.description().is_empty() {
                reasons.push("it has no description");
            }
            if commit.author().name.is_empty()
                || commit.author().email.is_empty()
                || commit.committer().name.is_empty()
                || commit.committer().email.is_empty()
            {
                reasons.push("it has no author and/or committer set");
            }
            if commit.has_conflict() {
                reasons.push("it has conflicts");
            }
            if !reasons.is_empty() {
                precondition!(
                    "Won't push revision {} since {}",
                    ws.format_change_id(commit.change_id()).prefix,
                    reasons.join(" and ")
                );
            }
        }

        // push to each remote
        for (remote_name, branch_updates) in remote_branch_updates.into_iter() {
            let targets = GitPushRefTargets {
                bookmarks: branch_updates,
                tags: Vec::new(),
            };
            let subprocess_options = git_subprocess_options(&ws.data.settings)?;
            let mut callback = ws.session.callbacks.git_subprocess_callback();
            let stats = git::push_refs(
                tx.repo_mut(),
                subprocess_options,
                RemoteName::new(remote_name),
                &targets,
                callback.as_mut(),
                &GitPushOptions::default(),
            )?;
            if !stats.all_ok() {
                return Err(anyhow!(
                    "git push rejected one or more references: {stats:?}"
                ));
            }
        }

        match ws.finish_transaction(
            tx,
            match *self {
                GitPush::AllBookmarks { remote_name } => {
                    format!("push all tracked branches to git remote {remote_name}")
                }
                GitPush::AllRemotes { branch_ref } => {
                    format!(
                        "push {} to all tracked git remotes",
                        branch_ref.as_branch()?
                    )
                }
                GitPush::RemoteBookmark {
                    remote_name,
                    branch_ref,
                } => {
                    format!(
                        "push {} to git remote {}",
                        branch_ref.as_branch()?,
                        remote_name
                    )
                }
            },
        )? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

impl Mutation for GitFetch {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let mut tx = ws.start_transaction()?;

        if ws.repo().store().backend_impl::<GitBackend>().is_none() {
            precondition!("No git backend");
        }

        let mut remote_patterns = Vec::new();
        match *self {
            GitFetch::AllBookmarks { remote_name } => {
                remote_patterns.push((remote_name, None));
            }
            GitFetch::AllRemotes { branch_ref } => {
                let branch_name = branch_ref.as_branch()?;
                for remote_name in ws.git_remote_names() {
                    remote_patterns.push((remote_name, Some(branch_name.to_owned())));
                }
            }
            GitFetch::RemoteBookmark {
                remote_name,
                branch_ref,
            } => {
                let branch_name = branch_ref.as_branch()?;
                remote_patterns.push((remote_name, Some(branch_name.to_owned())));
            }
        }
        let git_settings = GitSettings::from_settings(&ws.data.settings)?;
        let remote_settings = ws.data.settings.remote_settings()?;
        let import_options = load_git_import_options(&Ui::null(), &git_settings, &remote_settings)
            .map_err(|err| Error::new(err.error))?;

        for (remote_name, pattern) in remote_patterns {
            let bookmark_expr = pattern
                .map(StringExpression::exact)
                .unwrap_or_else(StringExpression::all);
            let refspecs = git::expand_fetch_refspecs(
                RemoteName::new(&remote_name),
                GitFetchRefExpression {
                    bookmark: bookmark_expr,
                    tag: StringExpression::none(),
                },
            )?;
            let subprocess_options = git_subprocess_options(&ws.data.settings)?;
            let mut callback = ws.session.callbacks.git_subprocess_callback();
            let mut fetcher =
                git::GitFetch::new(tx.repo_mut(), subprocess_options, &import_options)?;
            fetcher
                .fetch(
                    RemoteName::new(&remote_name),
                    refspecs,
                    callback.as_mut(),
                    None,
                    None,
                )
                .context("failed to fetch")?;
            fetcher
                .import_refs()
                .block_on()
                .context("failed to import fetched refs")?;
        }

        match ws.finish_transaction(tx, "fetch from git remote(s)".to_string())? {
            Some(new_status) => Ok(MutationResult::Updated { new_status }),
            None => Ok(MutationResult::Unchanged),
        }
    }
}

// this is another case where it would be nice if we could reuse jj-cli's error messages
impl Mutation for UndoOperation {
    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<MutationResult> {
        let head_op = op_walk::resolve_op_with_repo(ws.repo(), "@").block_on()?; // XXX this should be behind an abstraction, maybe reused in snapshot
        let parent_ops = head_op.parents().block_on()?;
        let parent_op = match parent_ops.as_slice() {
            [] => precondition!("Cannot undo repo initialization"),
            [parent_op] => parent_op,
            _ => precondition!("Cannot undo a merge operation"),
        };

        let mut tx = ws.start_transaction()?;
        let repo_loader = tx.base_repo().loader();
        let head_repo = repo_loader.load_at(&head_op).block_on()?;
        let parent_repo = repo_loader.load_at(parent_op).block_on()?;
        tx.repo_mut().merge(&head_repo, &parent_repo).block_on()?;
        let restored_view = tx.repo().view().store_view().clone();
        tx.repo_mut().set_view(restored_view);

        match ws.finish_transaction(tx, format!("undo operation {}", head_op.id().hex()))? {
            Some(new_status) => {
                let working_copy = ws.get_commit(ws.wc_id())?;
                let new_selection = ws.format_header(&working_copy, None)?;
                Ok(MutationResult::UpdatedSelection {
                    new_status,
                    new_selection,
                })
            }
            None => Ok(MutationResult::Unchanged),
        }
    }
}

fn combine_messages(source: &Commit, destination: &Commit, abandon_source: bool) -> String {
    if abandon_source {
        if source.description().is_empty() {
            destination.description().to_owned()
        } else if destination.description().is_empty() {
            source.description().to_owned()
        } else {
            destination.description().to_owned() + "\n" + source.description()
        }
    } else {
        destination.description().to_owned()
    }
}

fn combine_bookmarks(branch_names: &[impl Display]) -> String {
    match branch_names {
        [branch_name] => format!("bookmark {branch_name}"),
        branch_names => format!("bookmarks {}", branch_names.iter().join(", ")),
    }
}

fn build_matcher(paths: &[TreePath]) -> Result<Box<dyn Matcher>> {
    if paths.is_empty() {
        Ok(Box::new(EverythingMatcher))
    } else {
        let repo_paths: Vec<_> = paths
            .iter()
            .map(|p| RepoPath::from_internal_string(&p.repo_path))
            .try_collect()?;
        Ok(Box::new(FilesMatcher::new(&repo_paths)))
    }
}

fn git_subprocess_options(
    settings: &jj_lib::settings::UserSettings,
) -> Result<GitSubprocessOptions> {
    let mut options = GitSubprocessOptions::from_settings(settings)?;
    options
        .environment
        .insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    Ok(options)
}

fn classify_branch_push(
    branch_name: &str,
    remote_name: &str,
    targets: LocalAndRemoteRef,
) -> Result<Option<Diff<Option<CommitId>>>, String> {
    let push_action = refs::classify_ref_push_action(targets);
    match push_action {
        RefPushAction::AlreadyMatches => Ok(None),
        RefPushAction::Update(update) => Ok(Some(update)),
        RefPushAction::LocalConflicted => Err(format!("Bookmark {branch_name} is conflicted.")),
        RefPushAction::RemoteConflicted => Err(format!(
            "Bookmark {branch_name}@{remote_name} is conflicted. Try fetching first."
        )),
        RefPushAction::RemoteUntracked => Err(format!(
            "Non-tracking remote bookmark {branch_name}@{remote_name} exists. Try tracking it first."
        )),
    }
}
