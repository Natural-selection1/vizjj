//! Worker per window, owning repo data (jj-lib is not thread-safe)
//! The worker thread is a state machine, running different handle functions based on loaded data

mod gui_util;
mod mutations;
mod queries;
mod session;
#[cfg(all(test, not(feature = "ts-rs")))]
mod tests;

use std::{
    env::{self, VarError},
    fmt::Debug,
    fs, io,
    path::PathBuf,
};

use anyhow::{Error, Result, anyhow};
use jj_lib::{
    git::{GitProgress, GitSidebandLineTerminator, GitSubprocessCallback},
    settings::UserSettings,
};

use crate::{config::read_config, messages};
use gui_util::WorkspaceSession;
pub use session::{Session, SessionEvent};

/// implemented by structured-change commands
pub trait Mutation: Debug {
    fn describe(&self) -> String {
        std::any::type_name::<Self>().to_owned()
    }

    fn execute(self: Box<Self>, ws: &mut WorkspaceSession) -> Result<messages::MutationResult>;

    #[cfg(test)]
    fn execute_unboxed(self, ws: &mut WorkspaceSession) -> Result<messages::MutationResult>
    where
        Self: Sized,
    {
        Box::new(self).execute(ws)
    }
}

/// implemented by UI layers to request user input and receive progress
pub trait WorkerCallbacks {
    fn git_subprocess_callback(&self) -> Box<dyn GitSubprocessCallback>;
}

struct NoCallbacks;

impl WorkerCallbacks for NoCallbacks {
    fn git_subprocess_callback(&self) -> Box<dyn GitSubprocessCallback> {
        Box::new(LogGitCallback)
    }
}

pub struct LogGitCallback;

impl GitSubprocessCallback for LogGitCallback {
    fn needs_progress(&self) -> bool {
        false
    }

    fn progress(&mut self, progress: &GitProgress) -> io::Result<()> {
        log::debug!("git progress: {progress:?}");
        Ok(())
    }

    fn local_sideband(
        &mut self,
        message: &[u8],
        _term: Option<GitSidebandLineTerminator>,
    ) -> io::Result<()> {
        log::info!("git: {}", String::from_utf8_lossy(message));
        Ok(())
    }

    fn remote_sideband(
        &mut self,
        message: &[u8],
        _term: Option<GitSidebandLineTerminator>,
    ) -> io::Result<()> {
        log::info!("remote: {}", String::from_utf8_lossy(message));
        Ok(())
    }
}

/// state that doesn't depend on jj-lib borrowings
pub struct WorkerSession {
    pub force_log_page_size: Option<usize>,
    pub latest_query: Option<String>,
    pub callbacks: Box<dyn WorkerCallbacks>,
    pub working_directory: Option<PathBuf>,
    #[allow(dead_code, reason = "read in session.rs::handle_events")]
    pub global_settings: Option<UserSettings>,
}

impl WorkerSession {
    pub fn new<T: WorkerCallbacks + 'static>(callbacks: T, workspace: Option<PathBuf>) -> Self {
        let global_settings = match read_config(None, None) {
            Ok((settings, _, _)) => Some(settings),
            Err(_) => None,
        };
        WorkerSession {
            callbacks: Box::new(callbacks),
            working_directory: workspace,
            global_settings,
            ..Default::default()
        }
    }

    // AppImage runs the executable from somewhere weird, but sets OWD=cwd() first.
    pub fn get_cwd(&self) -> Result<PathBuf> {
        self.working_directory
            .as_ref()
            .map(|cwd| Ok(fs::canonicalize(cwd.clone())?))
            .or_else(|| match env::var("OWD") {
                Ok(var) => Some(Ok(PathBuf::from(var))),
                Err(VarError::NotPresent) => None,
                Err(err) => Some(Err(anyhow!(err))),
            })
            .unwrap_or_else(|| env::current_dir().map_err(Error::new))
    }
}

impl Default for WorkerSession {
    fn default() -> Self {
        WorkerSession {
            force_log_page_size: None,
            latest_query: None,
            callbacks: Box::new(NoCallbacks),
            working_directory: None,
            global_settings: None,
        }
    }
}
