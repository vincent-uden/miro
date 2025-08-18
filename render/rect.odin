package render

import clay "../clay-odin"
import "core:fmt"
import glm "core:math/linalg/glsl"
import gl "vendor:OpenGL"

ScissorRegion :: struct {
    x, y, width, height: i32,
}

RectRenderer :: struct {
    shader:        ^Shader,
    quadVAO:       u32,
    quadVBO:       u32,
    scissor_stack: [dynamic]ScissorRegion,
}

init_render_data :: proc(renderer: ^RectRenderer) {
    // odinfmt: disable
    vertices: []f32 = {
        // pos      // tex
        0.0, 1.0, 0.0, 1.0,
        1.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 0.0, 

        0.0, 1.0, 0.0, 1.0,
        1.0, 1.0, 1.0, 1.0,
        1.0, 0.0, 1.0, 0.0
    }
    // odinfmt: enable
    gl.GenVertexArrays(1, &renderer.quadVAO)
    gl.GenBuffers(1, &renderer.quadVBO)
    gl.BindBuffer(gl.ARRAY_BUFFER, renderer.quadVBO)
    gl.BufferData(
        gl.ARRAY_BUFFER,
        size_of(f32) * len(vertices),
        raw_data(vertices),
        gl.STATIC_DRAW,
    )

    gl.BindVertexArray(renderer.quadVAO)
    gl.EnableVertexAttribArray(0)
    // Bind VBO to VAO
    gl.VertexAttribPointer(0, 4, gl.FLOAT, gl.FALSE, 4 * size_of(f32), 0)

    // Unbind VBO
    gl.BindBuffer(gl.ARRAY_BUFFER, 0)
    // Unbind VAO
    gl.BindVertexArray(0)
}

cleanup_rect_renderer :: proc(renderer: ^RectRenderer) {
    gl.DeleteVertexArrays(1, &renderer.quadVAO)
    gl.DeleteBuffers(1, &renderer.quadVBO)
    delete(renderer.scissor_stack)
    gl.Disable(gl.SCISSOR_TEST)
}

draw_rect :: proc(
    renderer: ^RectRenderer,
    position: [2]f32,
    size: [2]f32,
    bg_color: [4]f32,
    border_color: [4]f32,
    border_thickness: f32,
    border_radius: [4]f32,
    edge_softness: f32,
) {
    use_shader(renderer.shader)
    // Odin extends matrices with ones on the diagonal. Since one f32 is a 1x1 
    // matrix this should be submatrix casted up to an identity 4x4
    model := glm.mat4(1.0)
    model = model * glm.mat4Translate({-0.5, -0.5, 0.0})
    scale := glm.vec3(1.0)
    scale.xy = size + edge_softness * 2.0
    model = model * glm.mat4Scale(scale)
    model = model * glm.mat4Translate({0.5, 0.5, 0.0} / scale)
    model = model * glm.mat4Translate({position.x, position.y, 0.0} / scale)
    model = model * glm.mat4Translate({-edge_softness, -edge_softness, 0.0} / scale)

    set_uniform(renderer.shader, "model", &model)
    set_uniform(renderer.shader, "bgColor", bg_color)
    set_uniform(renderer.shader, "borderColor", border_color)
    set_uniform(renderer.shader, "borderThickness", border_thickness)
    set_uniform(renderer.shader, "borderRadius", border_radius)
    set_uniform(renderer.shader, "edgeSoftness", edge_softness)
    set_uniform(renderer.shader, "size", size)

    gl.BindVertexArray(renderer.quadVAO)
    gl.DrawArrays(gl.TRIANGLES, 0, 6)
    gl.BindVertexArray(0)
}

// Renders Clay UI commands using the provided renderers
render_clay_commands :: proc(
    rect_renderer: ^RectRenderer,
    text_renderer: ^TextRenderer,
    render_commands: ^clay.ClayArray(clay.RenderCommand),
    window_height: i32 = 600,
) // Default fallback, should be passed from main
{
    for i in 0 ..< render_commands.length {
        cmd := clay.RenderCommandArray_Get(render_commands, i)

        switch cmd.commandType {
        case .Rectangle:
            draw_rect(
                rect_renderer,
                {cmd.boundingBox.x, cmd.boundingBox.y},
                {cmd.boundingBox.width, cmd.boundingBox.height},
                cmd.renderData.rectangle.backgroundColor,
                {0.0, 0.0, 0.0, 0.0},
                0.0,
                {
                    cmd.renderData.rectangle.cornerRadius.topLeft,
                    cmd.renderData.rectangle.cornerRadius.topRight,
                    cmd.renderData.rectangle.cornerRadius.bottomLeft,
                    cmd.renderData.rectangle.cornerRadius.bottomRight,
                },
                1.0,
            )

        case .Border:
            draw_rect(
                rect_renderer,
                {cmd.boundingBox.x, cmd.boundingBox.y},
                {cmd.boundingBox.width, cmd.boundingBox.height},
                {0.0, 0.0, 0.0, 0.0},
                cmd.renderData.border.color,
                f32(cmd.renderData.border.width.left),
                {
                    cmd.renderData.rectangle.cornerRadius.topLeft,
                    cmd.renderData.rectangle.cornerRadius.topRight,
                    cmd.renderData.rectangle.cornerRadius.bottomLeft,
                    cmd.renderData.rectangle.cornerRadius.bottomRight,
                },
                1.0,
            )

        case .Text:
            text_str := string(
                cmd.renderData.text.stringContents.chars[:cmd.renderData.text.stringContents.length],
            )
            // Position baseline at a reasonable position within the bounding box
            baseline_y := cmd.boundingBox.y + cmd.boundingBox.height * 0.8
            draw_text(
                text_renderer,
                text_str,
                {cmd.boundingBox.x, baseline_y},
                u32(cmd.renderData.text.fontSize), // Use Clay's font size
                1.0, // scale
                {
                    cmd.renderData.text.textColor.r,
                    cmd.renderData.text.textColor.g,
                    cmd.renderData.text.textColor.b,
                },
            )

        case .ScissorStart:
            // Convert Clay coordinates (top-left origin) to OpenGL coordinates (bottom-left origin)
            clay_y := i32(cmd.boundingBox.y)
            clay_height := i32(cmd.boundingBox.height)
            opengl_y := window_height - clay_y - clay_height

            new_region := ScissorRegion {
                x      = i32(cmd.boundingBox.x),
                y      = opengl_y,
                width  = i32(cmd.boundingBox.width),
                height = clay_height,
            }

            if len(rect_renderer.scissor_stack) > 0 {
                current := rect_renderer.scissor_stack[len(rect_renderer.scissor_stack) - 1]

                // Calculate intersection
                left := max(current.x, new_region.x)
                right := min(current.x + current.width, new_region.x + new_region.width)
                bottom := max(current.y, new_region.y)
                top := min(current.y + current.height, new_region.y + new_region.height)

                // Ensure valid intersection
                if left < right && bottom < top {
                    new_region = ScissorRegion {
                        x      = left,
                        y      = bottom,
                        width  = right - left,
                        height = top - bottom,
                    }
                } else {
                    // No intersection - create empty region
                    new_region = ScissorRegion {
                        x      = 0,
                        y      = 0,
                        width  = 0,
                        height = 0,
                    }
                }
            }

            append(&rect_renderer.scissor_stack, new_region)
            gl.Enable(gl.SCISSOR_TEST)
            gl.Scissor(new_region.x, new_region.y, new_region.width, new_region.height)

        case .ScissorEnd:
            if len(rect_renderer.scissor_stack) > 0 {
                pop(&rect_renderer.scissor_stack)
                if len(rect_renderer.scissor_stack) > 0 {
                    // Restore previous scissor region
                    region := rect_renderer.scissor_stack[len(rect_renderer.scissor_stack) - 1]
                    gl.Scissor(region.x, region.y, region.width, region.height)
                } else {
                    // No more scissor regions, disable scissor test
                    gl.Disable(gl.SCISSOR_TEST)
                }
            }

        // TODO: Implement these cases
        case .None:
        case .Image:
        case .Custom:
        }
    }
}
