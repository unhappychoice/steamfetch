use base64::{engine::general_purpose::STANDARD, Engine};
use image::DynamicImage;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::PathBuf;

use crate::ImageProtocol;

const KITTY_CHUNK_SIZE: usize = 4096;

pub async fn load_cached_or_download(url: &str, cache_key: &str) -> Option<DynamicImage> {
    let safe_key = sanitize_cache_key(cache_key);
    if let Some(img) = load_from_cache(&safe_key) {
        return Some(img);
    }
    let img = download_image(url).await?;
    save_to_cache(&safe_key, &img);
    Some(img)
}

fn sanitize_cache_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Print image and rewind cursor to the top of the image (like fastfetch).
/// Returns the number of terminal rows the image occupies.
pub fn print_image_and_rewind(
    img: &DynamicImage,
    protocol: &ImageProtocol,
    cols: u32,
    rows: u32,
) -> Option<u32> {
    let resolved = resolve_protocol(protocol);

    if matches!(resolved, ResolvedProtocol::Block) {
        let term_rows = print_block(img, cols).ok()?;
        print!("\x1b[1G\x1b[{}A", term_rows);
        return Some(term_rows);
    }

    // For graphic protocols, we need accurate cell pixel size
    let (cell_w, cell_h) = query_cell_size();
    let pixel_w = cols * cell_w;
    let pixel_h = rows * cell_h;

    let resized = img.resize_exact(pixel_w, pixel_h, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let raw = rgba.into_raw();

    let result = match resolved {
        ResolvedProtocol::Kitty => print_kitty(&raw, w, h),
        ResolvedProtocol::Iterm => print_iterm(&resized, w, h),
        ResolvedProtocol::Sixel => print_sixel(&raw, w as usize, h as usize),
        ResolvedProtocol::Block => unreachable!(),
    };
    result.ok()?;

    // Rewind cursor to top-left of image area (same as fastfetch)
    let image_rows = rows;
    print!("\x1b[1G\x1b[{}A", image_rows);
    io::stdout().flush().ok();
    Some(image_rows)
}

/// Move cursor right by `cols` columns.
pub fn cursor_right(cols: u32) {
    if cols > 0 {
        print!("\x1b[{}C", cols);
    }
}

// --- Terminal size detection ---

/// Returns (cell_width, cell_height) in pixels.
/// Tries ioctl first, then ESC[14t query, then defaults.
fn query_cell_size() -> (u32, u32) {
    #[cfg(unix)]
    {
        if let Some(size) = query_cell_size_ioctl() {
            return size;
        }
        if let Some(size) = query_cell_size_escape() {
            return size;
        }
    }
    (10, 20) // conservative default
}

#[cfg(unix)]
fn query_cell_size_ioctl() -> Option<(u32, u32)> {
    unsafe {
        let mut ws = std::mem::MaybeUninit::<libc::winsize>::zeroed().assume_init();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0
            && ws.ws_xpixel > 0
            && ws.ws_ypixel > 0
            && ws.ws_col > 0
            && ws.ws_row > 0
        {
            return Some((
                ws.ws_xpixel as u32 / ws.ws_col as u32,
                ws.ws_ypixel as u32 / ws.ws_row as u32,
            ));
        }
    }
    None
}

/// Query terminal pixel size via ESC[14t → ESC[4;{height};{width}t
/// Then get rows/cols via ioctl to compute cell size.
#[cfg(unix)]
fn query_cell_size_escape() -> Option<(u32, u32)> {
    use std::os::unix::io::AsRawFd;

    let tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()?;
    let fd = tty.as_raw_fd();

    // Get rows/cols from ioctl (these usually work even when pixel size doesn't)
    let (rows, cols) = unsafe {
        let mut ws = std::mem::MaybeUninit::<libc::winsize>::zeroed().assume_init();
        if libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) != 0 || ws.ws_row == 0 || ws.ws_col == 0 {
            return None;
        }
        (ws.ws_row as u32, ws.ws_col as u32)
    };

    // Save terminal state and switch to raw mode for reading response
    let mut orig_termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    unsafe {
        if libc::tcgetattr(fd, orig_termios.as_mut_ptr()) != 0 {
            return None;
        }
    }
    let orig_termios = unsafe { orig_termios.assume_init() };
    let mut raw = orig_termios;
    raw.c_lflag &= !(libc::ICANON | libc::ECHO);
    raw.c_cc[libc::VMIN] = 0;
    raw.c_cc[libc::VTIME] = 1; // 100ms timeout
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &raw);
    }

    // Send ESC[14t to query pixel size
    let query = b"\x1b[14t";
    let written = unsafe { libc::write(fd, query.as_ptr() as *const _, query.len()) };
    if written < 0 {
        unsafe { libc::tcsetattr(fd, libc::TCSANOW, &orig_termios) };
        return None;
    }

    // Read response: ESC[4;{height};{width}t
    let mut buf = [0u8; 64];
    let mut pos = 0;
    loop {
        let mut byte = [0u8; 1];
        let n = unsafe { libc::read(fd, byte.as_mut_ptr() as *mut _, 1) };
        if n <= 0 || pos >= buf.len() {
            break;
        }
        buf[pos] = byte[0];
        pos += 1;
        if byte[0] == b't' {
            break;
        }
    }

    // Restore terminal state
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, &orig_termios);
    }

    // Parse ESC[4;{height};{width}t
    let response = std::str::from_utf8(&buf[..pos]).ok()?;
    let inner = response.strip_prefix("\x1b[4;")?;
    let inner = inner.strip_suffix('t')?;
    let mut parts = inner.split(';');
    let ypixel: u32 = parts.next()?.parse().ok()?;
    let xpixel: u32 = parts.next()?.parse().ok()?;

    if xpixel > 0 && ypixel > 0 {
        Some((xpixel / cols, ypixel / rows))
    } else {
        None
    }
}

// --- Protocol detection ---

#[derive(Clone, Copy)]
enum ResolvedProtocol {
    Kitty,
    Iterm,
    Sixel,
    Block,
}

fn resolve_protocol(protocol: &ImageProtocol) -> ResolvedProtocol {
    match protocol {
        ImageProtocol::Kitty => ResolvedProtocol::Kitty,
        ImageProtocol::Iterm => ResolvedProtocol::Iterm,
        ImageProtocol::Sixel => ResolvedProtocol::Sixel,
        ImageProtocol::Auto => detect_protocol(),
    }
}

fn detect_protocol() -> ResolvedProtocol {
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return ResolvedProtocol::Kitty;
    }
    if std::env::var("ITERM_SESSION_ID").is_ok() {
        return ResolvedProtocol::Iterm;
    }
    if is_sixel_capable_terminal() {
        return ResolvedProtocol::Sixel;
    }
    ResolvedProtocol::Block
}

fn is_sixel_capable_terminal() -> bool {
    if std::env::var("WT_SESSION").is_ok() {
        return true;
    }
    let sixel_programs = ["WezTerm", "foot", "mlterm", "contour", "Black Box"];
    if let Ok(prog) = std::env::var("TERM_PROGRAM") {
        if sixel_programs.iter().any(|&s| prog.contains(s)) {
            return true;
        }
    }
    if let Ok(term) = std::env::var("LC_TERMINAL") {
        if term.contains("iTerm2") {
            return true;
        }
    }
    if std::env::var("XTERM_VERSION").is_ok() {
        return true;
    }
    false
}

// --- Protocol output ---

fn print_kitty(rgba: &[u8], w: u32, h: u32) -> io::Result<()> {
    let encoded = STANDARD.encode(rgba);
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(KITTY_CHUNK_SIZE)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();

    let mut stdout = io::stdout().lock();
    for (i, chunk) in chunks.iter().enumerate() {
        let more = if i < chunks.len() - 1 { 1 } else { 0 };
        if i == 0 {
            write!(
                stdout,
                "\x1b_Ga=T,f=32,s={},v={},m={};{}\x1b\\",
                w, h, more, chunk
            )?;
        } else {
            write!(stdout, "\x1b_Gm={};{}\x1b\\", more, chunk)?;
        }
    }
    stdout.flush()
}

fn print_iterm(img: &DynamicImage, _w: u32, _h: u32) -> io::Result<()> {
    let mut png_buf = Cursor::new(Vec::new());
    img.write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(io::Error::other)?;
    let encoded = STANDARD.encode(png_buf.into_inner());

    let mut stdout = io::stdout().lock();
    write!(
        stdout,
        "\x1b]1337;File=inline=1;preserveAspectRatio=1:{}\x07",
        encoded
    )?;
    stdout.flush()
}

fn print_sixel(rgba: &[u8], w: usize, h: usize) -> io::Result<()> {
    let sixel_img = icy_sixel::SixelImage::from_rgba(rgba.to_vec(), w, h);
    let encoded = sixel_img.encode().map_err(io::Error::other)?;

    let mut stdout = io::stdout().lock();
    write!(stdout, "{}", encoded)?;
    stdout.flush()
}

fn print_block(img: &DynamicImage, max_cols: u32) -> io::Result<u32> {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());

    let scale = (max_cols as f64) / (w as f64);
    let scaled_w = max_cols;
    let scaled_h = ((h as f64) * scale) as u32;

    let resized = img.resize_exact(scaled_w, scaled_h, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (rw, rh) = (rgba.width(), rgba.height());

    let mut stdout = io::stdout().lock();
    let term_rows = rh.div_ceil(2);

    for row in 0..term_rows {
        let top_y = row * 2;
        let bot_y = row * 2 + 1;
        for x in 0..rw {
            let top = rgba.get_pixel(x, top_y);
            let bot = if bot_y < rh {
                rgba.get_pixel(x, bot_y)
            } else {
                &image::Rgba([0, 0, 0, 0])
            };
            write!(
                stdout,
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▄",
                bot[0], bot[1], bot[2], top[0], top[1], top[2]
            )?;
        }
        writeln!(stdout, "\x1b[0m")?;
    }
    stdout.flush()?;
    Ok(term_rows)
}

// --- Cache helpers ---

async fn download_image(url: &str) -> Option<DynamicImage> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let bytes = client.get(url).send().await.ok()?.bytes().await.ok()?;
    image::load_from_memory(&bytes).ok()
}

fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("steamfetch").join("images"))
}

fn load_from_cache(key: &str) -> Option<DynamicImage> {
    let path = cache_dir()?.join(key);
    let bytes = fs::read(path).ok()?;
    image::load_from_memory(&bytes).ok()
}

fn save_to_cache(key: &str, img: &DynamicImage) {
    let Some(dir) = cache_dir() else { return };
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(key);
    let mut buf = Cursor::new(Vec::new());
    let _ = img.write_to(&mut buf, image::ImageFormat::Png);
    let _ = fs::write(path, buf.into_inner());
}
