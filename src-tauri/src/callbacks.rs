use jj_lib::git::GitSubprocessCallback;
use tauri::Window;

use crate::worker::{LogGitCallback, WorkerCallbacks};

pub struct FrontendCallbacks(#[allow(dead_code)] pub Window);

impl WorkerCallbacks for FrontendCallbacks {
    fn git_subprocess_callback(&self) -> Box<dyn GitSubprocessCallback> {
        Box::new(LogGitCallback)
    }
}
