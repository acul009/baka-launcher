//! Desktop Entry metadata and shell-free command expansion.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// A launchable desktop application's metadata and expanded argument vector.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Desktop file ID, including its `.desktop` suffix.
    pub id: String,
    /// Localized application name.
    pub name: String,
    /// Localized generic application category.
    pub generic_name: String,
    /// Localized search keywords.
    pub keywords: Vec<String>,
    /// Absolute icon path or installed icon name.
    pub icon: Option<String>,
    /// Program followed by arguments, ready for `Command`, without a shell.
    pub command: Vec<String>,
    /// Optional application working directory.
    pub directory: Option<PathBuf>,
    /// Whether the application requires a terminal emulator.
    pub terminal: bool,
    /// Executable required to make this entry available.
    pub try_exec: Option<String>,
    /// Desktop names on which this application may be shown.
    pub only_show_in: Vec<String>,
    /// Desktop names on which this application must not be shown.
    pub not_show_in: Vec<String>,
    /// Whether the application should be omitted from launcher menus.
    pub no_display: bool,
}

/// A malformed desktop entry cannot be launched safely.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// Required metadata is missing.
    #[error("desktop entry has no {0}")]
    Missing(&'static str),
    /// An escape sequence is not defined by the Desktop Entry format.
    #[error("invalid desktop entry escape: {0}")]
    Escape(char),
    /// An escape sequence is incomplete.
    #[error("desktop entry ends with an incomplete escape")]
    IncompleteEscape,
    /// A command quote is not closed.
    #[error("desktop command has an unclosed quote")]
    Quote,
    /// An invalid desktop command field code was used.
    #[error("unsupported desktop command field code: %{0}")]
    Field(char),
    /// A command ends with an incomplete field code.
    #[error("desktop command ends with an incomplete field code")]
    IncompleteField,
    /// A field code must occupy its own command argument.
    #[error("desktop command field %{0} must occupy a whole argument")]
    EmbeddedField(char),
    /// A command contains more than one file or URI field code.
    #[error("desktop command has multiple file or URI field codes")]
    MultipleFiles,
    /// The executable is empty or invalid.
    #[error("desktop command has an invalid executable")]
    Program,
}

impl Entry {
    /// Parses the main Desktop Entry group; hidden and non-application entries mask lower-priority files.
    pub fn parse(
        id: String,
        path: PathBuf,
        text: &str,
        locale: &str,
    ) -> Result<Option<Self>, Error> {
        let mut values = BTreeMap::new();
        let mut in_entry = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                if in_entry {
                    break;
                }
                in_entry = line == "[Desktop Entry]";
            } else if in_entry && let Some((key, value)) = line.split_once('=') {
                values.insert(key.trim(), value.trim());
            }
        }

        if values.get("Hidden") == Some(&"true") || values.get("Type") != Some(&"Application") {
            return Ok(None);
        }
        let name = localized(&values, "Name", locale).ok_or(Error::Missing("Name"))?;
        let name = unescape(name)?;
        if name.is_empty() {
            return Err(Error::Missing("Name"));
        }
        let generic_name = unescape(localized(&values, "GenericName", locale).unwrap_or(""))?;
        let keywords = list(localized(&values, "Keywords", locale).unwrap_or(""))?;
        let icon = values
            .get("Icon")
            .filter(|value| !value.is_empty())
            .map(|value| unescape(value))
            .transpose()?;
        let exec = values.get("Exec").ok_or(Error::Missing("Exec"))?;
        let tokens = tokenize(&unescape(exec)?)?;
        if tokens.first().is_none_or(|program| {
            program.is_empty() || program.contains('=') || program.replace("%%", "").contains('%')
        }) {
            return Err(Error::Program);
        }
        let mut command = Vec::new();
        let mut files = 0;
        for token in tokens {
            if token == "%i" {
                if let Some(icon) = &icon {
                    command.push("--icon".to_owned());
                    command.push(icon.clone());
                }
                continue;
            }
            if matches!(token.as_str(), "%f" | "%F" | "%u" | "%U") {
                files += 1;
                continue;
            }
            if matches!(token.as_str(), "%d" | "%D" | "%n" | "%N" | "%v" | "%m") {
                continue;
            }
            let mut expanded = String::new();
            let mut chars = token.chars();
            while let Some(character) = chars.next() {
                if character != '%' {
                    expanded.push(character);
                    continue;
                }
                match chars.next().ok_or(Error::IncompleteField)? {
                    '%' => expanded.push('%'),
                    'c' => expanded.push_str(&name),
                    'k' => expanded.push_str(&path.to_string_lossy()),
                    code @ ('i' | 'f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm') => {
                        return Err(Error::EmbeddedField(code));
                    }
                    code => return Err(Error::Field(code)),
                }
            }
            command.push(expanded);
        }
        if files > 1 {
            return Err(Error::MultipleFiles);
        }
        if command
            .first()
            .is_none_or(|program| program.is_empty() || program.contains('='))
        {
            return Err(Error::Program);
        }

        Ok(Some(Self {
            id,
            name,
            generic_name,
            keywords,
            icon,
            command,
            directory: values
                .get("Path")
                .filter(|value| !value.is_empty())
                .map(|value| unescape(value).map(PathBuf::from))
                .transpose()?,
            terminal: values.get("Terminal") == Some(&"true"),
            try_exec: values
                .get("TryExec")
                .filter(|value| !value.is_empty())
                .map(|value| unescape(value))
                .transpose()?,
            only_show_in: list(values.get("OnlyShowIn").copied().unwrap_or(""))?,
            not_show_in: list(values.get("NotShowIn").copied().unwrap_or(""))?,
            no_display: values.get("NoDisplay") == Some(&"true"),
        }))
    }

    /// Applies launcher visibility flags to the current desktop names.
    pub fn visible_on(&self, desktops: &[String]) -> bool {
        !self.no_display
            && (self.only_show_in.is_empty()
                || self.only_show_in.iter().any(|name| desktops.contains(name)))
            && !self.not_show_in.iter().any(|name| desktops.contains(name))
    }
}

fn localized<'a>(values: &BTreeMap<&str, &'a str>, key: &str, locale: &str) -> Option<&'a str> {
    // Encoding is irrelevant, but modifiers participate in locale fallback.
    let (locale, modifier) = locale.split_once('@').unwrap_or((locale, ""));
    let locale = locale.split('.').next().unwrap_or(locale);
    if !matches!(locale, "" | "C" | "POSIX") {
        let language = locale.split('_').next().unwrap_or(locale);
        let mut candidates = Vec::new();
        if !modifier.is_empty() {
            candidates.push(format!("{locale}@{modifier}"));
        }
        candidates.push(locale.to_owned());
        if language != locale {
            if !modifier.is_empty() {
                candidates.push(format!("{language}@{modifier}"));
            }
            candidates.push(language.to_owned());
        }
        for candidate in candidates {
            if let Some(value) = values.get(format!("{key}[{candidate}]").as_str()) {
                return Some(*value);
            }
        }
    }
    values.get(key).copied()
}

fn unescape(value: &str) -> Result<String, Error> {
    let mut decoded = String::new();
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        decoded.push(match chars.next().ok_or(Error::IncompleteEscape)? {
            's' => ' ',
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '\\' => '\\',
            code => return Err(Error::Escape(code)),
        });
    }
    Ok(decoded)
}

fn list(value: &str) -> Result<Vec<String>, Error> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        match character {
            ';' => {
                if !part.is_empty() {
                    parts.push(std::mem::take(&mut part));
                }
            }
            '\\' => match chars.next().ok_or(Error::IncompleteEscape)? {
                ';' => part.push(';'),
                's' => part.push(' '),
                'n' => part.push('\n'),
                't' => part.push('\t'),
                'r' => part.push('\r'),
                '\\' => part.push('\\'),
                code => return Err(Error::Escape(code)),
            },
            character => part.push(character),
        }
    }
    if !part.is_empty() {
        parts.push(part);
    }
    Ok(parts)
}

fn tokenize(value: &str) -> Result<Vec<String>, Error> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        match character {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' => {
                let next = chars.next().ok_or(Error::IncompleteEscape)?;
                if quoted && !matches!(next, '"' | '`' | '$' | '\\') {
                    token.push('\\');
                }
                token.push(next);
                started = true;
            }
            character if character.is_ascii_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut token));
                    started = false;
                }
            }
            character => {
                token.push(character);
                started = true;
            }
        }
    }
    if quoted {
        return Err(Error::Quote);
    }
    if started {
        tokens.push(token);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::{Entry, Error};
    use std::path::PathBuf;

    fn parse(extra: &str) -> Result<Option<Entry>, Error> {
        Entry::parse(
            "example.desktop".into(),
            PathBuf::from("/apps/example.desktop"),
            &format!("[Desktop Entry]\nType=Application\nName=Example App\n{extra}"),
            "en_US.UTF-8",
        )
    }

    #[test]
    fn quotes_and_empty_arguments_preserve_argv_boundaries() {
        let entry = parse("Exec=\"/opt/Example App/bin/run\" \"two words\" \"\" 'literal'\n")
            .unwrap()
            .unwrap();
        assert_eq!(
            entry.command,
            ["/opt/Example App/bin/run", "two words", "", "'literal'"]
        );
    }

    #[test]
    fn metadata_field_codes_are_single_arguments_and_files_are_omitted() {
        let entry = parse("Exec=app %c %k %i %% %U\nIcon=example-icon\n")
            .unwrap()
            .unwrap();
        assert_eq!(
            entry.command,
            [
                "app",
                "Example App",
                "/apps/example.desktop",
                "--icon",
                "example-icon",
                "%"
            ]
        );
        assert_eq!(parse("Exec=app %i %f").unwrap().unwrap().command, ["app"]);
    }

    #[test]
    fn two_escape_layers_preserve_quotes_backslashes_and_dollars() {
        let entry = parse(r#"Exec=app "a\\b" "say \\"hi\\"" "\\$HOME" "\\`literal\\`""#)
            .unwrap()
            .unwrap();
        assert_eq!(
            entry.command,
            ["app", r"a\b", "say \"hi\"", "$HOME", "`literal`"]
        );
    }

    #[test]
    fn malformed_commands_are_typed_errors() {
        for (exec, expected) in [
            ("app %x", Error::Field('x')),
            ("app %", Error::IncompleteField),
            ("app --file=%F", Error::EmbeddedField('F')),
            ("app %f %U", Error::MultipleFiles),
            ("app \"unclosed", Error::Quote),
            ("", Error::Program),
            ("FOO=bar app", Error::Program),
            ("%f app", Error::Program),
            ("%i app", Error::Program),
            ("%c", Error::Program),
        ] {
            assert_eq!(parse(&format!("Exec={exec}")).unwrap_err(), expected);
        }
    }

    #[test]
    fn action_sections_never_replace_the_main_command() {
        let entry = parse("Exec=main\n[Desktop Action New]\nExec=other\nName=Other\n")
            .unwrap()
            .unwrap();
        assert_eq!(entry.command, ["main"]);
        assert_eq!(entry.name, "Example App");
    }

    #[test]
    fn hidden_entries_mask_lower_precedence_without_other_metadata() {
        assert!(
            Entry::parse(
                "x.desktop".into(),
                PathBuf::new(),
                "[Desktop Entry]\nHidden=true",
                "C"
            )
            .unwrap()
            .is_none()
        );
        assert!(
            parse("Type=Link\nURL=https://example.org")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn localized_values_follow_country_and_modifier_fallback() {
        let text = "[Desktop Entry]\nType=Application\nName=Default\nName[sr]=Language\nName[sr@latin]=Modifier\nName[sr_RS]=Country\nName[sr_RS@latin]=Exact\nExec=app\nGenericName[sr]=Category\nKeywords[sr]=first;semi\\;colon;\n";
        let entry = Entry::parse("x".into(), PathBuf::new(), text, "sr_RS.UTF-8@latin")
            .unwrap()
            .unwrap();
        assert_eq!(entry.name, "Exact");
        assert_eq!(entry.generic_name, "Category");
        assert_eq!(entry.keywords, ["first", "semi;colon"]);
        let text = text.replace("Name[sr_RS@latin]=Exact\n", "");
        assert_eq!(
            Entry::parse("x".into(), PathBuf::new(), &text, "sr_RS.UTF-8@latin")
                .unwrap()
                .unwrap()
                .name,
            "Country"
        );
    }

    #[test]
    fn metadata_visibility_and_launch_options_are_preserved() {
        let entry = parse("Exec=app\nTerminal=true\nPath=/a\\sb\nTryExec=app\nOnlyShowIn=GNOME;COSMIC;\nNotShowIn=KDE;\n")
            .unwrap().unwrap();
        assert!(entry.terminal);
        assert_eq!(entry.directory, Some(PathBuf::from("/a b")));
        assert_eq!(entry.try_exec.as_deref(), Some("app"));
        assert!(entry.visible_on(&["COSMIC".into()]));
        assert!(!entry.visible_on(&["KDE".into()]));
        assert!(!entry.visible_on(&[]));
        assert!(
            !parse("Exec=app\nNoDisplay=true")
                .unwrap()
                .unwrap()
                .visible_on(&[])
        );
    }

    #[test]
    fn shell_syntax_is_only_literal_arguments() {
        let entry = parse("Exec=app \"$(touch /tmp/never)\" \";\" \"|\"")
            .unwrap()
            .unwrap();
        assert_eq!(entry.command, ["app", "$(touch /tmp/never)", ";", "|"]);
    }
}
