package main

import clay "../../clay-odin"
import "../../common"
import "../../render"
import "../app"
import "base:runtime"
import "core:c"
import "core:fmt"
import "core:log"
import glm "core:math/linalg/glsl"
import gl "vendor:OpenGL"
import glfw "vendor:glfw"

// Custom framebuffer callback for release (with Clay call)
framebuffer_size_callback :: proc "c" (
    window: glfw.WindowHandle,
    width, height: i32,
) {
    context = runtime.default_context()
    gl.Viewport(0, 0, width, height)
    common.window_width = width
    common.window_height = height
    clay.SetLayoutDimensions({f32(width), f32(height)})
}

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
    shaders, rect_renderer, text_renderer, setup_ok := common.setup_shaders_and_renderers()
    if !setup_ok {
        return
    }
    
    defer {
        common.opengl_destroy(&shaders, &rect_renderer, &text_renderer, window)
        delete(shaders)
    }

    log.info("OpenGL/GLFW initialization complete")
    
    // Initialize app directly (no DLL loading)
    app.app_init(&text_renderer, common.window_width, common.window_height)
    defer app.app_shutdown()

    for !glfw.WindowShouldClose(window) {
        glfw.PollEvents()

        // Update Clay with current mouse state
        app.app_set_mouse_state(f32(common.mouse_x), f32(common.mouse_y), common.mouse_left_down, common.mouse_left_was_down)

        // Store previous mouse state for click detection
        common.mouse_left_was_down = common.mouse_left_down

        // Update and draw app directly
        if !app.app_update() {
            break
        }
        
        app.app_draw(&rect_renderer, &text_renderer, common.window_width, common.window_height)

        glfw.SwapBuffers(window)
    }

    log.info("Shutting down")
}
