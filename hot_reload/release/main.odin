package main

import clay "../../clay-odin"
import "../../render"
import "../app"
import "base:runtime"
import "core:c"
import "core:fmt"
import "core:log"
import glm "core:math/linalg/glsl"
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

main :: proc() {
    context.logger = log.create_console_logger(runtime.Logger_Level.Debug)
    defer log.destroy_console_logger(context.logger)
    window := glfw_init()
    if window == nil {
        return
    }
    opengl_init()

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
    
    // Initialize app directly (no DLL loading)
    app.app_init(&text_renderer, window_width, window_height)
    defer app.app_shutdown()

    for !glfw.WindowShouldClose(window) {
        glfw.PollEvents()

        // Update Clay with current mouse state
        clay.SetPointerState({f32(mouse_x), f32(mouse_y)}, mouse_left_down)

        // Store previous mouse state for click detection
        mouse_left_was_down = mouse_left_down

        // Update and draw app directly
        if !app.app_update() {
            break
        }
        
        app.app_draw(&rect_renderer, &text_renderer, window_width, window_height)

        glfw.SwapBuffers(window)
    }

    log.info("Shutting down")
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