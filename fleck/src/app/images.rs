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

/// The Markdown line Fleck writes for a pasted image, with the blank line
/// that keeps it on its own (see `fleck_core::image_links`).
pub(super) fn link_line(name: &str) -> String {
    format!("\n![]({name})\n")
}

#[cfg(test)]
mod tests {
    use super::*;

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
