# Agent Guidelines for Odin Rounded Rect Project

## Build Commands
- **Build demo**: `odin build demo`
- **Run demo**: `./demo.bin` (after building)
- **Run tests in package**:: `odin test <dir>`

## Code Style & Conventions
- **Language**: Odin programming language
- **Line width**: 100 characters (per odinfmt.json)
- **Indentation**: Spaces, not tabs (per odinfmt.json)
- **Import sorting**: Enabled (per odinfmt.json)
- **Import order**: Standard library first, then vendor packages, then local packages
- **Naming**: snake_case for variables/functions, PascalCase for types/structs
- **Constants**: SCREAMING_SNAKE_CASE
- **Procedures**: Use `proc` keyword, specify calling convention when needed (`"c"` for C interop)
- **Comments**: Use `//` for single line, avoid unless necessary for clarity
- **Error handling**: Return boolean success flags or explicit error types
- **Foreign imports**: Platform-specific conditional compilation using `when ODIN_OS`
- **Memory**: Manual memory management, use `make()` and `delete()` appropriately
- **Structs**: Use struct literals with field names for clarity
- **Arrays**: Prefer slices `[]T` over fixed arrays `[N]T` unless size is known at compile time

## Project Structure
- `clay-odin/`: Clay UI library bindings
- `demo/`: Main application demo
- `render/`: OpenGL rendering utilities
- `shaders/`: GLSL shader files
- `freetype/`: Font rendering (if used)

## Tests
Exampel test to base more tests on:
```odin
package example

import "core:testing"

@(test)
my_test :: proc(t: ^testing.T) {
    n := 2 + 2
    testing.expect(t, n == 4, "2 + 2 failed to equal 4.")
}
```
