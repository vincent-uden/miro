package main

import "core:c"
import "core:fmt"
import "vendor:sdl3"

import "mupdf"

draw :: proc(renderer: ^sdl3.Renderer) {
	sdl3.SetRenderDrawColor(renderer, 255, 255, 255, 255)
	sdl3.RenderClear(renderer)
	sdl3.RenderPresent(renderer)
}

filter_event :: proc "cdecl" (user_data: rawptr, event: ^sdl3.Event) -> bool {
	if event.type == sdl3.EventType.WINDOW_RESIZED {
		return false
	}
	return true
}

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

	if !sdl3.Init(sdl3.INIT_VIDEO) {
		fmt.println("ERROR: ", sdl3.GetError())
	}
	defer sdl3.Quit()
	sdl3.SetEventFilter(filter_event, nil)

	window := sdl3.CreateWindow("Pdf", 800, 600, sdl3.WINDOW_RESIZABLE)
	if window == nil {
		fmt.println("ERROR: ", sdl3.GetError())
	}
	defer sdl3.DestroyWindow(window)
	renderer := sdl3.CreateRenderer(window, nil)
	if renderer == nil {
		fmt.println("ERROR: ", sdl3.GetError())
	}
	defer sdl3.DestroyRenderer(renderer)
	sdl3.SetRenderVSync(renderer, sdl3.RENDERER_VSYNC_ADAPTIVE)

	quit := false
	event: sdl3.Event
	for !quit {
		for sdl3.PollEvent(&event) {
			if event.type == sdl3.EventType.QUIT {
				quit = true
			}
		}
		draw(renderer)
	}
}
