use anyhow::Result;
use clap::Subcommand;
use dialoguer::{theme::ColorfulTheme, Select};
use indicatif::ProgressBar;

use crate::config::Config;

#[derive(Subcommand)]
pub enum Commands {
    /// 列出所有连接
    List,
    /// 运行选定的连接
    Run,
    /// 停止选定的连接
    Stop,
    /// 设置连接参数
    Set {
        /// 连接别名
        alias: String,
        /// 源地址
        #[arg(long)]
        source: Option<String>,
        /// 本地端口
        #[arg(long)]
        port: Option<u16>,
    },
    /// 移除连接
    Remove {
        /// 连接别名
        alias: String,
    },
    /// 添加新连接
    Add {
        /// 连接别名
        alias: String,
        /// 源地址
        #[arg(long)]
        source: String,
        /// 本地端口
        #[arg(long)]
        port: u16,
    },
}

impl Commands {
    pub async fn execute(&self) -> Result<()> {
        let mut config = Config::load()?;
        
        match self {
            Commands::List => {
                let tunnels = config.list_tunnels()?;
                println!("{:<15} {:<30} {:<20} {:<10}", "alias", "source", "target", "status");
                for tunnel in tunnels {
                    println!("{:<15} {:<30} {:<20} {:<10}", 
                        tunnel.alias,
                        tunnel.source,
                        format!("tcp://localhost:{}", tunnel.port),
                        tunnel.status()
                    );
                }
            }
            Commands::Run => {
                let tunnels = config.list_tunnels()?;
                if tunnels.is_empty() {
                    println!("没有配置任何连接，请先使用 'cfa add' 添加连接");
                    return Ok(());
                }

                let items: Vec<String> = tunnels.iter()
                    .map(|t| format!("{} ({})", t.alias, t.source))
                    .collect();
                
                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("选择要运行的连接")
                    .items(&items)
                    .default(0)
                    .interact()?;
                
                let tunnel = &tunnels[selection];
                let pb = ProgressBar::new_spinner();
                pb.set_message(format!("正在启动 {}...", tunnel.alias));
                
                match tunnel.start() {
                    Ok(_) => {
                        pb.finish_with_message(format!("{} 已启动", tunnel.alias));
                    }
                    Err(e) => {
                        pb.finish_with_message(format!("启动失败: {}", e));
                        return Err(e);
                    }
                }
            }
            Commands::Stop => {
                let tunnels = config.list_running_tunnels()?;
                if tunnels.is_empty() {
                    println!("没有正在运行的连接");
                    return Ok(());
                }
                
                let items: Vec<String> = tunnels.iter()
                    .map(|t| format!("{} ({})", t.alias, t.source))
                    .collect();
                
                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("选择要停止的连接")
                    .items(&items)
                    .default(0)
                    .interact()?;
                
                let tunnel = &tunnels[selection];
                let pb = ProgressBar::new_spinner();
                pb.set_message(format!("正在停止 {}...", tunnel.alias));
                
                match tunnel.stop() {
                    Ok(_) => {
                        pb.finish_with_message(format!("{} 已停止", tunnel.alias));
                    }
                    Err(e) => {
                        pb.finish_with_message(format!("停止失败: {}", e));
                        return Err(e);
                    }
                }
            }
            Commands::Set { alias, source, port } => {
                config.update_tunnel(alias, source.as_deref(), *port)?;
                println!("已更新连接 {}", alias);
            }
            Commands::Remove { alias } => {
                if let Some(tunnel) = config.list_tunnels()?.iter().find(|t| t.alias == *alias) {
                    if tunnel.is_running() {
                        tunnel.stop()?;
                    }
                }
                config.remove_tunnel(alias)?;
                println!("已移除连接 {}", alias);
            }
            Commands::Add { alias, source, port } => {
                config.add_tunnel(alias, source, *port)?;
                println!("已添加连接 {}", alias);
            }
        }
        
        Ok(())
    }
} 