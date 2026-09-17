//! Clipboard images. COSMIC's own clipboard only carries text, so pasting a
//! picture in and copying one back out go through `arboard`, which talks to
//! the Wayland clipboard directly. Both calls block, so callers run them off
//! the UI thread (see `Message::NotePasteImage`).

use std::borrow::Cow;
use std::path::Path;

/// The format pasted images are saved in. Clipboard images arrive as raw
/// pixels, so Fleck picks the encoding; PNG keeps screenshots and diagrams
/// sharp and is read by everything.
pub(super) const PASTED_FORMAT: &str = "png";

/// The image on the clipboard, encoded as PNG. `None` when the clipboard
/// holds no image (text, or nothing at all) or can't be read.
pub(super) fn from_clipboard() -> Option<Vec<u8>> {
    let image = arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_image())
        .map_err(|error| tracing::debug!(?error, "no image on the clipboard"))
        .ok()?;

    let width = u32::try_from(image.width).ok()?;
    let height = u32::try_from(image.height).ok()?;
    let pixels = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())?;

    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| tracing::warn!(?error, "failed to encode the pasted image"))
        .ok()?;
    Some(png)
}

/// Puts the image file at `path` on the clipboard, ready to paste elsewhere.
pub(super) fn to_clipboard(path: &Path) -> Result<(), String> {
    let pixels = image::open(path)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let (width, height) = pixels.dimensions();
    let image = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(pixels.into_raw()),
    };
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_image(image))
        .map_err(|error| error.to_string())
}

/// Image files Fleck accepts when they are dropped on a note: what both
/// `widget::image` and the copy-to-clipboard path can read.
const DROPPABLE: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// The mime type a file manager offers when files are dragged: a list of
/// `file://` URIs, one per line.
pub(super) const URI_LIST: &str = "text/uri-list";

/// The file paths in a `text/uri-list` drop. Anything that isn't a local
/// file - a URL dragged from a browser, say - is skipped.
pub(super) fn paths_from_uri_list(data: &[u8]) -> Vec<std::path::PathBuf> {
    String::from_utf8_lossy(data)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.strip_prefix("file://"))
        .map(|path| std::path::PathBuf::from(percent_decode(path)))
        .collect()
}

/// `%20`-style escapes turned back into bytes; anything else is kept as is.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let decoded = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| u8::from_str_radix(&text[i + 1..i + 3], 16).ok())
            .flatten();
        if let Some(byte) = decoded {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A dropped file's contents and extension, or `None` when it isn't an image
/// Fleck can show.
pub(super) fn from_file(path: &Path) -> Option<(String, Vec<u8>)> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    if !DROPPABLE.contains(&extension.as_str()) {
        return None;
    }
    let bytes = std::fs::read(path)
        .map_err(|error| tracing::warn!(?error, "failed to read the dropped image"))
        .ok()?;
    Some((extension, bytes))
}

/// The Markdown line Fleck writes for a pasted image, with the blank line
/// that keeps it on its own (see `fleck_core::image_links`).
pub(super) fn link_line(name: &str) -> String {
    format!("\n![]({name})\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_list_drops_yield_local_file_paths() {
        let data = b"file:///home/joel/My%20Pictures/shot.png\r\n\
                     https://example.com/remote.png\r\n\
                     file:///tmp/a.jpg\r\n";
        assert_eq!(
            paths_from_uri_list(data),
            vec![
                std::path::PathBuf::from("/home/joel/My Pictures/shot.png"),
                std::path::PathBuf::from("/tmp/a.jpg"),
            ]
        );
    }

    #[test]
    fn only_readable_image_files_are_accepted_from_a_drop() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.PNG");
        std::fs::write(&png, b"bytes").unwrap();
        assert_eq!(
            from_file(&png),
            Some(("png".to_string(), b"bytes".to_vec())),
            "the extension's case must not matter"
        );

        let notes = dir.path().join("notes.txt");
        std::fs::write(&notes, b"text").unwrap();
        assert_eq!(from_file(&notes), None, "a text file is not an image");
        assert_eq!(
            from_file(&dir.path().join("gone.png")),
            None,
            "a missing file"
        );
    }

    #[test]
    fn link_line_is_a_markdown_image_on_its_own_line() {
        let line = link_line("picture.png");
        assert_eq!(line, "\n![](picture.png)\n");
        assert_eq!(
            fleck_core::image_links(&line),
            vec![(1, "picture.png".to_string())],
            "what Fleck writes must be what it reads back"
        );
    }
}
