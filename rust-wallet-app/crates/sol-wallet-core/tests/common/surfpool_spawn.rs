#![allow(clippy::assertions_on_constants, clippy::let_underscore_future)]
//! `SurfpoolGuard` RAII helper — spawns a local surfpool validator on an
//! ephemeral port.

use std::process::Stdio;
use std::time::{Duration, Instant};

use sol_wallet_core::chain::{account::get_health, client::RpcClient};

const BOOT_DEADLINE: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Errors that can surface from `spawn_surfpool`.
#[derive(Debug)]
pub enum SurfpoolError {
    NotFound,
    SpawnFailed(String),
    BootTimeout,
}

impl std::fmt::Display for SurfpoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "surfpool binary not found on PATH"),
            Self::SpawnFailed(e) => write!(f, "surfpool spawn failed: {e}"),
            Self::BootTimeout => write!(
                f,
                "surfpool did not respond to get_health within {BOOT_DEADLINE:?}"
            ),
        }
    }
}

impl std::error::Error for SurfpoolError {}

/// RAII guard owning a surfpool child process + the ephemeral RPC URL.
pub struct SurfpoolGuard {
    rpc_url: String,
    child: Option<tokio::process::Child>,
}

impl SurfpoolGuard {
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    pub fn child(&mut self) -> &mut tokio::process::Child {
        self.child.as_mut().expect("surfpool child present")
    }
}

impl Drop for SurfpoolGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            let _ = child.wait();
        }
    }
}

/// Spawn surfpool on an ephemeral port and wait for `get_health` to return Ok.
pub async fn spawn_surfpool() -> Result<SurfpoolGuard, SurfpoolError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| SurfpoolError::SpawnFailed(format!("bind: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| SurfpoolError::SpawnFailed(format!("local_addr: {e}")))?
        .port();
    drop(listener);

    let rpc_url = format!("http://127.0.0.1:{port}");

    let mut cmd = tokio::process::Command::new("surfpool");
    // surfpool v1.5.0 CLI: `--port` is the RPC bind port (was `--rpc-port`
    // in older 0.x). `--no-tui` avoids trying to draw an interactive TUI
    // inside the test runner (no TTY). `--offline` skips the remote
    // datasource fork so the local fixture boots fully offline.
    cmd.args([
        "start",
        "--port",
        &port.to_string(),
        "--host",
        "127.0.0.1",
        "--no-tui",
        "--offline",
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true);

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SurfpoolError::NotFound);
        }
        Err(e) => return Err(SurfpoolError::SpawnFailed(e.to_string())),
    };

    let mut guard = SurfpoolGuard {
        rpc_url: rpc_url.clone(),
        child: Some(child),
    };

    let rpc = RpcClient::new(&rpc_url)
        .map_err(|e| SurfpoolError::SpawnFailed(format!("RpcClient::new: {e}")))?;
    let deadline = Instant::now() + BOOT_DEADLINE;
    loop {
        if get_health(&rpc).await.is_ok() {
            return Ok(guard);
        }
        if Instant::now() >= deadline {
            return Err(SurfpoolError::BootTimeout);
        }
        if let Some(status) = guard
            .child()
            .try_wait()
            .map_err(|e| SurfpoolError::SpawnFailed(format!("try_wait: {e}")))?
        {
            return Err(SurfpoolError::SpawnFailed(format!(
                "surfpool exited early with {status:?}"
            )));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
