# miro

A pdf viewer

## TODO:
- Experiment with rendering pdf as svg
    - Can we use vello to render the SVG? Iced perf is really bad for high levels of zoom
Additionally it's not only zooming which is slow, but also moving around which isnt slow on the image variant

## Dependencies
- [Raylib](https://www.raylib.com/), graphics library. Included as a vendored library in Odin.
- [MuPDF](https://mupdf.com/#mupdf-source-code), pdf manipulation and rendering library. Included as pre-built libraries for windows and linux.
