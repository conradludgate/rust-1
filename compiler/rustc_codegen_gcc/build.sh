#!/bin/bash

#set -x
set -e

codegen_channel=debug
sysroot_channel=debug

while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            codegen_channel=release
            shift
            ;;
        --release-sysroot)
            sysroot_channel=release
            shift
            ;;
        *)
            echo "Unknown option $1"
            exit 1
            ;;
    esac
done

if [ -f ./gcc_path ]; then
    export GCC_PATH=$(cat gcc_path)
else
    echo 'Please put the path to your custom build of libgccjit in the file `gcc_path`, see Readme.md for details'
    exit 1
fi

export LD_LIBRARY_PATH="$GCC_PATH"
export LIBRARY_PATH="$GCC_PATH"

features=

if [[ "$1" == "--features" ]]; then
    shift
    features="--features $1"
    shift
fi

if [[ "$codegen_channel" == "release" ]]; then
    export CHANNEL='release'
    CARGO_INCREMENTAL=1 cargo rustc --release $features
else
    echo $LD_LIBRARY_PATH
    export CHANNEL='debug'
    cargo rustc $features
fi

source config.sh

rm -r target/out || true
mkdir -p target/out/gccjit

echo "[BUILD] sysroot"
if [[ "$sysroot_channel" == "release" ]]; then
    time ./build_sysroot/build_sysroot.sh --release
else
    time ./build_sysroot/build_sysroot.sh
fi

