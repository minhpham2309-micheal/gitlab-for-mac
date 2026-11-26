#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b)
    }
}

pub const LOG_FG: Rgb = Rgb::new(0xE8, 0xEC, 0xF1);
pub const RED: Rgb = Rgb::new(0xFF, 0x6B, 0x68);
pub const GREEN: Rgb = Rgb::new(0xA5, 0xC2, 0x61);
pub const YELLOW: Rgb = Rgb::new(0xFF, 0xC6, 0x6D);
pub const BLUE: Rgb = Rgb::new(0x7C, 0xB7, 0xF1);
pub const MAGENTA: Rgb = Rgb::new(0xC7, 0x7D, 0xBB);
pub const CYAN: Rgb = Rgb::new(0x6C, 0xDA, 0xDA);
pub const GRAY: Rgb = Rgb::new(0xA0, 0xA0, 0xA0);
pub const WHITE: Rgb = Rgb::new(0xEE, 0xEE, 0xEE);

pub const ANSI: [Rgb; 16] = [
    Rgb::new(0x00, 0x00, 0x00),
    RED,
    GREEN,
    YELLOW,
    BLUE,
    MAGENTA,
    CYAN,
    WHITE,
    Rgb::new(0x55, 0x55, 0x55),
    Rgb::new(0xFF, 0x87, 0x85),
    Rgb::new(0xA5, 0xC2, 0x61),
    Rgb::new(0xFF, 0xC6, 0x6D),
    Rgb::new(0x7C, 0xB7, 0xF1),
    Rgb::new(0xC7, 0x7D, 0xBB),
    Rgb::new(0x6C, 0xDA, 0xDA),
    Rgb::new(0xFF, 0xFF, 0xFF),
];

#[derive(Clone, Copy)]
pub struct Style {
    pub fg: Option<Rgb>,
    pub bold: bool,
}

impl Style {
    pub fn reset() -> Self {
        Self { fg: None, bold: false }
    }

    pub fn color(self) -> Rgb {
        self.fg.unwrap_or(if self.bold { WHITE } else { LOG_FG })
    }
}

pub fn apply_sgr(style: &mut Style, params: &str) {
    if params.is_empty() {
        *style = Style::reset();
        return;
    }
    let codes: Vec<u32> = params
        .split(';')
        .filter_map(|part| part.parse().ok())
        .collect();
    let mut idx = 0;
    while idx < codes.len() {
        match codes[idx] {
            0 => *style = Style::reset(),
            1 => style.bold = true,
            2 => style.fg = Some(GRAY),
            90..=97 => style.fg = Some(ANSI[(8 + (codes[idx] - 90)) as usize]),
            22 => style.bold = false,
            30..=37 => style.fg = Some(ANSI[(codes[idx] - 30) as usize]),
            39 => style.fg = None,
            38 if idx + 1 < codes.len() => match codes[idx + 1] {
                5 if idx + 2 < codes.len() => {
                    style.fg = Some(ansi256(codes[idx + 2]));
                    idx += 2;
                }
                2 if idx + 4 < codes.len() => {
                    style.fg = Some(Rgb::new(
                        codes[idx + 2] as u8,
                        codes[idx + 3] as u8,
                        codes[idx + 4] as u8,
                    ));
                    idx += 4;
                }
                _ => {}
            },
            _ => {}
        }
        idx += 1;
    }
}

pub fn ansi256(code: u32) -> Rgb {
    match code {
        0..=15 => ANSI[code as usize],
        232..=255 => {
            let level = (8 + (code - 232) * 10) as u8;
            Rgb::new(level, level, level)
        }
        16..=231 => {
            let n = code - 16;
            let scale = |v: u32| if v == 0 { 0 } else { (40 * v + 55) as u8 };
            Rgb::new(scale(n / 36), scale((n / 6) % 6), scale(n % 6))
        }
        _ => LOG_FG,
    }
}

pub fn tint(text: &str, color: Rgb) -> Rgb {
    if color != LOG_FG && color != WHITE {
        return color;
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("failed") || lower.contains("fatal") {
        RED
    } else if lower.contains("warn") {
        YELLOW
    } else if lower.contains("success") || lower.contains("passed") || lower.contains("succeeded") {
        GREEN
    } else if text.contains('▶') {
        CYAN
    } else if text.contains('■') {
        GRAY
    } else {
        color
    }
}
