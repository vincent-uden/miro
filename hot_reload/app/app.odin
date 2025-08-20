
package app

import clay "../../clay-odin"
import "../../render"
import "core:fmt"
import "core:log"
import glm "core:math/linalg/glsl"
import gl "vendor:OpenGL"
import glfw "vendor:glfw"

// Nord color scheme - https://www.nordtheme.com/
// Polar Night (dark colors)
NORD0 := clay.Color{0.18, 0.20, 0.25, 1.0} // #2e3440
NORD1 := clay.Color{0.23, 0.26, 0.32, 1.0} // #3b4252
NORD2 := clay.Color{0.26, 0.30, 0.37, 1.0} // #434c5e
NORD3 := clay.Color{0.30, 0.34, 0.42, 1.0} // #4c566a
NORD4 := clay.Color{0.85, 0.87, 0.91, 1.0} // #d8dee9
NORD5 := clay.Color{0.90, 0.91, 0.94, 1.0} // #e5e9f0
NORD6 := clay.Color{0.93, 0.94, 0.96, 1.0} // #eceff4
NORD7 := clay.Color{0.56, 0.74, 0.73, 1.0} // #8fbcbb
NORD8 := clay.Color{0.53, 0.75, 0.82, 1.0} // #88c0d0
NORD9 := clay.Color{0.51, 0.63, 0.76, 1.0} // #81a1c1
NORD10 := clay.Color{0.37, 0.51, 0.67, 1.0} // #5e81ac
NORD11 := clay.Color{0.75, 0.38, 0.42, 1.0} // #bf616a (red)
NORD12 := clay.Color{0.82, 0.53, 0.44, 1.0} // #d08770 (orange)
NORD13 := clay.Color{0.92, 0.80, 0.55, 1.0} // #ebcb8b (yellow)
NORD14 := clay.Color{0.64, 0.75, 0.55, 1.0} // #a3be8c (green)
NORD15 := clay.Color{0.71, 0.56, 0.68, 1.0} // #b48ead (purple)
// Semantic color mapping
COLOR_LIGHT := NORD6
COLOR_PRIMARY := NORD10
COLOR_SECONDARY := NORD15
COLOR_SUCCESS := NORD14
COLOR_DANGER := NORD11
COLOR_BLACK := NORD0

AppMemory :: struct {
    some_state:    int,
    arena:         clay.Arena,
    text_renderer: ^render.TextRenderer,
    window_width:  i32,
    window_height: i32,
}

mem: ^AppMemory

/* Allocates the appMemory that we use to store
our app's state. We assign it to a global
variable so we can use it from the other
procedures. */
@(export)
app_init :: proc(
    text_renderer: ^render.TextRenderer,
    window_width, window_height: i32,
) {
    // Load OpenGL function pointers in the DLL
    log.info("Loading OpenGL function pointers in DLL...")
    gl.load_up_to(4, 3, glfw.gl_set_proc_address)
    log.info("OpenGL function pointers loaded")

    min_memory_size := clay.MinMemorySize()
    // TODO: free memory
    memory := make([^]u8, min_memory_size)
    arena: clay.Arena = clay.CreateArenaWithCapacityAndMemory(
        uint(min_memory_size),
        memory,
    )
    clay.Initialize(
        arena,
        {f32(window_width), f32(window_height)},
        {handler = clay_error_handler},
    )
    clay.SetMeasureTextFunction(
        render.clay_measure_text_callback,
        text_renderer,
    )

    mem = new(AppMemory)
    mem.arena = arena
    mem.text_renderer = text_renderer
    mem.window_width = window_width
    mem.window_height = window_height
}

/* Simulation and rendering goes here. Return
false when you wish to terminate the program. */
@(export)
app_update :: proc() -> bool {
    mem.some_state += 1
    return true
}

@(export)
app_draw :: proc(
    rect_renderer: ^render.RectRenderer,
    text_renderer: ^render.TextRenderer,
    window_width, window_height: i32,
) {
    // Update Clay layout dimensions if window was resized
    if mem.window_width != window_width || mem.window_height != window_height {
        mem.window_width = window_width
        mem.window_height = window_height
        clay.SetLayoutDimensions({f32(window_width), f32(window_height)})
    }

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
        rect_renderer,
        text_renderer,
        &render_commands,
        window_height,
    )
}

/* Called by the main program when the main loop
has exited. Clean up your memory here. */
@(export)
app_shutdown :: proc() {
    // TODO: Free arena
    free(mem)
}

/* Returns a pointer to the app memory. When
hot reloading, the main program needs a pointer
to the app memory. It can then load a new app
DLL and tell it to use the same memory by calling
app_hot_reloaded on the new app DLL, supplying
it the app memory pointer. */
@(export)
app_memory :: proc() -> rawptr {
    return mem
}

/* Used to set the app memory pointer after a
hot reload occurs. See app_memory comments. */
@(export)
app_hot_reloaded :: proc(m: ^AppMemory) {
    // Load OpenGL function pointers in the new DLL instance
    log.info("Reloading OpenGL function pointers in new DLL instance...")
    gl.load_up_to(4, 3, glfw.gl_set_proc_address)
    log.info("OpenGL function pointers reloaded")

    mem = m

    // Reinitialize Clay with the preserved arena
    log.info("Reinitializing Clay after hot reload...")
    clay.Initialize(
        mem.arena,
        {f32(mem.window_width), f32(mem.window_height)},
        {handler = clay_error_handler},
    )
    clay.SetMeasureTextFunction(
        render.clay_measure_text_callback,
        mem.text_renderer,
    )
    log.info("Clay reinitialized")
}

clay_error_handler :: proc "c" (errorData: clay.ErrorData) {
    // Do something
}


create_layout :: proc() -> clay.ClayArray(clay.RenderCommand) {
    clay.BeginLayout()
    if clay.UI()(
    {
        id = clay.ID("OuterContainer"),
        layout = {
            sizing = {
                width = clay.SizingGrow({}),
                height = clay.SizingGrow({}),
            },
            padding = {16, 16, 16, 16},
            childGap = 16,
        },
        backgroundColor = NORD4,
    },
    ) {}

    // Returns a list of render commands
    return clay.EndLayout()
}
