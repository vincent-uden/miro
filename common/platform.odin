package common

import "../render"
import "base:runtime"
import "core:fmt"
import gl "vendor:OpenGL"
import glfw "vendor:glfw"

// Callback type for framebuffer size changes
FramebufferSizeCallback :: proc "c" (window: glfw.WindowHandle, width, height: i32)

glfw_init :: proc(framebuffer_callback: FramebufferSizeCallback = framebuffer_size_callback_default) -> glfw.WindowHandle {
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
    glfw.SetFramebufferSizeCallback(window, framebuffer_callback)

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

// Default framebuffer callback
framebuffer_size_callback_default :: proc "c" (
    window: glfw.WindowHandle,
    width, height: i32,
) {
    context = runtime.default_context()
    gl.Viewport(0, 0, width, height)
    window_width = width
    window_height = height
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