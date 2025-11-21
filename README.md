# TiDB Artifacts Manager

This repository contains two tools for managing artifacts in the Tencent Generic repository: `uploader` and `downloader`.

## Shared Configuration

Both tools share a single `config.toml` file located at the root of the project. This file should contain your authentication credentials.

```toml
# config.toml
[auth]
username = "your-username"
token = "your-token"
```

---

## Uploader

The `uploader` is a command-line tool to upload files to the Generic repository.

### Usage

Navigate to the `uploader` directory to run the tool.

```bash
cd uploader

# Example: Upload a file
cargo run --release -- \
  --config ../config.toml \
  --file /path/to/your/local/file.tar.gz \
  --repo your-repo-name \
  --remote-path builds/tidb/ \
  --remote-filename custom-name.tar.gz

# Example: Upload with a 7-day expiration
cargo run --release -- \
  --config ../config.toml \
  --file /path/to/your/local/file.tar.gz \
  --repo your-repo-name \
  --remote-path builds/tidb/ \
  --expires-days 7
```

### Arguments

- `--config`: Path to the configuration file (defaults to `../config.toml`).
- `--file`: Absolute path to the local file you want to upload.
- `--repo`: The target repository name.
- `--remote-path`: The directory path within the repository.
- `--remote-filename` (Optional): The name for the file in the repository. If not provided, it uses the local filename.
- `--expires-days` (Optional): Number of days to keep the file. Defaults to `0` (permanent).
- `--dry-run` (Optional): A flag to simulate the upload process without actually sending the file.

---

## Downloader

The `downloader` is a command-line tool to find and download artifacts from the Generic repository.

### Usage

Navigate to the `downloader` directory to run the tool.

```bash
cd downloader

# Example: Find and download the latest version of a package
cargo run --release -- \
  --config ../config.toml \
  --repo "easygraph2_bin" \
  --package-name "tidb-community-server-v8.1.0-linux-amd64"

# Example: Run in interactive mode to choose from a list of found files
cargo run --release -- \
  --config ../config.toml \
  --repo "easygraph2_bin" \
  --package-name "tidb-community-server-v8.1.0-linux-amd64" \
  --interactive
```

By default, files are saved to a `download` directory in the project root.

### Arguments

- `--config`: Path to the configuration file (defaults to `../config.toml`).
- `--repo`: The repository name to search in.
- `--package-name`: The prefix or keyword to filter files by (e.g., `tidb-community-server-v8.1.0`).
- `--remote-path` (Optional): The specific directory within the repository to search. Defaults to the repository root.
- `--download-dir` (Optional): The local directory to save the downloaded file. Defaults to `../download`.
- `--interactive` (Optional): A flag to enable interactive mode, allowing you to select which file to download from a list of matches.
