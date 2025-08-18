package render

import clay "../clay-odin"
import freetype "../freetype"
import "base:runtime"
import "core:c"
import "core:fmt"
import glm "core:math/linalg/glsl"
import "core:strings"
import gl "vendor:OpenGL"


// Debugging blurry text:
// 
// It does not seem to be related to the freetype rendering. Text looks sharp at both 63 and 64 
// pixels when rendered to a bitmap in the tests.

// Cache key that includes font size for multi-size support
GlyphKey :: struct {
    character: rune,
    font_size: u32,
}

// Character info for glyph caching
Character :: struct {
    texture_id: u32, // OpenGL texture ID
    size:       [2]i32, // Size of glyph
    bearing:    [2]i32, // Offset from baseline to left/top of glyph
    advance:    i32, // Horizontal offset to advance to next glyph
}

TextRenderer :: struct {
    shader:     ^Shader,
    quadVAO:    u32,
    quadVBO:    u32,
    ft_library: freetype.Library,
    ft_face:    freetype.Face,
    characters: map[GlyphKey]Character,
}

init_text_renderer :: proc(renderer: ^TextRenderer, font_path: string) -> bool {
    // Initialize FreeType
    ft_error := freetype.init_free_type(&renderer.ft_library)
    if ft_error != .Ok {
        fmt.println("Failed to initialize FreeType library")
        return false
    }

    // Load font face
    ft_error = freetype.new_face(
        renderer.ft_library,
        strings.clone_to_cstring(font_path),
        0,
        &renderer.ft_face,
    )
    if ft_error != .Ok {
        fmt.println("Failed to load font")
        return false
    }

    // Disable byte-alignment restriction
    gl.PixelStorei(gl.UNPACK_ALIGNMENT, 1)

    // Initialize character cache
    renderer.characters = make(map[GlyphKey]Character)

    // Configure VAO/VBO for texture quads
    gl.GenVertexArrays(1, &renderer.quadVAO)
    gl.GenBuffers(1, &renderer.quadVBO)
    gl.BindVertexArray(renderer.quadVAO)
    gl.BindBuffer(gl.ARRAY_BUFFER, renderer.quadVBO)
    gl.BufferData(gl.ARRAY_BUFFER, size_of(f32) * 6 * 4, nil, gl.DYNAMIC_DRAW)
    gl.EnableVertexAttribArray(0)
    gl.VertexAttribPointer(0, 4, gl.FLOAT, gl.FALSE, 4 * size_of(f32), 0)
    gl.BindBuffer(gl.ARRAY_BUFFER, 0)
    gl.BindVertexArray(0)

    return true
}

load_character :: proc(renderer: ^TextRenderer, character: rune, font_size: u32) -> bool {
    // Set font size for this specific glyph
    ft_error := freetype.set_pixel_sizes(renderer.ft_face, 0, font_size)
    if ft_error != .Ok {
        return false
    }

    // Load character glyph
    ft_error = freetype.load_char(renderer.ft_face, c.ulong(character), {})
    if ft_error != .Ok {
        return false
    }

    // Render the glyph to bitmap
    ft_error = freetype.render_glyph(renderer.ft_face.glyph, .Normal)
    if ft_error != .Ok {
        return false
    }

    // Generate texture
    texture: u32
    gl.GenTextures(1, &texture)
    gl.BindTexture(gl.TEXTURE_2D, texture)
    gl.TexImage2D(
        gl.TEXTURE_2D,
        0,
        gl.RED,
        i32(renderer.ft_face.glyph.bitmap.width),
        i32(renderer.ft_face.glyph.bitmap.rows),
        0,
        gl.RED,
        gl.UNSIGNED_BYTE,
        renderer.ft_face.glyph.bitmap.buffer,
    )

    // Set texture options
    gl.TexParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
    gl.TexParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
    gl.TexParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST)
    gl.TexParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST)

    // Store character for later use with composite key
    key := GlyphKey {
        character = character,
        font_size = font_size,
    }
    char_info := Character {
        texture_id = texture,
        size       = {
            i32(renderer.ft_face.glyph.bitmap.width),
            i32(renderer.ft_face.glyph.bitmap.rows),
        },
        bearing    = {renderer.ft_face.glyph.bitmap_left, renderer.ft_face.glyph.bitmap_top},
        advance    = i32(renderer.ft_face.glyph.advance.x >> 6),
    }
    renderer.characters[key] = char_info

    gl.BindTexture(gl.TEXTURE_2D, 0)
    return true
}

draw_text :: proc(
    renderer: ^TextRenderer,
    text: string,
    position: [2]f32,
    font_size: u32,
    scale: f32,
    color: [3]f32,
) {
    use_shader(renderer.shader)
    set_uniform(renderer.shader, "textColor", color)
    set_uniform(renderer.shader, "text", i32(0)) // Set texture unit 0
    gl.ActiveTexture(gl.TEXTURE0)
    gl.BindVertexArray(renderer.quadVAO)

    x := position.x
    baseline_y := position.y

    for char in text {
        key := GlyphKey {
            character = char,
            font_size = font_size,
        }
        ch, exists := renderer.characters[key]
        if !exists {
            // Try to load character if not cached
            if !load_character(renderer, char, font_size) {
                continue
            }
            ch = renderer.characters[key]
        }

        // Position relative to baseline, rounded to pixel boundaries
        xpos := f32(i32(x + f32(ch.bearing.x) * scale + 0.5))
        ypos := f32(i32(baseline_y - f32(ch.bearing.y) * scale + 0.5)) // bearing.y is distance from baseline to top

        w := f32(ch.size.x) * scale
        h := f32(ch.size.y) * scale

        // Update VBO for each character (flip Y texture coordinates)
        vertices: [6][4]f32 = {
            {xpos, ypos + h, 0.0, 1.0},
            {xpos, ypos, 0.0, 0.0},
            {xpos + w, ypos, 1.0, 0.0},
            {xpos, ypos + h, 0.0, 1.0},
            {xpos + w, ypos, 1.0, 0.0},
            {xpos + w, ypos + h, 1.0, 1.0},
        }

        // Render glyph texture over quad
        gl.BindTexture(gl.TEXTURE_2D, ch.texture_id)
        // Update content of VBO memory
        gl.BindBuffer(gl.ARRAY_BUFFER, renderer.quadVBO)
        gl.BufferSubData(gl.ARRAY_BUFFER, 0, size_of(vertices), raw_data(&vertices))
        gl.BindBuffer(gl.ARRAY_BUFFER, 0)
        // Render quad
        gl.DrawArrays(gl.TRIANGLES, 0, 6)
        // Advance cursors for next glyph
        x += f32(ch.advance) * scale
    }

    gl.BindVertexArray(0)
    gl.BindTexture(gl.TEXTURE_2D, 0)
}

measure_text_size :: proc(renderer: ^TextRenderer, text: string, font_size: u32) -> [2]f32 {
    if len(text) == 0 || renderer == nil || renderer.ft_face == nil {
        return {0, f32(font_size)}
    }

    // Set font size
    ft_error := freetype.set_pixel_sizes(renderer.ft_face, 0, font_size)
    if ft_error != .Ok {
        return {f32(len(text)) * f32(font_size) * 0.6, f32(font_size)} // Fallback estimation
    }

    width: f32 = 0
    max_ascent: f32 = 0
    max_descent: f32 = 0

    for char in text {
        ft_error = freetype.load_char(renderer.ft_face, c.ulong(char), {.Bitmap_Metrics_Only})
        if ft_error != .Ok {
            // Fallback: estimate character width
            width += f32(font_size) * 0.6
            continue
        }

        glyph := renderer.ft_face.glyph

        // Accumulate width using advance
        width += f32(glyph.advance.x >> 6)

        // Track ascent (above baseline) and descent (below baseline)
        ascent := f32(glyph.bitmap_top)
        descent := f32(glyph.bitmap.rows) - ascent

        max_ascent = max(max_ascent, ascent)
        max_descent = max(max_descent, descent)
    }

    // Total height is ascent + descent
    height := max_ascent + max_descent
    if height == 0 {
        height = f32(font_size) // Fallback to font size
    }

    return {width, height}
}

cleanup_text_renderer :: proc(renderer: ^TextRenderer) {
    // Clean up textures
    for key, ch in renderer.characters {
        texture_id := ch.texture_id
        gl.DeleteTextures(1, &texture_id)
    }
    delete(renderer.characters)

    // Clean up FreeType
    freetype.done_face(renderer.ft_face)
    freetype.done_free_type(renderer.ft_library)

    // Clean up OpenGL objects
    gl.DeleteVertexArrays(1, &renderer.quadVAO)
    gl.DeleteBuffers(1, &renderer.quadVBO)
}

// Clay text measurement callback - can be used directly with clay.SetMeasureTextFunction
clay_measure_text_callback :: proc "c" (
    text: clay.StringSlice,
    config: ^clay.TextElementConfig,
    userData: rawptr,
) -> clay.Dimensions {
    context = runtime.default_context()

    if userData == nil || text.chars == nil || text.length <= 0 || config == nil {
        // Fallback to simple estimation
        return {width = f32(text.length * i32(config.fontSize)), height = f32(config.fontSize)}
    }

    text_renderer := cast(^TextRenderer)userData
    if text_renderer == nil {
        return {width = f32(text.length * i32(config.fontSize)), height = f32(config.fontSize)}
    }

    text_str := string(text.chars[:text.length])

    size := measure_text_size(text_renderer, text_str, u32(config.fontSize))
    return {width = size.x, height = size.y}
}
