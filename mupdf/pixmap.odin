package mupdf

import "core:c"

pixmap :: struct {
	storable:           storable,
	x:                  c.int,
	y:                  c.int,
	w:                  c.int,
	h:                  c.int,
	n, s, alpha, flags: c.uchar,
	stride:             c.ptrdiff_t,
	seps:               ^separations,
	xres, yres:         c.int,
	colorspace:         ^colorspace,
	samples:            cstring,
	underlying:         ^pixmap,
}

pdf_matrix :: struct {
	a, b, c, d, e, f: c.float,
}

separations :: struct {}

colorspace :: struct {}
