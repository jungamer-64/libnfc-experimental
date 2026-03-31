#!/bin/sh
PROJECT_DIR=$(readlink -e $(dirname $0))
cd "$PROJECT_DIR"

rm -rf build
mkdir build
cd build


case $1 in
32*)
    mingw32-cmake .. -DLIBNFC_ROOT_DIR=$PWD/.. -DLIBNFC_SYSCONFDIR='C:\\Program Files (x86)\\libnfc\\config'
    mingw32-make;;
64*)
    mingw64-cmake .. -DLIBNFC_ROOT_DIR=.. -DLIBNFC_SYSCONFDIR='C:\\Program Files\\libnfc\\config'
    mingw64-make;;
*)
    echo "specify whether to build 32-bit or 64-bit version by supplying 32 or 64 as parameter";;
esac
