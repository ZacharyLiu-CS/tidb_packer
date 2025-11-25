#!/bin/bash
echo "uncompressing tidb package..."
local_path=$(pwd)
cd downloads && mv tidb-community-server-v8.1.0-linux-amd64-*.zip tidb-community-server-v8.1.0-linux-amd64.zip
unzip tidb-community-server-v8.1.0-linux-amd64.zip

echo "copying tikv components..."
cd ..
cp /data/home/zacharyzliu/easygraph-tikv/bin/tikv-ctl ./downloads/
cp /data/home/zacharyzliu/easygraph-tikv/bin/tikv-server ./downloads/

echo "compressing tikv-server"
cd downloads && tar zcf tikv-v8.1.0-linux-amd64.tar.gz tikv-server
rm tikv-server
cd ${local_path}