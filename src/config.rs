use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::tunnel::Tunnel;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    tunnels: HashMap<String, Tunnel>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if !config_path.exists() {
            return Ok(Self {
                tunnels: HashMap::new(),
            });
        }
        
        let content = fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }
    
    pub fn list_tunnels(&self) -> Result<Vec<&Tunnel>> {
        Ok(self.tunnels.values().collect())
    }
    
    pub fn list_running_tunnels(&self) -> Result<Vec<&Tunnel>> {
        Ok(self.tunnels.values()
            .filter(|t| t.is_running())
            .collect())
    }
    
    pub fn add_tunnel(&mut self, alias: &str, source: &str, port: u16) -> Result<()> {
        let tunnel = Tunnel::new(alias, source, port);
        self.tunnels.insert(alias.to_string(), tunnel);
        self.save()?;
        Ok(())
    }
    
    pub fn update_tunnel(&mut self, alias: &str, source: Option<&str>, port: Option<u16>) -> Result<()> {
        if let Some(tunnel) = self.tunnels.get_mut(alias) {
            if let Some(source) = source {
                tunnel.source = source.to_string();
            }
            if let Some(port) = port {
                tunnel.port = port;
            }
            self.save()?;
        }
        Ok(())
    }
    
    pub fn remove_tunnel(&mut self, alias: &str) -> Result<()> {
        self.tunnels.remove(alias);
        self.save()?;
        Ok(())
    }
    
    fn config_path() -> Result<PathBuf> {
        let mut path = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        path.push("cf-manager");
        fs::create_dir_all(&path)?;
        path.push("config.json");
        Ok(path)
    }
} 