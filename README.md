# Tunnel 连接管理器 (tfa)

一个简单的命令行工具，用于管理 Tunnel 连接。

## 功能特性

- 添加、删除、修改 Tunnel 配置
- 启动和停止 Tunnel 连接
- 查看所有 Tunnel 状态
- 支持 Windows 11 和 macOS
- 自动检测并安装 cloudflared

## 安装

### Windows 11

1. 从 [Releases](https://github.com/mr-wuliu/tunnel-manager/releases) 页面下载 `tfa.exe`
2. 将文件移动到任意目录
3. 将该目录添加到系统 PATH 环境变量

### macOS

1. 从 [Releases](https://github.com/mr-wuliu/tunnel-manager/releases) 页面下载 `tfa-mac`
2. 添加执行权限：`chmod +x tfa-mac`
3. 将文件移动到任意目录
4. 将该目录添加到 PATH 环境变量

## 使用方法

```bash
# 列出所有连接
tfa list

# 添加新连接
tfa add --alias my-tunnel --source my-tunnel.example.com --port 8080

# 启动连接
tfa run

# 停止连接
tfa stop

# 修改连接
tfa set my-tunnel --port 8081

# 删除连接
tfa remove my-tunnel
```

## 依赖

- Cloudflare Tunnel CLI (`cloudflared`) 必须已安装并配置
  - 程序会自动检测是否安装了 cloudflared
  - 如果未安装，会提示是否自动安装
  - Windows 用户需要已安装 winget
  - macOS 用户需要已安装 Homebrew

## 开发

```bash
# 克隆仓库
git clone https://github.com/mr-wuliu/tunnel-manager.git
cd tunnel-manager

# 编译
cargo build --release

# 运行
cargo run --release
```

## 许可证

MIT

## 第三方软件声明

本项目使用 Cloudflare Tunnel CLI (`cloudflared`) 作为依赖。使用本软件即表示您同意遵守 Cloudflare 的服务条款和隐私政策。

### Cloudflare 相关声明

- Cloudflare Tunnel CLI (`cloudflared`) 是 Cloudflare 的专有软件
- 使用 `cloudflared` 需要遵守 [Cloudflare 服务条款](https://www.cloudflare.com/terms/)
- 使用 `cloudflared` 需要遵守 [Cloudflare 隐私政策](https://www.cloudflare.com/privacypolicy/)
- 本软件仅作为 `cloudflared` 的管理工具，不提供任何 Cloudflare 服务
- 使用 Cloudflare 服务需要单独的 Cloudflare 账户和订阅 