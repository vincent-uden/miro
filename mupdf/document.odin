package mupdf

import "core:c"

document :: struct {
	refs:                  c.int,
	drop_document:         ^document_drop_fn,
	needs_password:        ^document_needs_password_fn,
	authenticate_password: ^document_authenticate_password_fn,
	has_permission:        ^document_has_permission_fn,
	load_outline:          ^document_load_outline_fn,
	outline_iterator:      ^document_outline_iterator_fn,
	layout:                ^document_layout_fn,
	make_bookmark:         ^document_make_bookmark_fn,
	lookup_bookmark:       ^document_lookup_bookmark_fn,
	resolve_link_dest:     ^document_resolve_link_dest_fn,
	format_link_uri:       ^document_format_link_uri_fn,
	count_chapters:        ^document_count_chapters_fn,
	count_pages:           ^document_count_pages_fn,
	load_page:             ^document_load_page_fn,
	page_label:            ^document_page_label_fn,
	lookup_metadata:       ^document_lookup_metadata_fn,
	set_metadata:          ^document_set_metadata_fn,
	get_output_intent:     ^document_output_intent_fn,
	output_accelerator:    ^document_output_accelerator_fn,
	run_structure:         ^document_run_structure_fn,
	as_pdf:                ^document_as_pdf_fn,
}

permission :: struct {}

outline :: struct {}

outline_iterator :: struct {}

link_dest :: struct {}

pdf_page :: struct {
	refs:                                c.int,
	doc:                                 ^document,
	chapter, number, incomplete, in_doc: c.int,
	drop_page:                           ^page_drop_page_fn,
	bound_page:                          ^page_bound_page_fn,
	run_page_contents:                   ^page_run_page_fn,
	run_page_annots:                     ^page_run_page_fn,
	run_page_widgets:                    ^page_run_page_fn,
	load_links:                          ^page_load_links_fn,
	page_presentation:                   ^page_page_presentation_fn,
	control_separation:                  ^page_control_separation_fn,
	separation_disabled:                 ^page_separation_disabled_fn,
	separations:                         ^page_separations_fn,
	overprint:                           ^page_uses_overprint_fn,
	create_link:                         ^page_create_link_fn,
	delete_link:                         ^page_delete_link_fn,
	prev:                                ^^pdf_page,
	next:                                ^pdf_page,
}

document_drop_fn :: proc(ctx: ^pdf_context, doc: ^document)
document_needs_password_fn :: proc(ctx: ^pdf_context, doc: ^document) -> c.int
document_needs_authenticate_password_fn :: proc(
	ctx: ^pdf_context,
	doc: ^document,
	password: cstring,
) -> c.int
document_has_permission_fn :: proc(
	ctx: ^pdf_context,
	doc: ^document,
	permission: permission,
) -> c.int
document_load_outline_fn :: proc(ctx: ^pdf_context, doc: ^document) -> outline
document_outline_iterator_fn :: proc(ctx: ^pdf_context, doc: ^document) -> ^outline_iterator
document_layout_fn :: proc(ctx: ^pdf_context, doc: ^document, w, h, em: c.float)
document_resolve_link_dest_fn :: proc(ctx: ^pdf_context, doc: ^document, uri: cstring) -> link_dest
document_format_link_uri_fn :: proc(ctx: ^pdf_context, doc: ^document, dest: link_dest) -> cstring
document_count_chapters_fn :: proc(ctx: ^pdf_context, doc: ^document) -> c.int
document_count_pages_fn :: proc(ctx: ^pdf_context, doc: ^document, chapter: c.int) -> c.int
document_load_page_fn :: proc(ctx: ^pdf_context, doc: ^document, chapter, page: c.int) -> ^pdf_page
document_page_label_fn :: proc(
	ctx: ^pdf_context,
	doc: ^document,
	chapter, page: c.int,
	buf: ^c.char,
	size: c.size_t,
)
document_lookup_metadata_fn :: proc(
	ctx: ^pdf_context,
	doc: ^document,
	key: cstring,
	char: ^c.char,
	size: c.size_t,
) -> c.int
document_set_metadata_fn :: proc(
	ctx: ^pdf_context,
	doc: ^document,
	key: cstring,
	value: cstring,
) -> c.int
// TODO: 
document_output_intent_fn :: proc()
document_output_accelerator_fn :: proc()
document_run_structure_fn :: proc()
document_as_pdf_fn :: proc()
document_make_bookmark_fn :: proc()
document_lookup_bookmark_fn :: proc()
document_authenticate_password_fn :: proc()
page_drop_page_fn :: proc()
page_bound_page_fn :: proc()
page_run_page_fn :: proc()
page_load_links_fn :: proc()
page_page_presentation_fn :: proc()
page_control_separation_fn :: proc()
page_separation_disabled_fn :: proc()
page_separations_fn :: proc()
page_uses_overprint_fn :: proc()
page_create_link_fn :: proc()
page_delete_link_fn :: proc()
