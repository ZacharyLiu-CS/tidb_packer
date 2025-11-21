download:
	cd downloader && cargo run --release -- \
  	--config ../config.toml \
  	--repo "easygraph2_bin" \
  	--package-name "tidb-community-server-v8.1.0-linux-amd64" \
  	--interactive
upload:
	cd uploader && cargo run --release -- \
  	--config ../config.toml \
  	--file ../downloads/tidb-community-server-v8.1.0-linux-amd64-20250414.zip \
  	--repo easygraph-tidb \
  	--remote-path "./" \
  	--remote-filename tidb-community-server-v8.1.0-linux-amd64-20250414.zip
