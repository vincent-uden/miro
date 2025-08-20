package main

import clay "../../clay-odin"
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

WINDOW_WIDTH :: 1000
WINDOW_HEIGHT :: 800

// Global state for window dimensions
window_width: i32 = WINDOW_WIDTH
window_height: i32 = WINDOW_HEIGHT

// Global mouse state
mouse_x: f64 = 0
mouse_y: f64 = 0
mouse_left_down: bool = false
mouse_left_was_down: bool = false

// UI state
clicked_sidebar_item: i32 = -1
hover_color_intensity: f32 = 0.0
click_counter: i32 = 0

glfw_init :: proc() -> glfw.WindowHandle {
    if !glfw.Init() {
        fmt.println("Failed to initialize GLFW")
        return nil
    }

    glfw.WindowHint(glfw.CONTEXT_VERSION_MAJOR, 4)
    glfw.WindowHint(glfw.CONTEXT_VERSION_MINOR, 3)
    glfw.WindowHint(glfw.OPENGL_DEBUG_CONTEXT, 1)
    glfw.WindowHint(glfw.OPENGL_PROFILE, glfw.OPENGL_CORE_PROFILE)
    glfw.WindowHint(glfw.RESIZABLE, true)
    glfw.WindowHint(glfw.SAMPLES, 4)

    window := glfw.CreateWindow(WINDOW_WIDTH, WINDOW_HEIGHT, "App", nil, nil)
    if window == nil {
        fmt.println("Failed to create window")
        return nil
    }
    glfw.MakeContextCurrent(window)

    glfw.SetKeyCallback(window, key_callback)
    glfw.SetMouseButtonCallback(window, mouse_callback)
    glfw.SetCursorPosCallback(window, cursor_position_callback)
    glfw.SetScrollCallback(window, scroll_callback)
    glfw.SetFramebufferSizeCallback(window, framebuffer_size_callback)

    return window
}

glfw_destroy :: proc(window: glfw.WindowHandle) {
    glfw.MakeContextCurrent(nil)
    glfw.DestroyWindow(window)
    // NOTE: Skip glfw.Terminate() on Wayland due to possible segfault bug
    // glfw.Terminate()
}

opengl_init :: proc() {
    gl.load_up_to(4, 3, glfw.gl_set_proc_address)

    gl.Viewport(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT)
    gl.Enable(gl.BLEND)
    gl.Enable(gl.MULTISAMPLE)
    gl.BlendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA)

    gl.Enable(gl.DEBUG_OUTPUT)
    gl.DebugMessageCallback(message_callback, nil)
}

opengl_destroy :: proc(
    shaders: ^map[string]render.Shader,
    rect_renderer: ^render.RectRenderer,
    text_renderer: ^render.TextRenderer,
    window: glfw.WindowHandle,
) {
    glfw.MakeContextCurrent(window)

    for name, shader in shaders {
        gl.DeleteProgram(shader.id)
    }
    render.cleanup_rect_renderer(rect_renderer)
    render.cleanup_text_renderer(text_renderer)

    gl.Flush()
    gl.Finish()
}

// TODO: Full app reset
main :: proc() {
    context.logger = log.create_console_logger(runtime.Logger_Level.Debug)
    defer log.destroy_console_logger(context.logger)
    window := glfw_init()
    if window == nil {
        return
    }
    opengl_init()

    // TODO: Hot reload shaders
    shaders := make(map[string]render.Shader)

    shader, s_ok := render.load_shader_from_file(
        "shaders/rounded_rect.vs",
        "shaders/rounded_rect.frag",
    )
    if s_ok {
        fmt.println("Shaders compiled")
    } else {
        fmt.println("Unable to load shaders")
        return
    }
    shaders["rounded_rectangle"] = shader

    text_shader, text_ok := render.load_shader_from_file(
        "shaders/text.vs",
        "shaders/text.frag",
    )
    if text_ok {
        fmt.println("Text shaders compiled")
    } else {
        fmt.println("Unable to load text shaders")
        return
    }
    shaders["text"] = text_shader

    projection := glm.mat4Ortho3d(
        0.0,
        f32(window_width),
        f32(window_height),
        0.0,
        -1.0,
        1.0,
    )

    rect_renderer := render.RectRenderer {
        shader = &shaders["rounded_rectangle"],
    }
    render.use_shader(rect_renderer.shader)
    render.init_render_data(&rect_renderer)
    render.set_uniform(rect_renderer.shader, "projection", &projection)

    text_renderer := render.TextRenderer {
        shader = &shaders["text"],
    }
    if !render.init_text_renderer(
        &text_renderer,
        "freetype/demo/LiberationMono.ttf",
    ) {
        fmt.println("Failed to initialize text renderer")
        return
    }

    defer {
        opengl_destroy(&shaders, &rect_renderer, &text_renderer, window)
        delete(shaders)
        glfw_destroy(window)
    }


    render.use_shader(text_renderer.shader)
    render.set_uniform(text_renderer.shader, "projection", &projection)

    log.info("OpenGL/GLFW initialization complete")
    app_api_version := 0
    app_api, app_api_ok := load_app_api(app_api_version)

    if !app_api_ok {
        log.error("Failed to load app API")
        return
    }

    app_api_version += 1
    //Doesnt crash if the print is here
    log.info("###")
    app_api.init()
    log.info("###") //Does crash if the print is here

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
        if !app_api.update() {
            break
        }
        app_api.draw(
            &rect_renderer,
            &text_renderer,
            window_width,
            window_height,
        )

        // Update Clay with current mouse state
        // clay.SetPointerState({f32(mouse_x), f32(mouse_y)}, mouse_left_down)

        // Store previous mouse state for click detection
        // mouse_left_was_down = mouse_left_down

        glfw.SwapBuffers(window)

    }
    log.info("Shutting down")
    app_api.shutdown()
    unload_app_api(app_api)
}

key_callback :: proc "c" (
    window: glfw.WindowHandle,
    key, scancode, action, mods: i32,
) {
    if key == glfw.KEY_ESCAPE && action == glfw.PRESS {
        glfw.SetWindowShouldClose(window, true)
    }
}

mouse_callback :: proc "c" (
    window: glfw.WindowHandle,
    button, action, mods: i32,
) {
    context = runtime.default_context()
    if button == glfw.MOUSE_BUTTON_LEFT {
        mouse_left_down = action == glfw.PRESS || action == glfw.REPEAT
    }
}

cursor_position_callback :: proc "c" (
    window: glfw.WindowHandle,
    xpos, ypos: f64,
) {
    context = runtime.default_context()
    mouse_x = xpos
    mouse_y = ypos
}

scroll_callback :: proc "c" (
    window: glfw.WindowHandle,
    xoffset, yoffset: f64,
) {}

framebuffer_size_callback :: proc "c" (
    window: glfw.WindowHandle,
    width, height: i32,
) {
    context = runtime.default_context()
    gl.Viewport(0, 0, width, height)
    window_width = width
    window_height = height
    clay.SetLayoutDimensions({f32(width), f32(height)})
}

message_callback :: proc "c" (
    source: u32,
    type: u32,
    id: u32,
    severity: u32,
    length: i32,
    message: cstring,
    userParam: rawptr,
) {
    context = runtime.default_context()
    fmt.printfln("OPENGL: %v", message)
}

/* Contains pointers to the procedures exposed by the game DLL. */
AppAPI :: struct {
    init:         proc(),
    update:       proc() -> bool,
    draw:         proc(
        rect_renderer: ^render.RectRenderer,
        text_renderer: ^render.TextRenderer,
        window_width, window_height: i32,
    ),
    shutdown:     proc(),
    memory:       proc() -> rawptr,
    hot_reloaded: proc(_: rawptr),
    lib:          dynlib.Library,
    dll_time:     os.File_Time,
    api_version:  int,
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

    lib, lib_ok := dynlib.load_library(dll_name)
    if !lib_ok {
        log.error("Failed loading app DLL")
        return {}, false
    }

    api := AppAPI {
        init         = cast(proc(
        ))(dynlib.symbol_address(lib, "app_init") or_else nil),
        update       = cast(proc(
        ) -> bool)(dynlib.symbol_address(lib, "app_update") or_else nil),
        shutdown     = cast(proc(
        ))(dynlib.symbol_address(lib, "app_shutdown") or_else nil),
        memory       = cast(proc(
        ) -> rawptr)(dynlib.symbol_address(lib, "app_memory") or_else nil),
        hot_reloaded = cast(proc(
            _: rawptr,
        ))(dynlib.symbol_address(lib, "app_hot_reloaded") or_else nil),
        lib          = lib,
        dll_time     = dll_time,
        api_version  = api_version,
    }

    if api.init == nil ||
       api.update == nil ||
       api.shutdown == nil ||
       api.memory == nil ||
       api.hot_reloaded == nil {
        dynlib.unload_library(api.lib)
        fmt.println("App DLL missing required procedure")
        return {}, false
    }

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
