#!/bin/bash

# Usage: ./build.sh [reload|release] [debug]
# Default mode is reload

MODE="reload"
DEBUG_FLAGS=""

# Parse arguments
for arg in "$@"; do
    case $arg in
        reload|release)
            MODE="$arg"
            ;;
        debug)
            DEBUG_FLAGS="-debug -opt:0"
            ;;
        *)
            echo "Unknown argument: $arg"
            echo "Usage: ./build.sh [reload|release] [debug]"
            exit 1
            ;;
    esac
done

if [ "$MODE" = "reload" ]; then
    echo "Building hot reload version..."
    if [ -n "$DEBUG_FLAGS" ]; then
        echo "  With debug symbols"
    fi
    
    echo "Building app DLL..."
    odin build hot_reload/app/ -build-mode:dll -out:hot_reload/app.dll $DEBUG_FLAGS
    
    if [ $? -ne 0 ]; then
        echo "Failed to build app DLL"
        exit 1
    fi
    
    echo "Building reload main binary..."
    odin build hot_reload/reload -out:hot_reload.bin $DEBUG_FLAGS
    
    if [ $? -ne 0 ]; then
        echo "Failed to build reload main binary"
        exit 1
    fi
    
    echo "Hot reload build complete!"
    echo "Run with: ./hot_reload.bin"
    
elif [ "$MODE" = "release" ]; then
    echo "Building release version (statically linked)..."
    if [ -n "$DEBUG_FLAGS" ]; then
        echo "  With debug symbols"
    fi
    
    echo "Building release binary..."
    odin build hot_reload/release -out:release.bin $DEBUG_FLAGS
    
    if [ $? -ne 0 ]; then
        echo "Failed to build release binary"
        exit 1
    fi
    
    echo "Release build complete!"
    echo "Run with: ./release.bin"
fi
