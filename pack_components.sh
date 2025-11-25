#!/bin/bash
echo "publish components..."
local_path=$(pwd)
cd downloads
mv tikv-v8.1.0-linux-amd64.tar.gz tidb-community-server-v8.1.0-linux-amd64
mv tikv-ctl tidb-community-server-v8.1.0-linux-amd64
cd tidb-community-server-v8.1.0-linux-amd64
sh ./local_install.sh
tiup mirror publish ctl v8.1.0 ./tikv-ctl ctl -k keys/ca3657b5a28df076-pingcap.json
tiup mirror publish tikv v8.1.0 ./tikv-v8.1.0-linux-amd64.tar.gz tikv-server -k keys/ca3657b5a28df076-pingcap.json
rm -rf commits/
cd ..
tidb_version=$(date +%Y%m%d)
zip -r tidb-community-server-v8.1.0-linux-amd64-${tidb_version}.zip tidb-community-server-v8.1.0-linux-amd64
cd ${local_path}