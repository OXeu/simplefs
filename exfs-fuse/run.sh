#!/usr/bin/env bash
WORK_DIR="$HOME/flexOS/fuse_test"
FS="$WORK_DIR/fs"
umount "$FS"
rm -rf "$FS"
mkdir "$FS"
cd "$WORK_DIR" || exit
RUST_BACKTRACE=1 RUST_LOG=debug ~/.cargo/bin/cargo run --package exfs-fuse --bin exfs-fuse -- fs