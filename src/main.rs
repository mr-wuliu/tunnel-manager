mod cli;
mod config;
mod tunnel;

use anyhow::Result;
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Confirm};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: cli::Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 检查 cloudflared 是否已安装
    if !tunnel::Tunnel::check_cloudflared()? {
        println!("未检测到 cloudflared，这是运行本程序必需的。");
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("是否要自动安装 cloudflared？")
            .default(true)
            .interact()?
        {
            println!("正在安装 cloudflared...");
            tunnel::Tunnel::install_cloudflared()?;
            println!("cloudflared 安装完成！");
        } else {
            return Err(anyhow::anyhow!("请先安装 cloudflared 后再运行本程序"));
        }
    }

    let cli = Cli::parse();
    cli.command.execute().await?;
    Ok(())
}
