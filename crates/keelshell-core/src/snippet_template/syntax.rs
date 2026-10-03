//! Conservative simple-command parsing; no shell expansion or command evaluation.

use super::{
    MAX_SNIPPET_VARIABLES, Parameter, SnippetTemplateContext as Context,
    SnippetTemplateError as Error, unsupported, valid_name,
};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quote {
    None,
    Single,
    Double,
}

#[derive(Clone, Copy)]
enum Redirect {
    File,
    Descriptor,
}

struct Word {
    decoded: String,
    plain: bool,
    present: bool,
    assignment: bool,
    parameter: Option<(Range<usize>, String)>,
}

#[derive(Default)]
struct CommandState {
    started: bool,
    command: bool,
    continuation: bool,
    redirect: Option<Redirect>,
}

pub(super) fn compile(input: &str) -> Result<(Vec<String>, Vec<Parameter>), Error> {
    let mut variables = Vec::new();
    let mut parameters = Vec::new();
    let mut index = 0;
    let mut state = CommandState::default();
    while index < input.len() {
        let byte = input.as_bytes()[index];
        if matches!(byte, b' ' | b'\t') {
            index += 1;
            continue;
        }
        if byte == b'#' {
            let end = input[index..].find('\n').map_or(input.len(), |n| index + n);
            if let Some(at) = input[index..end]
                .find("{{")
                .or_else(|| input[index..end].find("}}"))
            {
                return Err(unsupported(index + at, Context::Comment));
            }
            index = end;
            continue;
        }
        if b"\n;|&<>".contains(&byte) {
            operator(input, &mut index, &mut state)?;
            continue;
        }
        let start = index;
        let word = word(input, &mut index)?;
        if !word.present {
            continue;
        }
        // An adjacent, unquoted IO_NUMBER belongs to its redirection, not to
        // the command position. This also preserves assignment-only commands.
        if word.plain
            && !word.decoded.is_empty()
            && word.decoded.bytes().all(|b| b.is_ascii_digit())
            && input
                .as_bytes()
                .get(index)
                .is_some_and(|b| b"<>".contains(b))
        {
            continue;
        }
        if let Some(redirect) = state.redirect.take() {
            if matches!(redirect, Redirect::Descriptor)
                && (word.parameter.is_some()
                    || (word.decoded != "-" && !word.decoded.bytes().all(|b| b.is_ascii_digit()))
                    || word.decoded.is_empty())
            {
                return Err(unsupported(start, Context::FileDescriptor));
            }
        } else if !state.command && !word.assignment {
            if word.plain && reserved(&word.decoded) {
                return Err(unsupported(start, Context::CompoundSyntax));
            }
            state.command = true;
        }
        state.started = true;
        state.continuation = false;
        if let Some((range, name)) = word.parameter {
            let variable = if let Some(index) = variables.iter().position(|prior| prior == &name) {
                index
            } else {
                if variables.len() == MAX_SNIPPET_VARIABLES {
                    return Err(Error::TooManyVariables);
                }
                variables.push(name);
                variables.len() - 1
            };
            parameters.push(Parameter { range, variable });
        }
    }
    if state.redirect.is_some() || state.continuation {
        return Err(unsupported(input.len(), Context::IncompleteSyntax));
    }
    Ok((variables, parameters))
}

fn operator(input: &str, index: &mut usize, state: &mut CommandState) -> Result<(), Error> {
    let rest = &input[*index..];
    if rest.starts_with("<<") {
        return Err(unsupported(*index, Context::HereDocument));
    }
    if [";;", ";&", "|&", "&>"]
        .iter()
        .any(|op| rest.starts_with(op))
    {
        return Err(unsupported(*index, Context::CompoundSyntax));
    }
    if state.redirect.is_some() {
        return Err(unsupported(*index, Context::IncompleteSyntax));
    }
    let byte = input.as_bytes()[*index];
    if matches!(byte, b'<' | b'>') {
        let (length, redirect) = if rest.starts_with("<&") || rest.starts_with(">&") {
            (2, Redirect::Descriptor)
        } else if [">>", ">|", "<>"].iter().any(|op| rest.starts_with(op)) {
            (2, Redirect::File)
        } else {
            (1, Redirect::File)
        };
        state.redirect = Some(redirect);
        *index += length;
    } else if byte == b'\n' {
        // POSIX permits line breaks between a pipeline/and-or operator and its
        // next command. Other line breaks terminate the preceding command.
        if !state.continuation {
            *state = CommandState::default();
        }
        *index += 1;
    } else {
        if !state.started {
            return Err(unsupported(*index, Context::IncompleteSyntax));
        }
        let paired = rest.starts_with("&&") || rest.starts_with("||");
        *state = CommandState {
            continuation: byte == b'|' || paired,
            ..CommandState::default()
        };
        *index += if paired { 2 } else { 1 };
    }
    Ok(())
}

fn word(input: &str, index: &mut usize) -> Result<Word, Error> {
    let start = *index;
    let mut quote = Quote::None;
    let mut result = Word {
        decoded: String::new(),
        plain: true,
        present: false,
        assignment: false,
        parameter: None,
    };
    while *index < input.len() {
        let rest = &input[*index..];
        let ch = rest
            .chars()
            .next()
            .ok_or_else(|| unsupported(*index, Context::IncompleteSyntax))?;
        if quote == Quote::None && " \t\n;|&<>".contains(ch) {
            break;
        }
        if rest.starts_with("{{") {
            if quote != Quote::None {
                return Err(unsupported(*index, Context::Quoted));
            }
            if result.parameter.is_some() {
                return Err(unsupported(*index, Context::WordFragment));
            }
            let end = rest
                .find("}}")
                .ok_or(Error::MalformedPlaceholder { offset: *index })?
                + *index
                + 2;
            let name = &input[*index + 2..end - 2];
            if !valid_name(name) {
                return Err(Error::InvalidName { offset: *index + 2 });
            }
            result.parameter = Some((*index..end, name.to_owned()));
            result.present = true;
            result.plain = false;
            *index = end;
            continue;
        }
        if rest.starts_with("}}") {
            return Err(Error::MalformedPlaceholder { offset: *index });
        }
        if (quote == Quote::None || quote == Quote::Single) && ch == '\'' {
            quote = if quote == Quote::Single {
                Quote::None
            } else {
                Quote::Single
            };
            result.plain = false;
            result.present = true;
            *index += 1;
            continue;
        }
        if (quote == Quote::None || quote == Quote::Double) && ch == '"' {
            quote = if quote == Quote::Double {
                Quote::None
            } else {
                Quote::Double
            };
            result.plain = false;
            result.present = true;
            *index += 1;
            continue;
        }
        if ch == '\\' && quote != Quote::Single {
            let next_offset = *index + 1;
            let next = input[next_offset..]
                .chars()
                .next()
                .ok_or_else(|| unsupported(*index, Context::IncompleteSyntax))?;
            if input[next_offset..].starts_with("{{") || input[next_offset..].starts_with("}}") {
                return Err(unsupported(next_offset, Context::Escaped));
            }
            if quote == Quote::None || matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                if next != '\n' {
                    result.decoded.push(next);
                    result.plain = false;
                    result.present = true;
                }
                *index = next_offset + next.len_utf8();
                continue;
            }
        }
        if quote != Quote::Single && matches!(ch, '$' | '`') {
            return Err(unsupported(*index, Context::Expansion));
        }
        if quote == Quote::None {
            if matches!(ch, '(' | ')' | '{' | '}') {
                return Err(unsupported(*index, Context::CompoundSyntax));
            }
            if matches!(ch, '*' | '?' | '[' | '!' | '~') {
                return Err(unsupported(*index, Context::Expansion));
            }
            if ch == '=' && shell_name(&input[start..*index].replace("\\\n", "")) {
                result.assignment = true;
            }
        }
        result.decoded.push(ch);
        result.present = true;
        *index += ch.len_utf8();
    }
    if quote != Quote::None {
        return Err(unsupported(*index, Context::IncompleteSyntax));
    }
    if let Some((range, _)) = &result.parameter {
        // An unquoted backslash-newline is removed before shell tokenization;
        // it does not concatenate literal data to either side of the marker.
        let prefix = input[start..range.start].replace("\\\n", "");
        let suffix = input[range.end..*index].replace("\\\n", "");
        let value_prefix = prefix
            .strip_suffix('=')
            .is_some_and(|prefix| shell_name(prefix) || long_option(prefix));
        if !suffix.is_empty() || (!prefix.is_empty() && !value_prefix) {
            return Err(unsupported(range.start, Context::WordFragment));
        }
    }
    Ok(result)
}

fn shell_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn long_option(value: &str) -> bool {
    value.strip_prefix("--").is_some_and(|name| {
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    })
}

fn reserved(word: &str) -> bool {
    matches!(
        word,
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
