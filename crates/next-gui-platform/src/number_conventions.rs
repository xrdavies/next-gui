//! Read-only numeric conventions from the user's operating-system locale.

/// A snapshot of numeric symbols, suitable for moving to a worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberConventions {
    /// Decimal separator.
    pub decimal: String,
    /// Digit-group separator; empty when grouping is unavailable.
    pub group: String,
    /// Group sizes from the decimal point leftwards; zero repeats the last size.
    pub grouping: Vec<u8>,
    /// Local digits, ordered zero through nine.
    pub digits: [char; 10],
    /// Local positive sign.
    pub positive: String,
    /// Local negative sign.
    pub negative: String,
    /// Exponent marker; ASCII fallback when the OS exposes no localized symbol.
    pub exponent: String,
}

impl Default for NumberConventions {
    fn default() -> Self {
        Self {
            decimal: ".".into(),
            group: ",".into(),
            grouping: vec![3, 0],
            digits: ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'],
            positive: "+".into(),
            negative: "-".into(),
            exponent: "e".into(),
        }
    }
}

impl NumberConventions {
    /// Read current system conventions without changing the process locale.
    /// Errors are returned rather than substituting separators for a different locale.
    pub fn system() -> Result<Self, String> {
        let format = native::system()?;
        if format.decimal.is_empty()
            || format.decimal == format.group
            || format
                .digits
                .iter()
                .enumerate()
                .any(|(index, digit)| format.digits[..index].contains(digit))
        {
            return Err("System number format has ambiguous numeric symbols".into());
        }
        Ok(format)
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::ffi::{c_char, c_void, CStr};
    type Ref = *const c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFLocaleCopyCurrent() -> Ref;
        fn CFNumberFormatterCreate(allocator: Ref, locale: Ref, style: isize) -> Ref;
        fn CFNumberFormatterCopyProperty(formatter: Ref, key: Ref) -> Ref;
        fn CFNumberFormatterCreateStringWithValue(
            allocator: Ref,
            formatter: Ref,
            kind: isize,
            value: Ref,
        ) -> Ref;
        fn CFNumberGetValue(value: Ref, kind: isize, output: *mut c_void) -> u8;
        fn CFStringGetLength(value: Ref) -> isize;
        fn CFStringGetCString(
            value: Ref,
            buffer: *mut c_char,
            capacity: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(value: Ref);
        static kCFNumberFormatterDecimalSeparator: Ref;
        static kCFNumberFormatterGroupingSeparator: Ref;
        static kCFNumberFormatterGroupingSize: Ref;
        static kCFNumberFormatterSecondaryGroupingSize: Ref;
        static kCFNumberFormatterPlusSign: Ref;
        static kCFNumberFormatterMinusSign: Ref;
        static kCFNumberFormatterExponentSymbol: Ref;
    }
    struct Owned(Ref);
    impl Owned {
        fn new(value: Ref) -> Result<Self, String> {
            if value.is_null() {
                Err("System number format is unavailable".into())
            } else {
                Ok(Self(value))
            }
        }
        fn text(&self) -> Result<String, String> {
            // Every caller owns a CFString returned by a string-valued formatter property.
            unsafe {
                let capacity = CFStringGetLength(self.0)
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(1))
                    .ok_or("System numeric symbol is too long")?;
                let mut bytes = vec![0u8; capacity as usize];
                if CFStringGetCString(self.0, bytes.as_mut_ptr().cast(), capacity, 0x08000100) == 0
                {
                    return Err("System numeric symbol is not UTF-8".into());
                }
                CStr::from_ptr(bytes.as_ptr().cast())
                    .to_str()
                    .map(str::to_owned)
                    .map_err(|error| error.to_string())
            }
        }
        fn group_size(&self) -> Result<u8, String> {
            let mut size = 0i64;
            if unsafe { CFNumberGetValue(self.0, 4, (&mut size as *mut i64).cast()) } == 0 {
                return Err("System digit grouping is unavailable".into());
            }
            u8::try_from(size).map_err(|error| error.to_string())
        }
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
    pub(super) fn system() -> Result<NumberConventions, String> {
        let locale = Owned::new(unsafe { CFLocaleCopyCurrent() })?;
        from_locale(&locale)
    }
    fn from_locale(locale: &Owned) -> Result<NumberConventions, String> {
        let formatter =
            Owned::new(unsafe { CFNumberFormatterCreate(std::ptr::null(), locale.0, 1) })?;
        let property = |key| Owned::new(unsafe { CFNumberFormatterCopyProperty(formatter.0, key) });
        let mut digits = ['0'; 10];
        for (index, digit) in digits.iter_mut().enumerate() {
            let value = index as i64;
            let text = Owned::new(unsafe {
                CFNumberFormatterCreateStringWithValue(
                    std::ptr::null(),
                    formatter.0,
                    4,
                    (&value as *const i64).cast(),
                )
            })?
            .text()?;
            let mut numeric = text.chars().filter(|ch| ch.is_numeric());
            *digit = numeric.next().ok_or("System number format has no digit")?;
            if numeric.next().is_some() {
                return Err("System number format needs single-scalar digits".into());
            }
        }
        Ok(NumberConventions {
            decimal: property(unsafe { kCFNumberFormatterDecimalSeparator })?.text()?,
            group: property(unsafe { kCFNumberFormatterGroupingSeparator })?.text()?,
            grouping: vec![
                property(unsafe { kCFNumberFormatterGroupingSize })?.group_size()?,
                property(unsafe { kCFNumberFormatterSecondaryGroupingSize })?.group_size()?,
                0,
            ],
            digits,
            positive: property(unsafe { kCFNumberFormatterPlusSign })?.text()?,
            negative: property(unsafe { kCFNumberFormatterMinusSign })?.text()?,
            exponent: property(unsafe { kCFNumberFormatterExponentSymbol })?.text()?,
        })
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn native_number_conventions_cover_comma_indian_groups_and_arabic_digits() {
            extern "C" {
                fn CFStringCreateWithCString(
                    allocator: Ref,
                    text: *const c_char,
                    encoding: u32,
                ) -> Ref;
                fn CFLocaleCreate(allocator: Ref, identifier: Ref) -> Ref;
            }
            for (id, decimal, group, zero, primary, secondary) in [
                (c"de_DE", ",", ".", '0', 3, 0),
                (c"hi_IN", ".", ",", '0', 3, 2),
                (c"ar_EG", "٫", "٬", '٠', 3, 0),
            ] {
                let id = Owned::new(unsafe {
                    CFStringCreateWithCString(std::ptr::null(), id.as_ptr(), 0x08000100)
                })
                .unwrap();
                let locale = Owned::new(unsafe { CFLocaleCreate(std::ptr::null(), id.0) }).unwrap();
                let format = from_locale(&locale).unwrap();
                assert_eq!(
                    (
                        format.decimal.as_str(),
                        format.group.as_str(),
                        format.digits[0],
                        format.grouping[0],
                        format.grouping[1]
                    ),
                    (decimal, group, zero, primary, secondary)
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocaleInfoEx(name: *const u16, kind: u32, output: *mut u16, capacity: i32) -> i32;
    }
    pub(super) fn system() -> Result<NumberConventions, String> {
        let get = |kind| {
            let length =
                unsafe { GetLocaleInfoEx(std::ptr::null(), kind, std::ptr::null_mut(), 0) };
            if length <= 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut output = vec![0; length as usize];
            let count =
                unsafe { GetLocaleInfoEx(std::ptr::null(), kind, output.as_mut_ptr(), length) };
            if count <= 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            String::from_utf16(&output[..count as usize - 1]).map_err(|error| error.to_string())
        };
        let raw = get(19)?;
        let digits: [char; 10] = raw
            .chars()
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "System number format needs ten digits")?;
        Ok(NumberConventions {
            decimal: get(14)?,
            group: get(15)?,
            grouping: get(16)?
                .split(';')
                .map(|part| part.parse::<u8>().map_err(|error| error.to_string()))
                .collect::<Result<_, _>>()?,
            digits,
            positive: get(80)?,
            negative: get(81)?,
            // shortcut: Win32 exposes no exponent symbol; add CLDR symbols for non-ASCII exponent parity.
            exponent: "e".into(),
        })
    }
}

#[cfg(target_os = "linux")]
mod native {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt;
    pub(super) fn system() -> Result<NumberConventions, String> {
        let numeric_locale = ["LC_ALL", "LC_NUMERIC", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
            .unwrap_or_else(|| "C".into());
        let numeric_locale =
            CString::new(numeric_locale.as_bytes()).map_err(|error| error.to_string())?;
        // Native digits must follow LC_NUMERIC even when LC_CTYPE selects another language.
        let locale = unsafe {
            libc::newlocale(
                libc::LC_ALL_MASK,
                numeric_locale.as_ptr(),
                std::ptr::null_mut(),
            )
        };
        if locale.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        struct Locale(libc::locale_t);
        impl Drop for Locale {
            fn drop(&mut self) {
                unsafe {
                    libc::freelocale(self.0);
                }
            }
        }
        let locale = Locale(locale);
        let text = |item| {
            let pointer = unsafe { libc::nl_langinfo_l(item, locale.0) };
            if pointer.is_null() {
                return Err("System numeric symbol is unavailable".into());
            }
            unsafe { CStr::from_ptr(pointer) }
                .to_str()
                .map(str::to_owned)
                .map_err(|error| error.to_string())
        };
        let decimal = text(libc::RADIXCHAR)?;
        let group = text(libc::THOUSEP)?;
        let mut result = NumberConventions {
            decimal,
            group,
            // shortcut: POSIX has no numeric signs/exponent metadata; add CLDR symbols for Qt locale parity.
            ..NumberConventions::default()
        };
        #[cfg(target_env = "gnu")]
        {
            // glibc places GROUPING immediately after THOUSEP in LC_NUMERIC.
            let pointer = unsafe { libc::nl_langinfo_l(libc::THOUSEP + 1, locale.0) };
            if pointer.is_null() {
                return Err("System digit grouping is unavailable".into());
            }
            result.grouping = unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec();
            result.grouping.push(0);
            let previous = unsafe { libc::uselocale(locale.0) };
            if previous.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            struct Restore(libc::locale_t);
            impl Drop for Restore {
                fn drop(&mut self) {
                    unsafe {
                        libc::uselocale(self.0);
                    }
                }
            }
            let _restore = Restore(previous);
            for (index, digit) in result.digits.iter_mut().enumerate() {
                let mut output = [0u8; 32];
                let count = unsafe {
                    libc::snprintf(
                        output.as_mut_ptr().cast(),
                        output.len(),
                        c"%Id".as_ptr(),
                        index as libc::c_int,
                    )
                };
                if count <= 0 || count as usize >= output.len() {
                    return Err("System numeric digit is unavailable".into());
                }
                let value = std::str::from_utf8(&output[..count as usize])
                    .map_err(|error| error.to_string())?;
                let mut chars = value.chars();
                *digit = chars.next().ok_or("System numeric digit is empty")?;
                if chars.next().is_some() {
                    return Err("System numeric digit needs one Unicode scalar".into());
                }
            }
        }
        Ok(result)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod native {
    use super::*;
    pub(super) fn system() -> Result<NumberConventions, String> {
        Err("System number conventions are unavailable on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_number_conventions_are_owned_and_have_numeric_symbols() {
        let format = NumberConventions::system().unwrap();
        assert!(!format.decimal.is_empty());
        assert_eq!(format.digits.len(), 10);
        assert!(format.digits.iter().all(|digit| digit.is_numeric()));
        let thread = std::thread::spawn(move || format.decimal);
        assert!(!thread.join().unwrap().is_empty());
    }
}
