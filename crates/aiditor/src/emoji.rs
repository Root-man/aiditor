//! Emoji when the colour emoji font can't be drawn (#237).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;

use gpui::{App, font};

/// The emoji font cosmic-text falls back to on Linux and BSD.
const COLOUR: &str = "Noto Color Emoji";

/// Drops the colour emoji font when GPUI can't draw it, so emoji fall back to a font that has
/// them, such as Noto Emoji or Symbola. GPUI's Linux rasterizer reads colour bitmaps and COLR v0
/// layers but not COLRv1, Fedora's only colour emoji.
pub(crate) fn drop_undrawable(cx: &App) {
    // Not a fontconfig system, as macOS: its colour emoji are drawn.
    let Ok(out) = Command::new("fc-match")
        .arg("--format=%{file}")
        .arg(COLOUR)
        .output()
    else {
        return;
    };
    let Ok(mut file) = File::open(String::from_utf8_lossy(&out.stdout).as_ref()) else {
        return;
    };
    if !drawable(&mut file).unwrap_or(true) {
        // shortcut: GPUI 0.2.2 removes a face it loads by name if it has no `m`, which takes it out of fallback; recheck on a GPUI upgrade.
        cx.text_system().resolve_font(&font(COLOUR));
        tracing::info!("{COLOUR} is COLRv1, which GPUI can't draw; emoji use a monochrome font");
    }
}

/// Whether a font file's colour glyphs, if it has any, are ones GPUI draws.
fn drawable(file: &mut (impl Read + Seek)) -> std::io::Result<bool> {
    let mut head = [0; 12];
    file.read_exact(&mut head)?;
    // shortcut: a collection's fonts aren't read, so it counts as drawable; read them if a colour emoji font ships as one.
    if &head[..4] == b"ttcf" {
        return Ok(true);
    }
    let mut tables = vec![0; 16 * u16::from_be_bytes([head[4], head[5]]) as usize];
    file.read_exact(&mut tables)?;
    let mut colr = None;
    for table in tables.chunks(16) {
        match &table[..4] {
            b"CBDT" | b"sbix" => return Ok(true),
            b"COLR" => {
                colr = Some(u32::from_be_bytes([
                    table[8], table[9], table[10], table[11],
                ]))
            }
            _ => {}
        }
    }
    let Some(offset) = colr else {
        return Ok(true);
    };
    file.seek(SeekFrom::Start(offset.into()))?;
    let mut header = [0; 4];
    file.read_exact(&mut header)?;
    // After the version comes the count of v0 base glyphs, the only ones GPUI draws.
    Ok(u16::from_be_bytes([header[2], header[3]]) > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A font file with `tables`, each tag pointing at its bytes.
    fn sfnt(tables: &[(&[u8; 4], &[u8])]) -> Cursor<Vec<u8>> {
        let mut file = vec![0, 1, 0, 0, 0, tables.len() as u8, 0, 0, 0, 0, 0, 0];
        let mut offset = 12 + 16 * tables.len();
        for (tag, data) in tables {
            file.extend_from_slice(*tag);
            file.extend_from_slice(&[0; 4]);
            file.extend_from_slice(&(offset as u32).to_be_bytes());
            file.extend_from_slice(&(data.len() as u32).to_be_bytes());
            offset += data.len();
        }
        tables
            .iter()
            .for_each(|(_, data)| file.extend_from_slice(data));
        Cursor::new(file)
    }

    #[test]
    fn only_colrv1_without_bitmaps_is_undrawable() {
        let v1 = [0, 1, 0, 0];
        let v0 = [0, 0, 0, 3];
        assert!(!drawable(&mut sfnt(&[(b"glyf", &[]), (b"COLR", &v1)])).unwrap());
        assert!(drawable(&mut sfnt(&[(b"COLR", &v0)])).unwrap());
        assert!(drawable(&mut sfnt(&[(b"COLR", &v1), (b"CBDT", &[])])).unwrap());
        assert!(drawable(&mut sfnt(&[(b"sbix", &[])])).unwrap());
        assert!(drawable(&mut sfnt(&[(b"glyf", &[])])).unwrap());
        assert!(drawable(&mut Cursor::new(b"ttcf".repeat(4))).unwrap());
        assert!(drawable(&mut Cursor::new(vec![0, 1])).is_err());
    }

    #[test]
    fn fedoras_colour_emoji_is_undrawable() {
        let path = "/usr/share/fonts/google-noto-color-emoji-fonts/Noto-COLRv1.ttf";
        if let Ok(mut file) = File::open(path) {
            assert!(!drawable(&mut file).unwrap());
        }
    }
}
