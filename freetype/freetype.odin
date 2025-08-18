package freetype

import "core:c"

when ODIN_OS == .Windows {
    foreign import freetype "freetype.lib"
} else when ODIN_OS == .Linux {
    foreign import freetype "system:freetype"
}

Library       :: distinct rawptr

Bool          :: distinct c.uchar
F26Dot6       :: distinct c.long
Fixed         :: distinct c.long
Pos           :: distinct c.long



Error :: enum c.int {
    Ok                            = 0x00,
    Cannot_Open_Resource          = 0x01,
    Unknown_File_Format           = 0x02,
    Invalid_File_Format           = 0x03,
    Invalid_Version               = 0x04,
    Lower_Module_Version          = 0x05,
    Invalid_Argument              = 0x06,
    Unimplemented_Feature         = 0x07,
    Invalid_Table                 = 0x08,
    Invalid_Offset                = 0x09,
    Array_Too_Large               = 0x0A,
    Missing_Module                = 0x0B,
    Missing_Property              = 0x0C,
    Invalid_Glyph_Index           = 0x10,
    Invalid_Character_Code        = 0x11,
    Invalid_Glyph_Format          = 0x12,
    Cannot_Render_Glyph           = 0x13,
    Invalid_Outline               = 0x14,
    Invalid_Composite             = 0x15,
    Too_Many_Hints                = 0x16,
    Invalid_Pixel_Size            = 0x17,
    Invalid_SVG_Document          = 0x18,
    Invalid_Handle                = 0x20,
    Invalid_Library_Handle        = 0x21,
    Invalid_Driver_Handle         = 0x22,
    Invalid_Face_Handle           = 0x23,
    Invalid_Size_Handle           = 0x24,
    Invalid_Slot_Handle           = 0x25,
    Invalid_CharMap_Handle        = 0x26,
    Invalid_Cache_Handle          = 0x27,
    Invalid_Stream_Handle         = 0x28,
    Too_Many_Drivers              = 0x30,
    Too_Many_Extensions           = 0x31,
    Out_Of_Memory                 = 0x40,
    Unlisted_Object               = 0x41,
    Cannot_Open_Stream            = 0x51,
    Invalid_Stream_Seek           = 0x52,
    Invalid_Stream_Skip           = 0x53,
    Invalid_Stream_Read           = 0x54,
    Invalid_Stream_Operation      = 0x55,
    Invalid_Frame_Operation       = 0x56,
    Nested_Frame_Access           = 0x57,
    Invalid_Frame_Read            = 0x58,
    Raster_Uninitialized          = 0x60,
    Raster_Corrupted              = 0x61,
    Raster_Overflow               = 0x62,
    Raster_Negative_Height        = 0x63,
    Too_Many_Caches               = 0x70,
    Invalid_Opcode                = 0x80,
    Too_Few_Arguments             = 0x81,
    Stack_Overflow                = 0x82,
    Code_Overflow                 = 0x83,
    Bad_Argument                  = 0x84,
    Divide_By_Zero                = 0x85,
    Invalid_Reference             = 0x86,
    Debug_OpCode                  = 0x87,
    ENDF_In_Exec_Stream           = 0x88,
    Nested_DEFS                   = 0x89,
    Invalid_CodeRange             = 0x8A,
    Execution_Too_Long            = 0x8B,
    Too_Many_Function_Defs        = 0x8C,
    Too_Many_Instruction_Defs     = 0x8D,
    Table_Missing                 = 0x8E,
    Horiz_Header_Missing          = 0x8F,
    Locations_Missing             = 0x90,
    Name_Table_Missing            = 0x91,
    CMap_Table_Missing            = 0x92,
    Hmtx_Table_Missing            = 0x93,
    Post_Table_Missing            = 0x94,
    Invalid_Horiz_Metrics         = 0x95,
    Invalid_CharMap_Format        = 0x96,
    Invalid_PPem                  = 0x97,
    Invalid_Vert_Metrics          = 0x98,
    Could_Not_Find_Context        = 0x99,
    Invalid_Post_Table_Format     = 0x9A,
    Invalid_Post_Table            = 0x9B,
    DEF_In_Glyf_Bytecode          = 0x9C,
    Missing_Bitmap                = 0x9D,
    Missing_SVG_Hooks             = 0x9E,
    Syntax_Error                  = 0xA0,
    Stack_Underflow               = 0xA1,
    Ignore                        = 0xA2,
    No_Unicode_Glyph_Name         = 0xA3,
    Glyph_Too_Big                 = 0xA4,
    Missing_Startfont_Field       = 0xB0,
    Missing_Font_Field            = 0xB1,
    Missing_Size_Field            = 0xB2,
    Missing_Fontboundingbox_Field = 0xB3,
    Missing_Chars_Field           = 0xB4,
    Missing_Startchar_Field       = 0xB5,
    Missing_Encoding_Field        = 0xB6,
    Missing_Bbx_Field             = 0xB7,
    Bbx_Too_Big                   = 0xB8,
    Corrupted_Font_Header         = 0xB9,
    Corrupted_Font_Glyphs         = 0xBA,
}


Char_Map      :: ^Char_Map_Rec
Driver        :: ^Driver_Rec
Driver_Class  :: ^Driver_Class_Rec
Face          :: ^Face_Rec
Glyph_Loader  :: ^Glyph_Loader_Rec
Glyph_Slot    :: ^Glyph_Slot_Rec
List_Node     :: ^List_Node_Rec
Memory        :: ^Memory_Rec
Module        :: ^Module_Rec
Size          :: ^Size_Rec
Size_Request  :: ^Size_Request_Rec
Stream        :: ^Stream_Rec
Sub_Glyph     :: ^Sub_Glyph_Rec

// Keeping internals rawptr for now.

Face_Internal :: rawptr
Size_Internal :: rawptr
Slot_Internal :: rawptr

Alloc_Func             :: #type proc "c" (memory: Memory, size: c.long) -> rawptr
Free_Func              :: #type proc "c" (memory: Memory, block: rawptr)
Realloc_Func           :: #type proc "c" (memory: Memory, cur_size, new_size: c.long, block: rawptr) -> rawptr

Generic_Finalizer      :: #type proc "c" (object: rawptr)

Face_Attach_Func       :: #type proc "c" (face: Face, stream: Stream) -> Error
Face_Init_Func         :: #type proc "c" (stream: Stream, face: Face, typeface_index, num_params: c.int, parameters: ^Parameter) -> Error
Face_Done_Func         :: #type proc "c" (face: Face)
Face_Get_Advances_Func :: #type proc "c" (face: Face, first, count: c.uint, flags: i32, advances: ^Fixed) -> Error
Face_Get_Kerning_Func  :: #type proc "c" (face: Face, left_glyph, right_glyph: c.uint, kerning: ^Vector) -> Error

Module_Constructor     :: #type proc "c" (module: Module) -> Error
Module_Destructor      :: #type proc "c" (module: Module)
Module_Requester       :: #type proc "c" (module: Module, name: cstring) -> rawptr

Size_Init_Func         :: #type proc "c" (size: Size) -> Error
Size_Done_Func         :: #type proc "c" (size: Size)
Size_Request_Func      :: #type proc "c" (size: Size, req: Size_Request) -> Error
Size_Select_Func       :: #type proc "c" (size: Size, size_index: c.ulong) -> Error

Slot_Init_Func         :: #type proc "c" (slot: Glyph_Slot) -> Error
Slot_Done_Func         :: #type proc "c" (slot: Glyph_Slot)
Slot_Load_Func         :: #type proc "c" (slot: Glyph_Slot, size: Size, glyph_index: c.uint, load_flags: i32) -> Error

Stream_IO_Func         :: #type proc "c" (stream: Stream, offset: c.ulong, buffer: ^c.uchar, count: c.ulong) -> c.ulong
Stream_Close_Func      :: #type proc "c" (stream: Stream)

B_Box :: struct {
    x_min, y_min, x_max, y_max : Pos,
}

Bitmap :: struct {
    rows         : c.uint,
    width        : c.uint,
    pitch        : c.int,
    buffer       : [^]c.uchar,
    num_grays    : c.ushort,
    pixel_mode   : c.char,
    palette_mode : c.uchar,
    palette      : rawptr,
}

Bitmap_Size :: struct {
    height : i16,
    width  : i16,

    size   : Pos,

    x_ppem : Pos,
    y_ppem : Pos,
}

Char_Map_Rec :: struct {
    face        : Face,
    encoding    : Encoding,
    platform_id : c.ushort,
    encoding_id : c.ushort,
}

Driver_Class_Rec :: struct {
    root             : Module_Class,

    face_object_size : c.long,
    size_object_size : c.long,
    slot_object_size : c.long,

    init_face        : Face_Init_Func,
    done_face        : Face_Done_Func,

    init_size        : Size_Init_Func,
    done_size        : Size_Done_Func,

    init_slot        : Slot_Init_Func,
    done_slot        : Slot_Done_Func,

    load_glyph       : Slot_Load_Func,

    get_kerning      : Face_Get_Kerning_Func,
    attach_file      : Face_Attach_Func,
    get_advances     : Face_Get_Advances_Func,

    request_size     : Size_Request_Func,
    select_size      : Size_Select_Func,
}

Driver_Rec :: struct {
    root         : Module_Rec,
    clazz        : Driver_Class,
    face_list    : List_Rec,
    glyph_loader : Glyph_Loader,
}

Encoding :: enum c.int {
    None            = 0,

    MS_Symbol       = c.int('s')<<24 | c.int('y')<<16 | c.int('m')<<8 | c.int('b')<<0,
    Unicode         = c.int('u')<<24 | c.int('n')<<16 | c.int('i')<<8 | c.int('c')<<0,

    Sjis            = c.int('s')<<24 | c.int('j')<<16 | c.int('i')<<8 | c.int('s')<<0,
    Prc             = c.int('g')<<24 | c.int('b')<<16 | c.int(' ')<<8 | c.int(' ')<<0,
    Big5            = c.int('b')<<24 | c.int('i')<<16 | c.int('g')<<8 | c.int('5')<<0,
    Wansung         = c.int('w')<<24 | c.int('a')<<16 | c.int('n')<<8 | c.int('s')<<0,
    Johab           = c.int('j')<<24 | c.int('o')<<16 | c.int('h')<<8 | c.int('a')<<0,

    Adobe_Standard  = c.int('A')<<24 | c.int('D')<<16 | c.int('O')<<8 | c.int('B')<<0,
    Adobe_Expert    = c.int('A')<<24 | c.int('D')<<16 | c.int('B')<<8 | c.int('E')<<0,
    Adobe_Custom    = c.int('A')<<24 | c.int('D')<<16 | c.int('B')<<8 | c.int('C')<<0,
    Adobe_Latin_1   = c.int('l')<<24 | c.int('a')<<16 | c.int('t')<<8 | c.int('1')<<0,

    Old_Latin_2     = c.int('l')<<24 | c.int('a')<<16 | c.int('t')<<8 | c.int('2')<<0,

    Apple_Roman     = c.int('a')<<24 | c.int('r')<<16 | c.int('m')<<8 | c.int('n')<<0,
}

Face_Rec :: struct {
    num_faces           : c.long,
    face_index          : c.long,

    face_flags          : c.long,
    style_flags         : c.long,

    num_glyphs          : c.long,

    family_name         : cstring,
    style_name          : cstring,

    num_fixed_sizes     : c.int,
    available_sizes     : [^]Bitmap_Size,

    num_charmaps        : c.int,
    charmaps            : [^]Char_Map,

    generic             : Generic,

    bbox                : B_Box,

    units_per_em        : c.ushort,
    ascender            : c.short,
    descender           : c.short,
    height              : c.short,

    max_advance_width   : c.short,
    max_advance_height  : c.short,

    underline_position  : c.short,
    underline_thickness : c.short,

    glyph               : Glyph_Slot,
    size                : Size,
    charmap             : Char_Map,

    driver              : Driver,
    memory              : Memory,
    stream              : Stream,

    sizes_list          : List_Rec,

    auto_hint           : Generic,
    extensions          : rawptr,

    internal            : Face_Internal,
}

Generic :: struct {
    data      : rawptr,
    finalizer : Generic_Finalizer,
}

Glyph_Format :: enum c.int {
    None       = 0,
    Composite  = c.int('c')<<24 | c.int('o')<<16 | c.int('m')<<8 | c.int('p')<<0,
    Bitmap     = c.int('b')<<24 | c.int('i')<<16 | c.int('t')<<8 | c.int('s')<<0,
    Outline    = c.int('o')<<24 | c.int('u')<<16 | c.int('t')<<8 | c.int('l')<<0,
    Plotter    = c.int('p')<<24 | c.int('l')<<16 | c.int('o')<<8 | c.int('t')<<0,
    Svg        = c.int('S')<<24 | c.int('V')<<16 | c.int('G')<<8 | c.int(' ')<<0,
}

Glyph_Load_Rec :: struct {
    outline       : Outline,
    extra_points  : ^Vector,
    extra_points2 : ^Vector,
    num_subglyphs : c.uint,
    subglyphs     : Sub_Glyph,
}

Glyph_Loader_Rec :: struct {
    memory    : Memory,
    points    : c.uint,
    contours  : c.uint,
    subglyphs : c.uint,
    extra     : Bool,

    base      : Glyph_Load_Rec,
    current   : Glyph_Load_Rec,

    other     : rawptr,
}

Glyph_Metrics :: struct {
    width          : Pos,
    height         : Pos,

    hori_bearing_x : Pos,
    hori_bearing_y : Pos,
    hori_advance   : Pos,

    vert_bearing_x : Pos,
    vert_bearing_y : Pos,
    vert_advance   : Pos,
}

Glyph_Slot_Rec :: struct {
    library             : Library,
    face                : Face,
    next                : Glyph_Slot,
    glyph_index         : c.uint,
    generic             : Generic,

    metrics             : Glyph_Metrics,
    linear_hori_advance : Fixed,
    linear_vert_advance : Fixed,
    advance             : Vector,

    format              : Glyph_Format,

    bitmap              : Bitmap,
    bitmap_left         : c.int,
    bitmap_top          : c.int,

    outline             : Outline,

    num_subglyphs       : c.uint,
    subglyphs           : Sub_Glyph,

    control_data        : rawptr,
    control_len         : c.long,

    lsb_delta           : Pos,
    rsb_delta           : Pos,

    other               : rawptr,

    internal            : Slot_Internal,
}

List_Node_Rec :: struct {
    prev, next: List_Node,
    data      : rawptr,
}

List_Rec :: struct {
    head, tail: List_Node,
}

Load_Flag :: enum i32 {
    No_Scale                    = 0,
    No_Hinting                  = 1,
    Render                      = 2,
    No_Bitmap                   = 3,
    Vertical_Layout             = 4,
    Force_Autohint              = 5,
    Crop_Bitmap                 = 6,
    Pedantic                    = 7,
    Ignore_Global_Advance_Width = 9,
    No_Recurse                  = 10,
    Ignore_Transform            = 11,
    Monochrome                  = 12,
    Linear_Design               = 13,
    S_Bits_Only                 = 14,
    No_Autohint                 = 15,
    Color                       = 20,
    Compute_Metrics             = 21,
    Bitmap_Metrics_Only         = 22,
    No_SVG                      = 24,

}

Load_Flags :: distinct bit_set[Load_Flag; i32]

Matrix :: struct {
    xx, xy, yx, yy : Fixed,
}

Memory_Rec :: struct {
    user    : rawptr,
    alloc   : Alloc_Func,
    free    : Free_Func,
    realloc : Realloc_Func,
}

Module_Class :: struct {
    module_flags     : c.ulong,
    module_size      : c.long,
    module_name      : cstring,
    module_version   : Fixed,
    module_requires  : Fixed,

    module_interface : rawptr,

    module_init      : Module_Constructor,
    module_done      : Module_Destructor,
    get_interface    : Module_Requester,
}

Module_Rec :: struct {
    clazz   : ^Module_Class,
    library : Library,
    memory  : Memory,
}

Outline :: struct {
    n_contours : c.short,
    n_points   : c.short,

    points     : ^Vector,
    tags       : ^c.char,
    contours   : ^c.short,

    flags      : c.int,
}

Parameter :: struct {
    tag  : c.ulong,
    data : rawptr,
}

Render_Mode :: enum {
    Normal = 0,
    Light  = 1,
    Mono   = 2,
    LCD    = 3,
    LCD_V  = 4,

    Max    = 5,
}

Size_Metrics :: struct {
    x_ppem      : c.ushort,
    y_ppem      : c.ushort,

    x_scale     : Fixed,
    y_scale     : Fixed,

    ascender    : Pos,
    descender   : Pos,
    height      : Pos,
    max_advance : Pos,
}

Size_Rec :: struct {
    face     : Face,
    generic  : Generic,
    metrics  : Size_Metrics,
    internal : Size_Internal,
}

Size_Request_Rec :: struct {
    type            : Size_Request_Type,
    width           : c.long,
    height          : c.long,
    hori_resolution : c.uint,
    vert_resolution : c.uint,
}

Size_Request_Type :: enum {
    Normal   = 0,
    Real_Dim = 1,
    B_Box    = 2,
    Cell     = 3,
    Scale    = 4,

    Max      = 5,
}

Stream_Desc :: struct {
    value  : c.long,
    ponter : rawptr,
}

Stream_Rec :: struct {
    base       : ^c.uchar,
    size       : c.ulong,
    pos        : c.ulong,

    descriptor : Stream_Desc,
    pathname   : Stream_Desc,
    read       : Stream_IO_Func,
    close      : Stream_Close_Func,

    memory     : Memory,
    cursor     : ^c.uchar,
    limit      : ^c.uchar,
}

Sub_Glyph_Rec :: struct {
    index     : c.int,
    flags     : c.ushort,
    arg1      : c.int,
    arg2      : c.int,
    transform : Matrix,
}

Vector :: struct {
    x, y : Pos,
}

@(default_calling_convention="c")
foreign freetype {
    @(link_name="FT_Init_FreeType") init_free_type :: proc(library: ^Library) -> Error ---
    @(link_name="FT_Done_FreeType") done_free_type :: proc(library: Library) -> Error ---

    @(link_name="FT_New_Face")        new_face        :: proc(library: Library, file_path_name: cstring, face_index: c.long, face: ^Face) -> Error ---
    @(link_name="FT_New_Memory_Face") new_memory_face :: proc(library: Library, file_base: ^byte, file_size, face_index: c.long, face: ^Face) -> Error ---
    @(link_name="FT_Done_Face")       done_face       :: proc(face: Face) -> Error ---

    @(link_name="FT_Load_Char")      load_char      :: proc(face: Face, char_code: c.ulong, load_flags: Load_Flags) -> Error ---
    @(link_name="FT_Set_Char_Size")  set_char_size  :: proc(face: Face, char_width, char_height: F26Dot6, horz_resolution, vert_resolution: c.uint) -> Error ---
    @(link_name="FT_Get_Char_Index") get_char_index :: proc(face: Face, code: c.ulong) -> c.uint ---
    
    @(link_name="FT_Load_Glyph")   load_glyph   :: proc(face: Face, index: c.uint, flags: Load_Flags) -> Error ---
    @(link_name="FT_Render_Glyph") render_glyph :: proc(slot: Glyph_Slot, render_mode: Render_Mode) -> Error ---

    @(link_name="FT_Set_Pixel_Sizes") set_pixel_sizes :: proc(face: Face, pixel_width, pixel_height: u32) -> Error ---
    @(link_name="FT_Request_Size")    request_size    :: proc(face: Face, req: Size_Request) -> Error ---
    @(link_name="FT_Select_Size")     select_size     :: proc(face: Face, strike_index: i32) -> Error ---

    @(link_name="FT_Set_Transform") set_transform   :: proc(face: Face, _matrix: ^Matrix, delta: ^Vector) ---
    @(link_name="FT_Get_Transform") get_transform   :: proc(face: Face, _matrix: ^Matrix, delta: ^Vector) ---
}