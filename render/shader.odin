package render

import "core:fmt"
import "core:log"
import glm "core:math/linalg/glsl"
import "core:os"
import "core:strings"
import gl "vendor:OpenGL"

Shader :: struct {
    id: u32,
}

use_shader :: proc(shader: ^Shader) {
    gl.UseProgram(shader.id)
}

compile_shader :: proc(
    vertex_src, frag_src, geo_src: ^cstring,
) -> (
    Shader,
    bool,
) {
    sVertex, sFrag, gShader: u32

    sVertex = gl.CreateShader(gl.VERTEX_SHADER)
    gl.ShaderSource(sVertex, 1, vertex_src, nil)
    gl.CompileShader(sVertex)
    success := check_compile_errors(sVertex, "VERTEX")
    defer gl.DeleteShader(sVertex)

    sFrag = gl.CreateShader(gl.FRAGMENT_SHADER)
    gl.ShaderSource(sFrag, 1, frag_src, nil)
    gl.CompileShader(sFrag)
    success = success && check_compile_errors(sFrag, "FRAGMENT")
    defer gl.DeleteShader(sFrag)

    if geo_src != nil {
        gShader = gl.CreateShader(gl.GEOMETRY_SHADER)
        gl.ShaderSource(gShader, 1, geo_src, nil)
        gl.CompileShader(gShader)
        success = success && check_compile_errors(gShader, "GEOMETRY")
        defer gl.DeleteShader(gShader)
    }

    if !success {
        return Shader{}, false
    }

    out := Shader {
        id = gl.CreateProgram(),
    }
    gl.AttachShader(out.id, sVertex)
    gl.AttachShader(out.id, sFrag)
    if geo_src != nil {
        gl.AttachShader(out.id, gShader)
    }
    gl.LinkProgram(out.id)
    check_compile_errors(out.id, "PROGRAM")
    return out, true
}

uniform_float :: proc(shader: ^Shader, name: cstring, value: f32) {
    loc := gl.GetUniformLocation(shader.id, name)
    gl.Uniform1f(loc, value)
}

uniform_int :: proc(shader: ^Shader, name: cstring, value: i32) {
    loc := gl.GetUniformLocation(shader.id, name)
    gl.Uniform1i(loc, value)
}

uniform_vec2 :: proc(shader: ^Shader, name: cstring, value: [2]f32) {
    loc := gl.GetUniformLocation(shader.id, name)
    gl.Uniform2f(loc, value.x, value.y)
}

uniform_vec3 :: proc(shader: ^Shader, name: cstring, value: [3]f32) {
    loc := gl.GetUniformLocation(shader.id, name)
    gl.Uniform3f(loc, value.x, value.y, value.z)
}

uniform_vec4 :: proc(shader: ^Shader, name: cstring, value: [4]f32) {
    loc := gl.GetUniformLocation(shader.id, name)
    gl.Uniform4f(loc, value.x, value.y, value.z, value.w)
}

uniform_mat4 :: proc(shader: ^Shader, name: cstring, value: ^glm.mat4) {
    loc := gl.GetUniformLocation(shader.id, name)
    gl.UniformMatrix4fv(loc, 1, false, raw_data(value))
}

// Sets a unifrom on the currently used shader
set_uniform :: proc {
    uniform_float,
    uniform_int,
    uniform_vec2,
    uniform_vec3,
    uniform_vec4,
    uniform_mat4,
}

check_compile_errors :: proc(object: u32, type: string) -> bool {
    success: i32
    infoLog: [1024]u8

    if type != "PROGRAM" {
        gl.GetShaderiv(object, gl.COMPILE_STATUS, &success)
        if success == 0 {
            gl.GetShaderInfoLog(object, 1024, nil, raw_data(&infoLog))
            fmt.printfln(
                "| ERROR::SHADER: Compile-time error: Type: %s Error flag: %v \n%s\n------------------",
                success,
                type,
                infoLog,
            )
        }
    } else {
        gl.GetProgramiv(object, gl.LINK_STATUS, &success)
        if success == 0 {
            gl.GetProgramInfoLog(object, 1024, nil, raw_data(&infoLog))
            fmt.printfln(
                "| ERROR::SHADER: Link-time error: Type: %s Error flag: %v \n%s\n------------------",
                success,
                type,
                infoLog,
            )
        }
    }

    return success != 0
}

load_shader_from_file :: proc(
    vertex_path, fragment_path: string,
    geo_path: string = "",
) -> (
    Shader,
    bool,
) {
    v_bytes, f_bytes, g_bytes: []u8
    v_ok := true
    f_ok := true
    g_ok := true

    v_bytes, v_ok = os.read_entire_file(vertex_path)
    f_bytes, f_ok = os.read_entire_file(fragment_path)
    if len(geo_path) > 0 {
        g_bytes, g_ok = os.read_entire_file(geo_path)
    }
    if !(v_ok && f_ok && g_ok) {
        return Shader{}, false
    }
    v_src := strings.clone_to_cstring(string(v_bytes))
    f_src := strings.clone_to_cstring(string(f_bytes))
    g_src := strings.clone_to_cstring(string(g_bytes))

    return compile_shader(&v_src, &f_src, &g_src if len(geo_path) > 0 else nil)
}
