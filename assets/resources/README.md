# UI resources

`values/` is the complete default resource set. Locale directories such as
`values-zh/` may override a subset of `strings.properties`; omitted strings
fall back to the default value.

Resource keys must match `[a-z][a-z0-9_]*` and are used unchanged in Rust:

```rust
text!(daily_change)
color!(surface_scrim_strong)
dimension!(top_overlay_height)
image!(previous)
```

`build.rs` validates every properties file and generates typed keys plus one
explicit macro arm per resource. Invalid keys, duplicate keys, malformed
values, unknown translations, and mismatched translation placeholders stop the
build.

## File formats

- `strings.properties`: UTF-8 text; named placeholders use `{name}`.
- `colors.properties`: `#RRGGBB`, `#RRGGBBAA`, or `rgba(r,g,b,a)`.
- `dimensions.properties`: `layout:number` or `text:number`.
- `images.properties`: `placeholder:text` or `file:images/relative-path`.

Real image files belong in `images/` and are embedded into the executable at
compile time. Navigation images currently use configured text placeholders.
