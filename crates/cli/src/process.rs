use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestProcess {
    pub id: Uuid,
    pub command: String,
    pub args: Vec<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub status: ProcessStatus,
    pub output_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessStatus {
    Running,
    Paused,
    Completed { exit_code: i32 },
    Failed { error: String },
    Stopped,
}

pub struct ProcessManager {
    processes: Arc<RwLock<HashMap<Uuid, TestProcess>>>,
    running_processes: Arc<Mutex<HashMap<Uuid, Child>>>,
    cache_dir: PathBuf,
}

impl ProcessManager {
    pub fn new() -> color_eyre::Result<Self> {
        let cache_dir = Self::get_cache_dir()?;
        fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            running_processes: Arc::new(Mutex::new(HashMap::new())),
            cache_dir,
        })
    }

    pub async fn start_process(
        &self,
        command: String,
        args: Vec<String>,
        output_dir: Option<PathBuf>,
    ) -> color_eyre::Result<Uuid> {
        let id = Uuid::new_v4();
        let output_file = output_dir.map(|dir| dir.join(format!("{}.json", id)));
        let test_process = TestProcess {
            id,
            command: command.clone(),
            args: args.clone(),
            started_at: chrono::Utc::now(),
            status: ProcessStatus::Running,
            output_file: output_file.clone(),
        };

        self.save_process_info(&test_process).await?;

        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        if let Some(output_path) = &output_file {
            let output_file = fs::File::create(output_path)?;
            cmd.stdout(output_file);
        }

        let child = cmd.spawn().map_err(|_| {
            sheila::Error::generic(format!("Failed to start process: {} {:?}", command, args))
        })?;

        {
            let mut running = self.running_processes.lock().unwrap();
            running.insert(id, child);
        }

        {
            let mut processes = self.processes.write().await;
            processes.insert(id, test_process);
        }

        Ok(id)
    }

    pub async fn stop_process(&self, id: Uuid) -> color_eyre::Result<()> {
        let mut running = self.running_processes.lock().unwrap();

        if let Some(mut child) = running.remove(&id) {
            child
                .kill()
                .map_err(|_| sheila::Error::generic(format!("Failed to kill process {}", id)))?;

            let mut processes = self.processes.write().await;
            if let Some(process) = processes.get_mut(&id) {
                process.status = ProcessStatus::Stopped;
                self.save_process_info(process).await?;
            }
        }

        Ok(())
    }

    pub async fn pause_process(&self, id: Uuid) -> color_eyre::Result<()> {
        #[cfg(unix)]
        {
            let running = self.running_processes.lock().unwrap();
            if let Some(child) = running.get(&id) {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGSTOP);
                }

                let mut processes = self.processes.write().await;
                if let Some(process) = processes.get_mut(&id) {
                    process.status = ProcessStatus::Paused;
                    self.save_process_info(process).await?;
                }
            }
        }

        #[cfg(not(unix))]
        {
            return Err(sheila::Error::generic(
                "Process pausing is not supported on this platform",
            ));
        }

        Ok(())
    }

    pub async fn resume_process(&self, id: Uuid) -> color_eyre::Result<()> {
        #[cfg(unix)]
        {
            let running = self.running_processes.lock().unwrap();
            if let Some(child) = running.get(&id) {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGCONT);
                }

                let mut processes = self.processes.write().await;
                if let Some(process) = processes.get_mut(&id) {
                    process.status = ProcessStatus::Running;
                    self.save_process_info(process).await?;
                }
            }
        }

        #[cfg(not(unix))]
        {
            return Err(sheila::Error::generic(
                "Process resuming is not supported on this platform",
            ));
        }

        Ok(())
    }

    pub async fn get_process(&self, id: Uuid) -> Option<TestProcess> {
        let processes = self.processes.read().await;
        processes.get(&id).cloned()
    }

    pub async fn list_processes(&self) -> Vec<TestProcess> {
        let processes = self.processes.read().await;
        processes.values().cloned().collect()
    }

    pub async fn cleanup_completed(&self) -> color_eyre::Result<()> {
        let mut to_remove = Vec::new();

        {
            let mut running = self.running_processes.lock().unwrap();
            let mut processes = self.processes.write().await;

            for (id, child) in running.iter_mut() {
                if let Ok(Some(exit_status)) = child.try_wait() {
                    if let Some(process) = processes.get_mut(id) {
                        process.status = ProcessStatus::Completed {
                            exit_code: exit_status.code().unwrap_or(-1),
                        };
                        self.save_process_info(process).await?;
                    }
                    to_remove.push(*id);
                }
            }

            for id in to_remove {
                running.remove(&id);
            }
        }

        Ok(())
    }

    pub async fn clear_cache(&self) -> color_eyre::Result<()> {
        {
            let mut processes = self.processes.write().await;
            processes.clear();
        }

        {
            let mut running = self.running_processes.lock().unwrap();
            running.clear();
        }

        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
            fs::create_dir_all(&self.cache_dir)?;
        }

        Ok(())
    }

    async fn save_process_info(&self, process: &TestProcess) -> color_eyre::Result<()> {
        let cache_file = self.cache_dir.join(format!("{}.json", process.id));
        let json = serde_json::to_string_pretty(process)?;
        fs::write(cache_file, json)?;
        Ok(())
    }

    pub async fn load_from_cache(&self) -> color_eyre::Result<()> {
        if !self.cache_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(process) = serde_json::from_str::<TestProcess>(&content) {
                        let mut processes = self.processes.write().await;
                        processes.insert(process.id, process);
                    }
                }
            }
        }

        Ok(())
    }

    fn get_cache_dir() -> color_eyre::Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| sheila::Error::generic("Could not find home directory"))?;
        Ok(home.join(".sheila").join("cache"))
    }
}
