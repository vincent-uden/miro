package render

import freetype "../freetype"
import "core:c"
import "core:fmt"
import "core:log"
import "core:os"
import "core:testing"
import stb_image "vendor:stb/image"

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
    font_size: u32 = 63
    ft_error = freetype.set_pixel_sizes(ft_face, 0, font_size)
    testing.expect(t, ft_error == .Ok, "Failed to set font size")

    // Create atlas for ASCII characters 32-126
    atlas_width: i32 = 512
    atlas_height: i32 = 512
    atlas_data := make([]u8, atlas_width * atlas_height)
    defer delete(atlas_data)

    // Clear atlas to black
    for i in 0 ..< len(atlas_data) {
        atlas_data[i] = 0
    }

    x: i32 = 0
    y: i32 = 0
    line_height: i32 = 0

    // Render characters 32-126 (printable ASCII)
    for char in 32 ..= 126 {
        ft_error = freetype.load_char(ft_face, c.ulong(char), {})
        if ft_error != .Ok {
            log.debugf("Failed to load character: %c", char)
            continue
        }

        // Render the glyph to bitmap
        ft_error = freetype.render_glyph(ft_face.glyph, .Normal)
        if ft_error != .Ok {
            log.debugf("Failed to render character: %c", char)
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
            for row in 0 ..< bitmap.rows {
                for col in 0 ..< bitmap.width {
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

        log.debugf(
            "Character '%c': size=%dx%d, advance=%d, bearing=(%d,%d)",
            char,
            bitmap.width,
            bitmap.rows,
            glyph.advance.x >> 6,
            glyph.bitmap_left,
            glyph.bitmap_top,
        )

        x += i32(bitmap.width) + 2
    }

    // Write atlas to file
    success := stb_image.write_png(
        "character_atlas.png",
        atlas_width,
        atlas_height,
        1,
        raw_data(atlas_data[:]),
        0,
    )
    testing.expect(t, success != 0, "Failed to write atlas file")

    if success != 0 {
        fmt.println("Character atlas written to character_atlas.png")
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

    log.debugf("Character 'A' at %dpx:", font_size)
    log.debugf("  Bitmap size: %dx%d", bitmap.width, bitmap.rows)
    log.debugf("  Bearing: (%d, %d)", glyph.bitmap_left, glyph.bitmap_top)
    log.debugf("  Advance: %d", glyph.advance.x >> 6)
    log.debugf("  Buffer pointer: %p", bitmap.buffer)
    log.debugf("  Pitch: %d", bitmap.pitch)

    testing.expect(t, bitmap.buffer != nil, "Bitmap buffer is null")
    testing.expect(t, bitmap.width > 0, "Bitmap width is zero")
    testing.expect(t, bitmap.rows > 0, "Bitmap height is zero")

    if bitmap.buffer != nil && bitmap.width > 0 && bitmap.rows > 0 {
        success := stb_image.write_png(
            "character_A.png",
            i32(bitmap.width),
            i32(bitmap.rows),
            1,
            bitmap.buffer,
            0,
        )
        testing.expect(t, success != 0, "Failed to write character file")

        if success != 0 {
            fmt.println("Character 'A' written to character_A.png")
        }
    }
}

