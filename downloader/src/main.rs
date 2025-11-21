use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, TimeZone, Utc};
use clap::Parser;
use dialoguer::{Select, theme::ColorfulTheme};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{Client, header::CONTENT_LENGTH};
use serde::Deserialize;
use tokio::{fs, io::AsyncWriteExt, task};

#[derive(Parser, Debug)]
#[command(
    name = "tidb-downloader",
    version,
    about = "列出并下载 mirrors.tencent.com Generic 仓库中的制品"
)]
struct Cli {
    #[arg(
        short,
        long,
        default_value = "../config.toml",
        help = "TOML 配置文件路径"
    )]
    config: PathBuf,

    #[arg(long, value_name = "REPO", help = "仓库名称，例如 easygraph2_bin")]
    repo: String,

    #[arg(
        long,
        value_name = "PACKAGE",
        help = "包名前缀，例如 tidb-community-server-v8.1.0-linux-amd64"
    )]
    package_name: String,

    #[arg(
        long,
        value_name = "REMOTE_DIR",
        default_value = "",
        help = "仓库内目录，留空表示仓库根目录"
    )]
    remote_path: String,

    #[arg(
        long,
        value_name = "DIR",
        default_value = "../downloads",
        help = "下载保存目录"
    )]
    download_dir: PathBuf,

    #[arg(long, default_value_t = false, help = "手动选择文件而不是自动选择最新")]
    interactive: bool,

    #[arg(
        long,
        value_name = "N",
        default_value_t = 50,
        help = "分页大小(1-1024)"
    )]
    page_size: u32,
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

#[derive(Debug, Deserialize)]
struct ListResponse {
    code: i32,
    msg: Option<String>,
    data: Option<ListData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ListData {
    #[serde(default)]
    records: Vec<ApiRecord>,
    #[serde(default)]
    pagination: bool,
    #[serde(default)]
    total_pages: Option<u32>,
    #[serde(default)]
    page_number: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiRecord {
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    folder: bool,
    #[serde(default)]
    last_modified_date: Option<String>,
    #[serde(default)]
    created_date: Option<String>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    md5: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Clone)]
struct Candidate {
    record: ApiRecord,
    timestamp: DateTime<Utc>,
}

impl ApiRecord {
    fn timestamp(&self) -> DateTime<Utc> {
        parse_timestamp(
            self.last_modified_date
                .as_deref()
                .or(self.created_date.as_deref()),
        )
    }

    fn relative_path(&self) -> String {
        let trimmed = self.path.trim_matches('/');
        if trimmed.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", trimmed, self.name)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config(&cli.config).await?;
    let client = Client::builder().build().context("无法构建 HTTP 客户端")?;

    let normalized_full_path = build_full_path(&cli.repo, &cli.remote_path);
    let page_size = cli.page_size.clamp(1, 1024);

    println!(
        "查询仓库: {}，目录: {}，过滤: {}",
        cli.repo, cli.remote_path, cli.package_name
    );

    let mut candidates = collect_candidates(
        &client,
        &config.auth,
        &normalized_full_path,
        &cli.package_name,
        page_size,
    )
    .await?;

    if candidates.is_empty() {
        bail!(
            "未在 {}/{} 中找到包含 `{}` 的文件",
            cli.repo,
            cli.remote_path,
            cli.package_name
        );
    }

    candidates.sort_by(|a, b| {
        b.timestamp
            .cmp(&a.timestamp)
            .then_with(|| b.record.name.cmp(&a.record.name))
    });
    for (idx, candidate) in candidates.iter().enumerate() {
        println!(
            "[{}] {:<80} | 修改时间: {} | 大小: {}",
            idx,
            candidate.record.name,
            candidate.timestamp,
            candidate.record.size.as_deref().unwrap_or("<未知>")
        );
    }

    let selection_idx = if cli.interactive {
        prompt_for_choice(&candidates)?
    } else {
        0
    };

    let chosen = &candidates[selection_idx];
    println!("选择: {} ({} )", chosen.record.name, chosen.timestamp);

    let destination =
        download_candidate(&client, &config.auth, &cli.repo, chosen, &cli.download_dir).await?;

    println!(
        "下载完成 -> {}\nMD5: {}\nSHA256: {}",
        destination.display(),
        chosen.record.md5.as_deref().unwrap_or("<未知>"),
        chosen.record.sha256.as_deref().unwrap_or("<未知>")
    );

    Ok(())
}

async fn load_config(path: &Path) -> Result<Config> {
    let raw = fs::read_to_string(path)
        .await
        .with_context(|| format!("无法读取配置文件: {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("解析配置失败: {}", path.display()))
}

async fn collect_candidates(
    client: &Client,
    auth: &AuthConfig,
    full_path: &str,
    keyword: &str,
    page_size: u32,
) -> Result<Vec<Candidate>> {
    let mut page = 1;
    let mut matches = Vec::new();

    loop {
        let data = fetch_page(client, auth, full_path, page, page_size).await?;
        let count = data.records.len();

        for record in data.records.into_iter() {
            if record.folder {
                continue;
            }
            if !record.name.contains(keyword) {
                continue;
            }
            matches.push(Candidate {
                timestamp: record.timestamp(),
                record,
            });
        }

        let total_pages = data.total_pages.unwrap_or(page);
        let has_more = data.pagination && page < total_pages && count as u32 >= page_size;
        if !has_more || count == 0 {
            break;
        }
        page += 1;
    }

    Ok(matches)
}

async fn fetch_page(
    client: &Client,
    auth: &AuthConfig,
    full_path: &str,
    page: u32,
    page_size: u32,
) -> Result<ListData> {
    let url = format!(
        "https://mirrors.tencent.com/mirrors/api/generic/list?full_path={}&pagesize={}&page={}",
        full_path, page_size, page
    );

    let response = client
        .get(&url)
        .basic_auth(&auth.username, Some(&auth.token))
        .send()
        .await
        .with_context(|| format!("请求列表失败: {}", url))?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        bail!("列表接口返回 {}: {}", status.as_u16(), body);
    }

    let parsed: ListResponse =
        serde_json::from_str(&body).with_context(|| format!("解析列表响应失败: {}", body))?;

    if parsed.code != 0 {
        bail!(
            "列表接口错误 code {}: {}",
            parsed.code,
            parsed.msg.unwrap_or_default()
        );
    }

    parsed.data.context("列表响应缺少 data")
}

async fn download_candidate(
    client: &Client,
    auth: &AuthConfig,
    repo: &str,
    candidate: &Candidate,
    download_dir: &Path,
) -> Result<PathBuf> {
    fs::create_dir_all(download_dir)
        .await
        .with_context(|| format!("无法创建下载目录: {}", download_dir.display()))?;

    let relative = candidate.record.relative_path();
    let url = format!(
        "https://mirrors.tencent.com/repository/generic/{}/{}",
        repo.trim_matches('/'),
        relative
    );

    let response = client
        .get(&url)
        .basic_auth(&auth.username, Some(&auth.token))
        .send()
        .await
        .with_context(|| format!("下载请求失败: {}", url))?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        bail!("下载失败 (HTTP {}): {}", status.as_u16(), text);
    }

    let total_size = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));

    let dest_path = download_dir.join(&candidate.record.name);
    let mut file = fs::File::create(&dest_path)
        .await
        .with_context(|| format!("无法创建文件: {}", dest_path.display()))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("下载过程中发生错误")?;
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }
    pb.finish_with_message("下载完成");

    file.flush().await?;

    Ok(dest_path)
}

fn build_full_path(repo: &str, remote_path: &str) -> String {
    let repo = repo.trim_matches('/');
    let path = remote_path.trim_matches('/');
    if path.is_empty() {
        repo.to_string()
    } else {
        format!("{}/{}", repo, path)
    }
}

fn parse_timestamp(raw: Option<&str>) -> DateTime<Utc> {
    raw.and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| Utc.timestamp_opt(0, 0).single())
        .unwrap()
}

fn prompt_for_choice(candidates: &[Candidate]) -> Result<usize> {
    let items: Vec<String> = candidates
        .iter()
        .map(|c| {
            format!(
                "{} | {} | {}",
                c.record.name,
                c.timestamp,
                c.record.size.as_deref().unwrap_or("<未知>")
            )
        })
        .collect();

    task::block_in_place(|| {
        Select::with_theme(&ColorfulTheme::default())
            .with_prompt("请选择要下载的文件")
            .items(&items)
            .default(0)
            .interact()
            .context("读取用户输入失败")
    })
}
