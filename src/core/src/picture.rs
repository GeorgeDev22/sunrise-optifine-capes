//! Which player names a cape server may be asked for, which of its answers are capes, and the
//! cape texture a cape's picture sits on.

/// The longest answer taken, in bytes: the Sunrise launcher's bound for the same pictures.
pub const MAX_BYTES: usize = 256 * 1024;

/// The largest picture taken. The biggest HD capes are 2048x1024; a larger header is refused
/// before the picture is decoded, so a small file can't claim a huge one.
const MAX_WIDTH: u32 = 2048;
const MAX_HEIGHT: u32 = 1024;

/// The widest texture a cape is uploaded as: 128x64 keeps an HD cape's detail, and sixteen of
/// them (a mod's most dynamic textures) stay well inside a mod's pixel budget.
pub const MAX_TEXTURE_WIDTH: u32 = 128;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// Whether a cape server may be asked for `name`'s cape: a Minecraft name, 1 to 16 of `A-Z`,
/// `a-z`, `0-9` and `_`. Nothing else ever goes into the address.
pub fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Whether a cape server's answer can be a cape: a PNG of at most [`MAX_BYTES`] whose header
/// names a picture of at most 2048x1024. Anything else counts as no cape.
pub fn accepts(body: &[u8]) -> bool {
    if body.len() > MAX_BYTES || !body.starts_with(&PNG_SIGNATURE) {
        return false;
    }
    // The first chunk is IHDR: its length, its type, then the width and height, big-endian.
    let Some(
        &[
            _,
            _,
            _,
            _,
            b'I',
            b'H',
            b'D',
            b'R',
            w0,
            w1,
            w2,
            w3,
            h0,
            h1,
            h2,
            h3,
        ],
    ) = body.get(8..24)
    else {
        return false;
    };
    let (width, height) = (
        u32::from_be_bytes([w0, w1, w2, w3]),
        u32::from_be_bytes([h0, h1, h2, h3]),
    );
    (1..=MAX_WIDTH).contains(&width) && (1..=MAX_HEIGHT).contains(&height)
}

/// The cape texture a `width`x`height` picture is laid on, its top left corner on the texture's:
/// the smallest 64·2^n by 32·2^n that holds it. A 46x22 or 64x32 picture gives 64x32, a 92x44 or
/// 128x64 one 128x64, so an HD cape keeps the layout's coordinates at twice the scale.
pub fn canvas(width: u32, height: u32) -> (u32, u32) {
    let (mut w, mut h) = (64u32, 32u32);
    while (w < width || h < height) && w <= u32::MAX / 2 {
        w *= 2;
        h *= 2;
    }
    (w, h)
}

/// Whether a `width`x`height` picture also dresses an elytra: it reaches into the elytra's part
/// of the cape layout, which starts 22 texels in on the 64-wide texture (and scales with it). An
/// old 22x17 cape stops short of it, so the elytra keeps its own texture instead of a blank one.
pub fn dresses_elytra(width: u32, height: u32) -> bool {
    let (canvas_width, _) = canvas(width, height);
    u64::from(width) * 64 > 22 * u64::from(canvas_width)
}

/// A cape as the mod uploads it: its picture on its cape texture, at most
/// [`MAX_TEXTURE_WIDTH`] wide (a whole-number upscale of the 64x32 layout the game's cape and
/// wings sample), and whether it dresses an elytra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cape {
    pub width: u32,
    pub height: u32,
    /// `width * height` pixels of four bytes, row by row from the top left.
    pub rgba: Vec<u8>,
    pub elytra: bool,
}

impl Cape {
    /// The `width`x`height` RGBA picture on its cape texture ([`canvas`]), that texture scaled
    /// down by a whole number (nearest texel) when it is wider than [`MAX_TEXTURE_WIDTH`]. Where
    /// the picture doesn't reach stays clear. `None` for no pixels or too few of them.
    pub fn lay_out(width: u32, height: u32, rgba: &[u8]) -> Option<Cape> {
        let (w, h) = (width as usize, height as usize);
        if w == 0 || h == 0 || rgba.len() < w * h * 4 {
            return None;
        }
        let (canvas_width, _) = canvas(width, height);
        let out_width = canvas_width.min(MAX_TEXTURE_WIDTH);
        let step = (canvas_width / out_width) as usize;
        let (ow, oh) = (out_width as usize, out_width as usize / 2);
        let mut out = vec![0u8; ow * oh * 4];
        for y in 0..oh {
            for x in 0..ow {
                let (sx, sy) = (x * step, y * step);
                if sx < w && sy < h {
                    let (s, d) = ((sy * w + sx) * 4, (y * ow + x) * 4);
                    out[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
                }
            }
        }
        Some(Cape {
            width: out_width,
            height: out_width / 2,
            rgba: out,
            elytra: dresses_elytra(width, height),
        })
    }

    /// A cape server's answer as a cape: one it [`accepts`] that decodes as a PNG. Anything else
    /// is no cape.
    pub fn from_png(body: &[u8]) -> Option<Cape> {
        if !accepts(body) {
            return None;
        }
        let mut decoder = png::Decoder::new(std::io::Cursor::new(body));
        // Palette, grey and 16-bit pictures come out as 8-bit RGB(A).
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().ok()?;
        let mut pixels = vec![0; reader.output_buffer_size()?];
        let frame = reader.next_frame(&mut pixels).ok()?;
        let rgba: Vec<u8> = match frame.color_type {
            png::ColorType::Rgba => pixels[..frame.buffer_size()].to_vec(),
            png::ColorType::Rgb => pixels[..frame.buffer_size()]
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => pixels[..frame.buffer_size()]
                .chunks_exact(2)
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
            png::ColorType::Grayscale => pixels[..frame.buffer_size()]
                .iter()
                .flat_map(|&g| [g, g, g, 255])
                .collect(),
            png::ColorType::Indexed => return None,
        };
        Cape::lay_out(frame.width, frame.height, &rgba)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_minecraft_names_are_asked_for() {
        for name in ["a", "Notch", "jeb_", "Player_1234", "ABCDEFGHIJKLMNOP"] {
            assert!(valid_name(name), "{name}");
        }
        for name in [
            "",
            "ABCDEFGHIJKLMNOPQ",
            "two words",
            "../capes",
            "name.png",
            "a?b=c",
            "100%",
            "Jörg",
            "名前",
            "tab\t",
            "nul\0",
        ] {
            assert!(!valid_name(name), "{name:?}");
        }
    }

    /// A PNG's first bytes: the signature and an IHDR naming `width`x`height`.
    fn png_head(width: u32, height: u32) -> Vec<u8> {
        let mut out = PNG_SIGNATURE.to_vec();
        out.extend(13u32.to_be_bytes());
        out.extend(b"IHDR");
        out.extend(width.to_be_bytes());
        out.extend(height.to_be_bytes());
        out.extend([8, 6, 0, 0, 0]);
        out
    }

    #[test]
    fn only_small_pngs_of_a_cape_size_count() {
        assert!(accepts(&png_head(46, 22)));
        assert!(accepts(&png_head(2048, 1024)));
        let mut largest = png_head(92, 44);
        largest.resize(MAX_BYTES, 0);
        assert!(accepts(&largest));
        largest.push(0);
        assert!(!accepts(&largest), "one byte over the bound");
        assert!(!accepts(&png_head(2049, 1024)));
        assert!(!accepts(&png_head(64, 1025)));
        assert!(!accepts(&png_head(0, 32)));
        assert!(!accepts(&png_head(u32::MAX, u32::MAX)));
        assert!(!accepts(b"<html>404 Not Found</html>"));
        assert!(!accepts(&PNG_SIGNATURE), "a signature with no header");
        let mut other_chunk = png_head(46, 22);
        other_chunk[12..16].copy_from_slice(b"tEXt");
        assert!(!accepts(&other_chunk));
        assert!(!accepts(&[]));
    }

    #[test]
    fn pictures_sit_on_the_smallest_cape_texture_that_holds_them() {
        assert_eq!(canvas(64, 32), (64, 32));
        assert_eq!(canvas(46, 22), (64, 32));
        assert_eq!(canvas(22, 17), (64, 32));
        assert_eq!(canvas(1, 1), (64, 32));
        assert_eq!(canvas(92, 44), (128, 64));
        assert_eq!(canvas(128, 64), (128, 64));
        assert_eq!(canvas(65, 10), (128, 64));
        assert_eq!(canvas(10, 33), (128, 64));
        assert_eq!(canvas(184, 88), (256, 128));
        assert_eq!(canvas(2048, 1024), (2048, 1024));
        // No size loops for ever or overflows.
        assert_eq!(canvas(u32::MAX, u32::MAX), (1 << 31, 1 << 30));
    }

    #[test]
    fn a_picture_that_reaches_the_elytras_part_dresses_it() {
        for (w, h) in [(46, 22), (64, 32), (92, 44), (128, 64), (184, 88)] {
            assert!(dresses_elytra(w, h), "{w}x{h}");
        }
        for (w, h) in [(22, 17), (10, 10), (44, 34)] {
            assert!(!dresses_elytra(w, h), "{w}x{h}");
        }
    }

    /// A `width`x`height` picture whose texel at (x, y) is `[x, y, 7, 255]`.
    fn picture(width: usize, height: usize) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                rgba.extend([x as u8, y as u8, 7, 255]);
            }
        }
        rgba
    }

    /// The picture keeps the cape layout's coordinates: texel for texel on its own texture, every
    /// other texel when that texture is scaled down to 128 wide. What it doesn't cover is clear.
    #[test]
    fn capes_keep_the_layout_of_their_texture() {
        // (picture, texture it is uploaded as, picture texels a texture texel steps)
        for (w, h, out, step) in [
            (64, 32, 64, 1),
            (46, 22, 64, 1),
            (22, 17, 64, 1),
            (92, 44, 128, 1),
            (128, 64, 128, 1),
            (184, 88, 128, 2),
        ] {
            let cape = Cape::lay_out(w as u32, h as u32, &picture(w, h)).unwrap();
            assert_eq!(
                (cape.width, cape.height),
                (out as u32, out as u32 / 2),
                "{w}x{h}"
            );
            assert_eq!(cape.rgba.len(), out * out / 2 * 4);
            for y in 0..out / 2 {
                for x in 0..out {
                    let (sx, sy) = (x * step, y * step);
                    let want = if sx < w && sy < h {
                        [sx as u8, sy as u8, 7, 255]
                    } else {
                        [0; 4]
                    };
                    let at = (y * out + x) * 4;
                    assert_eq!(cape.rgba[at..at + 4], want, "{w}x{h} at ({x}, {y})");
                }
            }
        }
        assert!(Cape::lay_out(46, 22, &[0; 4]).is_none());
    }

    fn encode(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        out
    }

    #[test]
    fn a_png_answer_becomes_a_cape_and_anything_else_none() {
        let rgba = picture(46, 22);
        let cape = Cape::from_png(&encode(46, 22, png::ColorType::Rgba, &rgba)).unwrap();
        assert_eq!((cape.width, cape.height, cape.elytra), (64, 32, true));
        assert_eq!(cape.rgba[(21 * 64 + 45) * 4..][..4], [45, 21, 7, 255]);
        // An RGB picture is opaque.
        let rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let opaque = Cape::from_png(&encode(46, 22, png::ColorType::Rgb, &rgb)).unwrap();
        assert_eq!(opaque.rgba[..4], [0, 0, 7, 255]);
        let old = Cape::from_png(&encode(22, 17, png::ColorType::Rgba, &picture(22, 17))).unwrap();
        assert!(!old.elytra);
        assert_eq!(Cape::from_png(b"<html>Not Found</html>"), None);
        let mut cut = encode(46, 22, png::ColorType::Rgba, &rgba);
        cut.truncate(cut.len() / 2);
        assert_eq!(Cape::from_png(&cut), None, "a half picture");
    }
}
