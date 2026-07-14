# Image assets

Place image files directly in this directory. Each filename stem becomes the
generated resource key:

```rust
image!(ic_left) // assets/resources/images/ic_left.svg
```

Supported extensions are SVG, PNG, JPEG, GIF, WebP, BMP, and ICO. The build
validates each filename and embeds the file bytes in the executable.
