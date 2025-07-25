#set page(width: 595pt, height: 842pt, margin: 0pt)
#set text(size: 10pt)

#let draw_point(x, y, label) = {
  place(
    top + left,
    dx: x * 1pt,
    dy: y * 1pt,
    circle(radius: 2pt, fill: red) + 
    place(dx: 5pt, dy: -3pt, text(size: 8pt, label))
  )
}

// Draw points at absolute page coordinates
#page()[
  // Page corner points
  #draw_point(0, 0, "(0,0)")
  #draw_point(595, 0, "(595,0)")
  #draw_point(0, 842, "(0,842)")
  #draw_point(595, 842, "(595,842)")
  
  // Page center point
  #draw_point(297, 421, "(297,421)")
  
  // Quarter points
  #draw_point(148, 210, "(148,210)")
  #draw_point(446, 210, "(446,210)")
  #draw_point(148, 631, "(148,631)")
  #draw_point(446, 631, "(446,631)")
  
  // Edge midpoints
  #draw_point(297, 0, "(297,0)")
  #draw_point(297, 842, "(297,842)")
  #draw_point(0, 421, "(0,421)")
  #draw_point(595, 421, "(595,421)")
  
  // Random test points
  #draw_point(100, 150, "(100,150)")
  #draw_point(200, 300, "(200,300)")
  #draw_point(350, 450, "(350,450)")
  #draw_point(400, 600, "(400,600)")
  #draw_point(75, 500, "(75,500)")
  #draw_point(520, 100, "(520,100)")
  #draw_point(50, 750, "(50,750)")
  #draw_point(545, 780, "(545,780)")
]

= Coordinate Points Test

This document shows points at various absolute coordinates on the page for testing purposes.

== Coordinate System Information

- Page size: 595pt × 842pt (A4 in points)
- No margins - coordinates are absolute from page origin
- Points are marked with red circles
- Coordinates are in pixels/points from top-left of page (0,0)
- Each point is labeled with its absolute (x,y) coordinates

== Test Points Summary

The following absolute coordinate points are marked on this page:

- Page corner points: (0,0), (595,0), (0,842), (595,842)
- Page center point: (297,421)
- Quarter points: (148,210), (446,210), (148,631), (446,631)
- Edge midpoints: (297,0), (297,842), (0,421), (595,421)
- Random points: (100,150), (200,300), (350,450), (400,600), (75,500), (520,100), (50,750), (545,780)

This document can be used to test coordinate-based functionality in PDF viewers and processing tools.