use next_gui_text::{Font, TextLayout};

fn space(ch: char) -> bool {
    ch.is_whitespace() && !matches!(ch, '\u{a0}' | '\u{202f}' | '\u{feff}')
}

pub(crate) fn layout(font: &Font, text: &str, size: f32, width: f32) -> TextLayout {
    let mut wrapped = String::with_capacity(text.len());
    let mut x = 0.0;
    for token in text.split_inclusive(space) {
        let word = token.trim_end_matches(space);
        let word_width = font.measure_line(word, size).0;
        if x > 0.0 && word_width <= width && x + word_width > width {
            wrapped.truncate(wrapped.trim_end_matches(|ch| space(ch) && ch != '\n').len());
            wrapped.push('\n');
            x = 0.0;
        }
        wrapped.push_str(token);
        for ch in token.chars() {
            if ch == '\n' {
                x = 0.0;
            } else {
                let mut bytes = [0; 4];
                let advance = font.measure_line(ch.encode_utf8(&mut bytes), size).0;
                if x > 0.0 && x + advance > width { x = 0.0; }
                x += advance;
            }
        }
    }
    font.layout(&wrapped, size, Some(width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_preserves_paragraphs_nonbreaking_spaces_and_oversized_tokens() {
        let font = next_gui_text::load_default_cjk_font().unwrap();
        let size = 14.0;
        for text in ["First line\n\nSecond line\n", "Save\u{a0}and\u{202f}apply", "", "  indented text"] {
            let actual = layout(&font, text, size, 600.0);
            let expected = font.layout(text, size, Some(600.0));
            assert_eq!(actual.width, expected.width);
            assert_eq!(actual.height, expected.height);
            assert_eq!(actual.glyphs.iter().map(|g| (g.ch, g.x, g.baseline_y)).collect::<Vec<_>>(), expected.glyphs.iter().map(|g| (g.ch, g.x, g.baseline_y)).collect::<Vec<_>>());
        }
        for text in ["long_database_identifier_without_spaces", "数据库设置保存应用", "/a/very/long/database/path.db"] {
            let actual = layout(&font, text, size, 60.0);
            assert!(actual.height > actual.line_height);
            assert!(actual.width <= 60.0);
            assert_eq!(actual.glyphs.iter().map(|g| g.ch).collect::<String>(), text);
        }
        let text = "MMMMMMMMMMMMMMMMMM abcdefghijklmnopqrstuvwxyz_very_long database";
        let actual = layout(&font, text, size, 80.0);
        assert!(actual.width <= 80.0);
        assert_eq!(actual.glyphs.iter().filter(|g| !space(g.ch)).map(|g| g.ch).collect::<String>(), text.chars().filter(|ch| !space(*ch)).collect::<String>());
    }
}
