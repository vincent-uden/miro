package render

import "core:testing"
import "core:fmt"
import "core:os"
import "core:c"
import freetype "../freetype"

// Simple PNG writer for debugging
write_png :: proc(filename: string, width, height: i32, data: []u8) -> bool {
    // Simple grayscale PNG writer
    file, err := os.open(filename, os.O_WRONLY | os.O_CREATE | os.O_TRUNC, 0o644)
    if err != os.ERROR_NONE {
        return false
    }
    defer os.close(file)

    // Write a simple PGM file instead (easier than PNG)
    header := fmt.aprintf("P5\n%d %d\n255\n", width, height)
    defer delete(header)
    
    os.write_string(file, header)
    os.write(file, data)
    return true
}

@(test)
test_character_atlas :: proc(t: ^testing.T) {
    fmt.println("Testing character atlas generation...")
    
    // Initialize FreeType
    ft_library: freetype.Library
    ft_error := freetype.init_free_type(&ft_library)
    testing.expect(t, ft_error == .Ok, "Failed to initialize FreeType")
    defer freetype.done_free_type(ft_library)

    // Load font
    ft_face: freetype.Face
    ft_error = freetype.new_face(ft_library, "freetype/demo/LiberationMono.ttf", 0, &ft_face)
    testing.expect(t, ft_error == .Ok, "Failed to load font face")
    defer freetype.done_face(ft_face)

    // Set font size
    font_size: u32 = 48
    ft_error = freetype.set_pixel_sizes(ft_face, 0, font_size)
    testing.expect(t, ft_error == .Ok, "Failed to set font size")

    // Create atlas for ASCII characters 32-126
    atlas_width: i32 = 512
    atlas_height: i32 = 512
    atlas_data := make([]u8, atlas_width * atlas_height)
    defer delete(atlas_data)

    // Clear atlas to black
    for i in 0..<len(atlas_data) {
        atlas_data[i] = 0
    }

    x: i32 = 0
    y: i32 = 0
    line_height: i32 = 0

    // Render characters 32-126 (printable ASCII)
    for char in 32..=126 {
        ft_error = freetype.load_char(ft_face, c.ulong(char), {})
        if ft_error != .Ok {
            fmt.printfln("Failed to load character: %c", char)
            continue
        }

        // Render the glyph to bitmap
        ft_error = freetype.render_glyph(ft_face.glyph, .Normal)
        if ft_error != .Ok {
            fmt.printfln("Failed to render character: %c", char)
            continue
        }

        glyph := ft_face.glyph
        bitmap := &glyph.bitmap

        // Check if we need to move to next line
        if x + i32(bitmap.width) > atlas_width {
            x = 0
            y += line_height + 2
            line_height = 0
        }

        // Check if we have space
        if y + i32(bitmap.rows) > atlas_height {
            fmt.println("Atlas too small for all characters")
            break
        }

        // Update line height
        line_height = max(line_height, i32(bitmap.rows))

        // Copy glyph bitmap to atlas
        if bitmap.buffer != nil {
            for row in 0..<bitmap.rows {
                for col in 0..<bitmap.width {
                    atlas_x := x + i32(col)
                    atlas_y := y + i32(row)
                    if atlas_x < atlas_width && atlas_y < atlas_height {
                        atlas_idx := atlas_y * atlas_width + atlas_x
                        glyph_idx := row * bitmap.width + col
                        atlas_data[atlas_idx] = bitmap.buffer[glyph_idx]
                    }
                }
            }
        }

        fmt.printfln("Character '%c': size=%dx%d, advance=%d, bearing=(%d,%d)", 
                    char, bitmap.width, bitmap.rows, 
                    glyph.advance.x >> 6, glyph.bitmap_left, glyph.bitmap_top)

        x += i32(bitmap.width) + 2
    }

    // Write atlas to file
    success := write_png("character_atlas.pgm", atlas_width, atlas_height, atlas_data)
    testing.expect(t, success, "Failed to write atlas file")
    
    if success {
        fmt.println("Character atlas written to character_atlas.pgm")
        fmt.println("You can view this with: convert character_atlas.pgm character_atlas.png")
    }
}

@(test)
test_single_character :: proc(t: ^testing.T) {
    fmt.println("Testing single character rendering...")
    
    // Initialize FreeType
    ft_library: freetype.Library
    ft_error := freetype.init_free_type(&ft_library)
    testing.expect(t, ft_error == .Ok, "Failed to initialize FreeType")
    defer freetype.done_free_type(ft_library)

    // Load font
    ft_face: freetype.Face
    ft_error = freetype.new_face(ft_library, "freetype/demo/LiberationMono.ttf", 0, &ft_face)
    testing.expect(t, ft_error == .Ok, "Failed to load font face")
    defer freetype.done_face(ft_face)

    // Set font size
    font_size: u32 = 64
    ft_error = freetype.set_pixel_sizes(ft_face, 0, font_size)
    testing.expect(t, ft_error == .Ok, "Failed to set font size")

    // Load character 'A'
    ft_error = freetype.load_char(ft_face, c.ulong('A'), {})
    testing.expect(t, ft_error == .Ok, "Failed to load character 'A'")

    // Render the glyph to bitmap
    ft_error = freetype.render_glyph(ft_face.glyph, .Normal)
    testing.expect(t, ft_error == .Ok, "Failed to render character 'A'")

    glyph := ft_face.glyph
    bitmap := &glyph.bitmap

    fmt.printfln("Character 'A' at %dpx:", font_size)
    fmt.printfln("  Bitmap size: %dx%d", bitmap.width, bitmap.rows)
    fmt.printfln("  Bearing: (%d, %d)", glyph.bitmap_left, glyph.bitmap_top)
    fmt.printfln("  Advance: %d", glyph.advance.x >> 6)
    fmt.printfln("  Buffer pointer: %p", bitmap.buffer)

    testing.expect(t, bitmap.buffer != nil, "Bitmap buffer is null")
    testing.expect(t, bitmap.width > 0, "Bitmap width is zero")
    testing.expect(t, bitmap.rows > 0, "Bitmap height is zero")

    if bitmap.buffer != nil && bitmap.width > 0 && bitmap.rows > 0 {
        // Write single character to file
        char_data := make([]u8, bitmap.width * bitmap.rows)
        defer delete(char_data)
        
        for i in 0..<(bitmap.width * bitmap.rows) {
            char_data[i] = bitmap.buffer[i]
        }
        
        success := write_png("character_A.pgm", i32(bitmap.width), i32(bitmap.rows), char_data)
        testing.expect(t, success, "Failed to write character file")
        
        if success {
            fmt.println("Character 'A' written to character_A.pgm")
        }
    }
}