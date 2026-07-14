use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

const RESOURCE_ROOT: &str = "assets/resources";

/// Loads validated properties resources and writes their strongly typed Rust representation.
fn main() {
    println!("cargo:rerun-if-changed={RESOURCE_ROOT}");
    let root = Path::new(RESOURCE_ROOT);
    let default_strings = read_properties(&root.join("values/strings.properties"));
    let chinese_strings = read_properties(&root.join("values-zh/strings.properties"));
    let colors = read_properties(&root.join("values/colors.properties"));
    let dimensions = read_properties(&root.join("values/dimensions.properties"));
    let images = read_image_files(&root.join("images"));

    validate_localized_keys(
        &default_strings,
        &chinese_strings,
        "values-zh/strings.properties",
    );

    let generated = generate_resources(
        &default_strings,
        &chinese_strings,
        &colors,
        &dimensions,
        &images,
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must define OUT_DIR"))
        .join("resources_generated.rs");
    fs::write(output, generated).expect("generated resources must be writable");
}

/// Discovers supported image files and derives resource keys from their file stems.
fn read_image_files(directory: &Path) -> BTreeMap<String, String> {
    let mut images = BTreeMap::new();
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read an entry in {}: {error}",
                directory.display()
            )
        });
        let path = entry.path();
        if !path.is_file() || !is_supported_image(&path) {
            continue;
        }
        let key = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_else(|| panic!("image filename {} is not valid UTF-8", path.display()));
        validate_image_key(&path, key);
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .expect("validated image filename is UTF-8");
        if images
            .insert(key.to_owned(), format!("images/{filename}"))
            .is_some()
        {
            panic!("{} duplicates image resource key `{key}`", path.display());
        }
    }

    images
}

/// Reports whether a file extension is supported as an embedded image resource.
fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "svg" | "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico"
            )
        })
}

/// Ensures an image file stem can be emitted unchanged as a Rust identifier.
fn validate_image_key(path: &Path, key: &str) {
    if !is_valid_key(key) {
        panic!(
            "image filename {} must have a stem matching [a-z][a-z0-9_]* without repeated or trailing underscores",
            path.display()
        );
    }
}

/// Parses a UTF-8 properties file with strict duplicate-key and identifier validation.
fn read_properties(path: &Path) -> BTreeMap<String, String> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let mut values = BTreeMap::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap_or_else(|| {
            panic!(
                "{}:{} must use key=value properties syntax",
                path.display(),
                index + 1
            )
        });
        let key = key.trim();
        validate_key(path, index + 1, key);
        if values
            .insert(key.to_owned(), value.trim().to_owned())
            .is_some()
        {
            panic!(
                "{}:{} duplicates resource key `{key}`",
                path.display(),
                index + 1
            );
        }
    }
    values
}

/// Ensures a resource key can be emitted unchanged as a lowercase Rust identifier.
fn validate_key(path: &Path, line: usize, key: &str) {
    if !is_valid_key(key) {
        panic!(
            "{}:{line} resource key `{key}` must match [a-z][a-z0-9_]* without repeated or trailing underscores",
            path.display()
        );
    }
}

/// Reports whether a key can be emitted unchanged as a lowercase Rust identifier.
fn is_valid_key(key: &str) -> bool {
    let mut characters = key.chars();
    let valid_start = characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase());
    let valid_rest = characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    });
    valid_start && valid_rest && !key.ends_with('_') && !key.contains("__")
}

/// Rejects localized keys that do not exist in the complete default language file.
fn validate_localized_keys(
    defaults: &BTreeMap<String, String>,
    localized: &BTreeMap<String, String>,
    source: &str,
) {
    for (key, value) in localized {
        let default = defaults
            .get(key)
            .unwrap_or_else(|| panic!("{source} contains unknown default-language key `{key}`"));
        if placeholders(default) != placeholders(value) {
            panic!("{source} key `{key}` must use the same placeholders as the default language");
        }
    }
}

/// Generates static resource descriptors and literal-key macro arms from validated inputs.
fn generate_resources(
    default_strings: &BTreeMap<String, String>,
    chinese_strings: &BTreeMap<String, String>,
    colors: &BTreeMap<String, String>,
    dimensions: &BTreeMap<String, String>,
    images: &BTreeMap<String, String>,
) -> String {
    let mut output =
        String::from("// @generated by build.rs from assets/resources. Do not edit.\n\n");
    generate_text_resources(&mut output, default_strings, chinese_strings);
    generate_color_resources(&mut output, colors);
    generate_dimension_resources(&mut output, dimensions);
    generate_image_resources(&mut output, images);
    output
}

/// Generates localized text descriptors and parameter-aware literal macro arms.
fn generate_text_resources(
    output: &mut String,
    defaults: &BTreeMap<String, String>,
    chinese: &BTreeMap<String, String>,
) {
    output.push_str("#[allow(dead_code, non_upper_case_globals)]\npub(crate) mod generated_text {\n    use super::TextResource;\n");
    for (key, default) in defaults {
        let localized = chinese.get(key).map_or_else(
            || "None".to_owned(),
            |value| format!("Some({})", rust_string(value)),
        );
        output.push_str(&format!(
            "    pub(crate) const {key}: TextResource = TextResource::new({}, {localized});\n",
            rust_string(default)
        ));
    }
    output.push_str("}\n\nmacro_rules! text {\n");
    for (key, value) in defaults {
        let placeholders = placeholders(value);
        if placeholders.is_empty() {
            output.push_str(&format!(
                "    ({key}) => {{ crate::resources::generated_text::{key}.resolve(&[]) }};\n"
            ));
        } else {
            let parameters = placeholders
                .iter()
                .map(|name| format!("${name}:expr"))
                .collect::<Vec<_>>()
                .join(", ");
            let arguments = placeholders
                .iter()
                .map(|name| format!("({}, ::std::format!(\"{{}}\", ${name}))", rust_string(name)))
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "    ({key}, {parameters}) => {{ crate::resources::generated_text::{key}.resolve(&[{arguments}]) }};\n"
            ));
        }
    }
    output.push_str("}\n\n");
}

/// Generates static color descriptors and literal macro arms.
fn generate_color_resources(output: &mut String, colors: &BTreeMap<String, String>) {
    output.push_str("#[allow(dead_code, non_upper_case_globals)]\npub(crate) mod generated_colors {\n    use super::ColorResource;\n");
    for (key, value) in colors {
        let [red, green, blue, alpha] = parse_color(key, value);
        output.push_str(&format!(
            "    pub(crate) const {key}: ColorResource = ColorResource::new([{red:?}, {green:?}, {blue:?}, {alpha:?}]);\n"
        ));
    }
    output.push_str("}\n\nmacro_rules! color {\n");
    for key in colors.keys() {
        output.push_str(&format!(
            "    ({key}) => {{ crate::resources::generated_colors::{key}.resolve() }};\n"
        ));
    }
    output.push_str("}\n\n");
}

/// Generates static unscaled dimensions and literal macro arms.
fn generate_dimension_resources(output: &mut String, dimensions: &BTreeMap<String, String>) {
    output.push_str("#[allow(dead_code, non_upper_case_globals)]\npub(crate) mod generated_dimensions {\n    use super::DimensionResource;\n");
    for (key, value) in dimensions {
        let number: f32 = value
            .parse()
            .unwrap_or_else(|_| panic!("dimension `{key}` contains invalid number `{value}`"));
        if !number.is_finite() || number < 0.0 {
            panic!("dimension `{key}` must be a finite non-negative number");
        }
        output.push_str(&format!(
            "    pub(crate) const {key}: DimensionResource = DimensionResource::new({number:?});\n"
        ));
    }
    output.push_str("}\n\nmacro_rules! dimension {\n");
    for key in dimensions.keys() {
        output.push_str(&format!(
            "    ({key}) => {{ crate::resources::generated_dimensions::{key}.resolve() }};\n"
        ));
    }
    output.push_str("}\n\n");
}

/// Generates static image descriptors for discovered asset files.
fn generate_image_resources(output: &mut String, images: &BTreeMap<String, String>) {
    output.push_str("#[allow(dead_code, non_upper_case_globals)]\npub(crate) mod generated_images {\n    use super::ImageResource;\n");
    for (key, path) in images {
        output.push_str(&format!(
            "    pub(crate) const {key}: ImageResource = ImageResource::new({}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/resources/{path}\")));\n",
            rust_string(path)
        ));
    }
    output.push_str("}\n\nmacro_rules! image {\n");
    for key in images.keys() {
        output.push_str(&format!(
            "    ({key}) => {{ crate::resources::generated_images::{key} }};\n"
        ));
    }
    output.push_str("}\n");
}

/// Parses a supported hexadecimal or rgba functional color into normalized channels.
fn parse_color(key: &str, value: &str) -> [f32; 4] {
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 && hex.len() != 8 {
            panic!("color `{key}` must use #RRGGBB or #RRGGBBAA");
        }
        let channel = |range| {
            u8::from_str_radix(&hex[range], 16)
                .unwrap_or_else(|_| panic!("color `{key}` contains invalid hex digits"))
                as f32
                / 255.0
        };
        return [
            channel(0..2),
            channel(2..4),
            channel(4..6),
            if hex.len() == 8 { channel(6..8) } else { 1.0 },
        ];
    }
    if let Some(arguments) = value
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let parts = arguments.split(',').map(str::trim).collect::<Vec<_>>();
        if parts.len() != 4 {
            panic!("color `{key}` rgba value must contain four channels");
        }
        let byte = |index: usize| {
            parts[index]
                .parse::<u8>()
                .unwrap_or_else(|_| panic!("color `{key}` has invalid byte channel"))
                as f32
                / 255.0
        };
        let alpha = parts[3]
            .parse::<f32>()
            .unwrap_or_else(|_| panic!("color `{key}` has invalid alpha channel"));
        if !(0.0..=1.0).contains(&alpha) {
            panic!("color `{key}` alpha must be between zero and one");
        }
        return [byte(0), byte(1), byte(2), alpha];
    }
    panic!("color `{key}` must use #RRGGBB, #RRGGBBAA, or rgba(r,g,b,a)");
}

/// Extracts unique named placeholders while preserving their first appearance order.
fn placeholders(template: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        if rest[..start].contains('}') {
            panic!("text template `{template}` has an unmatched closing brace");
        }
        let after_start = &rest[start + 1..];
        let end = after_start
            .find('}')
            .unwrap_or_else(|| panic!("text template `{template}` has an unclosed placeholder"));
        let name = &after_start[..end];
        if name.is_empty()
            || !name
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '_')
        {
            panic!("text template `{template}` contains invalid placeholder `{{{name}}}`");
        }
        if seen.insert(name.to_owned()) {
            names.push(name.to_owned());
        }
        rest = &after_start[end + 1..];
    }
    if rest.contains('}') {
        panic!("text template `{template}` has an unmatched closing brace");
    }
    names
}

/// Escapes arbitrary properties text as a Rust string literal.
fn rust_string(value: &str) -> String {
    format!("{value:?}")
}
