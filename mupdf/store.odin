package mupdf

import "core:c"

storable :: struct {
	refs:      c.int,
	drop:      ^store_drop_fn,
	droppable: ^store_droppable_fn,
}

store_drop_fn :: proc(ctx: ^pdf_context, store: ^storable)
store_droppable_fn :: proc(ctx: ^pdf_context, store: ^storable) -> c.int
