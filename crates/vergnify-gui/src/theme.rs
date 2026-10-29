// @author Cursor

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: u32,
    pub card: u32,
    pub text: u32,
    pub muted: u32,
    pub green: u32,
    pub green_bg: u32,
    pub orange: u32,
    pub orange_bg: u32,
    pub red: u32,
    pub red_bg: u32,
    pub gray: u32,
    pub gray_bg: u32,
    pub icon_bg: u32,
    pub icon_text: u32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: 0x1e1f22,
            card: 0x2b2d31,
            text: 0xf2f3f5,
            muted: 0xa0a3aa,
            green: 0x1f8f4e,
            green_bg: 0x173824,
            orange: 0xd4892a,
            orange_bg: 0x3a2c14,
            red: 0xd4524e,
            red_bg: 0x3a1c1c,
            gray: 0x8b9098,
            gray_bg: 0x33353a,
            icon_bg: 0xf4f5f7,
            icon_text: 0x1e1f22,
        }
    } else {
        Palette {
            bg: 0xf2f3f5,
            card: 0xffffff,
            text: 0x1e1f22,
            muted: 0x5e6570,
            green: 0x0f7a3a,
            green_bg: 0xe5f6ec,
            orange: 0x8a5a00,
            orange_bg: 0xfff3d6,
            red: 0xb42318,
            red_bg: 0xfdecea,
            gray: 0x4b5563,
            gray_bg: 0xe8eaed,
            icon_bg: 0xf4f5f7,
            icon_text: 0x1e1f22,
        }
    }
}

pub fn load() -> ThemeChoice {
    let Ok(text) = std::fs::read_to_string(settings_path()) else {
        return ThemeChoice::System;
    };
    parse(&text)
}

pub fn save(choice: ThemeChoice) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, choice.as_str());
}

pub fn parse(text: &str) -> ThemeChoice {
    match text.trim() {
        "light" => ThemeChoice::Light,
        "dark" => ThemeChoice::Dark,
        _ => ThemeChoice::System,
    }
}

impl ThemeChoice {
    fn as_str(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }
}

fn settings_path() -> PathBuf {
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config).join("vergnify").join("appearance");
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if cfg!(target_os = "macos") {
            return home.join("Library/Application Support/vergnify/appearance");
        }
        return home.join(".config/vergnify/appearance");
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local).join("vergnify").join("appearance");
    }
    PathBuf::from(".vergnify-appearance")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_appearance_follows_the_system() {
        assert_eq!(parse("dark"), ThemeChoice::Dark);
        assert_eq!(parse(" light "), ThemeChoice::Light);
        assert_eq!(parse("nope"), ThemeChoice::System);
        assert_eq!(parse(""), ThemeChoice::System);
    }
}
