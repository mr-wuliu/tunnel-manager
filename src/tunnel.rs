use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, Arc};
use std::net::TcpListener;
use std::io::{BufRead, BufReader, Read};
use std::thread;
use std::collections::VecDeque;

const MAX_LOG_LINES: usize = 1000;

#[derive(Debug, Clone)]
pub struct LogBuffer {
    lines: Arc<Mutex<VecDeque<String>>>,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            lines: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES))),
        }
    }

    pub fn add_line(&self, line: String) {
        if let Ok(mut lines) = self.lines.lock() {
            if lines.len() >= MAX_LOG_LINES {
                lines.pop_front();
            }
            lines.push_back(line);
        }
    }

    pub fn get_lines(&self) -> Vec<String> {
        if let Ok(lines) = self.lines.lock() {
            lines.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tunnel {
    pub alias: String,
    pub source: String,
    pub port: u16,
    #[serde(skip)]
    process: Mutex<Option<Child>>,
    #[serde(skip)]
    log_buffer: LogBuffer,
}

impl Tunnel {
    pub fn new(alias: &str, source: &str, port: u16) -> Self {
        Self {
            alias: alias.to_string(),
            source: source.to_string(),
            port,
            process: Mutex::new(None),
            log_buffer: LogBuffer::new(),
        }
    }
    
    pub fn check_cloudflared() -> anyhow::Result<bool> {
        let output = if cfg!(windows) {
            Command::new("where")
                .arg("cloudflared")
                .output()?
        } else {
            Command::new("which")
                .arg("cloudflared")
                .output()?
        };
        
        Ok(output.status.success())
    }

    pub fn install_cloudflared() -> anyhow::Result<()> {
        if cfg!(windows) {
            // Windows 使用 winget 安装
            let status = Command::new("winget")
                .args(&["install", "--id", "Cloudflare.cloudflared", "--silent"])
                .status()?;
            
            if !status.success() {
                return Err(anyhow::anyhow!("安装 cloudflared 失败，请手动运行: winget install --id Cloudflare.cloudflared"));
            }
        } else if cfg!(target_os = "macos") {
            // macOS 使用 brew 安装
            let status = Command::new("brew")
                .args(&["install", "cloudflared"])
                .status()?;
            
            if !status.success() {
                return Err(anyhow::anyhow!("安装 cloudflared 失败，请手动运行: brew install cloudflared"));
            }
        } else {
            return Err(anyhow::anyhow!("不支持的操作系统"));
        }
        
        Ok(())
    }
    
    pub fn is_running(&self) -> bool {
        // 检查是否有 cloudflared 进程在使用特定端口
        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("netstat")
                .args(&["-aon"])
                .output() 
            {
                let output = String::from_utf8_lossy(&output.stdout);
                for line in output.lines() {
                    if line.contains(&format!(":{}", self.port)) {
                        if let Some(pid) = line.split_whitespace().last() {
                            if let Ok(pid) = pid.parse::<u32>() {
                                // 检查进程名称
                                if let Ok(tasklist) = Command::new("tasklist")
                                    .args(&["/FI", &format!("PID eq {}", pid)])
                                    .output()
                                {
                                    let tasklist = String::from_utf8_lossy(&tasklist.stdout);
                                    if tasklist.contains("cloudflared.exe") {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(output) = Command::new("lsof")
                .args(&["-i", &format!(":{}", self.port)])
                .output() 
            {
                let output = String::from_utf8_lossy(&output.stdout);
                if output.contains("cloudflared") {
                    return true;
                }
            }
        }

        false
    }
    
    pub fn status(&self) -> &'static str {
        if self.is_running() {
            "running"
        } else {
            "stopped"
        }
    }

    fn is_port_available(&self) -> bool {
        // 先检查端口是否被占用
        if TcpListener::bind(format!("127.0.0.1:{}", self.port)).is_err() {
            return false;
        }

        // 再检查是否有 cloudflared 进程在使用这个端口
        if cfg!(windows) {
            if let Ok(output) = Command::new("netstat")
                .args(&["-aon"])
                .output() 
            {
                let output = String::from_utf8_lossy(&output.stdout);
                for line in output.lines() {
                    if line.contains(&format!(":{}", self.port)) {
                        if let Some(pid) = line.split_whitespace().last() {
                            if let Ok(pid) = pid.parse::<u32>() {
                                // 检查进程名称
                                if let Ok(tasklist) = Command::new("tasklist")
                                    .args(&["/FI", &format!("PID eq {}", pid)])
                                    .output()
                                {
                                    let tasklist = String::from_utf8_lossy(&tasklist.stdout);
                                    if tasklist.contains("cloudflared") {
                                        return false;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            if let Ok(output) = Command::new("lsof")
                .args(&["-i", &format!(":{}", self.port)])
                .output() 
            {
                let output = String::from_utf8_lossy(&output.stdout);
                if output.contains("cloudflared") {
                    return false;
                }
            }
        }

        true
    }
    
    pub fn start(&self) -> anyhow::Result<()> {
        // 1. 先检查端口
        if !self.is_port_available() {
            return Err(anyhow::anyhow!(
                "端口 {} 已被占用，请确保没有其他程序正在使用该端口",
                self.port
            ));
        }

        // 2. 启动 cloudflared
        let cloudflared_path = if cfg!(windows) {
            let output = Command::new("where")
                .arg("cloudflared")
                .output()?;
            
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("cloudflared")
                    .trim()
                    .to_string()
            } else {
                "cloudflared".to_string()
            }
        } else {
            "cloudflared".to_string()
        };
        
        let mut process = Command::new(cloudflared_path)
            .args(&["access", "tcp", "--hostname", &self.source, "--url", &format!("tcp://localhost:{}", self.port)])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = process.stdout.take().unwrap();
        let stderr = process.stderr.take().unwrap();
        let log_buffer = self.log_buffer.clone();
        let alias = self.alias.clone();

        // 3. 日志线程
        let log_buffer_clone = log_buffer.clone();
        let alias_clone = alias.clone();
        thread::spawn(move || {
            let stdout_reader = BufReader::new(stdout);
            for line in stdout_reader.lines() {
                if let Ok(line) = line {
                    let log_line = format!("[{}] {}", alias_clone, line);
                    println!("{}", log_line);
                    log_buffer_clone.add_line(log_line);
                }
            }
        });

        // 4. 启动后等待1秒，检查进程是否已退出
        std::thread::sleep(std::time::Duration::from_secs(1));
        if let Ok(Some(status)) = process.try_wait() {
            // 进程已退出，采集 stderr
            let mut err_reader = BufReader::new(stderr);
            let mut err_msg = String::new();
            let _ = err_reader.read_to_string(&mut err_msg);
            let error_msg = if err_msg.trim().is_empty() {
                format!("cloudflared 启动失败，退出码: {}", status)
            } else {
                format!("cloudflared 启动失败: {}", err_msg.trim())
            };
            log_buffer.add_line(format!("[{}][stderr] {}", alias, error_msg));
            return Err(anyhow::anyhow!(error_msg));
        }

        // 5. 再次检查进程是否真的在运行
        if !self.is_running() {
            let error_msg = "cloudflared 进程启动后立即退出";
            log_buffer.add_line(format!("[{}][error] {}", alias, error_msg));
            return Err(anyhow::anyhow!(error_msg));
        }

        if let Ok(mut guard) = self.process.lock() {
            *guard = Some(process);
        }

        Ok(())
    }
    
    pub fn stop(&self) -> anyhow::Result<()> {
        // 首先尝试停止我们自己启动的进程
        if let Ok(mut guard) = self.process.lock() {
            if let Some(mut process) = guard.take() {
                process.kill()?;
            }
        }

        // 然后尝试停止系统中使用相同端口的 cloudflared 进程
        if cfg!(windows) {
            if let Ok(output) = Command::new("netstat")
                .args(&["-aon"])
                .output() 
            {
                let output = String::from_utf8_lossy(&output.stdout);
                for line in output.lines() {
                    if line.contains(&format!(":{}", self.port)) {
                        if let Some(pid) = line.split_whitespace().last() {
                            if let Ok(pid) = pid.parse::<u32>() {
                                let _ = Command::new("taskkill")
                                    .args(&["/F", "/PID", &pid.to_string()])
                                    .output();
                            }
                        }
                    }
                }
            }
        } else {
            if let Ok(output) = Command::new("lsof")
                .args(&["-i", &format!(":{}", self.port)])
                .output() 
            {
                let output = String::from_utf8_lossy(&output.stdout);
                for line in output.lines() {
                    if line.contains("cloudflared") {
                        if let Some(pid) = line.split_whitespace().nth(1) {
                            if let Ok(pid) = pid.parse::<u32>() {
                                let _ = Command::new("kill")
                                    .args(&["-9", &pid.to_string()])
                                    .output();
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_logs(&self) -> Vec<String> {
        self.log_buffer.get_lines()
    }
} 