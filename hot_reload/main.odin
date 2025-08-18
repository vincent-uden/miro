package main

import clay "../clay-odin"
import "../render"
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

main :: proc() {
    window := glfw_init()
    if window == nil {
        return
    }
    opengl_init()

    // Rendering
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

    // Load text shaders
    text_shader, text_ok := render.load_shader_from_file("shaders/text.vs", "shaders/text.frag")
    if text_ok {
        fmt.println("Text shaders compiled")
    } else {
        fmt.println("Unable to load text shaders")
        return
    }
    shaders["text"] = text_shader

    projection := glm.mat4Ortho3d(0.0, f32(window_width), f32(window_height), 0.0, -1.0, 1.0)

    // Initialize rect renderer
    rect_renderer := render.RectRenderer {
        shader = &shaders["rounded_rectangle"],
    }
    render.use_shader(rect_renderer.shader)
    render.init_render_data(&rect_renderer)
    render.set_uniform(rect_renderer.shader, "projection", &projection)

    // Initialize text renderer
    text_renderer := render.TextRenderer {
        shader = &shaders["text"],
    }
    if !render.init_text_renderer(&text_renderer, "freetype/demo/LiberationMono.ttf") {
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


    // Clay
    min_memory_size := clay.MinMemorySize()
    memory := make([^]u8, min_memory_size) // TODO: free memory
    arena: clay.Arena = clay.CreateArenaWithCapacityAndMemory(uint(min_memory_size), memory)
    clay.Initialize(arena, {f32(window_width), f32(window_height)}, {handler = clay_error_handler})
    clay.SetMeasureTextFunction(render.clay_measure_text_callback, &text_renderer)

    for !glfw.WindowShouldClose(window) {
        glfw.PollEvents()

        // Update Clay with current mouse state
        clay.SetPointerState({f32(mouse_x), f32(mouse_y)}, mouse_left_down)

        // Store previous mouse state for click detection
        defer mouse_left_was_down = mouse_left_down

        // Update projection matrix if window was resized
        new_projection := glm.mat4Ortho3d(
            0.0,
            f32(window_width),
            f32(window_height),
            0.0,
            -1.0,
            1.0,
        )
        render.use_shader(rect_renderer.shader)
        render.set_uniform(rect_renderer.shader, "projection", &new_projection)
        render.use_shader(text_renderer.shader)
        render.set_uniform(text_renderer.shader, "projection", &new_projection)

        gl.ClearColor(0.2, 0.2, 0.2, 1.0)
        gl.Clear(gl.COLOR_BUFFER_BIT)

        // Generate layout each frame (handles resizing)
        render_commands := create_layout()

        // Render all Clay commands using the render library
        render.render_clay_commands(
            &rect_renderer,
            &text_renderer,
            &render_commands,
            window_height,
        )

        glfw.SwapBuffers(window)
    }
}

key_callback :: proc "c" (window: glfw.WindowHandle, key, scancode, action, mods: i32) {
    if key == glfw.KEY_ESCAPE && action == glfw.PRESS {
        glfw.SetWindowShouldClose(window, true)
    }
}

mouse_callback :: proc "c" (window: glfw.WindowHandle, button, action, mods: i32) {
    context = runtime.default_context()
    if button == glfw.MOUSE_BUTTON_LEFT {
        mouse_left_down = action == glfw.PRESS || action == glfw.REPEAT
    }
}

cursor_position_callback :: proc "c" (window: glfw.WindowHandle, xpos, ypos: f64) {
    context = runtime.default_context()
    mouse_x = xpos
    mouse_y = ypos
}

scroll_callback :: proc "c" (window: glfw.WindowHandle, xoffset, yoffset: f64) {}

framebuffer_size_callback :: proc "c" (window: glfw.WindowHandle, width, height: i32) {
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

clay_error_handler :: proc "c" (errorData: clay.ErrorData) {
    // Do something
}


// Nord color scheme - https://www.nordtheme.com/
// Polar Night (dark colors)
NORD0 := clay.Color{0.18, 0.20, 0.25, 1.0} // #2e3440
NORD1 := clay.Color{0.23, 0.26, 0.32, 1.0} // #3b4252
NORD2 := clay.Color{0.26, 0.30, 0.37, 1.0} // #434c5e
NORD3 := clay.Color{0.30, 0.34, 0.42, 1.0} // #4c566a

// Snow Storm (light colors)
NORD4 := clay.Color{0.85, 0.87, 0.91, 1.0} // #d8dee9
NORD5 := clay.Color{0.90, 0.91, 0.94, 1.0} // #e5e9f0
NORD6 := clay.Color{0.93, 0.94, 0.96, 1.0} // #eceff4

// Frost (blue colors)
NORD7 := clay.Color{0.56, 0.74, 0.73, 1.0} // #8fbcbb
NORD8 := clay.Color{0.53, 0.75, 0.82, 1.0} // #88c0d0
NORD9 := clay.Color{0.51, 0.63, 0.76, 1.0} // #81a1c1
NORD10 := clay.Color{0.37, 0.51, 0.67, 1.0} // #5e81ac

// Aurora (accent colors)
NORD11 := clay.Color{0.75, 0.38, 0.42, 1.0} // #bf616a (red)
NORD12 := clay.Color{0.82, 0.53, 0.44, 1.0} // #d08770 (orange)
NORD13 := clay.Color{0.92, 0.80, 0.55, 1.0} // #ebcb8b (yellow)
NORD14 := clay.Color{0.64, 0.75, 0.55, 1.0} // #a3be8c (green)
NORD15 := clay.Color{0.71, 0.56, 0.68, 1.0} // #b48ead (purple)

// Semantic color mapping
COLOR_LIGHT := NORD6 // Snow storm lightest
COLOR_PRIMARY := NORD10 // Frost blue
COLOR_SECONDARY := NORD15 // Aurora purple
COLOR_SUCCESS := NORD14 // Aurora green
COLOR_DANGER := NORD11 // Aurora red
COLOR_BLACK := NORD0 // Polar night darkest

// Layout config is just a struct that can be declared statically, or inline
sidebar_item_layout := clay.LayoutConfig {
    sizing = {width = clay.SizingGrow({}), height = clay.SizingFixed(50)},
    padding = {16, 16, 16, 16},
    childAlignment = {x = .Left, y = .Center},
}

// Re-useable components are just normal procs.
sidebar_item_component :: proc(index: u32) {
    item_id := clay.GetElementIdWithIndex(clay.MakeString("SidebarBlob"), index)
    is_hovered := clay.PointerOver(item_id)

    // Change color based on hover and click state
    bg_color := COLOR_PRIMARY
    if clicked_sidebar_item == i32(index) {
        bg_color = COLOR_SECONDARY
    } else if is_hovered {
        bg_color = NORD9 // Lighter frost blue for hover
    }

    if clay.UI()(
    {
        id = item_id,
        layout = sidebar_item_layout,
        backgroundColor = bg_color,
        border = {color = COLOR_BLACK, width = {left = 4.0, right = 4.0, top = 4.0, bottom = 4.0}},
    },
    ) {
        // Check for clicks (only on mouse release)
        if is_hovered && !mouse_left_down && mouse_left_was_down {
            clicked_sidebar_item = i32(index)
        }

        // Add text to show the item number and state
        text_content := fmt.aprintf("Item %d", index)
        if clicked_sidebar_item == i32(index) {
            text_content = fmt.aprintf("Item %d (Clicked!)", index)
        }

        clay.TextDynamic(text_content, clay.TextConfig({textColor = COLOR_BLACK, fontSize = 14}))
    }
}

// An example function to create your layout tree
create_layout :: proc() -> clay.ClayArray(clay.RenderCommand) {
    // Begin constructing the layout.
    clay.BeginLayout()

    // An example of laying out a UI with a fixed-width sidebar and flexible-width main content
    // NOTE: To create a scope for child components, the Odin API uses `if` with components that have children
    if clay.UI()(
    {
        id = clay.ID("OuterContainer"),
        layout = {
            sizing = {width = clay.SizingGrow({}), height = clay.SizingGrow({})},
            padding = {16, 16, 16, 16},
            childGap = 16,
        },
        backgroundColor = NORD4,
    },
    ) {
        if clay.UI()(
        {
            id = clay.ID("SideBar"),
            layout = {
                layoutDirection = .TopToBottom,
                sizing = {width = clay.SizingFixed(300), height = clay.SizingGrow({})},
                padding = {16, 16, 16, 16},
                childGap = 16,
            },
            backgroundColor = COLOR_LIGHT,
        },
        ) {
            if clay.UI()(
            {
                id = clay.ID("ProfilePictureOuter"),
                layout = {
                    sizing = {width = clay.SizingGrow({})},
                    padding = {16, 16, 16, 16},
                    childGap = 16,
                    childAlignment = {y = .Center},
                },
                backgroundColor = COLOR_DANGER,
                cornerRadius = {6, 6, 6, 6},
            },
            ) {
                clay.Text(
                    "Clay - UI Library",
                    clay.TextConfig({textColor = COLOR_BLACK, fontSize = 16}),
                )
            }

            // Standard Odin code like loops, etc. work inside components.
            // Here we render 5 sidebar items.
            for i in u32(0) ..< 5 {
                sidebar_item_component(i)
            }
        }

        main_content_id := clay.ID("MainContent")
        main_content_hovered := clay.PointerOver(main_content_id)
        main_bg_color := COLOR_LIGHT
        if main_content_hovered {
            main_bg_color = NORD5
        }

        if clay.UI()(
        {
            id = main_content_id,
            layout = {
                sizing = {width = clay.SizingGrow({}), height = clay.SizingGrow({})},
                padding = {16, 16, 16, 16},
                childGap = 16,
                layoutDirection = .TopToBottom,
            },
            backgroundColor = main_bg_color,
        },
        ) {
            clay.Text(
                "This is the main content area. Resize the window to see how the text reflows and the layout adapts to different window sizes. The sidebar should remain fixed width while this area grows and shrinks.",
                clay.TextConfig({textColor = COLOR_BLACK, fontSize = 24}),
            )

            clay.Text(
                "Here's another paragraph of text to demonstrate text wrapping behavior. When the window becomes narrow, this text should wrap to multiple lines to fit within the available space.",
                clay.TextConfig({textColor = COLOR_DANGER, fontSize = 18}),
            )

            clay.Text(
                "And a third paragraph with different styling to show how multiple text elements behave in a resizable layout. ÅÄÖ",
                clay.TextConfig({textColor = COLOR_PRIMARY, fontSize = 18}),
            )

            // Interactive click counter button
            button_id := clay.ID("ClickButton")
            button_hovered := clay.PointerOver(button_id)
            button_color := COLOR_SUCCESS
            if button_hovered {
                button_color = NORD13 // Nord yellow for hover
            }

            if clay.UI()(
            {
                id = button_id,
                layout = {
                    sizing = {width = clay.SizingFixed(200), height = clay.SizingFixed(50)},
                    padding = {12, 12, 12, 12},
                    childAlignment = {x = .Center, y = .Center},
                },
                backgroundColor = button_color,
                cornerRadius = {8, 8, 8, 8},
                border = {color = COLOR_BLACK, width = {left = 2, right = 2, top = 2, bottom = 2}},
            },
            ) {
                // Check for clicks (only on mouse release)
                if button_hovered && !mouse_left_down && mouse_left_was_down {
                    click_counter += 1
                }

                button_text := fmt.aprintf("Clicks: %d", click_counter)
                clay.TextDynamic(
                    button_text,
                    clay.TextConfig({textColor = COLOR_BLACK, fontSize = 16}),
                )
            }

            // Scissor test demo section
            clay.Text(
                "Scissor Test Demo:",
                clay.TextConfig({textColor = COLOR_BLACK, fontSize = 18}),
            )

            // Clipped container with overflowing content
            if clay.UI()(
            {
                id = clay.ID("ClippedContainer"),
                layout = {
                    sizing = {width = clay.SizingFixed(300), height = clay.SizingFixed(150)},
                    padding = {8, 8, 8, 8},
                    childGap = 8,
                    layoutDirection = .TopToBottom,
                },
                backgroundColor = NORD5,
                cornerRadius = {4, 4, 4, 4},
                border = {color = COLOR_BLACK, width = {left = 2, right = 2, top = 2, bottom = 2}},
                clip = {horizontal = true, vertical = true},
            },
            ) {
                clay.Text(
                    "This content is clipped!",
                    clay.TextConfig({textColor = COLOR_DANGER, fontSize = 16}),
                )

                // Large content that should be clipped
                for i in 0 ..< 8 {
                    text_content := fmt.aprintf(
                        "Overflowing line %d - this text should be clipped when it goes beyond the container bounds",
                        i + 1,
                    )
                    clay.TextDynamic(
                        text_content,
                        clay.TextConfig({textColor = COLOR_PRIMARY, fontSize = 14}),
                    )
                }
            }

            // Nested clipping demo
            if clay.UI()(
            {
                id = clay.ID("NestedClipOuter"),
                layout = {
                    sizing = {width = clay.SizingFixed(400), height = clay.SizingFixed(200)},
                    padding = {16, 16, 16, 16},
                    childGap = 8,
                },
                backgroundColor = NORD8,
                cornerRadius = {8, 8, 8, 8},
                border = {color = COLOR_BLACK, width = {left = 3, right = 3, top = 3, bottom = 3}},
                clip = {horizontal = true, vertical = true},
            },
            ) {
                clay.Text(
                    "Outer clipped container",
                    clay.TextConfig({textColor = COLOR_BLACK, fontSize = 16}),
                )

                if clay.UI()(
                {
                    id = clay.ID("NestedClipInner"),
                    layout = {
                        sizing = {width = clay.SizingFixed(250), height = clay.SizingFixed(100)},
                        padding = {8, 8, 8, 8},
                        childGap = 4,
                        layoutDirection = .TopToBottom,
                    },
                    backgroundColor = NORD12,
                    cornerRadius = {4, 4, 4, 4},
                    border = {
                        color = COLOR_DANGER,
                        width = {left = 2, right = 2, top = 2, bottom = 2},
                    },
                    clip = {horizontal = true, vertical = true},
                },
                ) {
                    clay.Text(
                        "Inner clipped container",
                        clay.TextConfig({textColor = COLOR_DANGER, fontSize = 14}),
                    )

                    for i in 0 ..< 6 {
                        text_content := fmt.aprintf(
                            "Nested clip line %d - double clipping test",
                            i + 1,
                        )
                        clay.TextDynamic(
                            text_content,
                            clay.TextConfig({textColor = COLOR_BLACK, fontSize = 12}),
                        )
                    }
                }
            }
        }
    }

    // Returns a list of render commands
    return clay.EndLayout()
}

/* Contains pointers to the procedures exposed by the game DLL. */
AppAPI :: struct {
    init:         proc(),
    update:       proc() -> bool,
    shutdown:     proc(),
    memory:       proc() -> rawptr,
    hot_reloaded: proc(_: rawptr),
    lib:          dynlib.Library,
    dll_time:     os.File_Time,
    api_version:  int,
}

/* Load the game DLL and return a new GameAPI that contains pointers to the 
 * required procedures of the game DLL. */
load_game_api :: proc(api_version: int) -> (AppAPI, bool) {
    dll_time, dll_time_err := os.last_write_time_by_name("app.dll")
    if dll_time_err != os.ERROR_NONE {
        log.error("Could not fetch last write date of api.dll")
        return {}, false
    }

    dll_name := fmt.tprintf("api_{0}.dll", api_version)
    copy_cmd := fmt.ctprintf("copy api.dll {0}", dll_name)
    if libc.system(copy_cmd) != 0 {
        log.errorf("Failed to copy api.dll to {0}", dll_name)
        return {}, false
    }

    lib, lib_ok := dynlib.load_library(dll_name)
    if !lib_ok {
        log.error("Failed loading app DLL")
        return {}, false
    }

    api := AppAPI {
        init         = cast(proc())(dynlib.symbol_address(lib, "app_init") or_else nil),
        update       = cast(proc() -> bool)(dynlib.symbol_address(lib, "app_update") or_else nil),
        shutdown     = cast(proc())(dynlib.symbol_address(lib, "app_shutdown") or_else nil),
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

    return api, true
}
