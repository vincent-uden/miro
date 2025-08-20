#!/usr/bin/env nu

# Usage: nu build.nu [unified|dll|reload|clean|help] [debug]
# Default mode is unified
#
# Modes:
#   unified - Build a single unified binary (no hot reloading)
#   dll     - Build only the app DLL for hot reloading
#   reload  - Build both the app DLL and reload main binary
#   clean   - Delete all built binaries and DLL files
#   help    - Show this help message
#
# Options:
#   debug   - Build with debug symbols and no optimization

def main [...args] {
    let mode = if ($args | length) > 0 { 
        if ($args.0 in ["unified", "dll", "reload", "clean", "help"]) { $args.0 } else { "unified" }
    } else { "unified" }
    
    let debug = ($args | any {|arg| $arg == "debug"})
    let debug_flags = if $debug { ["-debug", "-o:none"] } else { [] }
    
    # Show help if requested
    if $mode == "help" {
        print $"(ansi yellow_bold)Usage:(ansi reset) nu build.nu [unified|dll|reload|clean|help] [debug]"
        print ""
        print $"(ansi blue_bold)Modes:(ansi reset)"
        print $"  (ansi cyan)unified(ansi reset) - Build a single unified binary \(no hot reloading\)"
        print $"  (ansi cyan)dll(ansi reset)     - Build only the app DLL for hot reloading"
        print $"  (ansi cyan)reload(ansi reset)  - Build both the app DLL and reload main binary"
        print $"  (ansi cyan)clean(ansi reset)   - Delete all built binaries and DLL files"
        print $"  (ansi cyan)help(ansi reset)    - Show this help message"
        print ""
        print $"(ansi blue_bold)Options:(ansi reset)"
        print $"  (ansi cyan)debug(ansi reset)   - Build with debug symbols and no optimization"
        print ""
        print $"(ansi blue_bold)Examples:(ansi reset)"
        print $"  (ansi green)nu build.nu(ansi reset)              # Build unified binary"
        print $"  (ansi green)nu build.nu dll debug(ansi reset)    # Build DLL with debug symbols"
        print $"  (ansi green)nu build.nu reload(ansi reset)       # Build full hot reload setup"
        print $"  (ansi green)nu build.nu clean(ansi reset)        # Clean all built files"
        return
    }
    
    # Validate arguments
    let valid_args = ["unified", "dll", "reload", "clean", "help", "debug"]
    for arg in $args {
        if not ($arg in $valid_args) {
            print $"(ansi red_bold)Unknown argument: ($arg)(ansi reset)"
            print $"(ansi yellow_bold)Usage:(ansi reset) nu build.nu [unified|dll|reload|clean|help] [debug]"
            print ""
            print $"(ansi blue_bold)Modes:(ansi reset)"
            print $"  (ansi cyan)unified(ansi reset) - Build a single unified binary \(no hot reloading\)"
            print $"  (ansi cyan)dll(ansi reset)     - Build only the app DLL for hot reloading"
            print $"  (ansi cyan)reload(ansi reset)  - Build both the app DLL and reload main binary"
            print $"  (ansi cyan)clean(ansi reset)   - Delete all built binaries and DLL files"
            print $"  (ansi cyan)help(ansi reset)    - Show this help message"
            print ""
            print $"(ansi blue_bold)Options:(ansi reset)"
            print $"  (ansi cyan)debug(ansi reset)   - Build with debug symbols and no optimization"
            exit 1
        }
    }
    
    if $mode == "unified" {
        print $"(ansi blue_bold)Building unified version \(statically linked\)...(ansi reset)"
        if $debug {
            print $"  (ansi yellow)With debug symbols(ansi reset)"
        }
        
        print $"(ansi cyan)Building unified binary...(ansi reset)"
        let unified_cmd = ["build", "hot_reload/release", "-out:release.bin"] ++ $debug_flags
        let unified_result = (^odin ...($unified_cmd) | complete)
        
        if $unified_result.exit_code != 0 {
            print $"(ansi red_bold)Failed to build unified binary(ansi reset)"
            if ($unified_result.stderr | str length) > 0 {
                print $unified_result.stderr
            }
            if ($unified_result.stdout | str length) > 0 {
                print $unified_result.stdout
            }
            exit 1
        }
        
        print $"(ansi green_bold)Unified build complete!(ansi reset)"
        print $"(ansi green)Run with: ./release.bin(ansi reset)"
        
    } else if $mode == "dll" {
        print $"(ansi blue_bold)Building app DLL only...(ansi reset)"
        if $debug {
            print $"  (ansi yellow)With debug symbols(ansi reset)"
        }
        
        print $"(ansi cyan)Building app DLL...(ansi reset)"
        let dll_cmd = ["build", "hot_reload/app/", "-build-mode:dll", "-out:hot_reload/app.dll"] ++ $debug_flags
        let dll_result = (^odin ...($dll_cmd) | complete)
        
        if $dll_result.exit_code != 0 {
            print $"(ansi red_bold)Failed to build app DLL(ansi reset)"
            if ($dll_result.stderr | str length) > 0 {
                print $dll_result.stderr
            }
            if ($dll_result.stdout | str length) > 0 {
                print $dll_result.stdout
            }
            exit 1
        }
        
        print $"(ansi green_bold)DLL build complete!(ansi reset)"
        print $"(ansi green)DLL ready for hot reloading(ansi reset)"
        
    } else if $mode == "reload" {
        print $"(ansi blue_bold)Building full hot reload version...(ansi reset)"
        if $debug {
            print $"  (ansi yellow)With debug symbols(ansi reset)"
        }
        
        print $"(ansi cyan)Building app DLL...(ansi reset)"
        let dll_cmd = ["build", "hot_reload/app/", "-build-mode:dll", "-out:hot_reload/app.dll"] ++ $debug_flags
        let dll_result = (^odin ...($dll_cmd) | complete)
        
        if $dll_result.exit_code != 0 {
            print $"(ansi red_bold)Failed to build app DLL(ansi reset)"
            if ($dll_result.stderr | str length) > 0 {
                print $dll_result.stderr
            }
            if ($dll_result.stdout | str length) > 0 {
                print $dll_result.stdout
            }
            exit 1
        }
        
        print $"(ansi cyan)Building reload main binary...(ansi reset)"
        let reload_cmd = ["build", "hot_reload/reload", "-out:hot_reload.bin"] ++ $debug_flags
        let reload_result = (^odin ...($reload_cmd) | complete)
        
        if $reload_result.exit_code != 0 {
            print $"(ansi red_bold)Failed to build reload main binary(ansi reset)"
            if ($reload_result.stderr | str length) > 0 {
                print $reload_result.stderr
            }
            if ($reload_result.stdout | str length) > 0 {
                print $reload_result.stdout
            }
            exit 1
        }
        
        print $"(ansi green_bold)Hot reload build complete!(ansi reset)"
        print $"(ansi green)Run with: ./hot_reload.bin(ansi reset)"
        
    } else if $mode == "clean" {
        print $"(ansi magenta_bold)Cleaning built files...(ansi reset)"
        
        let files_to_clean = [
            "release.bin"
            "hot_reload.bin"
            "hot_reload/*.dll"
            "app.dll"
        ]
        
        for file in $files_to_clean {
            if ($file | path exists) {
                rm $file
                print $"(ansi red)Deleted: ($file)(ansi reset)"
            }
        }
        
        print $"(ansi green_bold)Clean complete!(ansi reset)"
    }
}
