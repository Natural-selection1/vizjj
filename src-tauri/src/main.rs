#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod callbacks;
mod config;
mod handler;
mod messages;
mod worker;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use jj_lib::config::ConfigSource;
use log::LevelFilter;
use tauri::{Emitter, State, Window, WindowEvent};
use tauri::{Manager, ipc::InvokeError};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_window_state::StateFlags;
#[cfg(windows)]
use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

use messages::{
    AbandonRevisions, BackoutRevisions, CheckoutRevision, CopyChanges, CreateRef, CreateRevision,
    DeleteRef, DescribeRevision, DuplicateRevisions, GitFetch, GitPush, InputResponse,
    InsertRevision, MoveChanges, MoveRef, MoveRevision, MoveSource, MutationResult, RenameBranch,
    RevId, TrackBranch, UndoOperation, UntrackBranch,
};
use worker::{Mutation, Session, SessionEvent, WorkerSession};

use crate::callbacks::FrontendCallbacks;

#[derive(Parser, Debug)]
#[command(version, author)]
struct Args {
    #[arg(
        index(1),
        help = "Open this directory (instead of the current working directory)."
    )]
    workspace: Option<PathBuf>,
    #[arg(short, long, help = "Enable debug logging.")]
    debug: bool,
}

struct WindowState {
    _worker: JoinHandle<()>,
    worker_channel: Sender<SessionEvent>,
    input_channel: Option<Sender<InputResponse>>,
}

#[derive(Default)]
struct AppState(Mutex<Option<WindowState>>);

impl AppState {
    fn get_session(&self) -> Sender<SessionEvent> {
        self.0
            .lock()
            .expect("state mutex poisoned")
            .as_ref()
            .expect("session not found")
            .worker_channel
            .clone()
    }

    fn set_input(&self, tx: Sender<InputResponse>) {
        self.0
            .lock()
            .expect("state mutex poisoned")
            .as_mut()
            .expect("session not found")
            .input_channel = Some(tx);
    }

    fn take_input(&self) -> Option<Sender<InputResponse>> {
        self.0
            .lock()
            .expect("state mutex poisoned")
            .as_mut()
            .expect("session not found")
            .input_channel
            .take()
    }
}

fn main() -> Result<()> {
    // before parsing args, attach a console on windows
    // will fail if not started from a shell, but that's fine
    #[cfg(windows)]
    // safety: FFI
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };

    let args = Args::parse();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                log::info!("vizjj is a single instance app, restoring existing window");
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    StateFlags::SIZE
                        | StateFlags::POSITION
                        | StateFlags::SIZE
                        | StateFlags::FULLSCREEN,
                )
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(LevelFilter::Warn)
                .level_for(
                    "vizjj",
                    match args.debug {
                        true => LevelFilter::Debug,
                        false => LevelFilter::Warn,
                    },
                )
                .level_for(
                    "tao",
                    match args.debug {
                        true => LevelFilter::Info,
                        false => LevelFilter::Error,
                    },
                )
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            notify_window_ready,
            notify_input,
            forward_accelerator,
            query_log,
            query_log_next_page,
            query_revision,
            query_remotes,
            abandon_revisions,
            backout_revisions,
            checkout_revision,
            create_revision,
            describe_revision,
            duplicate_revisions,
            insert_revision,
            move_revision,
            move_source,
            move_changes,
            copy_changes,
            track_branch,
            untrack_branch,
            rename_branch,
            create_ref,
            delete_ref,
            move_ref,
            git_push,
            git_fetch,
            undo_operation,
            query_recent_workspaces,
            open_workspace_at_path,
            open_file_in_explorer,
            open_file_with_default_app,
            menu_repo_open,
            menu_repo_reopen,
        ])
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or(anyhow!("preconfigured window not found"))?;
            let (sender, receiver) = channel();

            let mut handle = window.as_ref().window();
            let window_worker = thread::spawn(move || {
                log::info!("start worker");
                while let Err(err) =
                    WorkerSession::new(FrontendCallbacks(handle.clone()), args.workspace.clone())
                        .handle_events(&receiver)
                        .context("worker")
                {
                    log::info!("restart worker: {err:#}");
                    // it's ok if the worker has to restart,
                    // as long as we can notify the frontend of it
                    handler::fatal!(handle.emit(
                        "vizjj://repo/config",
                        messages::RepoConfig::WorkerError {
                            message: format!("{err:#}"),
                        },
                    ));
                }
            });

            handle = window.as_ref().window();
            window.on_window_event(move |event| handle_window_event(&handle, event));

            let app_state = app.state::<AppState>();
            *app_state.0.lock().unwrap() = Some(WindowState {
                _worker: window_worker,
                worker_channel: sender,
                input_channel: None,
            });

            Ok(())
        })
        .manage(AppState::default())
        .run(tauri::generate_context!())?;

    Ok(())
}

#[tauri::command(async)]
fn notify_window_ready(window: Window) {
    log::debug!("window opened; loading cwd");
    handler::fatal!(window.show());
    handler::nonfatal!(try_open_repository(&window, None));
}

#[tauri::command(async)]
fn notify_input(app_state: State<AppState>, response: InputResponse) -> Result<(), InvokeError> {
    let response_tx = app_state
        .take_input()
        .ok_or(anyhow!("Nobody is listening."))
        .map_err(InvokeError::from_anyhow)?;
    response_tx.send(response).map_err(InvokeError::from_error)
}

#[tauri::command]
fn forward_accelerator(window: Window, key: char) {
    (key == 'o').then(|| open_repo(&window));
}

#[tauri::command(async)]
fn query_log(app_state: State<AppState>, revset: String) -> Result<messages::LogPage, InvokeError> {
    let session_tx: Sender<SessionEvent> = app_state.get_session();
    let (call_tx, call_rx) = channel();

    session_tx
        .send(SessionEvent::QueryLog {
            tx: call_tx,
            query: revset,
        })
        .map_err(InvokeError::from_error)?;
    call_rx
        .recv()
        .map_err(InvokeError::from_error)?
        .map_err(InvokeError::from_anyhow)
}

#[tauri::command(async)]
fn query_log_next_page(app_state: State<AppState>) -> Result<messages::LogPage, InvokeError> {
    let session_tx: Sender<SessionEvent> = app_state.get_session();
    let (call_tx, call_rx) = channel();

    session_tx
        .send(SessionEvent::QueryLogNextPage { tx: call_tx })
        .map_err(InvokeError::from_error)?;
    call_rx
        .recv()
        .map_err(InvokeError::from_error)?
        .map_err(InvokeError::from_anyhow)
}

#[tauri::command(async)]
fn query_revision(
    app_state: State<AppState>,
    id: RevId,
) -> Result<messages::RevResult, InvokeError> {
    let session_tx: Sender<SessionEvent> = app_state.get_session();
    let (call_tx, call_rx) = channel();

    session_tx
        .send(SessionEvent::QueryRevision { tx: call_tx, id })
        .map_err(InvokeError::from_error)?;
    call_rx
        .recv()
        .map_err(InvokeError::from_error)?
        .map_err(InvokeError::from_anyhow)
}

#[tauri::command(async)]
fn query_remotes(
    app_state: State<AppState>,
    tracking_branch: Option<String>,
) -> Result<Vec<String>, InvokeError> {
    let session_tx: Sender<SessionEvent> = app_state.get_session();
    let (call_tx, call_rx) = channel();

    session_tx
        .send(SessionEvent::QueryRemotes {
            tx: call_tx,
            tracking_branch,
        })
        .map_err(InvokeError::from_error)?;
    call_rx
        .recv()
        .map_err(InvokeError::from_error)?
        .map_err(InvokeError::from_anyhow)
}

#[tauri::command(async)]
fn abandon_revisions(
    app_state: State<AppState>,
    mutation: AbandonRevisions,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn backout_revisions(
    app_state: State<AppState>,
    mutation: BackoutRevisions,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn checkout_revision(
    app_state: State<AppState>,
    mutation: CheckoutRevision,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn create_revision(
    app_state: State<AppState>,
    mutation: CreateRevision,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn insert_revision(
    app_state: State<AppState>,
    mutation: InsertRevision,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn describe_revision(
    app_state: State<AppState>,
    mutation: DescribeRevision,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn duplicate_revisions(
    app_state: State<AppState>,
    mutation: DuplicateRevisions,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn move_revision(
    app_state: State<AppState>,
    mutation: MoveRevision,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn move_source(
    app_state: State<AppState>,
    mutation: MoveSource,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn move_changes(
    app_state: State<AppState>,
    mutation: MoveChanges,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn copy_changes(
    app_state: State<AppState>,
    mutation: CopyChanges,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn track_branch(
    app_state: State<AppState>,
    mutation: TrackBranch,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn untrack_branch(
    app_state: State<AppState>,
    mutation: UntrackBranch,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn rename_branch(
    app_state: State<AppState>,
    mutation: RenameBranch,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn create_ref(
    app_state: State<AppState>,
    mutation: CreateRef,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn delete_ref(
    app_state: State<AppState>,
    mutation: DeleteRef,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn move_ref(app_state: State<AppState>, mutation: MoveRef) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn git_push(app_state: State<AppState>, mutation: GitPush) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn git_fetch(
    app_state: State<AppState>,
    mutation: GitFetch,
) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, mutation)
}

#[tauri::command(async)]
fn undo_operation(app_state: State<AppState>) -> Result<MutationResult, InvokeError> {
    try_mutate(app_state, UndoOperation)
}

fn try_open_repository(window: &Window, cwd: Option<PathBuf>) -> Result<()> {
    log::info!("load workspace {cwd:#?}");

    let app_state = window.state::<AppState>();

    let session_tx: Sender<SessionEvent> = app_state.get_session();
    let (call_tx, call_rx) = channel();

    session_tx.send(SessionEvent::OpenWorkspace {
        tx: call_tx,
        wd: cwd.clone(),
    })?;

    match call_rx.recv()? {
        Ok(config) => {
            log::debug!("load workspace succeeded");
            if let messages::RepoConfig::Workspace { absolute_path, .. } = &config {
                let repo_path = absolute_path.0.clone();
                let window = window.clone();
                thread::spawn(move || {
                    handler::nonfatal!(add_recent_workspaces(window, &repo_path));
                });
            }
            window.emit("vizjj://repo/config", config)?;
        }
        Err(err) => {
            log::warn!("load workspace failed: {err}");
            window.emit(
                "vizjj://repo/config",
                messages::RepoConfig::LoadError {
                    absolute_path: cwd.unwrap_or_default().into(),
                    message: format!("{err:#?}"),
                },
            )?;
        }
    }

    Ok(())
}

fn try_mutate<T: Mutation + Send + Sync + 'static>(
    app_state: State<AppState>,
    mutation: T,
) -> Result<MutationResult, InvokeError> {
    let session_tx: Sender<SessionEvent> = app_state.get_session();
    let (call_tx, call_rx) = channel();

    session_tx
        .send(SessionEvent::ExecuteMutation {
            tx: call_tx,
            mutation: Box::new(mutation),
        })
        .map_err(InvokeError::from_error)?;
    call_rx.recv().map_err(InvokeError::from_error)
}

fn handle_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::Focused(true) = *event {
        log::debug!("window focused; requesting snapshot");

        let app_state = window.state::<AppState>();

        let session_tx: Sender<SessionEvent> = app_state.get_session();
        let (call_tx, call_rx) = channel();

        handler::nonfatal!(session_tx.send(SessionEvent::ExecuteSnapshot { tx: call_tx }));

        // events are handled on the main thread, so don't wait for
        // a worker response - that's a recipe for deadlock
        let window = window.clone();
        thread::spawn(move || {
            if let Some(status) = handler::nonfatal!(call_rx.recv()) {
                handler::nonfatal!(window.emit("vizjj://repo/status", status));
            }
        });
    }
}

fn add_recent_workspaces(window: Window, repo_path: &str) -> Result<()> {
    let app_state = window.state::<AppState>();
    let session_tx: Sender<SessionEvent> = app_state.get_session();

    let (read_tx, read_rx) = channel();
    session_tx.send(SessionEvent::ReadConfigArray {
        key: vec![
            "vizjj".to_string(),
            "ui".to_string(),
            "recent-workspaces".to_string(),
        ],
        tx: read_tx,
    })?;
    let mut recent = read_rx.recv()??;
    recent.retain(|x| x != repo_path);
    recent.insert(0, repo_path.to_owned());
    recent.truncate(5);

    session_tx.send(SessionEvent::WriteConfigArray {
        key: vec![
            "vizjj".to_string(),
            "ui".to_string(),
            "recent-workspaces".to_string(),
        ],
        scope: ConfigSource::User,
        values: recent,
    })?;

    Ok(())
}

#[tauri::command(async)]
fn query_recent_workspaces(app_state: State<AppState>) -> Result<Vec<String>, InvokeError> {
    let session_tx: Sender<SessionEvent> = app_state.get_session();
    let (call_tx, call_rx) = channel();
    session_tx
        .send(SessionEvent::ReadConfigArray {
            key: vec![
                "vizjj".to_string(),
                "ui".to_string(),
                "recent-workspaces".to_string(),
            ],
            tx: call_tx,
        })
        .map_err(InvokeError::from_error)?;

    match call_rx.recv().map_err(InvokeError::from_error)? {
        Ok(workspaces) => Ok(workspaces),
        Err(_) => Ok(vec![]),
    }
}

#[tauri::command]
fn open_workspace_at_path(window: Window, path: String) -> Result<(), InvokeError> {
    match try_open_repository(&window, Some(PathBuf::from(path))) {
        Ok(_) => Ok(()),
        Err(err) => {
            log::error!("try_open_repository: {err:#}");
            Err(InvokeError::from_anyhow(err))
        }
    }
}

#[tauri::command(async, rename_all = "snake_case")]
fn open_file_in_explorer(file_path: String) -> Result<(), InvokeError> {
    opener::reveal(&file_path).map_err(|e| InvokeError::from_anyhow(e.into()))?;
    Ok(())
}

#[tauri::command(async, rename_all = "snake_case")]
fn open_file_with_default_app(file_path: String) -> Result<(), InvokeError> {
    opener::open(&file_path).map_err(|e| InvokeError::from_anyhow(e.into()))?;
    Ok(())
}

#[tauri::command]
fn menu_repo_open(window: Window) {
    open_repo(&window);
}

#[tauri::command]
fn menu_repo_reopen(window: Window) {
    handler::fatal!(crate::try_open_repository(&window, None).context("try_open_repository"));
}

fn open_repo(window: &Window) {
    let window = window.clone();
    window.dialog().file().pick_folder(move |picked| {
        if let Some(FilePath::Path(cwd)) = picked {
            handler::fatal!(
                crate::try_open_repository(&window, Some(cwd)).context("try_open_repository")
            );
        }
    });
}
