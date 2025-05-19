use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::net::TcpListener;
use std::io::{BufRead, BufReader};
use std::thread;

#[derive(Debug, Serialize, Deserialize)]
pub struct Tunnel {
    pub alias: String,
    pub source: String,
    pub port: u16,
    #[serde(skip)]
    process: Mutex<Option<Child>>,
}

impl Tunnel {
    pub fn new(alias: &str, source: &str, port: u16) -> Self {
        Self {
            alias: alias.to_string(),
            source: source.to_string(),
            port,
            process: Mutex::new(None),
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
        // 首先检查我们自己启动的进程
        if let Ok(guard) = self.process.lock() {
            if let Some(process) = guard.as_ref() {
                if process.id() != 0 {
                    return true;
                }
            }
        }

        // 然后检查系统中是否有使用相同端口的 cloudflared 进程
        if cfg!(windows) {
            // 在 Windows 上使用 netstat 检查端口
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
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // 在 Unix 系统上使用 lsof 检查端口
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
        TcpListener::bind(format!("127.0.0.1:{}", self.port)).is_ok()
    }
    
    pub fn start(&self) -> anyhow::Result<()> {
        if self.is_running() {
            return Ok(());
        }

        // 检查端口是否可用
        if !self.is_port_available() {
            return Err(anyhow::anyhow!(
                "端口 {} 已被占用，请确保没有其他程序正在使用该端口",
                self.port
            ));
        }
        
        // 在 Windows 上使用 where 命令查找 cloudflared 的路径
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
        
        // 启动 cloudflared 并捕获输出
        let mut process = Command::new(cloudflared_path)
            .args(&["access", "tcp", "--hostname", &self.source, "--url", &format!("tcp://localhost:{}", self.port)])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = process.stdout.take().unwrap();
        let mut stderr = process.stderr.take().unwrap();

        // 只读取前2秒的错误输出
        let mut err_buf = String::new();
        let start = std::time::Instant::now();
        let mut reader = BufReader::new(&mut stderr);
        while start.elapsed().as_secs() < 2 {
            let mut line = String::new();
            let n = reader.read_line(&mut line)?;
            if n == 0 { break; }
            err_buf.push_str(&line);
            if line.to_lowercase().contains("error") || line.to_lowercase().contains("failed") {
                // 进程启动失败，直接杀掉进程并返回错误
                let _ = process.kill();
                return Err(anyhow::anyhow!("cloudflared 启动失败: {}", line.trim()));
            }
        }

        // 后台线程继续打印日志
        let alias = self.alias.clone();
        thread::spawn(move || {
            let stdout_reader = BufReader::new(stdout);
            for line in stdout_reader.lines() {
                if let Ok(line) = line {
                    println!("[{}] {}", alias, line);
                }
            }
        });

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
} 