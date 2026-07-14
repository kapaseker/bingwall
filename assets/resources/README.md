# UI resources

`values/` is the complete default resource set. Locale directories such as
`values-zh/` may override a subset of `strings.properties`; omitted strings
fall back to the default value.

Resource keys must match `[a-z][a-z0-9_]*` and are used unchanged in Rust:

```rust
text!(daily_change)
color!(surface_scrim_strong)
dimension!(top_overlay_height)
image!(ic_left)
```

`build.rs` validates every properties file and image filename, then generates one typed
static descriptor plus one explicit macro arm per resource. Macros reference descriptors
directly; no key-to-value lookup table is generated. Invalid keys, duplicate keys,
malformed values, unknown translations, and mismatched translation placeholders stop
the build.

Text resources read the application-wide locale at resolution time, so changing
the locale does not require a per-function resource binding. Colors, dimensions,
and images are compile-time static resources.

## File formats

- `strings.properties`: UTF-8 text; named placeholders use `{name}`.
- `colors.properties`: `#RRGGBB`, `#RRGGBBAA`, or `rgba(r,g,b,a)`.
- `dimensions.properties`: a finite non-negative number representing fixed logical pixels.
Real image files belong directly in `images/` and are embedded into the executable at
compile time. Each filename stem becomes its Rust resource key, so `ic_left.svg` is used
as `image!(ic_left)`.
