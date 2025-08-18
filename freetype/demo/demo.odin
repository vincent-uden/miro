package demo

import freetype "../"
import "core:sys/windows"

FONT_FILE_NAME :: "LiberationMono.ttf"

font_height: u32 = 18
should_run: bool = true

RGBA :: struct {
	r, g, b, a: u8,
}

main :: proc() {
	wnd_class: windows.WNDCLASSEXW
	wnd_class.cbSize = size_of(wnd_class)
	wnd_class.lpfnWndProc = wnd_proc
	wnd_class.lpszClassName = windows.utf8_to_wstring("WNDCLASS")
	wnd_class.hInstance = cast(windows.HINSTANCE)windows.GetModuleHandleW(nil)
	windows.RegisterClassExW(&wnd_class)

	window := windows.CreateWindowExW(
		0,
		wnd_class.lpszClassName,
		windows.utf8_to_wstring("FreeType Demo"),
		windows.WS_OVERLAPPEDWINDOW | windows.WS_VISIBLE,
		windows.CW_USEDEFAULT,
		windows.CW_USEDEFAULT,
		800,
		600,
		nil,
		nil,
		wnd_class.hInstance,
		nil,
	)

	hdc := windows.GetDC(window)
	dpi := windows.GetDpiForWindow(window)

	ft_library: freetype.Library
	ft_error := freetype.init_free_type(&ft_library)
	assert(ft_error == .Ok)

	ft_face: freetype.Face
	ft_error = freetype.new_face(ft_library, FONT_FILE_NAME, 0, &ft_face)
	assert(ft_error == .Ok)

	window_width: i32
	window_height: i32

	bitmap_data: []RGBA
	bitmap_info: windows.BITMAPINFO

	current_font_height := font_height

	for should_run {

		rect: windows.RECT
		windows.GetClientRect(window, &rect)
		new_window_width := rect.right - rect.left
		new_window_height := rect.bottom - rect.top

		if (window_width != new_window_width || window_height != new_window_height) {
			window_width = new_window_width
			window_height = new_window_height

			if (bitmap_data != nil) {
				delete(bitmap_data)
			}

			bitmap_info.bmiHeader.biSize = size_of(bitmap_info.bmiHeader)
			bitmap_info.bmiHeader.biWidth = window_width
			bitmap_info.bmiHeader.biHeight = -window_height
			bitmap_info.bmiHeader.biPlanes = 1
			bitmap_info.bmiHeader.biBitCount = 32
			bitmap_info.bmiHeader.biCompression = windows.BI_RGB

			bitmap_data = make([]RGBA, window_width * window_height)

			render_font(ft_face, dpi, window_width, window_height, bitmap_data)
		}

		if (current_font_height != font_height) {
			current_font_height = font_height

			render_font(ft_face, dpi, window_width, window_height, bitmap_data)
		}

		msg: windows.MSG
		for windows.PeekMessageW(&msg, window, 0, 0, windows.PM_REMOVE) {
			windows.TranslateMessage(&msg)
			windows.DispatchMessageW(&msg)
		}

		if (window_width > 0 && window_height > 0) {
			windows.StretchDIBits(
				hdc,
				0,
				0,
				window_width,
				window_height,
				0,
				0,
				window_width,
				window_height,
				raw_data(bitmap_data),
				&bitmap_info,
				windows.DIB_RGB_COLORS,
				windows.SRCCOPY,
			)
		}
	}
}

wnd_proc :: proc "system" (
	hwnd: windows.HWND,
	msg: windows.UINT,
	w_param: windows.WPARAM,
	l_param: windows.LPARAM,
) -> windows.LRESULT {
	switch msg {
	case windows.WM_CLOSE:
		{
			should_run = false
			return 0
		}
	case windows.WM_KEYUP:
		{
			scan_code: u32 = cast(u32)(l_param & 0x00ff0000) >> 16
			vk_code := windows.MapVirtualKeyW(scan_code, windows.MAPVK_VSC_TO_VK_EX)
			switch vk_code {
			case windows.VK_UP:
				font_height += 1
			case windows.VK_DOWN:
				font_height -= 1
			}
			font_height = clamp(font_height, 1, 128)
		}
	}
	return windows.DefWindowProcW(hwnd, msg, w_param, l_param)
}

render_font :: proc(ft_face: freetype.Face, dpi: u32, window_width: i32, window_height: i32, bitmap_data: []RGBA) {
	for i in 0 ..< len(bitmap_data) {
		bitmap_data[i].r = 0
		bitmap_data[i].g = 0
		bitmap_data[i].b = 0
		bitmap_data[i].a = 255
	}

	ft_error := freetype.set_char_size(ft_face, 0, cast(freetype.F26Dot6)font_height * 64, dpi, dpi)
	assert(ft_error == .Ok)

	line_height: i32
	for c in 0 ..< 255 {
		ft_error = freetype.load_char(ft_face, cast(u32)c, {.Bitmap_Metrics_Only})
		assert(ft_error == .Ok)

		line_height = max(line_height, cast(i32)ft_face.glyph.bitmap.rows)
	}

	bitmap_margin: i32 = 4

	bitmap_offset: [2]i32
	bitmap_offset.x = bitmap_margin
	bitmap_offset.y = bitmap_margin + line_height

	for i in 0 ..< 255 {
		ft_error = freetype.load_char(ft_face, cast(u32)i, {})
		assert(ft_error == .Ok)

		ft_error = freetype.render_glyph(ft_face.glyph, .Normal)
		assert(ft_error == .Ok)

		if (bitmap_offset.x + cast(i32)ft_face.glyph.bitmap.width > window_width) {
			bitmap_offset.x = bitmap_margin
			bitmap_offset.y += bitmap_margin
			bitmap_offset.y += line_height
		}

		if (ft_face.glyph.bitmap.buffer != nil) {
			for y in 0 ..< ft_face.glyph.bitmap.rows {
				for x in 0 ..< ft_face.glyph.bitmap.width {
					bx := bitmap_offset.x + ft_face.glyph.bitmap_left + cast(i32)x
					by := bitmap_offset.y - ft_face.glyph.bitmap_top + cast(i32)y
					if (bx >= window_width || by >= window_height) {
						continue
					}
					b := bx + by * window_width
					g := x + y * ft_face.glyph.bitmap.width
					bitmap_data[b].r = ft_face.glyph.bitmap.buffer[g]
					bitmap_data[b].g = ft_face.glyph.bitmap.buffer[g]
					bitmap_data[b].b = ft_face.glyph.bitmap.buffer[g]
				}
			}
		}

		bitmap_offset.x += bitmap_margin
		bitmap_offset.x += cast(i32)(ft_face.glyph.advance.x >> 6)
	}
}
