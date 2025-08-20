#!/usr/bin/env nu

# Usage: nu build.nu [unified|dll|reload|clean] [debug]
# Default mode is unified
#
# Modes:
#   unified - Build a single unified binary (no hot reloading)
#   dll     - Build only the app DLL for hot reloading
#   reload  - Build both the app DLL and reload main binary
#   clean   - Delete all built binaries and DLL files
#
# Options:
#   debug   - Build with debug symbols and no optimization

def main [...args] {
    let mode = if ($args | length) > 0 { 
        if ($args.0 in ["unified", "dll", "reload", "clean"]) { $args.0 } else { "unified" }
    } else { "unified" }
    
    let debug = ($args | any {|arg| $arg == "debug"})
    let debug_flags = if $debug { ["-debug", "-opt:0"] } else { [] }
    
    # Validate arguments
    let valid_args = ["unified", "dll", "reload", "clean", "debug"]
    for arg in $args {
        if not ($arg in $valid_args) {
            print $"Unknown argument: ($arg)"
            print "Usage: nu build.nu [unified|dll|reload|clean] [debug]"
            print ""
            print "Modes:"
            print "  unified - Build a single unified binary (no hot reloading)"
            print "  dll     - Build only the app DLL for hot reloading"
            print "  reload  - Build both the app DLL and reload main binary"
            print "  clean   - Delete all built binaries and DLL files"
            print ""
            print "Options:"
            print "  debug   - Build with debug symbols and no optimization"
            exit 1
        }
    }
    
    if $mode == "unified" {
        print "Building unified version (statically linked)..."
        if $debug {
            print "  With debug symbols"
        }
        
        print "Building unified binary..."
        let unified_cmd = ["build", "hot_reload/release", "-out:release.bin"] ++ $debug_flags
        let unified_result = (^odin ...($unified_cmd) | complete)
        
        if $unified_result.exit_code != 0 {
            print "Failed to build unified binary"
            exit 1
        }
        
        print "Unified build complete!"
        print "Run with: ./release.bin"
        
    } else if $mode == "dll" {
        print "Building app DLL only..."
        if $debug {
            print "  With debug symbols"
        }
        
        print "Building app DLL..."
        let dll_cmd = ["build", "hot_reload/app/", "-build-mode:dll", "-out:hot_reload/app.dll"] ++ $debug_flags
        let dll_result = (^odin ...($dll_cmd) | complete)
        
        if $dll_result.exit_code != 0 {
            print "Failed to build app DLL"
            exit 1
        }
        
        print "DLL build complete!"
        print "DLL ready for hot reloading"
        
    } else if $mode == "reload" {
        print "Building full hot reload version..."
        if $debug {
            print "  With debug symbols"
        }
        
        print "Building app DLL..."
        let dll_cmd = ["build", "hot_reload/app/", "-build-mode:dll", "-out:hot_reload/app.dll"] ++ $debug_flags
        let dll_result = (^odin ...($dll_cmd) | complete)
        
        if $dll_result.exit_code != 0 {
            print "Failed to build app DLL"
            exit 1
        }
        
        print "Building reload main binary..."
        let reload_cmd = ["build", "hot_reload/reload", "-out:hot_reload.bin"] ++ $debug_flags
        let reload_result = (^odin ...($reload_cmd) | complete)
        
        if $reload_result.exit_code != 0 {
            print "Failed to build reload main binary"
            exit 1
        }
        
        print "Hot reload build complete!"
        print "Run with: ./hot_reload.bin"
        
    } else if $mode == "clean" {
        print "Cleaning built files..."
        
        let files_to_clean = [
            "release.bin"
            "hot_reload.bin"
            "hot_reload/app.dll"
            "app.dll"
        ]
        
        for file in $files_to_clean {
            if ($file | path exists) {
                rm $file
                print $"Deleted: ($file)"
            }
        }
        
        print "Clean complete!"
    }
}