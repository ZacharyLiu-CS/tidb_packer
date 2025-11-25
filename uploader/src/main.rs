use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use md5::Context as Md5Context;
use reqwest::{header::CONTENT_LENGTH, Body, Client};
use serde::Deserialize;
use sha2::{Digest as ShaDigest, Sha256};

use tokio_util::io::ReaderStream;

#[derive(Parser, Debug)]
#[command(
    name = "tidb-uploader",
    version,
    about = "将 tidb-server.tar.gz 上传到腾讯内部 Generic 仓库"
)]
struct Cli {
    #[arg(
        short,
        long,
        default_value = "../config.toml",
        help = "TOML 配置文件路径"
    )]
    config: PathBuf,

    #[arg(
        long,
        value_name = "FILE",
        help = "待上传的 tidb-server.tar.gz 文件路径"
    )]
    file: PathBuf,

    #[arg(long, value_name = "REPO", help = "目标 Generic 仓库名称")]
    repo: String,

    #[arg(
        long,
        value_name = "REMOTE_DIR",
        help = "仓库内目录，例如 releases/tidb/"
    )]
    remote_path: String,

    #[arg(long, value_name = "FILENAME", help = "远端文件名，默认沿用本地文件名")]
    remote_filename: Option<String>,

    #[arg(
        long,
        value_name = "DAYS",
        default_value_t = 0,
        help = "保存天数，0 表示永久"
    )]
    expires_days: u32,

    #[arg(long, default_value_t = false, help = "仅打印操作，不真正上传")]
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
struct Config {
    auth: AuthConfig,
}

#[derive(Debug, Deserialize)]
struct AuthConfig {
    username: String,
    token: String,
}

#[derive(Debug)]
struct FileStats {
    size: u64,
    md5: String,
    sha256: String,
}

#[derive(Debug)]
struct UploadPlan {
    local_path: PathBuf,
    repo: String,
    remote_path: String,
    remote_filename: String,
    expires_days: u32,
}

impl UploadPlan {
    fn remote_relative_path(&self) -> String {
        let trimmed = self.remote_path.trim_matches('/');
        if trimmed.is_empty() {
            self.remote_filename.clone()
        } else {
            format!("{}/{}", trimmed, self.remote_filename)
        }
    }

    fn remote_url(&self) -> String {
        format!(
            "https://mirrors.tencent.com/repository/generic/{}/{}",
            self.repo.trim_matches('/'),
            self.remote_relative_path()
        )
    }
}

fn build_plan(cli: &Cli) -> Result<UploadPlan> {
    let remote_filename = if let Some(name) = &cli.remote_filename {
        name.clone()
    } else {
        cli.file
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .context("无法从 --file 推导文件名，请通过 --remote-filename 指定")?
    };

    Ok(UploadPlan {
        local_path: cli.file.clone(),
        repo: cli.repo.clone(),
        remote_path: cli.remote_path.clone(),
        remote_filename,
        expires_days: cli.expires_days,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadResponse {
    download_uri: Option<String>,
    uri: Option<String>,
    size: Option<String>,
    checksums: Option<RemoteChecksums>,
}

#[derive(Debug, Deserialize)]
struct RemoteChecksums {
    md5: Option<String>,
    sha256: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config_raw = tokio::fs::read_to_string(&cli.config)
        .await
        .with_context(|| format!("无法读取配置文件: {}", cli.config.display()))?;
    let config: Config = toml::from_str(&config_raw)
        .with_context(|| format!("解析配置失败: {}", cli.config.display()))?;

    let plan = build_plan(&cli)?;
    let stats = collect_file_stats(&plan.local_path)?;
    let remote_path = plan.remote_relative_path();
    println!("待上传文件: {}", plan.local_path.display());
    println!("目标 repo: {}", plan.repo);
    println!("目标路径: {}", remote_path);
    println!("保留天数: {} (0 表示永久)", plan.expires_days);
    println!(
        "本地校验 -> size: {} bytes, md5: {}, sha256: {}",
        stats.size, stats.md5, stats.sha256
    );

    upload_file(&config.auth, &plan, &stats, cli.dry_run).await
}

async fn upload_file(
    auth: &AuthConfig,
    plan: &UploadPlan,
    stats: &FileStats,
    dry_run: bool,
) -> Result<()> {
    let remote_url = plan.remote_url();
    let remote_path = plan.remote_relative_path();

    if dry_run {
        println!(
            "[dry-run] 将上传 {} -> {}",
            plan.local_path.display(),
            remote_url
        );
        return Ok(());
    }

    let file = tokio::fs::File::open(&plan.local_path)
        .await
        .with_context(|| format!("无法打开文件: {}", plan.local_path.display()))?;

    let pb = ProgressBar::new(stats.size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
            .progress_chars("#>-"),
    );

    let stream = ReaderStream::new(pb.wrap_async_read(file));
    let body = Body::wrap_stream(stream);

    let client = Client::builder().build().context("无法构建 HTTP 客户端")?;

    let response = client
        .put(&remote_url)
        .basic_auth(&auth.username, Some(&auth.token))
        .header("X-BKREPO-EXPIRES", plan.expires_days.to_string())
        .header(CONTENT_LENGTH, stats.size.to_string())
        .body(body)
        .send()
        .await
        .with_context(|| format!("发送上传请求失败: {}", remote_url))?;

    let status = response.status();
    let text = response.text().await.context("读取上传响应失败")?;

    if !status.is_success() {
        pb.abandon_with_message("上传失败");
        bail!("上传失败 (HTTP {}): {}", status.as_u16(), text);
    }

    pb.finish_with_message("上传完成");
    println!("上传成功: {}", remote_url);

    if let Ok(parsed) = serde_json::from_str::<UploadResponse>(&text) {
        let remote_uri = parsed
            .download_uri
            .or(parsed.uri)
            .unwrap_or_else(|| remote_url.clone());
        println!("可下载地址: {}", remote_uri);

        if let Some(checksums) = parsed.checksums {
            println!(
                "远端校验 -> md5: {} | sha256: {}",
                checksums.md5.unwrap_or_default(),
                checksums.sha256.unwrap_or_default()
            );
        }

        if let Some(size) = parsed.size {
            println!("远端记录的 size: {}", size);
        }
    } else {
        println!("服务端响应: {}", text);
    }

    println!("最终路径: /{}/{}", plan.repo.trim_matches('/'), remote_path);

    Ok(())
}

fn collect_file_stats(path: &Path) -> Result<FileStats> {
    let file =
        File::open(path).with_context(|| format!("无法打开文件以计算校验: {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("无法读取文件元数据: {}", path.display()))?;
    let mut reader = BufReader::with_capacity(8 * 1024 * 1024, file);
    let mut md5_ctx = Md5Context::new();
    let mut sha256_hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let read_bytes = reader.read(&mut buffer)?;
        if read_bytes == 0 {
            break;
        }
        md5_ctx.consume(&buffer[..read_bytes]);
        sha256_hasher.update(&buffer[..read_bytes]);
    }

    Ok(FileStats {
        size: metadata.len(),
        md5: format!("{:x}", md5_ctx.compute()),
        sha256: format!("{:x}", sha256_hasher.finalize()),
    })
}
