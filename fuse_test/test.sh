#!/usr/bin/env bash
#sleep "5s"
WORK_DIR="$HOME/flexOS/fuse_test"
FS="$WORK_DIR/fs"
cd "$FS" || exit
set -x
pwd && ls
mkdir "level1"
touch "top_hello.txt"
pwd && ls
cd "level1" || exit
pwd && ls
mkdir "sub_dir"
ls