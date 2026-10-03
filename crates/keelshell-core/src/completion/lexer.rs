//! Literal POSIX words and a deliberately bounded simple-command grammar.

use super::{CompletionUnsupported as Unsupported, WordQuery, safe_char};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quote {
    None,
    Single,
    Double,
}

struct Word {
    value: String,
    caret: Option<usize>,
    plain: bool,
    assignment: Option<(usize, usize)>,
    option_value: Option<(usize, usize)>,
    present: bool,
}

#[derive(Clone, Copy)]
enum Redirect {
    File,
    Descriptor,
}

enum TokenKind {
    Word(Word),
    Redirect(Redirect),
    Separator { newline: bool },
}

struct Token {
    range: Range<usize>,
    kind: TokenKind,
}

#[derive(Default)]
struct Context {
    command: Option<String>,
    redirect: Option<Redirect>,
    started: bool,
}

pub(super) fn analyze(input: &str, caret: usize) -> Result<(Range<usize>, WordQuery), Unsupported> {
    let tokens = tokenize(input, caret)?;
    let active = tokens.iter().position(|token| {
        matches!(token.kind, TokenKind::Word(_))
            && token.range.start <= caret
            && caret <= token.range.end
    });
    let mut context = Context::default();
    let mut result = None;
    for (index, token) in tokens.iter().enumerate() {
        if result.is_none() && active.is_none() && token.range.start >= caret {
            result = Some(empty_plan(caret, &context)?);
        }
        match &token.kind {
            TokenKind::Word(word) => {
                let is_fd = word.plain
                    && word.value.bytes().all(|b| b.is_ascii_digit())
                    && !word.value.is_empty()
                    && tokens.get(index + 1).is_some_and(|next| {
                        next.range.start == token.range.end
                            && matches!(next.kind, TokenKind::Redirect(_))
                    });
                if is_fd {
                    if active == Some(index) {
                        return Err(Unsupported::FileDescriptor);
                    }
                    continue;
                }
                if word.plain
                    && context.command.is_none()
                    && context.redirect.is_none()
                    && is_reserved(&word.value)
                {
                    return Err(Unsupported::ComplexSyntax);
                }
                if active == Some(index) {
                    result = Some(word_plan(token, word, &context)?);
                }
                let redirect_operand = context.redirect.take().is_some();
                if !redirect_operand && context.command.is_none() && word.assignment.is_none() {
                    context.command = Some(word.value.clone());
                }
                context.started = true;
            }
            TokenKind::Redirect(kind) => {
                if token.range.start < caret && caret < token.range.end {
                    return Err(Unsupported::Grammar);
                }
                if context.redirect.is_some() {
                    return Err(Unsupported::Grammar);
                }
                context.redirect = Some(*kind);
                context.started = true;
            }
            TokenKind::Separator { newline } => {
                if token.range.start < caret && caret < token.range.end {
                    return Err(Unsupported::Grammar);
                }
                if context.redirect.is_some() || (!newline && !context.started) {
                    return Err(Unsupported::Grammar);
                }
                context = Context::default();
            }
        }
    }
    result.map_or_else(|| empty_plan(caret, &context), Ok)
}

fn empty_plan(caret: usize, context: &Context) -> Result<(Range<usize>, WordQuery), Unsupported> {
    if matches!(context.redirect, Some(Redirect::Descriptor)) {
        return Err(Unsupported::FileDescriptor);
    }
    let word = if context.command.is_none() && context.redirect.is_none() {
        WordQuery::Commands {
            prefix: String::new(),
            suffix: String::new(),
        }
    } else {
        path_query(
            "",
            0,
            context.redirect.is_none() && context.command.as_deref() == Some("cd"),
        )
    };
    Ok((caret..caret, word))
}

fn word_plan(
    token: &Token,
    word: &Word,
    context: &Context,
) -> Result<(Range<usize>, WordQuery), Unsupported> {
    if matches!(context.redirect, Some(Redirect::Descriptor)) {
        return Err(Unsupported::FileDescriptor);
    }
    let mut range = token.range.clone();
    let mut value = word.value.as_str();
    let mut offset = word.caret.ok_or(Unsupported::EscapeBoundary)?;
    let value_start = match (context.redirect, context.command.as_ref()) {
        // Assignment syntax only precedes a command. Afterwards `a=file` is
        // one ordinary argument, unless the command's own grammar says otherwise.
        (None, None) => word.assignment,
        (None, Some(_)) => word.option_value,
        (Some(_), _) => None,
    };
    if let Some((raw_start, decoded_start)) = value_start {
        if offset < decoded_start {
            return Err(Unsupported::AssignmentName);
        }
        range.start = raw_start;
        value = &value[decoded_start..];
        offset -= decoded_start;
    }
    if !value.chars().all(safe_char) {
        return Err(Unsupported::ControlCharacter);
    }
    let command = context.command.is_none()
        && context.redirect.is_none()
        && word.assignment.is_none()
        && !value.contains('/');
    let query = if command {
        WordQuery::Commands {
            prefix: value[..offset].to_owned(),
            suffix: value[offset..].to_owned(),
        }
    } else {
        path_query(
            value,
            offset,
            context.redirect.is_none() && context.command.as_deref() == Some("cd"),
        )
    };
    Ok((range, query))
}

fn path_query(value: &str, caret: usize, directories_only: bool) -> WordQuery {
    let start = value[..caret].rfind('/').map_or(0, |index| index + 1);
    let end = value[caret..]
        .find('/')
        .map_or(value.len(), |index| caret + index);
    WordQuery::Paths {
        directory: value[..start].to_owned(),
        prefix: value[start..caret].to_owned(),
        suffix: value[caret..end].to_owned(),
        tail: value[end..].to_owned(),
        directories_only: directories_only || end < value.len(),
    }
}

fn tokenize(input: &str, caret: usize) -> Result<Vec<Token>, Unsupported> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < input.len() {
        let byte = input.as_bytes()[index];
        if matches!(byte, b' ' | b'\t') {
            index += 1;
            continue;
        }
        if byte == b'#' {
            let end = input[index..].find('\n').map_or(input.len(), |n| index + n);
            if (index..=end).contains(&caret) {
                return Err(Unsupported::Comment);
            }
            index = end;
            continue;
        }
        if b"\n;|&<>".contains(&byte) {
            let start = index;
            let rest = &input[index..];
            if ["<<", "<<<", ";;", ";&", "|&", "&>"]
                .iter()
                .any(|op| rest.starts_with(op))
            {
                return Err(Unsupported::ComplexSyntax);
            }
            let (len, kind) = if rest.starts_with("&&") || rest.starts_with("||") {
                (2, TokenKind::Separator { newline: false })
            } else if rest.starts_with("<&") || rest.starts_with(">&") {
                (2, TokenKind::Redirect(Redirect::Descriptor))
            } else if [">>", ">|", "<>"].iter().any(|op| rest.starts_with(op)) {
                (2, TokenKind::Redirect(Redirect::File))
            } else if matches!(byte, b'<' | b'>') {
                (1, TokenKind::Redirect(Redirect::File))
            } else {
                (
                    1,
                    TokenKind::Separator {
                        newline: byte == b'\n',
                    },
                )
            };
            index += len;
            tokens.push(Token {
                range: start..index,
                kind,
            });
            continue;
        }
        let start = index;
        let word = read_word(input, caret, &mut index)?;
        if word.present {
            tokens.push(Token {
                range: start..index,
                kind: TokenKind::Word(word),
            });
        }
    }
    Ok(tokens)
}

fn read_word(input: &str, caret: usize, index: &mut usize) -> Result<Word, Unsupported> {
    let start = *index;
    let mut word = Word {
        value: String::new(),
        caret: None,
        plain: true,
        assignment: None,
        option_value: None,
        present: false,
    };
    let mut quote = Quote::None;
    while *index < input.len() {
        if *index == caret {
            word.caret = Some(word.value.len());
        }
        let ch = input[*index..].chars().next().ok_or(Unsupported::Grammar)?;
        if quote == Quote::None && " \t\n;|&<>".contains(ch) {
            break;
        }
        if !safe_char(ch) && !matches!(ch, '\n' | '\t') {
            return Err(Unsupported::ControlCharacter);
        }
        if (quote == Quote::None && ch == '\'') || (quote == Quote::Single && ch == '\'') {
            quote = if quote == Quote::Single {
                Quote::None
            } else {
                Quote::Single
            };
            word.plain = false;
            word.present = true;
            *index += 1;
            continue;
        }
        if (quote == Quote::None && ch == '"') || (quote == Quote::Double && ch == '"') {
            quote = if quote == Quote::Double {
                Quote::None
            } else {
                Quote::Double
            };
            word.plain = false;
            word.present = true;
            *index += 1;
            continue;
        }
        if ch == '\\' && quote != Quote::Single {
            let next_offset = *index + 1;
            let next = input[next_offset..]
                .chars()
                .next()
                .ok_or(Unsupported::EscapeBoundary)?;
            if !safe_char(next) && !matches!(next, '\n' | '\t') {
                return Err(Unsupported::ControlCharacter);
            }
            if quote == Quote::None || matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                if caret == next_offset {
                    return Err(Unsupported::EscapeBoundary);
                }
                if next != '\n' {
                    word.value.push(next);
                    word.present = true;
                }
                *index = next_offset + next.len_utf8();
                word.plain = false;
                continue;
            }
        }
        if quote != Quote::Single && matches!(ch, '$' | '`') {
            return Err(Unsupported::Expansion);
        }
        if quote == Quote::None {
            if matches!(ch, '(' | ')' | '{' | '}') {
                return Err(Unsupported::ComplexSyntax);
            }
            if matches!(ch, '*' | '?' | '[' | '!')
                || (ch == '~'
                    && (*index == start || word.assignment.is_some_and(|(raw, _)| raw == *index)))
            {
                return Err(Unsupported::Expansion);
            }
            if ch == '=' && is_name(&input[start..*index]) {
                word.assignment = Some((*index + 1, word.value.len() + 1));
            } else if ch == '=' && is_long_option(&input[start..*index]) {
                word.option_value = Some((*index + 1, word.value.len() + 1));
            }
        }
        word.value.push(ch);
        word.present = true;
        *index += ch.len_utf8();
    }
    if *index == caret {
        word.caret = Some(word.value.len());
    }
    if quote != Quote::None && caret != input.len() {
        return Err(Unsupported::IncompleteQuote);
    }
    Ok(word)
}

fn is_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn is_long_option(value: &str) -> bool {
    value.strip_prefix("--").is_some_and(|name| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    })
}

fn is_reserved(value: &str) -> bool {
    matches!(
        value,
        "if" | "then"
            | "else"
            | "elif"
            | "fi"
            | "for"
            | "while"
            | "until"
            | "do"
            | "done"
            | "case"
            | "esac"
            | "in"
            | "function"
            | "select"
            | "time"
            | "coproc"
            | "[["
            | "]]"
    )
}
