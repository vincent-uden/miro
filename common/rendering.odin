package common

import "../render"
import "core:fmt"
import glm "core:math/linalg/glsl"
import gl "vendor:OpenGL"

setup_shaders_and_renderers :: proc() -> (
    shaders: map[string]render.Shader,
    rect_renderer: render.RectRenderer,
    text_renderer: render.TextRenderer,
    success: bool,
) {
    shaders = make(map[string]render.Shader)

    // Load rounded rectangle shader
    shader, s_ok := render.load_shader_from_file(
        "shaders/rounded_rect.vs",
        "shaders/rounded_rect.frag",
    )
    if s_ok {
        fmt.println("Shaders compiled")
    } else {
        fmt.println("Unable to load shaders")
        delete(shaders)
        return {}, {}, {}, false
    }
    shaders["rounded_rectangle"] = shader

    // Load text shader
    text_shader, text_ok := render.load_shader_from_file(
        "shaders/text.vs",
        "shaders/text.frag",
    )
    if text_ok {
        fmt.println("Text shaders compiled")
    } else {
        fmt.println("Unable to load text shaders")
        delete(shaders)
        return {}, {}, {}, false
    }
    shaders["text"] = text_shader

    // Setup projection matrix
    projection := glm.mat4Ortho3d(
        0.0,
        f32(window_width),
        f32(window_height),
        0.0,
        -1.0,
        1.0,
    )

    // Setup rectangle renderer
    rect_renderer = render.RectRenderer {
        shader = &shaders["rounded_rectangle"],
    }
    render.use_shader(rect_renderer.shader)
    render.init_render_data(&rect_renderer)
    render.set_uniform(rect_renderer.shader, "projection", &projection)

    // Setup text renderer
    text_renderer = render.TextRenderer {
        shader = &shaders["text"],
    }
    if !render.init_text_renderer(
        &text_renderer,
        "freetype/demo/LiberationMono.ttf",
    ) {
        fmt.println("Failed to initialize text renderer")
        delete(shaders)
        return {}, {}, {}, false
    }

    render.use_shader(text_renderer.shader)
    render.set_uniform(text_renderer.shader, "projection", &projection)

    return shaders, rect_renderer, text_renderer, true
}