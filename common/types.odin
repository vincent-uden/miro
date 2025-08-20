package common

// Constants
WINDOW_WIDTH :: 1000
WINDOW_HEIGHT :: 800

// Global state for window dimensions
window_width: i32 = WINDOW_WIDTH
window_height: i32 = WINDOW_HEIGHT

// Global mouse state
mouse_x: f64 = 0
mouse_y: f64 = 0
mouse_left_down: bool = false
mouse_left_was_down: bool = false

// UI state
clicked_sidebar_item: i32 = -1
hover_color_intensity: f32 = 0.0
click_counter: i32 = 0