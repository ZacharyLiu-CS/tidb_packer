#!/bin/bash
local_path=$(pwd)
sudo rm -rf ./downloads/* 
sudo rm -rf ~/.tiup/
cd ${local_path}