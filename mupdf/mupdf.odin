package mupdf

import "core:c"

when ODIN_OS == .Windows do foreign import libthirdparty "Release/libthirdparty.lib"
when ODIN_OS == .Windows do foreign import mupdf "Release/libmupdf.lib"

when ODIN_OS == .Linux {
	@(require)
	foreign import mupdf "release/libmupdf.a"
}
when ODIN_OS == .Linux {
	@(require)
	foreign import libthirdparty "release/libmupdf-third.a"
}

pdf_context :: struct {
	alloc_context: rawptr,
	locks_context: rawptr,
	error_context: ^error_context,
	warn_context:  rawptr,
	font_context:  rawptr,
	aa_context:    rawptr,
	store:         rawptr,
	glyph_cache:   rawptr,
}

error_context :: struct {
	top:        ^error_stack_slot,
	stack:      [256]error_stack_slot,
	padding:    error_stack_slot,
	stack_base: ^error_stack_slot,
	errcode:    c.int,
	errnum:     c.int,
	print_user: rawptr,
	print:      proc(user: rawptr, message: cstring),
	message:    [256]c.char,
}

// Not the actual definition
error_stack_slot :: struct {
	buffer:  jmp_buf,
	state:   c.int,
	code:    c.int,
	padding: [32 - 8]c.char,
}

setjmp_float128 :: struct {
	part: [2]c.uint64_t,
}

jmp_buf :: [16]setjmp_float128

@(default_calling_convention = "c", link_prefix = "fz_")
foreign mupdf {
	@(link_name = "fz_new_context_imp")
	new_context :: proc(alloc, locks: rawptr, max_store: c.size_t, version: cstring) -> ^pdf_context ---
	@(link_name = "setjmp")
	setjmp :: proc(buf: c.int) -> c.int ---
	register_document_handlers :: proc(ctx: ^pdf_context) ---
	open_document :: proc(ctx: ^pdf_context, input: cstring) -> ^document ---
	count_pages :: proc(ctx: ^pdf_context, doc: ^document) -> c.int ---
	scale :: proc(sx: c.float, sy: c.float) -> pdf_matrix ---
	pre_rotate :: proc(m: pdf_matrix, degrees: c.float) -> pdf_matrix ---
	new_pixmap_from_page_number :: proc(ctx: ^pdf_context, doc: ^document, number: c.int, ctm: pdf_matrix, cs: ^colorspace, alpha: c.int) -> ^pixmap ---
	device_rgb :: proc(ctx: ^pdf_context) -> ^colorspace ---
	report_error :: proc(ctx: ^pdf_context) ---
	push_try :: proc(ctx: ^pdf_context) -> ^c.int ---
	do_try :: proc(ctx: ^pdf_context) -> c.int ---
	do_catch :: proc(ctx: ^pdf_context) -> c.int ---
}
