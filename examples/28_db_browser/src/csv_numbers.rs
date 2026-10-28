//! Strict localized numbers shared by CSV inference and typed SQLite binding.
use next_gui_platform::number_conventions::NumberConventions;

#[derive(Debug, PartialEq)]
pub enum Number {
    Integer(i64),
    Real(f64),
}

pub fn parse(text: &str, format: &NumberConventions) -> Option<Number> {
    let mut remaining = text.trim();
    let mut value = String::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let ch = remaining.chars().next()?;
        let digit = if ch.is_ascii_digit() {
            ch.to_digit(10).map(|n| n as usize)
        } else {
            format.digits.iter().position(|digit| *digit == ch)
        };
        if let Some(digit) = digit {
            value.push(char::from(b'0' + digit as u8));
            remaining = &remaining[ch.len_utf8()..];
            continue;
        }
        let token = [
            (&format.decimal, '.'),
            (&format.group, ','),
            (&format.positive, '+'),
            (&format.negative, '-'),
            (&format.exponent, 'e'),
        ]
        .into_iter()
        .find(|(symbol, _)| !symbol.is_empty() && remaining.starts_with(symbol.as_str()));
        if let Some((symbol, normalized)) = token {
            value.push(normalized);
            remaining = &remaining[symbol.len()..];
        } else if (format.group == "\u{a0}" || format.group == "\u{202f}")
            && [' ', '\u{a0}', '\u{202f}'].contains(&ch)
        {
            value.push(',');
            remaining = &remaining[ch.len_utf8()..];
        } else if ch.is_ascii_alphabetic() || ['+', '-'].contains(&ch) {
            value.push(ch);
            remaining = &remaining[ch.len_utf8()..];
        } else {
            return None;
        }
    }
    let lower = value.to_ascii_lowercase();
    match lower.as_str() {
        "inf" | "+inf" => return Some(Number::Real(f64::INFINITY)),
        "-inf" => return Some(Number::Real(f64::NEG_INFINITY)),
        "nan" | "+nan" | "-nan" => return Some(Number::Real(f64::NAN)),
        _ => {}
    }
    if value.contains(',') {
        let whole = value
            .split(['.', 'e', 'E'])
            .next()?
            .trim_start_matches(['+', '-']);
        if whole.bytes().filter(|ch| *ch == b',').count()
            != value.bytes().filter(|ch| *ch == b',').count()
        {
            return None;
        }
        let groups: Vec<_> = whole.split(',').collect();
        let mut index = 0;
        let mut size = 0;
        for (position, group) in groups.iter().rev().enumerate() {
            if group.is_empty() || !group.bytes().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            let next = *format.grouping.get(index).unwrap_or(&0);
            let left = position == groups.len() - 1;
            if next == 127 {
                if !left {
                    return None;
                }
                break;
            }
            if next > 0 {
                size = next as usize;
                index += 1;
            }
            if size == 0
                || if left {
                    group.len() > size
                } else {
                    group.len() != size
                }
            {
                return None;
            }
        }
        value.retain(|ch| ch != ',');
    }
    if let Ok(integer) = value.parse::<i64>() {
        return Some(Number::Integer(integer));
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .map(Number::Real)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_numbers_validate_grouping_decimal_exponents_and_full_integer_range() {
        let c = NumberConventions::default();
        for (text, expected) in [
            ("1,234", 1234),
            ("+9,223,372,036,854,775,807", i64::MAX),
            ("-9223372036854775808", i64::MIN),
        ] {
            assert_eq!(parse(text, &c), Some(Number::Integer(expected)));
        }
        for text in [
            "1,23", "1234,567", "1,,234", "1.2,3", "1e1,234", "1,234,", "1e309", "12%", "$12", "",
        ] {
            assert_eq!(parse(text, &c), None, "{text}");
        }
        assert!(matches!(
            parse("9223372036854775808", &c),
            Some(Number::Real(_))
        ));
        assert_eq!(parse(" -1,234.5e+2 ", &c), Some(Number::Real(-123450.0)));
        let german = NumberConventions {
            decimal: ",".into(),
            group: ".".into(),
            ..c.clone()
        };
        assert_eq!(parse("-1.234,50e2", &german), Some(Number::Real(-123450.0)));
        assert_eq!(parse("1.2", &german), None);
        assert_eq!(parse("1,2", &german), Some(Number::Real(1.2)));
        assert_eq!(parse("12,34.5", &german), None);
        let french = NumberConventions {
            decimal: ",".into(),
            group: "\u{202f}".into(),
            ..c.clone()
        };
        for text in ["1 234,5", "1\u{a0}234,5", "1\u{202f}234,5"] {
            assert_eq!(parse(text, &french), Some(Number::Real(1234.5)));
        }
        let indian = NumberConventions {
            grouping: vec![3, 2, 0],
            ..c
        };
        assert_eq!(parse("12,34,567", &indian), Some(Number::Integer(1234567)));
        for text in ["1,234,567", "123,45,678"] {
            assert_eq!(parse(text, &indian), None);
        }
    }
    #[test]
    fn csv_numbers_accept_native_digits_signs_and_explicit_non_finite_values() {
        let arabic = NumberConventions {
            decimal: "٫".into(),
            group: "٬".into(),
            digits: ['٠', '١', '٢', '٣', '٤', '٥', '٦', '٧', '٨', '٩'],
            negative: "\u{61c}-".into(),
            positive: "\u{61c}+".into(),
            exponent: "اس".into(),
            ..NumberConventions::default()
        };
        assert_eq!(
            parse("\u{61c}-١٬٢٣٤٫٥اس٢", &arabic),
            Some(Number::Real(-123450.0))
        );
        assert_eq!(parse("-١٢٣", &arabic), Some(Number::Integer(-123)));
        assert_eq!(parse("١٢٬٣", &arabic), None);
        assert_eq!(parse("1,234.5", &arabic), None);
        assert!(matches!(parse("NaN",&arabic),Some(Number::Real(number)) if number.is_nan()));
        assert_eq!(
            parse("\u{61c}-INF", &arabic),
            Some(Number::Real(f64::NEG_INFINITY))
        );
    }
}
