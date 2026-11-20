# zcall mark

The open gateway mark uses one continuous outline per half. Use the dark-ink mark on light surfaces and the light-ink mark on dark surfaces. Keep it small beside the application name; status and controls take priority.

The native renderer in src/main.rs uses matching convex contours and the current surface background for the cutout. Its GPU drawing does not load raster images.

The app-icon SVG is the source for the checked-in PNG, Windows ICO and macOS ICNS files. Run make icons with librsvg installed to regenerate those developer assets; application builds and runtime do not require the renderer. ICO entries contain compact PNG images at six sizes, while ICNS contains four native PNG sizes.
