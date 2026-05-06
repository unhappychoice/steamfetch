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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_cache_key_keeps_alphanumeric() {
        assert_eq!(sanitize_cache_key("abcDEF123"), "abcDEF123");
    }

    #[test]
    fn test_sanitize_cache_key_keeps_allowed_punctuation() {
        assert_eq!(sanitize_cache_key("file_name-1.png"), "file_name-1.png");
    }

    #[test]
    fn test_sanitize_cache_key_replaces_path_separators() {
        assert_eq!(sanitize_cache_key("a/b\\c"), "a_b_c");
    }

    #[test]
    fn test_sanitize_cache_key_replaces_spaces_and_specials() {
        assert_eq!(sanitize_cache_key("hello world!?:*"), "hello_world____",);
    }

    #[test]
    fn test_sanitize_cache_key_empty_string() {
        assert_eq!(sanitize_cache_key(""), "");
    }

    #[test]
    fn test_sanitize_cache_key_keeps_unicode_alphanumeric() {
        // Japanese characters are alphanumeric per Rust's char::is_alphanumeric
        assert_eq!(sanitize_cache_key("テスト_1"), "テスト_1");
    }

    #[test]
    fn test_resolve_protocol_kitty() {
        assert!(matches!(
            resolve_protocol(&ImageProtocol::Kitty),
            ResolvedProtocol::Kitty
        ));
    }

    #[test]
    fn test_resolve_protocol_iterm() {
        assert!(matches!(
            resolve_protocol(&ImageProtocol::Iterm),
            ResolvedProtocol::Iterm
        ));
    }

    #[test]
    fn test_resolve_protocol_sixel() {
        assert!(matches!(
            resolve_protocol(&ImageProtocol::Sixel),
            ResolvedProtocol::Sixel
        ));
    }

    #[test]
    fn test_cache_dir_is_under_steamfetch_images() {
        if let Some(dir) = cache_dir() {
            assert!(dir.ends_with("steamfetch/images"));
        }
    }

    #[test]
    fn test_cursor_right_zero_is_noop() {
        // The zero branch returns without writing any escape sequence.
        // We can't capture stdout here, but the call must not panic.
        cursor_right(0);
    }

    #[test]
    fn test_cursor_right_nonzero_does_not_panic() {
        // Exercises the formatted-write branch.
        cursor_right(5);
    }

    #[cfg(unix)]
    #[test]
    fn test_query_cell_size_ioctl_does_not_panic_on_captured_stdout() {
        // Under `cargo test` the test binary's stdout is captured by the
        // runner, so `ioctl(STDOUT_FILENO, TIOCGWINSZ, ...)` typically
        // returns -1 and the function short-circuits to None. Exercises
        // the function entry, the ioctl syscall, the short-circuit on
        // `r != 0`, and the trailing `None` fallback — paths that no
        // existing test reaches because nothing else calls the helper
        // directly. Even on hosts where ioctl succeeds without pixel
        // info (`ws_xpixel == 0`), the contract is still that the call
        // returns without panicking.
        let _ = query_cell_size_ioctl();
    }

    #[cfg(unix)]
    #[test]
    fn test_query_cell_size_ioctl_uses_stdout_pixel_dimensions() {
        struct StdoutGuard {
            saved_stdout: i32,
            master: i32,
            slave: i32,
        }

        impl Drop for StdoutGuard {
            fn drop(&mut self) {
                unsafe {
                    libc::dup2(self.saved_stdout, libc::STDOUT_FILENO);
                    libc::close(self.saved_stdout);
                    libc::close(self.slave);
                    libc::close(self.master);
                }
            }
        }

        let mut master = -1;
        let mut slave = -1;
        let size = libc::winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 800,
            ws_ypixel: 384,
        };

        let opened = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        };
        assert_eq!(opened, 0, "openpty should create a pseudo terminal");

        let saved_stdout = unsafe { libc::dup(libc::STDOUT_FILENO) };
        assert!(saved_stdout >= 0, "stdout should be duplicated");
        let _guard = StdoutGuard {
            saved_stdout,
            master,
            slave,
        };

        unsafe {
            libc::dup2(slave, libc::STDOUT_FILENO);
        }
        assert_eq!(query_cell_size(), (10, 16));
    }

    #[cfg(unix)]
    #[test]
    fn test_query_cell_size_escape_does_not_panic_without_tty_response() {
        let _ = query_cell_size_escape();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_query_cell_size_escape_parses_terminal_response() {
        use std::io::Read;
        use std::os::fd::FromRawFd;
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};

        let mut master = -1;
        let mut slave = -1;
        let size = libc::winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let opened = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        };
        assert_eq!(opened, 0, "openpty should create a pseudo terminal");

        let mut pipe_fds = [-1, -1];
        assert_eq!(
            unsafe { libc::pipe(pipe_fds.as_mut_ptr()) },
            0,
            "pipe should be created"
        );

        let mut child = unsafe {
            let mut command = Command::new(std::env::current_exe().expect("current test binary"));
            command
                .arg("--exact")
                .arg("image_display::tests::query_cell_size_escape_child_process")
                .arg("--nocapture")
                .env(
                    "STEAMFETCH_QUERY_CELL_SIZE_RESULT_FD",
                    pipe_fds[1].to_string(),
                )
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .pre_exec(move || {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::ioctl(slave, libc::TIOCSCTTY, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            command.spawn().expect("child test process should spawn")
        };

        unsafe {
            libc::close(slave);
            libc::close(pipe_fds[1]);
        }

        let mut query = [0u8; 16];
        let n = unsafe { libc::read(master, query.as_mut_ptr() as *mut _, query.len()) };
        assert!(n > 0, "child should write the terminal query");
        assert!(std::str::from_utf8(&query[..n as usize])
            .expect("query should be utf8")
            .contains("\x1b[14t"));

        let response = b"\x1b[4;480;800t";
        assert_eq!(
            unsafe { libc::write(master, response.as_ptr() as *const _, response.len()) },
            response.len() as isize,
            "terminal response should be written"
        );

        let mut result = String::new();
        let mut pipe = unsafe { std::fs::File::from_raw_fd(pipe_fds[0]) };
        pipe.read_to_string(&mut result)
            .expect("child result should be readable");

        unsafe {
            libc::close(master);
        }
        let status = child.wait().expect("child test process should exit");

        assert!(status.success(), "child should exit normally");
        assert_eq!(result, "10,20");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn query_cell_size_escape_child_process() {
        use std::io::Write;
        use std::os::fd::FromRawFd;

        let Ok(fd) = std::env::var("STEAMFETCH_QUERY_CELL_SIZE_RESULT_FD") else {
            return;
        };
        let fd: i32 = fd.parse().expect("result fd should be an integer");
        let result = query_cell_size_escape()
            .map(|(w, h)| format!("{},{}", w, h))
            .unwrap_or_else(|| "none".to_string());
        let mut pipe = unsafe { std::fs::File::from_raw_fd(fd) };
        pipe.write_all(result.as_bytes())
            .expect("result should be written");
    }

    #[cfg(unix)]
    #[test]
    fn test_query_cell_size_uses_default_without_terminal_size() {
        assert_eq!(query_cell_size(), (10, 20));
    }

    mod print_tests {
        use super::super::*;
        use image::{DynamicImage, ImageBuffer, Rgba};

        fn make_test_image(w: u32, h: u32) -> DynamicImage {
            let mut buf: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let r = ((x * 255) / w.max(1)) as u8;
                    let g = ((y * 255) / h.max(1)) as u8;
                    buf.put_pixel(x, y, Rgba([r, g, 128, 255]));
                }
            }
            DynamicImage::ImageRgba8(buf)
        }

        #[test]
        fn test_print_block_returns_term_rows_for_even_height() {
            // 4 wide, 4 tall image scaled to 4 cols → scaled_h = 4 → ceil(4/2) = 2 rows.
            let img = make_test_image(4, 4);
            let rows = print_block(&img, 4).expect("print_block should succeed");
            assert_eq!(rows, 2);
        }

        #[test]
        fn test_print_block_rounds_odd_height_up() {
            // 4 wide, 3 tall image at 2 cols → scale 0.5, scaled_h = 1, ceil(1/2) = 1.
            let img = make_test_image(4, 3);
            let rows = print_block(&img, 2).expect("print_block should succeed");
            assert_eq!(rows, 1);
        }

        #[test]
        fn test_print_kitty_chunked_encoding_does_not_panic() {
            // Use a buffer larger than a single Kitty chunk so the multi-chunk
            // branch (the `else` arm in the chunk loop) is exercised.
            let w = 64u32;
            let h = 64u32;
            let rgba = vec![123u8; (w * h * 4) as usize];
            print_kitty(&rgba, w, h).expect("print_kitty should succeed");
        }

        #[test]
        fn test_print_kitty_single_chunk_does_not_panic() {
            // Tiny buffer: one chunk total → first-chunk branch only.
            let rgba = vec![0u8; 4 * 4 * 4];
            print_kitty(&rgba, 4, 4).expect("print_kitty should succeed");
        }

        #[test]
        fn test_print_iterm_writes_inline_png() {
            let img = make_test_image(4, 4);
            print_iterm(&img, 4, 4).expect("print_iterm should succeed");
        }

        #[test]
        fn test_print_sixel_encodes_small_image() {
            let w = 4usize;
            let h = 4usize;
            let rgba = vec![200u8; w * h * 4];
            print_sixel(&rgba, w, h).expect("print_sixel should succeed");
        }

        #[test]
        fn test_print_sixel_errors_when_rgba_buffer_is_too_short() {
            let err = print_sixel(&[], 1, 1).expect_err("empty RGBA buffer should fail");
            assert_eq!(err.kind(), io::ErrorKind::Other);

            let err = print_sixel(&[255, 0, 0], 1, 1)
                .expect_err("RGB-only buffer should fail without alpha");
            assert_eq!(err.kind(), io::ErrorKind::Other);
            assert!(!err.to_string().is_empty());
        }

        #[test]
        fn test_print_image_and_rewind_kitty_returns_input_rows() {
            let img = make_test_image(8, 8);
            let rows = print_image_and_rewind(&img, &ImageProtocol::Kitty, 4, 3);
            assert_eq!(rows, Some(3));
        }

        #[test]
        fn test_print_image_and_rewind_iterm_returns_input_rows() {
            let img = make_test_image(8, 8);
            let rows = print_image_and_rewind(&img, &ImageProtocol::Iterm, 4, 3);
            assert_eq!(rows, Some(3));
        }

        #[test]
        fn test_print_image_and_rewind_sixel_returns_input_rows() {
            let img = make_test_image(8, 8);
            let rows = print_image_and_rewind(&img, &ImageProtocol::Sixel, 4, 3);
            assert_eq!(rows, Some(3));
        }

        #[test]
        fn test_print_image_and_rewind_returns_none_when_output_fails() {
            let img = make_test_image(8, 8);
            let rows = print_image_and_rewind(&img, &ImageProtocol::Sixel, 0, 3);
            assert_eq!(rows, None);
        }
    }

    mod env_tests {
        use super::super::*;
        use crate::test_support::lock_env;
        use std::env;

        const PROTOCOL_VARS: &[&str] = &[
            "KITTY_WINDOW_ID",
            "ITERM_SESSION_ID",
            "WT_SESSION",
            "TERM_PROGRAM",
            "LC_TERMINAL",
            "XTERM_VERSION",
        ];

        struct EnvScope {
            saved: Vec<(&'static str, Option<String>)>,
        }

        impl EnvScope {
            fn clear_all() -> Self {
                let saved = PROTOCOL_VARS
                    .iter()
                    .map(|&k| {
                        let prev = env::var(k).ok();
                        env::remove_var(k);
                        (k, prev)
                    })
                    .collect();
                Self { saved }
            }
        }

        impl Drop for EnvScope {
            fn drop(&mut self) {
                for (k, v) in &self.saved {
                    match v {
                        Some(val) => env::set_var(k, val),
                        None => env::remove_var(k),
                    }
                }
            }
        }

        #[test]
        fn test_is_sixel_capable_terminal_returns_false_when_no_env_set() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            assert!(!is_sixel_capable_terminal());
        }

        #[test]
        fn test_is_sixel_capable_terminal_detects_wt_session() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("WT_SESSION", "1");
            assert!(is_sixel_capable_terminal());
        }

        #[test]
        fn test_is_sixel_capable_terminal_detects_known_term_program() {
            let _guard = lock_env();
            for prog in ["WezTerm", "foot", "mlterm", "contour", "Black Box"] {
                let _scope = EnvScope::clear_all();
                env::set_var("TERM_PROGRAM", prog);
                assert!(
                    is_sixel_capable_terminal(),
                    "TERM_PROGRAM={} should be sixel-capable",
                    prog
                );
            }
        }

        #[test]
        fn test_is_sixel_capable_terminal_unknown_term_program_is_false() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("TERM_PROGRAM", "Apple_Terminal");
            assert!(!is_sixel_capable_terminal());
        }

        #[test]
        fn test_is_sixel_capable_terminal_detects_lc_terminal_iterm2() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("LC_TERMINAL", "iTerm2");
            assert!(is_sixel_capable_terminal());
        }

        #[test]
        fn test_is_sixel_capable_terminal_lc_terminal_other_is_false() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("LC_TERMINAL", "something-else");
            assert!(!is_sixel_capable_terminal());
        }

        #[test]
        fn test_is_sixel_capable_terminal_detects_xterm_version() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("XTERM_VERSION", "XTerm(370)");
            assert!(is_sixel_capable_terminal());
        }

        #[test]
        fn test_detect_protocol_prefers_kitty() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("KITTY_WINDOW_ID", "42");
            // Even with iterm/sixel hints set, kitty wins.
            env::set_var("ITERM_SESSION_ID", "xx");
            env::set_var("WT_SESSION", "1");
            assert!(matches!(detect_protocol(), ResolvedProtocol::Kitty));
        }

        #[test]
        fn test_detect_protocol_iterm_when_no_kitty() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("ITERM_SESSION_ID", "abc");
            env::set_var("WT_SESSION", "1"); // sixel hint should not override
            assert!(matches!(detect_protocol(), ResolvedProtocol::Iterm));
        }

        #[test]
        fn test_detect_protocol_sixel_when_only_sixel_hint() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("WT_SESSION", "1");
            assert!(matches!(detect_protocol(), ResolvedProtocol::Sixel));
        }

        #[test]
        fn test_detect_protocol_falls_back_to_block() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            assert!(matches!(detect_protocol(), ResolvedProtocol::Block));
        }

        #[test]
        fn test_resolve_protocol_auto_uses_detect() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            env::set_var("KITTY_WINDOW_ID", "1");
            assert!(matches!(
                resolve_protocol(&ImageProtocol::Auto),
                ResolvedProtocol::Kitty
            ));
        }

        #[test]
        fn test_resolve_protocol_auto_falls_back_to_block() {
            let _guard = lock_env();
            let _scope = EnvScope::clear_all();
            assert!(matches!(
                resolve_protocol(&ImageProtocol::Auto),
                ResolvedProtocol::Block
            ));
        }

        #[test]
        fn test_print_image_and_rewind_auto_block_returns_block_rows() {
            use image::{DynamicImage, ImageBuffer, Rgba};

            let _guard = lock_env();
            let _scope = EnvScope::clear_all();

            // Build a 2x2 image; with cols=4 the scale is 2, scaled_h is 4,
            // so print_block returns ceil(4/2) = 2 terminal rows.
            let mut buf: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(2, 2);
            for y in 0..2 {
                for x in 0..2 {
                    buf.put_pixel(x, y, Rgba([10, 20, 30, 255]));
                }
            }
            let img = DynamicImage::ImageRgba8(buf);

            // `rows` arg is intentionally different from the block-rendered
            // height; the Block branch returns the height computed by
            // print_block, not the caller-provided `rows`.
            let result = print_image_and_rewind(&img, &ImageProtocol::Auto, 4, 99);
            assert_eq!(result, Some(2));
        }

        #[test]
        fn test_envscope_drop_removes_protocol_vars_when_prev_was_none() {
            // Other tests in this module may run with one or more PROTOCOL_VARS
            // already set, so EnvScope::Drop's `Some(v)` arm dominates. Force
            // every var to be unset before EnvScope::clear_all so all six
            // `prev` slots capture None — the Drop then runs the `None`
            // branch for each, exercising the `None => env::remove_var(k)` arm.
            let _guard = lock_env();
            let outer: Vec<(&'static str, Option<String>)> = PROTOCOL_VARS
                .iter()
                .map(|&k| (k, env::var(k).ok()))
                .collect();
            for (k, _) in &outer {
                env::remove_var(k);
            }

            {
                let _scope = EnvScope::clear_all();
                // While in scope, set one var so we can verify Drop removes it.
                env::set_var("WT_SESSION", "scoped-marker");
                assert_eq!(env::var("WT_SESSION").unwrap(), "scoped-marker");
            }

            // Drop ran the `None => env::remove_var(k)` branch for each var.
            for &k in PROTOCOL_VARS {
                assert!(env::var(k).is_err(), "{} should have been removed", k);
            }

            for (k, v) in &outer {
                match v {
                    Some(val) => env::set_var(k, val),
                    None => env::remove_var(k),
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    mod cache_fs_tests {
        use super::super::*;
        use crate::test_support::lock_env;
        use image::{DynamicImage, ImageBuffer, Rgba};
        use std::env;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn unique_cache_root(label: &str) -> std::path::PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            env::temp_dir().join(format!(
                "steamfetch-image-cache-test-{}-{}-{}",
                label,
                std::process::id(),
                nanos
            ))
        }

        struct EnvScope {
            prev: Option<String>,
        }

        impl EnvScope {
            fn set(root: &std::path::Path) -> Self {
                let prev = env::var("XDG_CACHE_HOME").ok();
                env::set_var("XDG_CACHE_HOME", root);
                Self { prev }
            }
        }

        impl Drop for EnvScope {
            fn drop(&mut self) {
                match &self.prev {
                    Some(v) => env::set_var("XDG_CACHE_HOME", v),
                    None => env::remove_var("XDG_CACHE_HOME"),
                }
            }
        }

        fn make_test_image() -> DynamicImage {
            let mut buf: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(2, 2);
            buf.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
            buf.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
            buf.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
            buf.put_pixel(1, 1, Rgba([255, 255, 0, 255]));
            DynamicImage::ImageRgba8(buf)
        }

        #[test]
        fn test_cache_dir_starts_with_xdg_root() {
            let _guard = lock_env();
            let root = unique_cache_root("dir");
            let _scope = EnvScope::set(&root);

            let dir = cache_dir().expect("XDG_CACHE_HOME set, cache_dir must exist");
            assert!(dir.starts_with(&root));
            assert!(dir.ends_with("steamfetch/images"));
        }

        #[test]
        fn test_load_from_cache_returns_none_when_file_missing() {
            let _guard = lock_env();
            let root = unique_cache_root("missing");
            let _scope = EnvScope::set(&root);
            assert!(!root.exists());

            assert!(load_from_cache("does-not-exist.png").is_none());

            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn test_load_from_cache_returns_none_when_file_is_not_image() {
            let _guard = lock_env();
            let root = unique_cache_root("corrupt");
            let _scope = EnvScope::set(&root);

            let dir = cache_dir().unwrap();
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("not-an-image.png");
            std::fs::write(&path, b"this is not a PNG").unwrap();

            assert!(load_from_cache("not-an-image.png").is_none());

            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn test_save_then_load_roundtrip_returns_equivalent_image() {
            let _guard = lock_env();
            let root = unique_cache_root("rt");
            let _scope = EnvScope::set(&root);

            let img = make_test_image();
            save_to_cache("roundtrip.png", &img);

            let loaded = load_from_cache("roundtrip.png")
                .expect("image written by save_to_cache should be loadable");
            assert_eq!(loaded.width(), img.width());
            assert_eq!(loaded.height(), img.height());
            // PNG round-trip preserves RGBA bytes exactly.
            assert_eq!(loaded.to_rgba8().into_raw(), img.to_rgba8().into_raw());

            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn test_save_to_cache_creates_directory_when_missing() {
            let _guard = lock_env();
            let root = unique_cache_root("mkdir");
            let _scope = EnvScope::set(&root);
            assert!(!root.exists());

            save_to_cache("auto-mkdir.png", &make_test_image());

            let dir = cache_dir().unwrap();
            assert!(dir.exists(), "save_to_cache should create cache dir");
            assert!(dir.join("auto-mkdir.png").exists());

            let _ = std::fs::remove_dir_all(&root);
        }

        // Helper: build a single-thread runtime so we can drive async code
        // from a sync test that owns the env lock for its full lifetime.
        fn run_async<F: std::future::Future>(fut: F) -> F::Output {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(fut)
        }

        #[test]
        fn test_load_cached_or_download_returns_cached_without_network() {
            let _guard = lock_env();
            let root = unique_cache_root("cached");
            let _scope = EnvScope::set(&root);

            // Pre-populate the cache so the network branch is never reached.
            let img = make_test_image();
            save_to_cache("warm.png", &img);

            // URL is intentionally bogus — must not be hit on a cache hit.
            let loaded = run_async(load_cached_or_download(
                "http://127.0.0.1:1/never",
                "warm.png",
            ))
            .expect("cached image should be returned without download");
            assert_eq!(loaded.to_rgba8().into_raw(), img.to_rgba8().into_raw());

            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn test_load_cached_or_download_uses_sanitized_key_lookup() {
            let _guard = lock_env();
            let root = unique_cache_root("sanitize");
            let _scope = EnvScope::set(&root);

            // Pre-populate using the sanitized form of the raw key.
            let raw_key = "user/with spaces!.png";
            let safe_key = "user_with_spaces_.png";
            let img = make_test_image();
            save_to_cache(safe_key, &img);

            let loaded = run_async(load_cached_or_download("http://127.0.0.1:1/never", raw_key))
                .expect("sanitized key should match the cached file");
            assert_eq!(loaded.to_rgba8().into_raw(), img.to_rgba8().into_raw());

            let _ = std::fs::remove_dir_all(&root);
        }

        // Bind to a random port, capture it, then drop the listener.
        // Guarantees nothing is listening on the returned URL so reqwest
        // fails fast with a connection error rather than timing out.
        fn unbound_localhost_url(suffix: &str) -> String {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
            let port = listener.local_addr().expect("local_addr").port();
            drop(listener);
            format!("http://127.0.0.1:{}/{}", port, suffix)
        }

        #[test]
        fn test_download_image_returns_none_when_url_unreachable() {
            // download_image is private; reach it through this submodule's
            // `use super::super::*;`. Connection refused → reqwest's send
            // returns Err, hits `.ok()?` and returns None.
            let url = unbound_localhost_url("never.png");
            let result = run_async(download_image(&url));
            assert!(result.is_none());
        }

        #[test]
        fn test_load_cached_or_download_returns_none_when_cache_miss_and_download_fails() {
            let _guard = lock_env();
            let root = unique_cache_root("miss-then-fail");
            let _scope = EnvScope::set(&root);
            assert!(!root.exists());

            // Cache is empty, so the function falls through to download_image,
            // which fails (connection refused) — overall result is None.
            let url = unbound_localhost_url("absent.png");
            let result = run_async(load_cached_or_download(&url, "absent.png"));
            assert!(result.is_none());

            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn test_download_image_returns_none_when_response_is_not_an_image() {
            // Spawn a tiny one-shot HTTP server that returns 200 OK with a
            // non-image body. reqwest succeeds, bytes are read, but
            // image::load_from_memory fails → returns None.
            // This exercises the final `.ok()` on line 347.
            use std::io::{Read, Write};
            use std::net::TcpListener;

            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{}/junk.png", addr);

            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept");
                // Drain request headers (best-effort).
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let body = b"not actually a PNG";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(body);
                let _ = stream.flush();
            });

            let result = run_async(download_image(&url));
            assert!(result.is_none());
            let _ = server.join();
        }

        #[test]
        fn test_download_image_returns_none_when_body_read_fails() {
            // Server promises Content-Length: 100 in a 200 OK response, then
            // closes the connection before sending any body bytes. reqwest's
            // `.send().await` succeeds (headers parsed cleanly), but
            // `.bytes().await` fails on the truncated payload — exercising the
            // second `.ok()?` on line 346 of `download_image`. The other
            // download_image tests reach either the `.send()` failure path
            // (connection refused) or the `image::load_from_memory` failure
            // path (non-image bytes), so this is the remaining `.ok()?` arm.
            use std::io::{Read, Write};
            use std::net::TcpListener;

            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{}/truncated.png", addr);

            let server = std::thread::spawn(move || {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    // Promise 100 bytes in the body but deliver none. The
                    // `Connection: close` header tells the client that the
                    // stream end is the body end, so the missing bytes
                    // surface as a partial-response error rather than a hang.
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n",
                    );
                    let _ = stream.flush();
                    // Drop the stream → FIN reaches the client mid-body.
                }
            });

            let result = run_async(download_image(&url));
            assert!(result.is_none());
            let _ = server.join();
        }

        // Encodes a tiny image as PNG bytes for use as a mock HTTP response.
        fn png_bytes(img: &DynamicImage) -> Vec<u8> {
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png)
                .expect("encode PNG");
            buf.into_inner()
        }

        // Spawn a one-shot HTTP server that returns the given PNG bytes
        // with status 200, then exits.
        fn spawn_png_server(png: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
            use std::io::{Read, Write};
            use std::net::TcpListener;

            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{}/avatar.png", addr);

            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    png.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&png);
                let _ = stream.flush();
            });

            (url, server)
        }

        #[test]
        fn test_download_image_returns_some_when_response_is_a_png() {
            // Cache-independent: just verifies the success branch of
            // download_image where reqwest yields bytes that decode to a
            // valid image.
            let img = make_test_image();
            let (url, server) = spawn_png_server(png_bytes(&img));

            let result = run_async(download_image(&url)).expect("PNG body should decode");
            assert_eq!(result.width(), img.width());
            assert_eq!(result.height(), img.height());
            assert_eq!(result.to_rgba8().into_raw(), img.to_rgba8().into_raw());

            let _ = server.join();
        }

        #[test]
        fn test_envscope_drop_removes_xdg_cache_home_when_prev_was_none() {
            // Sibling tests in other modules (cache.rs, display.rs, client.rs
            // cache_hit_tests) hold their own ENV_LOCKs but XDG_CACHE_HOME is
            // process-global; whichever value is set when an EnvScope here
            // captures `prev` dictates which Drop arm runs. In typical runs
            // some other module has set the var, so EnvScope's
            // `Some(v) => env::set_var(...)` arm dominates and the
            // `None => env::remove_var("XDG_CACHE_HOME")` branch on line 798
            // never runs. Force XDG_CACHE_HOME to be unset before EnvScope::set
            // so prev = None, then verify Drop removes the value we set during
            // the scope.
            let _guard = lock_env();
            let outer_prev = env::var("XDG_CACHE_HOME").ok();
            env::remove_var("XDG_CACHE_HOME");

            let root = unique_cache_root("envscope-none-arm");
            std::fs::create_dir_all(&root).unwrap();
            {
                let _scope = EnvScope::set(&root);
                assert_eq!(env::var("XDG_CACHE_HOME").unwrap(), root.to_string_lossy());
            }

            // Drop ran the `None => env::remove_var("XDG_CACHE_HOME")` branch.
            assert!(env::var("XDG_CACHE_HOME").is_err());

            match outer_prev {
                Some(v) => env::set_var("XDG_CACHE_HOME", v),
                None => env::remove_var("XDG_CACHE_HOME"),
            }

            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn test_envscope_drop_restores_previous_xdg_cache_home() {
            let _guard = lock_env();
            let outer_prev = env::var("XDG_CACHE_HOME").ok();
            let sentinel_root = unique_cache_root("envscope-restore-sentinel");
            env::set_var("XDG_CACHE_HOME", &sentinel_root);

            let scoped_root = unique_cache_root("envscope-restore-scoped");
            {
                let _scope = EnvScope::set(&scoped_root);
                assert_eq!(
                    env::var("XDG_CACHE_HOME").unwrap(),
                    scoped_root.to_string_lossy(),
                );
            }

            assert_eq!(
                env::var("XDG_CACHE_HOME").unwrap(),
                sentinel_root.to_string_lossy(),
            );

            match outer_prev {
                Some(v) => env::set_var("XDG_CACHE_HOME", v),
                None => env::remove_var("XDG_CACHE_HOME"),
            }
        }

        #[test]
        fn test_load_cached_or_download_fetches_and_saves_when_cache_miss() {
            // Empty cache + reachable HTTP server returning a real PNG →
            // exercises the `download_image(...).await?` and
            // `save_to_cache(...)` lines that the cache-hit and
            // download-failure tests don't reach.
            let _guard = lock_env();
            let root = unique_cache_root("download-then-save");
            let _scope = EnvScope::set(&root);
            assert!(!root.exists());

            let img = make_test_image();
            let (url, server) = spawn_png_server(png_bytes(&img));

            let key = "fresh-avatar.png";
            let downloaded = run_async(load_cached_or_download(&url, key))
                .expect("server returns a PNG, decode must succeed");
            assert_eq!(downloaded.to_rgba8().into_raw(), img.to_rgba8().into_raw());

            // The save_to_cache call should have written the image under
            // the sanitized key inside cache_dir().
            let cached_path = cache_dir()
                .expect("XDG_CACHE_HOME is set, cache_dir must resolve")
                .join(key);
            assert!(
                cached_path.exists(),
                "save_to_cache should have written {:?}",
                cached_path
            );

            // Round-trip: load_from_cache on the same key yields the same
            // bytes; this confirms the saved file is a valid image.
            let reloaded =
                load_from_cache(key).expect("file just written by save_to_cache should reload");
            assert_eq!(reloaded.to_rgba8().into_raw(), img.to_rgba8().into_raw());

            let _ = server.join();
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}
