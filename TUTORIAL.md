# TiDB Packer 工具使用

## 1. 环境准备

### 下载 tidb packer
```bash
git clone git@git.woa.com:zacharyzliu/tidb_packer.git
cd tidb_packer
```

### 安装依赖
确保系统已安装以下工具：
- Rust 工具链 (用于编译上传器和下载器)
- unzip (用于解压文件)
- tiup (用于组件发布)

```bash
# 安装 Rust (如果未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 安装其他依赖 (Ubuntu/Debian)
sudo apt update
sudo apt install unzip
```

## 2. 配置认证信息

创建配置文件 `config.toml` 在项目根目录：

```toml
# config.toml
[auth]
username = "your-username"
token = "your-token"
```

> **注意**：请将 `your-username` 和 `your-token` 替换为实际的认证信息。

## 3. 工具使用流程

### 全自动流程（推荐）
执行以下命令完成完整的下载、拷贝、打包、上传流程：

```bash
make all
```

这个命令会自动执行以下步骤：
1. 下载最新的 TiDB 镜像包
2. 拷贝 TiKV 组件
3. 发布组件到 TiUP 镜像
4. 重新打包并上传到仓库
5. 清理工作空间

### 分步执行流程

#### 步骤 1：下载 TiDB 镜像包
```bash
make download
```

此命令会：
- 连接到 easygraph2_bin 仓库
- 搜索 tidb-community-server-v8.1.0-linux-amd64 包
- 以交互模式让用户选择要下载的文件
- 下载文件到 `downloads/` 目录

#### 步骤 2：拷贝 TiKV 组件
```bash
bash cp_components.sh
```

此脚本会：
- 解压下载的 TiDB 包
- 从指定路径拷贝 tikv-ctl 和 tikv-server 组件
- 将 tikv-server 打包为 tikv-v8.1.0-linux-amd64.tar.gz

#### 步骤 3：发布组件并重新打包
```bash
bash pack_components.sh
```

此脚本会：
- 将 TiKV 组件移动到 TiDB 包目录
- 执行本地安装脚本
- 使用 TiUP 发布 ctl 和 tikv 组件
- 重新打包为带日期版本的文件

#### 步骤 4：上传到仓库
```bash
make upload
```

此命令会：
- 上传重新打包的文件到 easygraph2_bin 仓库
- 文件名格式：tidb-community-server-v8.1.0-linux-amd64-YYYYMMDD.zip

#### 步骤 5：清理工作空间
```bash
bash clean_workspace.sh
```

清理下载文件和 TiUP 缓存，释放磁盘空间。

## 4. 工具详细说明

### 上传器 (Uploader)

上传器用于将文件上传到 Generic 仓库。

```bash
cd uploader
cargo run --release -- \
  --config ../config.toml \
  --file /path/to/file.tar.gz \
  --repo repository-name \
  --remote-path builds/tidb/ \
  --remote-filename custom-name.tar.gz
```

**参数说明：**
- `--config`: 配置文件路径
- `--file`: 要上传的本地文件绝对路径
- `--repo`: 目标仓库名称
- `--remote-path`: 仓库内目录路径
- `--remote-filename`: 仓库中的文件名（可选）
- `--expires-days`: 文件保留天数（默认永久）
- `--dry-run`: 模拟上传过程

### 下载器 (Downloader)

下载器用于从 Generic 仓库查找和下载文件。

```bash
cd downloader
cargo run --release -- \
  --config ../config.toml \
  --repo "easygraph2_bin" \
  --package-name "tidb-community-server-v8.1.0-linux-amd64" \
  --interactive
```

**参数说明：**
- `--config`: 配置文件路径
- `--repo`: 要搜索的仓库名称
- `--package-name`: 文件前缀或关键词
- `--remote-path`: 仓库内搜索目录（可选）
- `--download-dir`: 本地保存目录（默认 ../download）
- `--interactive`: 启用交互模式选择文件

## 5. 故障排除

### 常见问题

1. **认证失败**
   - 检查 config.toml 文件中的 username 和 token 是否正确
   - 确认有访问目标仓库的权限

2. **文件下载失败**
   - 检查网络连接
   - 确认仓库中存在匹配的文件
   - 尝试使用 --interactive 模式手动选择文件

3. **TiUP 发布失败**
   - 确认 tiup 已正确安装
   - 检查密钥文件是否存在
   - 验证组件文件完整性

### 日志查看

所有操作都会在控制台输出详细日志，可以根据日志信息定位问题。

## 6. 注意事项

- 确保有足够的磁盘空间（至少 2GB）
- 操作需要 sudo 权限来清理工作空间
- 建议在稳定的网络环境下执行下载和上传操作
- 定期清理工作空间以避免磁盘空间不足