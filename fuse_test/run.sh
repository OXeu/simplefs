#!/usr/bin/env bash
WORK_DIR="$HOME/flexOS/fuse_test"
FS="$WORK_DIR/fs"
umount "$FS"
rm -rf "$FS"
mkdir "$FS"
cd "$WORK_DIR" || exit
RUST_BACKTRACE=1 RUST_LOG=debug cargo run --package fuse_test --bin fuse_test -- fs