package main

import "../../common"
import "../../render"
import "../app"
import "base:runtime"
import "core:c"
import "core:c/libc"
import "core:dynlib"
import "core:fmt"
import "core:log"
import glm "core:math/linalg/glsl"
import "core:os"
import gl "vendor:OpenGL"
import glfw "vendor:glfw"
import clay "../../clay-odin"

// Custom framebuffer callback for reload (no Clay call)
framebuffer_size_callback :: proc "c" (
    window: glfw.WindowHandle,
    width, height: i32,
) {
    context = runtime.default_context()
    gl.Viewport(0, 0, width, height)
    common.window_width = width
    common.window_height = height
    // Note: Clay layout dimensions will be updated by the DLL when it renders
}

// TODO: Full app reset
main :: proc() {
    context.logger = log.create_console_logger(runtime.Logger_Level.Debug)
    defer log.destroy_console_logger(context.logger)

    window := common.glfw_init(framebuffer_size_callback)
    if window == nil {
        return
    }
    defer common.glfw_destroy(window)

    common.opengl_init()

    // Setup shaders and renderers
    shaders, rect_renderer, text_renderer, setup_ok :=
        common.setup_shaders_and_renderers()
    if !setup_ok {
        return
    }

    defer {
        common.opengl_destroy(&shaders, &rect_renderer, &text_renderer, window)
        delete(shaders)
    }

    log.info("OpenGL/GLFW initialization complete")
    app_api_version := 0
    app_api, app_api_ok := load_app_api(app_api_version)

    if !app_api_ok {
        log.error("Failed to load app API")
        return
    }

    app_api_version += 1
    app_api.init(&text_renderer, common.window_width, common.window_height)

    for {
        when ODIN_OS == .Windows {
            dll_path := ".\\hot_reload\\app.dll"
        } else {
            dll_path := "./hot_reload/app.dll"
        }
        dll_time, dll_time_err := os.last_write_time_by_name(dll_path)
        reload := dll_time_err == os.ERROR_NONE && app_api.dll_time != dll_time

        if reload {
            log.info("Hot reloading")
            // Load a new game API. Might fail due to game.dll still being 
            // written by compiler. In that case it will try again next frame.
            new_api, new_api_ok := load_app_api(app_api_version)
            if new_api_ok {
                app_memory := app_api.memory()
                // The old memory survives this action. We'll reuse it as long 
                // as the memory model is still the same
                unload_app_api(app_api)
                app_api = new_api
                app_api.hot_reloaded(app_memory)
                app_api_version += 1
            }
        }

        if glfw.WindowShouldClose(window) {
            break
        }
        glfw.PollEvents()

        // Update Clay with current mouse state
        app_api.set_mouse_state(f32(common.mouse_x), f32(common.mouse_y), common.mouse_left_down)

        // Store previous mouse state for click detection
        common.mouse_left_was_down = common.mouse_left_down

        if !app_api.update() {
            break
        }
        app_api.draw(
            &rect_renderer,
            &text_renderer,
            common.window_width,
            common.window_height,
        )

        glfw.SwapBuffers(window)

    }
    log.info("Shutting down")
    app_api.shutdown()
    unload_app_api(app_api)
}


/* Contains pointers to the procedures exposed by the game DLL. */
AppAPI :: struct {
    init:            proc(
        text_renderer: ^render.TextRenderer,
        window_width, window_height: i32,
    ),
    update:          proc() -> bool,
    draw:            proc(
        rect_renderer: ^render.RectRenderer,
        text_renderer: ^render.TextRenderer,
        window_width, window_height: i32,
    ),
    set_mouse_state: proc(mouse_x, mouse_y: f32, mouse_down: bool),
    shutdown:        proc(),
    memory:          proc() -> rawptr,
    hot_reloaded:    proc(_: rawptr),
    lib:             dynlib.Library,
    dll_time:        os.File_Time,
    api_version:     int,
}

/* Load the game DLL and return a new GameAPI that contains pointers to the 
 * required procedures of the game DLL. */
load_app_api :: proc(api_version: int) -> (AppAPI, bool) {
    log.infof("Loading app api version {0}", api_version)
    when ODIN_OS == .Windows {
        base_dll_path := ".\\hot_reload\\app.dll"
    } else {
        base_dll_path := "./hot_reload/app.dll"
    }
    dll_time, dll_time_err := os.last_write_time_by_name(base_dll_path)
    if dll_time_err != os.ERROR_NONE {
        log.error("Could not fetch last write date of api.dll")
        return {}, false
    }

    when ODIN_OS == .Windows {
        dll_name := fmt.tprintf(".\\hot_reload\\app_{0}.dll", api_version)
        src_path := ".\\hot_reload\\app.dll"
    } else {
        dll_name := fmt.tprintf("./hot_reload/app_{0}.dll", api_version)
        src_path := "./hot_reload/app.dll"
    }
    when ODIN_OS == .Windows {
        copy_cmd := fmt.ctprintf("copy {0} {1}", src_path, dll_name)
    } else {
        copy_cmd := fmt.ctprintf("cp {0} {1}", src_path, dll_name)
    }

    if libc.system(copy_cmd) != 0 {
        log.errorf("Failed to copy app.dll to {0}", dll_name)
        return {}, false
    }

    api := AppAPI{}
    _, lib_ok := dynlib.initialize_symbols(&api, dll_name, "app_", "lib")
    if !lib_ok {
        log.error("Failed loading app DLL")
        return {}, false
    }

    api.dll_time = dll_time
    api.api_version = api_version
    log.infof("Successfully loaded app api version {0}", api_version)
    return api, true
}

// Unloads the old dll without deleteing its memory to allow for re-use with the new dll
unload_app_api :: proc(api: AppAPI) {
    if api.lib != nil {
        dynlib.unload_library(api.lib)
    }

    when ODIN_OS == .Windows {
        del_cmd := fmt.ctprintf(
            "del .\\hot_reload\\app_{0}.dll",
            api.api_version,
        )
    } else {
        del_cmd := fmt.ctprintf("rm ./hot_reload/app_{0}.dll", api.api_version)
    }

    if libc.system(del_cmd) != 0 {
        log.errorf("Failed to remove app_{0}.dll", api.api_version)
    }
}
