download:
	cd downloader && cargo run --release -- \
  	--config ../config.toml \
  	--repo "easygraph2_bin" \
  	--package-name "tidb-community-server-v8.1.0-linux-amd64" \
  	--interactive
upload:
	cd uploader && cargo run --release -- \
  	--config ../config.toml \
  	--file ../downloads/tidb-community-server-v8.1.0-linux-amd64-$$(date +%Y%m%d).zip \
  	--repo easygraph2_bin \
  	--remote-path "./" \
  	--remote-filename tidb-community-server-v8.1.0-linux-amd64-$$(date +%Y%m%d).zip
all:
	make download
	bash cp_components.sh
	bash pack_components.sh
	make upload
	bash clean_workspace.sh
