package main

import "core:c"
import "core:fmt"
import "vendor:raylib"

import "mupdf"

main :: proc() {
	file_name: cstring = "./test.pdf"
	zoom: f32 = 1.0
	rotate: f32 = 0.0
	page_number := 0

	ctx: ^mupdf.pdf_context
	doc: ^mupdf.document
	pix: ^mupdf.pixmap
	ctm: mupdf.pdf_matrix

	ctx = mupdf.new_context(nil, nil, 0, "1.26.0")
	mupdf.register_document_handlers(ctx)
	doc = mupdf.open_document(ctx, file_name)

	page_count := mupdf.count_pages(ctx, doc)
	fmt.println("Number of pages: ", page_count)

	ctm = mupdf.scale(1.0, 1.0)
	ctm = mupdf.pre_rotate(ctm, 0.0)
	pix = mupdf.new_pixmap_from_page_number(ctx, doc, 1, ctm, mupdf.device_rgb(ctx), 0)

	fmt.println("Bitmap W: ", pix.w, " H: ", pix.h, " n: ", pix.n)

	n: i32 = auto_cast pix.n
	samples: [^]c.char = auto_cast pix.samples
	stride: i32 = auto_cast pix.stride

	raylib.InitWindow(1600, 900, "Pdf")
	raylib.SetTargetFPS(60)

	img := raylib.Image {
		data    = auto_cast pix.samples,
		width   = pix.w,
		height  = pix.h,
		mipmaps = 1,
		format  = raylib.PixelFormat.UNCOMPRESSED_R8G8B8,
	}
	texture := raylib.LoadTextureFromImage(img)
	defer raylib.UnloadTexture(texture)


	for !raylib.WindowShouldClose() {
		raylib.BeginDrawing()
		raylib.ClearBackground(raylib.RAYWHITE)
		raylib.DrawTexture(texture, 0, 0, raylib.WHITE)
		raylib.DrawText("Hello world", 700, 400, 20, raylib.LIGHTGRAY)
		raylib.EndDrawing()
	}

	raylib.CloseWindow()
}

// Compile with odin.exe run . -extra-linker-flags:"/NODEFAULTLIB:libcmt" on windows
