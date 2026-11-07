//! The sandbox choice: host execution or the Docker container, behind one
//! enum so the loop, the verifier and the manifest do not care which.

use std::path::Path;

use crate::docker_executor::DockerExecutor;
use crate::json::Value;
use crate::local_executor::LocalToolExecutor;
use crate::protocol::{RunCommandArgs, ToolCall, ToolObservation};

#[derive(Debug)]
pub enum Executor {
    Local(LocalToolExecutor),
    Docker(DockerExecutor),
}

impl Executor {
    pub fn workspace(&self) -> &Path {
        match self {
            Executor::Local(local) => local.workspace(),
            Executor::Docker(docker) => docker.files().workspace(),
        }
    }

    /// `"local"` or `"docker"`, as the manifest records it.
    pub fn mode(&self) -> &'static str {
        match self {
            Executor::Local(_) => "local",
            Executor::Docker(_) => "docker",
        }
    }

    pub fn execute(&self, call: &ToolCall) -> ToolObservation {
        match self {
            Executor::Local(local) => local.execute(call),
            Executor::Docker(docker) => docker.execute(call),
        }
    }

    /// # Errors
    ///
    /// The Python exception text, as the tool observation would carry it.
    pub fn run_command(&self, args: &RunCommandArgs) -> Result<Value, String> {
        match self {
            Executor::Local(local) => local.run_command(args),
            Executor::Docker(docker) => docker.run_command(args),
        }
    }

    pub fn image(&self) -> Option<&str> {
        match self {
            Executor::Local(_) => None,
            Executor::Docker(docker) => Some(&docker.image),
        }
    }

    /// The container resource band, or `None` for host execution (uncapped).
    pub fn resources(&self) -> Option<Value> {
        match self {
            Executor::Local(_) => None,
            Executor::Docker(docker) => Some(docker.resources.to_dict()),
        }
    }

    pub fn mem_ceiling(&self) -> Option<&str> {
        match self {
            Executor::Local(_) => None,
            Executor::Docker(docker) => Some(&docker.resources.mem_ceiling),
        }
    }

    pub fn oom_kills(&self) -> Option<i64> {
        match self {
            Executor::Local(_) => None,
            Executor::Docker(docker) => Some(docker.oom_kill_count()),
        }
    }

    /// Tears the container down; a no-op for host execution.
    pub fn close(&self) {
        if let Executor::Docker(docker) = self {
            docker.close();
        }
    }
}
