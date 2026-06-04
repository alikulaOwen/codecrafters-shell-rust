#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Done,
}

pub struct Job {
    pub id: usize,
    pub child: std::process::Child,
    pub command: String,
    pub status: JobStatus,
}

pub struct JobManager {
    pub jobs: Vec<Job>,
}

fn clean_command(command: &str) -> String {
    let mut s = command.trim().to_string();
    if s.ends_with('&') {
        s.pop();
    }
    s.trim().to_string()
}

impl JobManager {
    pub fn new() -> Self {
        JobManager { jobs: Vec::new() }
    }

    // Find the next available job ID by recycling unused lower numbers
    pub fn get_next_id(&self) -> usize {
        let mut id = 1;
        while self.jobs.iter().any(|j| j.id == id) {
            id += 1;
        }
        id
    }

    pub fn add_job(&mut self, child: std::process::Child, command: String) -> usize {
        let id = self.get_next_id();
        self.jobs.push(Job {
            id,
            child,
            command,
            status: JobStatus::Running,
        });
        id
    }

    // Non-blocking reap and print notification for jobs that have finished
    pub fn reap_and_notify(&mut self) {
        // Poll running jobs
        for job in &mut self.jobs {
            if job.status == JobStatus::Running {
                if let Ok(Some(_)) = job.child.try_wait() {
                    job.status = JobStatus::Done;
                }
            }
        }

        let len = self.jobs.len();
        let mut to_remove = Vec::new();

        for (idx, job) in self.jobs.iter().enumerate() {
            if job.status == JobStatus::Done {
                let symbol = if idx == len - 1 {
                    "+"
                } else if len >= 2 && idx == len - 2 {
                    "-"
                } else {
                    " "
                };
                let cmd_str = clean_command(&job.command);
                println!("[{}]{}  {:<24}{}", job.id, symbol, "Done", cmd_str);
                to_remove.push(job.id);
            }
        }

        self.jobs.retain(|j| !to_remove.contains(&j.id));
    }

    // Prints status for all active jobs, then reaps any finished jobs
    pub fn list_jobs(&mut self) {
        for job in &mut self.jobs {
            if job.status == JobStatus::Running {
                if let Ok(Some(_)) = job.child.try_wait() {
                    job.status = JobStatus::Done;
                }
            }
        }

        let len = self.jobs.len();
        let mut to_remove = Vec::new();

        for (idx, job) in self.jobs.iter().enumerate() {
            let symbol = if idx == len - 1 {
                "+"
            } else if len >= 2 && idx == len - 2 {
                "-"
            } else {
                " "
            };

            let status_str = match job.status {
                JobStatus::Running => "Running",
                JobStatus::Done => "Done",
            };

            let cmd_str = match job.status {
                JobStatus::Running => format!("{} &", clean_command(&job.command)),
                JobStatus::Done => clean_command(&job.command),
            };

            println!("[{}]{}  {:<24}{}", job.id, symbol, status_str, cmd_str);
            if job.status == JobStatus::Done {
                to_remove.push(job.id);
            }
        }

        self.jobs.retain(|j| !to_remove.contains(&j.id));
    }
}
