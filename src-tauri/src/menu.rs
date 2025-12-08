use anyhow::{Context, Result};
use tauri::{
    AppHandle, Emitter, Manager, Window, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem},
};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    AppState, handler,
    messages::{Operand, StoreRef},
};

#[allow(clippy::type_complexity)]
pub fn build_context(
    app_handle: &AppHandle<Wry>,
) -> Result<(Menu<Wry>, Menu<Wry>, Menu<Wry>), tauri::Error> {
    let revision_menu = Menu::with_items(
        app_handle,
        &[
            &MenuItem::with_id(app_handle, "revision_new", "New child", true, None::<&str>)?,
            &MenuItem::with_id(
                app_handle,
                "revision_edit",
                "Edit as working copy",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "revision_backout",
                "Backout into working copy",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "revision_duplicate",
                "Duplicate",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "revision_abandon",
                "Abandon",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app_handle)?,
            &MenuItem::with_id(
                app_handle,
                "revision_squash",
                "Squash into parent",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "revision_restore",
                "Restore from parent",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app_handle)?,
            &MenuItem::with_id(
                app_handle,
                "revision_branch",
                "Create bookmark",
                true,
                None::<&str>,
            )?,
        ],
    )?;

    let tree_menu = Menu::with_items(
        app_handle,
        &[
            &MenuItem::with_id(
                app_handle,
                "tree_squash",
                "Squash into parent",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "tree_restore",
                "Restore from parent",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app_handle)?,
            &MenuItem::with_id(
                app_handle,
                "file_open_with_default_app",
                "Open with Default App",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "file_open_in_explorer",
                "Open in File Explorer",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "file_copy_full_path",
                "Copy File Path",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app_handle,
                "file_copy_relative_path",
                "Copy Relative Path",
                true,
                None::<&str>,
            )?,
        ],
    )?;

    let ref_menu = Menu::with_items(
        app_handle,
        &[
            &MenuItem::with_id(app_handle, "branch_track", "Track", true, None::<&str>)?,
            &MenuItem::with_id(app_handle, "branch_untrack", "Untrack", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &MenuItem::with_id(app_handle, "branch_push_all", "Push", true, None::<&str>)?,
            &MenuItem::with_id(
                app_handle,
                "branch_push_single",
                "Push to remote...",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(app_handle, "branch_fetch_all", "Fetch", true, None::<&str>)?,
            &MenuItem::with_id(
                app_handle,
                "branch_fetch_single",
                "Fetch from remote...",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app_handle)?,
            &MenuItem::with_id(app_handle, "branch_rename", "Rename...", true, None::<&str>)?,
            &MenuItem::with_id(app_handle, "branch_delete", "Delete", true, None::<&str>)?,
        ],
    )?;

    Ok((revision_menu, tree_menu, ref_menu))
}

// enables context menu items for a revision and shows the menu
pub fn handle_context(window: Window, ctx: Operand) -> Result<()> {
    log::debug!("handling context {ctx:?}");

    let state = window.state::<AppState>();
    let guard = state.0.lock().expect("state mutex poisoned");

    match ctx {
        Operand::Revision { header } => {
            let context_menu = &guard
                .get(window.label())
                .expect("session not found")
                .revision_menu;

            context_menu.enable("revision_new", true)?;
            context_menu.enable(
                "revision_edit",
                !header.is_immutable && !header.is_working_copy,
            )?;
            context_menu.enable("revision_backout", true)?;
            context_menu.enable("revision_duplicate", true)?;
            context_menu.enable("revision_abandon", !header.is_immutable)?;
            context_menu.enable(
                "revision_squash",
                !header.is_immutable && header.parent_ids.len() == 1,
            )?;
            context_menu.enable(
                "revision_restore",
                !header.is_immutable && header.parent_ids.len() == 1,
            )?;
            context_menu.enable("revision_branch", true)?;

            window.popup_menu(context_menu)?;
        }
        Operand::Change { header, .. } => {
            let context_menu = &guard
                .get(window.label())
                .expect("session not found")
                .tree_menu;

            context_menu.enable(
                "tree_squash",
                !header.is_immutable && header.parent_ids.len() == 1,
            )?;
            context_menu.enable(
                "tree_restore",
                !header.is_immutable && header.parent_ids.len() == 1,
            )?;

            window.popup_menu(context_menu)?;
        }
        Operand::Ref { r#ref, .. } => {
            let context_menu = &guard
                .get(window.label())
                .expect("session not found")
                .ref_menu;

            // give remotes a local, or undelete them
            context_menu.enable(
                "branch_track",
                matches!(
                    r#ref,
                    StoreRef::RemoteBookmark {
                        is_tracked: false,
                        ..
                    }
                ),
            )?;

            // remove a local's remotes, or a remote from its local
            context_menu.enable(
                "branch_untrack",
                matches!(
                    r#ref,
                    StoreRef::LocalBookmark { ref tracking_remotes, ..} if !tracking_remotes.is_empty()
                )
                || matches!(
                    r#ref,
                    StoreRef::RemoteBookmark {
                        is_synced: false, // we can *see* the remote ref, and
                        is_tracked: true, // it has a local, and
                        is_absent: false, // that local is somewhere else
                        ..
                    }
                ),
            )?;

            // push a local to its remotes, or finish a CLI delete
            context_menu.enable("branch_push_all",
                matches!(r#ref, StoreRef::LocalBookmark { ref tracking_remotes, .. } if !tracking_remotes.is_empty())
                || matches!(r#ref, StoreRef::RemoteBookmark { is_tracked: true, is_absent: true, .. }))?;

            // push a local to a selected remote, tracking first if necessary
            context_menu.enable("branch_push_single",
                matches!(r#ref, StoreRef::LocalBookmark { potential_remotes, .. } if potential_remotes > 0))?;

            // fetch a local's remotes, or just a remote (unless we're deleting it; that would be silly)
            context_menu.enable("branch_fetch_all",
                matches!(r#ref, StoreRef::LocalBookmark { ref tracking_remotes, .. } if !tracking_remotes.is_empty())
                || matches!(r#ref, StoreRef::RemoteBookmark { is_tracked, is_absent, .. } if (!is_tracked || !is_absent)))?;

            // fetch a local, tracking first if necessary
            context_menu.enable("branch_fetch_single",
                matches!(r#ref, StoreRef::LocalBookmark { available_remotes, .. } if available_remotes > 0))?;

            // rename a local, which also untracks remotes
            context_menu.enable(
                "branch_rename",
                matches!(r#ref, StoreRef::LocalBookmark { .. }),
            )?;

            // remove a local, or make a remote absent
            context_menu.enable(
                "branch_delete",
                !matches!(
                    r#ref,
                    StoreRef::RemoteBookmark {
                        is_absent: true,
                        is_tracked: true,
                        ..
                    }
                ),
            )?;

            window.popup_menu(context_menu)?;
        }
        _ => (), // no popup required
    };

    Ok(())
}

pub fn handle_event(window: &Window, event: tauri::menu::MenuEvent) -> Result<()> {
    log::debug!("handling event {event:?}");

    match event.id.0.as_str() {
        "revision_new" => window.emit("vizjj://context/revision", "new")?,
        "revision_edit" => window.emit("vizjj://context/revision", "edit")?,
        "revision_backout" => window.emit("vizjj://context/revision", "backout")?,
        "revision_duplicate" => window.emit("vizjj://context/revision", "duplicate")?,
        "revision_abandon" => window.emit("vizjj://context/revision", "abandon")?,
        "revision_squash" => window.emit("vizjj://context/revision", "squash")?,
        "revision_restore" => window.emit("vizjj://context/revision", "restore")?,
        "revision_branch" => window.emit("vizjj://context/revision", "branch")?,
        "tree_squash" => window.emit("vizjj://context/tree", "squash")?,
        "tree_restore" => window.emit("vizjj://context/tree", "restore")?,
        "branch_track" => window.emit("vizjj://context/branch", "track")?,
        "branch_untrack" => window.emit("vizjj://context/branch", "untrack")?,
        "branch_push_all" => window.emit("vizjj://context/branch", "push-all")?,
        "branch_push_single" => window.emit("vizjj://context/branch", "push-single")?,
        "branch_fetch_all" => window.emit("vizjj://context/branch", "fetch-all")?,
        "branch_fetch_single" => window.emit("vizjj://context/branch", "fetch-single")?,
        "branch_rename" => window.emit("vizjj://context/branch", "rename")?,
        "branch_delete" => window.emit("vizjj://context/branch", "delete")?,
        "file_open_with_default_app" => {
            window.emit("vizjj://context/file", "open-with-default-app")?
        }
        "file_open_in_explorer" => window.emit("vizjj://context/file", "open-in-explorer")?,
        "file_copy_full_path" => window.emit("vizjj://context/file", "copy-full-path")?,
        "file_copy_relative_path" => window.emit("vizjj://context/file", "copy-relative-path")?,
        _ => (),
    };

    Ok(())
}

pub fn repo_open(window: &Window) {
    let window = window.clone();
    window.dialog().file().pick_folder(move |picked| {
        if let Some(FilePath::Path(cwd)) = picked {
            handler::fatal!(
                crate::try_open_repository(&window, Some(cwd)).context("try_open_repository")
            );
        }
    });
}

pub fn repo_reopen(window: &Window) {
    handler::fatal!(crate::try_open_repository(window, None).context("try_open_repository"));
}

trait Enabler {
    fn enable(&self, id: &str, value: bool) -> tauri::Result<()>;
}

impl Enabler for Menu<Wry> {
    fn enable(&self, id: &str, value: bool) -> tauri::Result<()> {
        match self.get(id).as_ref().and_then(|item| item.as_menuitem()) {
            Some(item) => item.set_enabled(value),
            None => Ok(()),
        }
    }
}
