//! SQL editor search. Ranges use Unicode scalars, matching the shared TextArea API.

use pcre2::bytes::{CaptureLocations, Regex, RegexBuilder};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub case_sensitive: bool,
    pub whole_words: bool,
    pub wrap: bool,
    pub backwards: bool,
    pub selection_only: bool,
    pub regex: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self::from_flags([false, false, true, false, false, false])
    }
}

impl Options {
    pub fn from_flags(
        [case_sensitive, whole_words, wrap, backwards, selection_only, regex]: [bool; 6],
    ) -> Self {
        Self {
            case_sensitive,
            whole_words,
            wrap,
            backwards,
            selection_only,
            regex,
        }
    }

    pub fn flags(self) -> [bool; 6] {
        [
            self.case_sensitive,
            self.whole_words,
            self.wrap,
            self.backwards,
            self.selection_only,
            self.regex,
        ]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Find,
    FindReverse,
    FindAll,
    Replace,
    ReplaceAll,
}

pub struct Edit {
    pub range: Range<usize>,
    pub text: String,
}

pub struct Outcome {
    pub selection: Option<Range<usize>>,
    pub edit: Option<Edit>,
    pub highlights: Vec<Range<usize>>,
    pub message: String,
}

#[derive(Default)]
pub struct Search {
    pub options: Options,
    scope: Option<Range<usize>>,
    document: String,
    query: String,
    previous_options: Options,
    last: Option<Range<usize>>,
    last_replaced: bool,
}

struct Hit {
    chars: Range<usize>,
    bytes: Range<usize>,
    offset: usize,
}

fn byte_offset(text: &str, position: usize) -> usize {
    text.char_indices()
        .nth(position)
        .map_or(text.len(), |(i, _)| i)
}

fn compile(pattern: &str, case_sensitive: bool) -> Result<Regex, String> {
    RegexBuilder::new()
        .utf(true)
        .ucp(true)
        .caseless(!case_sensitive)
        .multi_line(true)
        .build(&format!(
            "(*LIMIT_MATCH=1000000)(*LIMIT_DEPTH=1000){pattern}"
        ))
        .map_err(|e| format!("Invalid search pattern: {e}"))
}

fn expand(template: &str, text: &str, captures: &CaptureLocations) -> Result<String, String> {
    let mut output = String::new();
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            output.push(c);
            continue;
        }
        match chars.next() {
            Some(c @ '0'..='9') => {
                if let Some((start, end)) = captures.get(c as usize - '0' as usize) {
                    output.push_str(
                        text.get(start..end)
                            .ok_or("Search capture splits a Unicode character")?,
                    );
                }
            }
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some('t') => output.push('\t'),
            Some('\\') => output.push('\\'),
            Some(c) => {
                output.push('\\');
                output.push(c);
            }
            None => output.push('\\'),
        }
    }
    Ok(output)
}

fn scan(
    text: &str,
    needle: &str,
    options: Options,
    scope: &Range<usize>,
    (overlapping, first_only): (bool, bool),
    deadline: std::time::Instant,
) -> Result<(Regex, Vec<Hit>), String> {
    let mut pattern = String::new();
    if options.regex {
        pattern.push_str(needle);
    } else {
        for c in needle.chars() {
            if "\\^$.|?*+()[]{}".contains(c) {
                pattern.push('\\');
            }
            pattern.push(c);
        }
    }
    let re = compile(&pattern, options.case_sensitive)?;
    if scope.start > scope.end {
        return Ok((re, Vec::new()));
    }
    let word = options
        .whole_words
        .then(|| compile(r"\A\w\z", true))
        .transpose()?;
    let is_word = |c: Option<char>| -> Result<bool, String> {
        let Some(c) = c else {
            return Ok(false);
        };
        word.as_ref()
            .unwrap()
            .is_match(c.encode_utf8(&mut [0; 4]).as_bytes())
            .map_err(|e| e.to_string())
    };
    let mut captures = re.capture_locations();
    let end = byte_offset(text, scope.end);
    let subject = &text[..end];
    let mut offset = byte_offset(text, scope.start);
    let mut previous = None;
    let mut hits = Vec::new();
    let (mut byte_cursor, mut char_cursor) = (0, 0);
    while offset <= subject.len() {
        if std::time::Instant::now() >= deadline {
            return Err(
                "Search time limit exceeded; narrow the selection or simplify the expression"
                    .into(),
            );
        }
        let Some(found) = re
            .captures_read_at(&mut captures, subject.as_bytes(), offset)
            .map_err(|e| format!("Search failed: {e}"))?
        else {
            break;
        };
        let bytes = found.start()..found.end();
        if bytes.start > bytes.end
            || !text.is_char_boundary(bytes.start)
            || !text.is_char_boundary(bytes.end)
        {
            return Err("Search pattern splits a Unicode character".into());
        }
        // The crate iterator advances one byte after empty matches; advance a Unicode scalar instead.
        let input_offset = offset;
        let advance = if overlapping { bytes.start } else { bytes.end };
        offset = if overlapping || bytes.is_empty() {
            match subject[advance..].chars().next() {
                Some(c) => advance + c.len_utf8(),
                None => subject.len() + 1,
            }
        } else {
            bytes.end
        };
        if !overlapping && bytes.is_empty() && previous == Some(bytes.end) {
            continue;
        }
        previous = Some(bytes.end);
        if word.is_some()
            && (is_word(text[..bytes.start].chars().next_back())?
                || is_word(text[bytes.end..].chars().next())?)
        {
            continue;
        }
        char_cursor += text[byte_cursor..bytes.start].chars().count();
        let end = char_cursor + text[bytes.clone()].chars().count();
        byte_cursor = bytes.start;
        hits.push(Hit {
            chars: char_cursor..end,
            bytes,
            offset: input_offset,
        });
        if first_only {
            break;
        }
    }
    Ok((re, hits))
}

impl Search {
    pub fn reset(&mut self, text: &str, selection: Range<usize>) {
        self.scope = (self.options.selection_only && !selection.is_empty()).then_some(selection);
        self.document = text.to_owned();
        self.query.clear();
        self.last = None;
        self.last_replaced = false;
    }

    pub fn set_options(&mut self, options: Options, text: &str, selection: Range<usize>) {
        if self.options.selection_only != options.selection_only {
            self.options = options;
            self.reset(text, selection);
        } else {
            self.options = options;
        }
    }

    pub fn apply(
        &mut self,
        action: Action,
        text: &str,
        needle: &str,
        replacement: &str,
        selection: Range<usize>,
    ) -> Result<Outcome, String> {
        if needle.is_empty() {
            return Err("Enter text or a regular expression to find".into());
        }
        let len = text.chars().count();
        if selection.start > selection.end || selection.end > len {
            return Err("Invalid search selection".into());
        }
        if self.document != text {
            self.last = None;
            self.document = text.to_owned();
            if self.options.selection_only && self.scope.take().is_some() {
                return Err("Document changed; select the search range again".into());
            }
        }
        if self.query != needle || self.previous_options != self.options {
            self.last = None;
        }
        self.query = needle.to_owned();
        self.previous_options = self.options;
        let scope = if self.options.selection_only {
            self.scope.get_or_insert_with(|| selection.clone()).clone()
        } else {
            self.scope = None;
            0..len
        };
        if self.options.selection_only && scope.is_empty() {
            self.scope = None;
            return Err("Select text in the SQL editor for Selection only".into());
        }
        // shortcut: backward overlap scans share a one-second UI budget; use a cancellable worker for large SQL documents.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        let navigation = !matches!(action, Action::FindAll | Action::ReplaceAll);
        let backwards = self.options.backwards ^ (action == Action::FindReverse);
        let mut anchor = selection.clone();
        if self.options.selection_only && selection == scope && self.last.is_none() {
            let edge = if backwards { scope.end } else { scope.start };
            anchor = edge..edge;
        }
        let (re, hits) = if navigation && !backwards {
            let repeat_empty = anchor.is_empty()
                && self.last.as_ref() == Some(&anchor)
                && (action != Action::Replace || self.last_replaced);
            let start = if action == Action::Replace && !repeat_empty {
                selection.start
            } else {
                anchor.end + usize::from(repeat_empty)
            };
            let mut search_scope = start.max(scope.start)..scope.end;
            let (mut re, mut hits) = scan(
                text,
                needle,
                self.options,
                &search_scope,
                (false, true),
                deadline,
            )?;
            if action == Action::Replace
                && hits
                    .first()
                    .is_some_and(|hit| hit.chars != selection && hit.chars.start < anchor.end)
            {
                search_scope.start = anchor.end.max(scope.start);
                (re, hits) = scan(
                    text,
                    needle,
                    self.options,
                    &search_scope,
                    (false, true),
                    deadline,
                )?;
            }
            if hits.is_empty() && self.options.wrap {
                (re, hits) = scan(text, needle, self.options, &scope, (false, true), deadline)?;
            }
            (re, hits)
        } else {
            scan(
                text,
                needle,
                self.options,
                &scope,
                (navigation, false),
                deadline,
            )?
        };
        let mut captures = re.capture_locations();
        let subject = &text[..byte_offset(text, scope.end)];
        let mut replace = |hit: &Hit| -> Result<String, String> {
            if !self.options.regex {
                return Ok(replacement.to_owned());
            }
            let found = re
                .captures_read_at(&mut captures, subject.as_bytes(), hit.offset)
                .map_err(|e| format!("Search failed: {e}"))?
                .ok_or("Search match disappeared")?;
            if found.start() != hit.bytes.start || found.end() != hit.bytes.end {
                return Err("Search match changed".into());
            }
            expand(replacement, subject, &captures)
        };
        let mut outcome = Outcome {
            selection: None,
            edit: None,
            highlights: Vec::new(),
            message: "Not found".into(),
        };
        if action == Action::FindAll {
            outcome.highlights = hits.iter().map(|hit| hit.chars.clone()).collect();
            outcome.message = format!("{} match(es)", hits.len());
            return Ok(outcome);
        }
        if hits.is_empty() {
            return Ok(outcome);
        }
        let edit = if action == Action::ReplaceAll {
            let bytes = byte_offset(text, scope.start)..byte_offset(text, scope.end);
            let mut output = String::new();
            let mut position = bytes.start;
            for hit in &hits {
                if std::time::Instant::now() >= deadline {
                    return Err("Search time limit exceeded; narrow the selection or simplify the expression".into());
                }
                output.push_str(&text[position..hit.bytes.start]);
                output.push_str(&replace(hit)?);
                position = hit.bytes.end;
            }
            output.push_str(&text[position..bytes.end]);
            outcome.message = format!("Replaced {}", hits.len());
            Some(Edit {
                range: scope.clone(),
                text: output,
            })
        } else {
            let selected = (action == Action::Replace)
                .then(|| {
                    hits.iter().position(|hit| {
                        hit.chars == selection
                            && (!selection.is_empty()
                                || (!self.last_replaced && self.last.as_ref() == Some(&selection)))
                    })
                })
                .flatten();
            let next = || {
                let backwards = self.options.backwards ^ (action == Action::FindReverse);
                let selection =
                    if self.options.selection_only && selection == scope && self.last.is_none() {
                        let edge = if backwards { scope.end } else { scope.start };
                        edge..edge
                    } else {
                        selection.clone()
                    };
                let eligible = |hit: &Hit| {
                    let repeated_empty = hit.chars.is_empty()
                        && self.last.as_ref() == Some(&hit.chars)
                        && hit.chars == selection;
                    !repeated_empty
                        && if backwards {
                            hit.chars.end <= selection.start
                        } else {
                            hit.chars.start >= selection.end
                        }
                };
                let found = if backwards {
                    hits.iter().rposition(eligible)
                } else {
                    hits.iter().position(eligible)
                };
                found.or_else(|| {
                    self.options
                        .wrap
                        .then_some(if backwards { hits.len() - 1 } else { 0 })
                })
            };
            let Some(index) = selected.or_else(next) else {
                return Ok(outcome);
            };
            let hit = &hits[index];
            self.last = Some(hit.chars.clone());
            self.last_replaced = false;
            outcome.message = if action == Action::Replace {
                "Replaced one".into()
            } else {
                "Match selected".into()
            };
            if matches!(action, Action::Find | Action::FindReverse) {
                outcome.selection = Some(hit.chars.clone());
                None
            } else {
                Some(Edit {
                    range: hit.chars.clone(),
                    text: replace(hit)?,
                })
            }
        };
        if let Some(edit) = edit {
            let inserted = edit.text.chars().count();
            if let Some(scope) = &mut self.scope {
                scope.end = scope.end - edit.range.len() + inserted;
            }
            self.document = text.to_owned();
            self.document.replace_range(
                byte_offset(text, edit.range.start)..byte_offset(text, edit.range.end),
                &edit.text,
            );
            let end = edit.range.start + inserted;
            self.last = Some(end..end);
            self.last_replaced = true;
            outcome.selection = if action == Action::ReplaceAll && self.options.selection_only {
                self.scope.clone()
            } else {
                Some(end..end)
            };
            outcome.edit = Some(edit);
        }
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        search: &mut Search,
        action: Action,
        text: &mut String,
        selection: &mut Range<usize>,
        needle: &str,
        replacement: &str,
    ) -> Result<Outcome, String> {
        let outcome = search.apply(action, text, needle, replacement, selection.clone())?;
        if let Some(edit) = &outcome.edit {
            text.replace_range(
                byte_offset(text, edit.range.start)..byte_offset(text, edit.range.end),
                &edit.text,
            );
        }
        if let Some(range) = &outcome.selection {
            *selection = range.clone();
        }
        Ok(outcome)
    }

    #[test]
    fn literal_case_word_and_metacharacters_keep_unicode_ranges() {
        let mut search = Search::default();
        let (mut text, mut selection) = ("αΒ αβx αβ_ αβ αβ".to_owned(), 0..0);
        let all = run(
            &mut search,
            Action::FindAll,
            &mut text,
            &mut selection,
            "αβ",
            "",
        )
        .unwrap();
        assert_eq!(all.highlights, [0..2, 3..5, 7..9, 11..13, 14..16]);
        search.options.whole_words = true;
        assert_eq!(
            run(
                &mut search,
                Action::FindAll,
                &mut text,
                &mut selection,
                "αβ",
                ""
            )
            .unwrap()
            .highlights,
            [0..2, 11..13, 14..16]
        );
        search.options.case_sensitive = true;
        assert_eq!(
            run(
                &mut search,
                Action::FindAll,
                &mut text,
                &mut selection,
                "αβ",
                ""
            )
            .unwrap()
            .highlights,
            [11..13, 14..16]
        );
        search.options.whole_words = false;
        text = r".[]\$*+?{}()^| 中文".into();
        assert_eq!(
            run(
                &mut search,
                Action::FindAll,
                &mut text,
                &mut selection,
                r".[]\$*+?{}()^|",
                ""
            )
            .unwrap()
            .highlights,
            std::iter::once(0..14).collect::<Vec<_>>()
        );
    }

    #[test]
    fn direction_wrap_and_reverse_do_not_change_document_or_options() {
        let mut search = Search::default();
        search.options.wrap = false;
        let (mut text, mut selection) = ("one ONE one".to_owned(), 0..0);
        for expected in [0..3, 4..7, 8..11] {
            run(
                &mut search,
                Action::Find,
                &mut text,
                &mut selection,
                "one",
                "",
            )
            .unwrap();
            assert_eq!(selection, expected);
        }
        assert_eq!(
            run(
                &mut search,
                Action::Find,
                &mut text,
                &mut selection,
                "one",
                ""
            )
            .unwrap()
            .message,
            "Not found"
        );
        run(
            &mut search,
            Action::FindReverse,
            &mut text,
            &mut selection,
            "one",
            "",
        )
        .unwrap();
        assert_eq!(selection, 4..7);
        assert!(!search.options.backwards);
        search.options.backwards = true;
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "one",
            "",
        )
        .unwrap();
        assert_eq!(selection, 0..3);
        search.options.wrap = true;
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "one",
            "",
        )
        .unwrap();
        assert_eq!(selection, 8..11);
        assert_eq!(text, "one ONE one");
    }

    #[test]
    fn caret_search_can_start_inside_overlapping_matches() {
        let mut search = Search::default();
        search.options.wrap = false;
        let (mut text, mut selection) = ("ababa".to_owned(), 1..1);
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "aba",
            "",
        )
        .unwrap();
        assert_eq!(selection, 2..5);
        selection = 5..5;
        search.options.backwards = true;
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "aba",
            "",
        )
        .unwrap();
        assert_eq!(selection, 2..5);
        run(
            &mut search,
            Action::Replace,
            &mut text,
            &mut selection,
            "aba",
            "X",
        )
        .unwrap();
        assert_eq!(text, "abX");
        text = "fooBAR".into();
        selection = 0..0;
        search.options.regex = true;
        run(
            &mut search,
            Action::ReplaceAll,
            &mut text,
            &mut selection,
            r"foo\KBAR",
            r"\0",
        )
        .unwrap();
        assert_eq!(text, "fooBAR");
    }

    #[test]
    fn selection_scope_survives_matches_and_own_edits_but_rejects_external_changes() {
        let mut search = Search::default();
        let (mut text, mut selection) = ("outside 中文 中文 outside".to_owned(), 8..13);
        let options = Options {
            selection_only: true,
            wrap: false,
            ..Options::default()
        };
        search.set_options(options, &text, selection.clone());
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "中文",
            "",
        )
        .unwrap();
        assert_eq!(selection, 8..10);
        run(
            &mut search,
            Action::Replace,
            &mut text,
            &mut selection,
            "中文",
            "東京🙂",
        )
        .unwrap();
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "中文",
            "",
        )
        .unwrap();
        assert_eq!(selection, 12..14);
        run(
            &mut search,
            Action::ReplaceAll,
            &mut text,
            &mut selection,
            "中文",
            "Z",
        )
        .unwrap();
        assert_eq!(text, "outside 東京🙂 Z outside");
        assert_eq!(selection, 8..13);
        assert!(run(
            &mut search,
            Action::FindAll,
            &mut text,
            &mut selection,
            "outside",
            ""
        )
        .unwrap()
        .highlights
        .is_empty());
        text.insert(0, '!');
        assert!(run(
            &mut search,
            Action::ReplaceAll,
            &mut text,
            &mut selection,
            "Z",
            "lost"
        )
        .err()
        .unwrap()
        .contains("Document changed"));
        assert!(!text.contains("lost"));
        search.set_options(Options::default(), &text, selection.clone());
        assert_eq!(
            run(
                &mut search,
                Action::FindAll,
                &mut text,
                &mut selection,
                "outside",
                ""
            )
            .unwrap()
            .highlights
            .len(),
            2
        );
        search.set_options(options, &text, 0..0);
        assert!(run(&mut search, Action::Find, &mut text, &mut (0..0), "Z", "").is_err());
    }

    #[test]
    fn regex_captures_replacements_and_multiline_anchors_are_atomic() {
        let mut search = Search::default();
        search.options.regex = true;
        let (mut text, mut selection) = ("前🙂 A=12\na=34 尾".to_owned(), 0..0);
        let result = run(
            &mut search,
            Action::ReplaceAll,
            &mut text,
            &mut selection,
            r"([A-Z])=(\d+)",
            r"\2:\1\t\0\\\q",
        )
        .unwrap();
        assert_eq!(result.message, "Replaced 2");
        assert_eq!(text, "前🙂 12:A\tA=12\\\\q\n34:a\ta=34\\\\q 尾");
        assert_eq!(
            run(
                &mut search,
                Action::FindAll,
                &mut text,
                &mut selection,
                r"^\d+",
                ""
            )
            .unwrap()
            .highlights
            .len(),
            1
        );
        search.options.regex = false;
        run(
            &mut search,
            Action::ReplaceAll,
            &mut text,
            &mut selection,
            "12",
            r"\1",
        )
        .unwrap();
        assert!(text.starts_with(r"前🙂 \1:A"));
    }

    #[test]
    fn zero_width_unicode_matches_advance_and_replace_without_loops() {
        let mut search = Search::default();
        search.options.regex = true;
        search.options.wrap = false;
        let (mut text, mut selection) = ("🙂中".to_owned(), 0..0);
        for expected in [0..0, 1..1] {
            run(
                &mut search,
                Action::Find,
                &mut text,
                &mut selection,
                "(?=.)",
                "",
            )
            .unwrap();
            assert_eq!(selection, expected);
        }
        assert_eq!(
            run(
                &mut search,
                Action::Find,
                &mut text,
                &mut selection,
                "(?=.)",
                ""
            )
            .unwrap()
            .message,
            "Not found"
        );
        let result = run(
            &mut search,
            Action::ReplaceAll,
            &mut text,
            &mut selection,
            "(?=.)|$",
            "X",
        )
        .unwrap();
        assert_eq!(result.message, "Replaced 3");
        assert_eq!(text, "X🙂X中X");
        text = "🙂中".into();
        selection = 2..2;
        search.reset(&text, selection.clone());
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "$",
            "",
        )
        .unwrap();
        assert_eq!(selection, 2..2);
        assert_eq!(
            run(
                &mut search,
                Action::Find,
                &mut text,
                &mut selection,
                "$",
                ""
            )
            .unwrap()
            .message,
            "Not found"
        );
        search.reset("🙂中", 0..0);
        text = "🙂中".into();
        selection = 0..0;
        run(
            &mut search,
            Action::Replace,
            &mut text,
            &mut selection,
            "(?=.)",
            "",
        )
        .unwrap();
        run(
            &mut search,
            Action::Replace,
            &mut text,
            &mut selection,
            "(?=.)",
            "",
        )
        .unwrap();
        assert_eq!(selection, 1..1);
    }

    #[test]
    fn invalid_patterns_limits_and_split_unicode_never_return_an_edit() {
        let mut search = Search::default();
        search.options.regex = true;
        let (mut text, mut selection) = ("🙂 中文".to_owned(), 0..0);
        for pattern in ["[", r"\C", ""] {
            assert!(run(
                &mut search,
                Action::ReplaceAll,
                &mut text,
                &mut selection,
                pattern,
                "lost"
            )
            .is_err());
            assert_eq!(text, "🙂 中文");
            assert_eq!(selection, 0..0);
        }
        text = format!("{}!", "a".repeat(30000));
        assert!(run(
            &mut search,
            Action::ReplaceAll,
            &mut text,
            &mut selection,
            "(a+)+$",
            "lost"
        )
        .is_err());
        assert!(!text.contains("lost"));
    }

    #[test]
    fn greedy_forward_navigation_does_not_enumerate_every_overlapping_suffix() {
        let mut search = Search::default();
        search.options.regex = true;
        let (mut text, mut selection) = ("a".repeat(200000), 0..0);
        run(
            &mut search,
            Action::Find,
            &mut text,
            &mut selection,
            "a+",
            "",
        )
        .unwrap();
        assert_eq!(selection, 0..200000);
        search.options.backwards = true;
        selection = 200000..200000;
        let error = run(
            &mut search,
            Action::Replace,
            &mut text,
            &mut selection,
            "a+",
            "lost",
        )
        .err()
        .unwrap();
        assert!(error.contains("time limit"));
        assert_eq!(text.len(), 200000);
        assert!(!text.contains("lost"));
    }

    #[test]
    fn selection_word_boundaries_use_neighbors_outside_the_scope() {
        let mut search = Search::default();
        let (mut text, mut selection) = ("xfoo foox foo".to_owned(), 1..4);
        search.set_options(
            Options {
                selection_only: true,
                whole_words: true,
                ..Options::default()
            },
            &text,
            selection.clone(),
        );
        assert!(run(
            &mut search,
            Action::FindAll,
            &mut text,
            &mut selection,
            "foo",
            ""
        )
        .unwrap()
        .highlights
        .is_empty());
        search.reset(&text, 5..8);
        assert!(run(
            &mut search,
            Action::FindAll,
            &mut text,
            &mut selection,
            "foo",
            ""
        )
        .unwrap()
        .highlights
        .is_empty());
        search.reset(&text, 10..13);
        assert_eq!(
            run(
                &mut search,
                Action::FindAll,
                &mut text,
                &mut selection,
                "foo",
                ""
            )
            .unwrap()
            .highlights,
            std::iter::once(10..13).collect::<Vec<_>>()
        );
    }
}
