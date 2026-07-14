# Image assets

Place real image files in this directory, then map them from
`../values/images.properties` using a path relative to `assets/resources`:

```properties
refresh=file:images/refresh.svg
```

The build validates the path and embeds the file bytes in the executable.
