use std::sync::OnceLock;

use quick_xml::{
    escape::resolve_xml_entity,
    events::{BytesRef, Event},
    reader::Reader,
};
use regex::Regex;
use url::Url;

use super::{FeedError, WallpaperEntry};

#[derive(Debug, Default)]
struct RssItem {
    title: String,
    published: String,
    content: String,
}

#[derive(Debug, Clone, Copy)]
enum ItemField {
    Title,
    Published,
    Content,
}

/// Extracts valid Spotlight wallpaper entries from RSS in source order.
pub(super) fn parse(rss: &str) -> Result<Vec<WallpaperEntry>, FeedError> {
    let mut reader = Reader::from_str(rss);
    let mut item = None;
    let mut field = None;
    let mut entries = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match element.name().as_ref() {
                b"item" => item = Some(RssItem::default()),
                b"title" if item.is_some() => field = Some(ItemField::Title),
                b"pubDate" if item.is_some() => field = Some(ItemField::Published),
                b"content:encoded" if item.is_some() => field = Some(ItemField::Content),
                _ => {}
            },
            Ok(Event::Text(text)) => {
                if let (Some(item), Some(field), Ok(text)) = (item.as_mut(), field, text.decode()) {
                    field_value(item, field).push_str(&text);
                }
            }
            Ok(Event::CData(cdata)) => {
                if let (Some(item), Some(field), Ok(text)) = (item.as_mut(), field, cdata.decode())
                {
                    field_value(item, field).push_str(&text);
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let (Some(item), Some(field)) = (item.as_mut(), field) {
                    append_reference(field_value(item, field), &reference);
                }
            }
            Ok(Event::End(element)) => match element.name().as_ref() {
                b"item" => {
                    field = None;
                    if let Some(entry) = item.take().and_then(build_entry) {
                        entries.push(entry);
                    }
                }
                b"title" | b"pubDate" | b"content:encoded" => field = None,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    if entries.is_empty() {
        Err(FeedError::NoEntries)
    } else {
        Ok(entries)
    }
}

/// Selects the mutable RSS item field currently receiving decoded content.
fn field_value(item: &mut RssItem, field: ItemField) -> &mut String {
    match field {
        ItemField::Title => &mut item.title,
        ItemField::Published => &mut item.published,
        ItemField::Content => &mut item.content,
    }
}

/// Resolves a safe XML character or built-in entity into a field value.
fn append_reference(target: &mut String, reference: &BytesRef<'_>) {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        target.push(character);
        return;
    }
    if let Ok(name) = reference.decode()
        && let Some(value) = resolve_xml_entity(&name)
    {
        target.push_str(value);
    }
}

/// Converts a complete RSS item into a Wallpaper Entry when all fields are valid.
fn build_entry(item: RssItem) -> Option<WallpaperEntry> {
    let description = item.title.trim();
    if description.is_empty() {
        return None;
    }
    Some(WallpaperEntry {
        date: publication_date(&item.published)?,
        description: description.to_owned(),
        image_url: landscape_original(&item.content)?,
    })
}

/// Converts an RFC 2822-style publication timestamp into the UI date format.
fn publication_date(published: &str) -> Option<String> {
    let parts = published.split_ascii_whitespace().collect::<Vec<_>>();
    let month_index = parts.iter().position(|part| month_number(part).is_some())?;
    let day = parts.get(month_index.checked_sub(1)?)?.parse::<u8>().ok()?;
    let year = parts.get(month_index + 1)?.parse::<u16>().ok()?;
    if !(1..=31).contains(&day) || !(1000..=9999).contains(&year) {
        return None;
    }
    Some(format!(
        "{year:04}-{:02}-{day:02}",
        month_number(parts[month_index])?
    ))
}

/// Maps an English RFC 2822 month abbreviation to its calendar number.
fn month_number(month: &str) -> Option<u8> {
    Some(match month {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    })
}

/// Finds the first non-resized HTTPS image link whose embedded image is landscape.
fn landscape_original(html: &str) -> Option<String> {
    static ANCHOR: OnceLock<Regex> = OnceLock::new();
    static WIDTH: OnceLock<Regex> = OnceLock::new();
    static HEIGHT: OnceLock<Regex> = OnceLock::new();
    static RESIZED: OnceLock<Regex> = OnceLock::new();
    let anchor = ANCHOR.get_or_init(|| {
        Regex::new(r#"(?is)<a\b[^>]*\bhref\s*=\s*["'](https://[^"']+)["'][^>]*>(.*?)</a>"#)
            .expect("the Spotlight anchor regex is valid")
    });
    let width = WIDTH.get_or_init(|| {
        Regex::new(r#"(?i)\bwidth\s*=\s*["']?(\d+)"#).expect("the image width regex is valid")
    });
    let height = HEIGHT.get_or_init(|| {
        Regex::new(r#"(?i)\bheight\s*=\s*["']?(\d+)"#).expect("the image height regex is valid")
    });
    let resized = RESIZED.get_or_init(|| {
        Regex::new(r"(?i)-\d+x\d+\.jpe?g$").expect("the resized image regex is valid")
    });

    anchor.captures_iter(html).find_map(|capture| {
        let image_url = capture.get(1)?.as_str();
        let image_markup = capture.get(2)?.as_str();
        let parsed = Url::parse(image_url).ok()?;
        if parsed.scheme() != "https"
            || parsed.host_str() != Some("windows10spotlight.com")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
            || !matches!(
                parsed.path().rsplit_once('.'),
                Some((_, extension)) if extension.eq_ignore_ascii_case("jpg")
                    || extension.eq_ignore_ascii_case("jpeg")
            )
            || resized.is_match(parsed.path())
        {
            return None;
        }
        let width = width
            .captures(image_markup)?
            .get(1)?
            .as_str()
            .parse::<u32>()
            .ok()?;
        let height = height
            .captures(image_markup)?
            .get(1)?
            .as_str()
            .parse::<u32>()
            .ok()?;
        (width > height).then(|| image_url.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::FeedError;

    /// Wraps Spotlight RSS items in the minimum valid channel structure.
    fn rss(items: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <rss xmlns:content="http://purl.org/rss/1.0/modules/content/" version="2.0">
              <channel>{items}</channel>
            </rss>"#
        )
    }

    /// Creates one Spotlight item with supplied metadata and embedded HTML.
    fn item(title: &str, published: &str, html: &str) -> String {
        format!(
            r#"<item>
              <title>{title}</title>
              <pubDate>{published}</pubDate>
              <content:encoded><![CDATA[{html}]]></content:encoded>
            </item>"#
        )
    }

    #[test]
    /// Verifies RSS order, XML entities, dates, and landscape originals are preserved.
    fn parses_ordered_landscape_originals() {
        let first = item(
            "Rock &amp; Sea",
            "Sat, 01 Aug 2026 12:00:00 +0000",
            r#"<p><a href="https://windows10spotlight.com/wp-content/uploads/2026/08/rock.jpg"><img src="https://windows10spotlight.com/wp-content/uploads/2026/08/rock-1024x576.jpg" width="728" height="410" /></a><a href="https://windows10spotlight.com/wp-content/uploads/2026/08/rock-portrait.jpg"><img width="410" height="728" /></a></p>"#,
        );
        let second = item(
            "Forest",
            "Fri, 31 Jul 2026 08:30:00 +0000",
            r#"<a href="https://windows10spotlight.com/wp-content/uploads/2026/07/forest.jpg"><img width="1920" height="1080" /></a>"#,
        );

        let entries = parse(&rss(&format!("{first}{second}"))).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].description, "Rock & Sea");
        assert_eq!(entries[0].date, "2026-08-01");
        assert_eq!(
            entries[0].image_url,
            "https://windows10spotlight.com/wp-content/uploads/2026/08/rock.jpg"
        );
        assert_eq!(entries[1].description, "Forest");
        assert_eq!(entries[1].date, "2026-07-31");
    }

    #[test]
    /// Verifies malformed items and unsafe image candidates are skipped.
    fn skips_malformed_portrait_thumbnail_and_insecure_items() {
        let missing_title = item(
            "",
            "Sat, 01 Aug 2026 12:00:00 +0000",
            r#"<a href="https://windows10spotlight.com/missing-title.jpg"><img width="1920" height="1080" /></a>"#,
        );
        let portrait = item(
            "Portrait",
            "Sat, 01 Aug 2026 12:00:00 +0000",
            r#"<a href="https://windows10spotlight.com/portrait.jpg"><img width="1080" height="1920" /></a>"#,
        );
        let thumbnail = item(
            "Thumbnail",
            "Sat, 01 Aug 2026 12:00:00 +0000",
            r#"<a href="https://windows10spotlight.com/image-1024x576.jpg"><img width="1920" height="1080" /></a>"#,
        );
        let insecure = item(
            "Insecure",
            "Sat, 01 Aug 2026 12:00:00 +0000",
            r#"<a href="http://windows10spotlight.com/image.jpg"><img width="1920" height="1080" /></a>"#,
        );
        let external = item(
            "External",
            "Sat, 01 Aug 2026 12:00:00 +0000",
            r#"<a href="https://example.com/image.jpg"><img width="1920" height="1080" /></a>"#,
        );
        let valid = item(
            "Valid",
            "Sat, 01 Aug 2026 12:00:00 +0000",
            r#"<script>ignored()</script><a href="https://windows10spotlight.com/valid.jpg"><img width="1920" height="1080" /></a>"#,
        );

        let entries = parse(&rss(&format!(
            "{missing_title}{portrait}{thumbnail}{insecure}{external}{valid}"
        )))
        .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].description, "Valid");
        assert_eq!(
            entries[0].image_url,
            "https://windows10spotlight.com/valid.jpg"
        );
    }

    #[test]
    /// Verifies an invalid document or a Feed without valid items is rejected.
    fn rejects_invalid_and_empty_feeds() {
        assert_eq!(parse("<rss><broken>"), Err(FeedError::NoEntries));
        assert_eq!(parse(&rss("")), Err(FeedError::NoEntries));
    }
}
