use crate::tag::{NbtCompound, NbtList, NbtTag};
use std::{
    char,
    convert::{AsRef, TryFrom, TryInto},
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    iter::Peekable,
    mem,
    str::{self, Chars},
};

/// Parses the given string into an NBT tag compound.
pub fn parse<T: AsRef<str> + ?Sized>(string_nbt: &T) -> Result<NbtCompound, ParserError> {
    let mut tokens = Lexer::new(string_nbt.as_ref());
    let open_curly = tokens.assert_next(Token::OpenCurly)?;
    parse_compound_tag(&mut tokens, &open_curly)
}

// Parses the next value in the token stream
fn parse_next_value<'a>(tokens: &mut Lexer<'a>) -> Result<NbtTag, ParserError> {
    let token = tokens.next().transpose()?;
    parse_value(tokens, token)
}

// Parses a token into a value
fn parse_value<'a>(
    tokens: &mut Lexer<'a>,
    token: Option<TokenData>,
) -> Result<NbtTag, ParserError>
{
    match token {
        // Open curly brace indicates a compound tag is present
        Some(
            td
            @ TokenData {
                token: Token::OpenCurly,
                ..
            },
        ) => parse_compound_tag(tokens, &td).map(Into::into),

        // Open square brace indicates that some kind of list is present
        Some(
            td
            @
            TokenData {
                token: Token::OpenSquare,
                ..
            },
        ) => parse_list(tokens, &td),

        // Could be a value token or delimiter token
        Some(td @ _) => match td.into_tag() {
            Ok(tag) => Ok(tag),
            Err(td) => Err(ParserError::unexpected_token(
                tokens.raw,
                Some(&td),
                "value",
            )),
        },

        // We expected a value but ran out of data
        None => Err(ParserError::unexpected_eos("value")),
    }
}

// Parses a list, which can be either a generic tag list or vector of primitives
fn parse_list<'a>(tokens: &mut Lexer<'a>, open_square: &TokenData) -> Result<NbtTag, ParserError> {
    match tokens.next().transpose()? {
        // Empty list ('[]') with no type specifier is treates as an empty NBT tag list
        Some(TokenData {
            token: Token::ClosedSquare,
            ..
        }) => Ok(NbtList::new().into()),

        // A string as the first "element" can either be a type specifier such as in [I; 1, 2], or
        // a regular string in a tag list, such as in ['i', 'j', 'k'].
        Some(TokenData {
            token: Token::String(string),
            position,
            width,
        }) => {
            // Peek at the next token to see if it's a semicolon, which would indicate a primitive vector
            match tokens.peek() {
                // Parse as a primitive vector
                Some(Ok(TokenData {
                    token: Token::Semicolon,
                    ..
                })) => {
                    // Moves past the peeked semicolon
                    tokens.next();

                    // Determine the primitive type and parse it
                    match string.as_str() {
                        "b" | "B" => parse_prim_list::<i8>(tokens, open_square),
                        "i" | "I" => parse_prim_list::<i32>(tokens, open_square),
                        "l" | "L" => parse_prim_list::<i64>(tokens, open_square),
                        _ => Err(ParserError::unexpected_token_at(
                            tokens.raw,
                            position,
                            width,
                            "'B', 'I', or 'L'",
                        )),
                    }
                }

                // Parse as a tag list (token errors are delegated to this function)
                _ => parse_tag_list(tokens, open_square, NbtTag::StringModUtf8(string))
                    .map(Into::into),
            }
        }

        // Any other pattern is delegated to the general tag list parser
        td @ _ => {
            let first_element = parse_value(tokens, td)?;
            parse_tag_list(tokens, open_square, first_element).map(Into::into)
        }
    }
}

fn parse_prim_list<'a, T>(
    tokens: &mut Lexer<'a>,
    open_square: &TokenData,
) -> Result<NbtTag, ParserError>
where
    T: TryFrom<NbtTag>,
    NbtTag: From<Vec<T>>,
{
    // Get the first value or return an empty primitive vector
    let first_element = match tokens.next().transpose()? {
        Some(TokenData {
            token: Token::ClosedSquare,
            ..
        }) => return Ok(Vec::new().into()),
        td @ _ => parse_value(tokens, td)?,
    };

    // Parse the test of the tokens as a tag list
    let tag_list = parse_tag_list(tokens, open_square, first_element)?;

    // Convert the tag list into a primitive vector
    let mut list: Vec<T> = Vec::with_capacity(tag_list.as_ref().capacity());
    for element in tag_list.into_inner() {
        list.push(
            element
                .try_into()
                .map_err(|_| ParserError::non_homogenous_list(tokens.raw, open_square.position))?,
        );
    }

    Ok(list.into())
}

fn parse_tag_list<'a>(
    tokens: &mut Lexer<'a>,
    open_square: &TokenData,
    first_element: NbtTag,
) -> Result<NbtList, ParserError>
{
    // Construct the list and use the first element to determine the list's type
    let mut list = NbtList::new();
    let descrim = mem::discriminant(&first_element);
    list.add(first_element);

    loop {
        match tokens.next().transpose()? {
            // Finish off the list
            Some(TokenData {
                token: Token::ClosedSquare,
                ..
            }) => return Ok(list),

            // Indicates another value should be parsed
            Some(TokenData {
                token: Token::Comma,
                ..
            }) => {
                let element = parse_next_value(tokens)?;

                // Ensure type homogeneity
                if mem::discriminant(&element) != descrim {
                    return Err(ParserError::non_homogenous_list(
                        tokens.raw,
                        open_square.position,
                    ));
                }

                list.add(element);
            }

            // Some invalid token
            td @ _ =>
                return Err(ParserError::unexpected_token(
                    tokens.raw,
                    td.as_ref(),
                    "',' or ']'",
                )),
        }
    }
}

fn parse_compound_tag<'a>(
    tokens: &mut Lexer<'a>,
    open_curly: &TokenData,
) -> Result<NbtCompound, ParserError>
{
    let mut compound = NbtCompound::new();
    // Zero is used as a niche value so the first iteration of the loop runs correctly
    let mut comma: Option<usize> = Some(0);

    loop {
        match tokens.next().transpose()? {
            // Finish off the compound tag
            Some(TokenData {
                token: Token::ClosedCurly,
                ..
            }) => {
                match comma {
                    // First loop iteration or no comma
                    Some(0) | None => return Ok(compound),
                    // Later iteration with a trailing comma
                    Some(position) =>
                        return Err(ParserError::trailing_comma(tokens.raw, position)),
                }
            }

            // Parse a new key-value pair
            Some(TokenData {
                token: Token::String(key),
                position,
                width,
            }) => {
                match comma {
                    // Fir looper iteration or a comma indicated that more data is present
                    Some(_) => {
                        tokens.assert_next(Token::Colon)?;
                        compound.set(key, parse_next_value(tokens)?);
                        comma = None;
                    }

                    // There was not a comma before this string so therefore the token is unexpected
                    None =>
                        return Err(ParserError::unexpected_token_at(
                            tokens.raw,
                            position,
                            width,
                            Token::Comma.as_expectation(),
                        )),
                }
            }

            // Denote that another key-value pair is anticipated
            Some(TokenData {
                token: Token::Comma,
                position,
                ..
            }) => comma = Some(position),

            // Catch-all for unexpected tokens
            Some(td @ _) =>
                return Err(ParserError::unexpected_token(
                    tokens.raw,
                    Some(&td),
                    "compound key, '}', or ','",
                )),

            // End of file / unmatched brace
            None =>
                return Err(ParserError::unmatched_brace(
                    tokens.raw,
                    open_curly.position,
                )),
        }
    }
}

pub(crate) struct Lexer<'a> {
    raw: &'a str,
    chars: Peekable<Chars<'a>>,
    position: usize,
    raw_token_buffer: String,
    errored: bool,
    peeked: Option<Option<<Self as Iterator>::Item>>,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(raw: &'a str) -> Self {
        Lexer {
            raw,
            chars: raw.chars().peekable(),
            position: 0,
            raw_token_buffer: String::with_capacity(16),
            errored: false,
            peeked: None,
        }
    }

    fn peek(&mut self) -> Option<&<Self as Iterator>::Item> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next());
        }

        self.peeked.as_ref().unwrap().as_ref()
    }

    // Asserts that the next token is the same type as the provided token
    fn assert_next(&mut self, token: Token) -> Result<TokenData, ParserError> {
        match self.next().transpose()? {
            // We found a token so check the token type
            Some(td) =>
                if mem::discriminant(&td.token) == mem::discriminant(&token) {
                    Ok(td)
                } else {
                    Err(ParserError::unexpected_token(
                        self.raw,
                        Some(&td),
                        token.as_expectation(),
                    ))
                },

            // No tokens were left so return an unexpected end of string error
            None => Err(ParserError::unexpected_eos(token.as_expectation())),
        }
    }

    // Collects a token from the character iterator
    fn slurp_token(&mut self) -> Result<TokenData, ParserError> {
        let start = self.position;
        // Last non-whitespace character position
        let mut last_nws_char_pos = start;

        // State of the token slurper
        #[derive(PartialEq, Eq)]
        enum State {
            FirstChar,
            Unquoted,
            InSingleQuotes,
            InDoubleQuotes,
            Finalized,
        }
        let mut state: State = State::FirstChar;
        // If this flag is set to true, then the token is a string in quotes
        let mut quoted = false;

        loop {
            match state {
                // The first character determines how the width of the token is determined
                State::FirstChar => match self.chars.next() {
                    Some('\'') => {
                        state = State::InSingleQuotes;
                        quoted = true;
                    }
                    Some('"') => {
                        state = State::InDoubleQuotes;
                        quoted = true;
                    }
                    Some(ch @ _) => {
                        state = State::Unquoted;
                        self.raw_token_buffer.push(ch)
                    }
                    None => unreachable!(),
                },

                // Unquoted text, which includes numbers and strings
                State::Unquoted => match self.chars.peek() {
                    Some('{' | '}' | '[' | ']' | ',' | ':' | ';') | None => {
                        self.raw_token_buffer.truncate(
                            self.raw_token_buffer.len() - (self.position - last_nws_char_pos) + 1,
                        );
                        state = State::Finalized;
                        continue;
                    }
                    Some('\'' | '"') => {
                        self.errored = true;
                        return Err(ParserError::unexpected_quote(self.raw, self.position));
                    }
                    Some(&ch @ _) => {
                        self.raw_token_buffer.push(ch);
                        if !ch.is_ascii_whitespace() {
                            last_nws_char_pos = self.position;
                        }
                        self.chars.next();
                    }
                },

                // Handle quotes strings
                State::InSingleQuotes | State::InDoubleQuotes => match self.chars.next() {
                    Some('\\') => {
                        // Handle escape characters
                        match self.chars.next() {
                            // These are just directly quoted
                            Some(ch @ ('\'' | '"' | '\\')) => self.raw_token_buffer.push(ch),

                            // Convert to the rust equivalent
                            Some('n') => self.raw_token_buffer.push('\n'),
                            Some('r') => self.raw_token_buffer.push('\r'),
                            Some('t') => self.raw_token_buffer.push('\t'),

                            // Parse a unicode escape sequence
                            Some('u') => {
                                let mut buffer = [0u8; 4];
                                for ch in buffer.iter_mut() {
                                    *ch = (self.chars.next().ok_or(ParserError::unexpected_eos(
                                        "four-character hex unicode value",
                                    ))? as u8)
                                        & 0x7F;
                                }

                                let ch = str::from_utf8(buffer.as_ref())
                                    .ok()
                                    .map(|string| u32::from_str_radix(string, 16).ok())
                                    .flatten()
                                    .map(|n| char::from_u32(n))
                                    .flatten()
                                    .ok_or(ParserError::unknown_escape_sequence(
                                        self.raw,
                                        self.position,
                                        6,
                                    ))?;

                                self.raw_token_buffer.push(ch);
                                self.position += 4;
                            }

                            // Unknown sequence
                            Some(_) => {
                                self.errored = true;
                                return Err(ParserError::unknown_escape_sequence(
                                    self.raw,
                                    self.position,
                                    2,
                                ));
                            }

                            // Unexpected end of string / unmatched quotation
                            None => {
                                self.errored = true;
                                return Err(ParserError::unmatched_quote(self.raw, start));
                            }
                        }

                        self.position += 1;
                    }

                    // Close off the string if the quote type matches
                    Some('\'') =>
                        if state == State::InSingleQuotes {
                            state = State::Finalized;
                        } else {
                            self.raw_token_buffer.push('\'');
                        },
                    Some('"') =>
                        if state == State::InDoubleQuotes {
                            state = State::Finalized;
                        } else {
                            self.raw_token_buffer.push('"');
                        },

                    // Directly quote a character
                    Some(ch @ _) => self.raw_token_buffer.push(ch),

                    // Unexpected end of string / unmatched quotation
                    None => {
                        self.errored = true;
                        return Err(ParserError::unmatched_quote(self.raw, start));
                    }
                },

                // Once the token is isolated, parse it
                State::Finalized => return self.parse_token(start, quoted),
            }

            self.position += 1;
        }
    }

    // Parses an isolated token
    fn parse_token(&mut self, start: usize, quoted: bool) -> Result<TokenData, ParserError> {
        // Copy the token string for easier handling
        let token_string = self.raw_token_buffer.clone();
        self.raw_token_buffer.clear();

        // Get the first and last characters
        let first = match token_string.chars().next() {
            Some(ch) => ch,

            // Only strings can be empty tokens
            None => return Ok(TokenData::new(Token::String(token_string), start, 2)),
        };
        let last = token_string.chars().rev().next().unwrap();

        // Identify if the token is not a number (a string)
        if !(first == '-' || (first.is_ascii() && first.is_numeric())) {
            let width = token_string.len() + if quoted { 2 } else { 0 };
            return Ok(TokenData::new(Token::String(token_string), start, width));
        }

        let width = token_string.len();

        // Determine whether to parse as an integer or decimal
        if token_string.contains('.') {
            // Parse with highest precision ignoring the type suffix
            let value: Option<f64> = match last {
                'f' | 'F' | 'd' | 'D' => token_string[.. token_string.len() - 1].parse().ok(),
                _ =>
                    if last.is_numeric() {
                        token_string.parse().ok()
                    } else {
                        None
                    },
            };

            // Apply the type suffix if it is valid
            match value {
                Some(value) => match last {
                    'f' | 'F' =>
                        return Ok(TokenData::new(Token::Float(value as f32), start, width)),
                    _ => return Ok(TokenData::new(Token::Double(value), start, width)),
                },
                _ => {}
            }

            self.errored = true;
            return Err(ParserError::invalid_number(self.raw, start, width));
        } else {
            // Parse with highest precision ignoring the type suffix
            let value: Option<i64> = match last {
                'b' | 'B' | 's' | 'S' | 'l' | 'L' =>
                    token_string[.. token_string.len() - 1].parse().ok(),
                _ =>
                    if last.is_numeric() {
                        token_string.parse().ok()
                    } else {
                        None
                    },
            };

            // Apply the type suffix if it is valid
            match value {
                Some(value) => match last {
                    'b' | 'B' => return Ok(TokenData::new(Token::Byte(value as i8), start, width)),
                    's' | 'S' =>
                        return Ok(TokenData::new(Token::Short(value as i16), start, width)),
                    'l' | 'L' => return Ok(TokenData::new(Token::Long(value), start, width)),
                    'f' | 'F' =>
                        return Ok(TokenData::new(Token::Float(value as f32), start, width)),
                    'd' | 'D' =>
                        return Ok(TokenData::new(Token::Double(value as f64), start, width)),
                    _ => return Ok(TokenData::new(Token::Int(value as i32), start, width)),
                },
                _ => {}
            }

            self.errored = true;
            return Err(ParserError::invalid_number(self.raw, start, width));
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<TokenData, ParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we encountered an error then cancel the token provision
        if self.errored {
            return None;
        }

        // Manage the peeking function
        match self.peeked.take() {
            Some(item) => {
                self.position += 1;
                return item;
            }
            None => {}
        };

        // Skip whitespace
        while self.chars.peek()?.is_ascii_whitespace() {
            self.chars.next();
            self.position += 1;
        }

        // Manage single-char tokens and pass multichar tokens to a designated function
        let tk = match self.chars.peek()? {
            '{' => Some(Ok(TokenData::new(Token::OpenCurly, self.position,1 ))),
            '}' => Some(Ok(TokenData::new(Token::ClosedCurly, self.position, 1))),
            '[' => Some(Ok(TokenData::new(Token::OpenSquare, self.position, 1))),
            ']' => Some(Ok(TokenData::new(Token::ClosedSquare, self.position, 1))),
            ',' => Some(Ok(TokenData::new(Token::Comma, self.position, 1))),
            ':' => Some(Ok(TokenData::new(Token::Colon, self.position, 1))),
            ';' => Some(Ok(TokenData::new(Token::Semicolon, self.position, 1))),
            _ => return Some(self.slurp_token()),
        };

        self.chars.next();
        self.position += 1;
        tk
    }
}

#[derive(Debug)]
pub(crate) struct TokenData {
    token: Token,
    position: usize,
    width: usize,
}

impl TokenData {
    fn new(token: Token, position: usize, width: usize) -> Self {
        TokenData {
            token,
            position,
            width,
        }
    }

    fn into_tag(self) -> Result<NbtTag, Self> {
        match self.token.into_tag() {
            Ok(tag) => Ok(tag),
            Err(tk) => Err(Self::new(tk, self.position, self.width)),
        }
    }
}

#[derive(Debug)]
enum Token {
    OpenCurly,
    ClosedCurly,
    OpenSquare,
    ClosedSquare,
    Comma,
    Colon,
    Semicolon,
    String(String),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl Token {
    fn as_expectation(&self) -> &'static str {
        match self {
            Token::OpenCurly => "'{'",
            Token::ClosedCurly => "'}'",
            Token::OpenSquare => "'['",
            Token::ClosedSquare => "']'",
            Token::Comma => "','",
            Token::Colon => "':'",
            Token::Semicolon => "';'",
            _ => "value",
        }
    }

    fn into_tag(self) -> Result<NbtTag, Self> {
        match self {
            Token::String(value) => Ok(NbtTag::StringModUtf8(value)),
            Token::Byte(value) => Ok(NbtTag::Byte(value)),
            Token::Short(value) => Ok(NbtTag::Short(value)),
            Token::Int(value) => Ok(NbtTag::Int(value)),
            Token::Long(value) => Ok(NbtTag::Long(value)),
            Token::Float(value) => Ok(NbtTag::Float(value)),
            Token::Double(value) => Ok(NbtTag::Double(value)),
            tk @ _ => Err(tk),
        }
    }
}

/// An error that occurs during the parsing process.
pub struct ParserError {
    segment: String,
    error: ParserErrorType,
}

impl ParserError {
    fn unmatched_quote(input: &str, position: usize) -> Self {
        ParserError {
            segment: Self::segment(input, position, 1, 7, 7),
            error: ParserErrorType::UnmatchedQuote { position },
        }
    }

    fn unexpected_quote(input: &str, position: usize) -> Self {
        ParserError {
            segment: Self::segment(input, position, 1, 7, 7),
            error: ParserErrorType::UnexpectedQuote { position },
        }
    }

    fn unknown_escape_sequence(input: &str, position: usize, width: usize) -> Self {
        ParserError {
            segment: Self::segment(input, position, width, 0, 0),
            error: ParserErrorType::UnknownEscapeSequence,
        }
    }

    fn invalid_number(input: &str, position: usize, width: usize) -> Self {
        ParserError {
            segment: Self::segment(input, position, width, 0, 0),
            error: ParserErrorType::InvalidNumber,
        }
    }

    fn unexpected_token(input: &str, token: Option<&TokenData>, expected: &'static str) -> Self {
        match token {
            Some(token) => Self::unexpected_token_at(input, token.position, token.width, expected),
            None => Self::unexpected_eos(expected),
        }
    }

    fn unexpected_token_at(
        input: &str,
        position: usize,
        width: usize,
        expected: &'static str,
    ) -> Self
    {
        ParserError {
            segment: Self::segment(input, position, width, 15, 0),
            error: ParserErrorType::UnexpectedToken { position, expected },
        }
    }

    fn unexpected_eos(expected: &'static str) -> Self {
        ParserError {
            segment: String::new(),
            error: ParserErrorType::UnexpectedEOS { expected },
        }
    }

    fn trailing_comma(input: &str, position: usize) -> Self {
        ParserError {
            segment: Self::segment(input, position, 1, 15, 1),
            error: ParserErrorType::TrailingComma { position },
        }
    }

    fn unmatched_brace(input: &str, position: usize) -> Self {
        ParserError {
            segment: Self::segment(input, position, 1, 0, 15),
            error: ParserErrorType::UnmatchedBrace { position },
        }
    }

    fn non_homogenous_list(input: &str, position: usize) -> Self {
        ParserError {
            segment: Self::segment(input, position, 1, 0, 15),
            error: ParserErrorType::NonHomogenousList { position },
        }
    }

    fn segment(input: &str, index: usize, width: usize, before: usize, after: usize) -> String {
        input[index - usize::min(before, index)
            .. usize::min(index + width.min(20) + after, input.len())]
            .to_owned()
    }
}

impl Display for ParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.error {
            &ParserErrorType::UnmatchedQuote { position } => write!(
                f,
                "Unmatched quote: column {} near '{}'",
                position, self.segment
            ),
            &ParserErrorType::UnexpectedQuote { position } => write!(
                f,
                "Unexpected quote: column {} near '{}'",
                position, self.segment
            ),
            &ParserErrorType::UnknownEscapeSequence =>
                write!(f, "Unknown escape sequence: '{}'", self.segment),
            &ParserErrorType::InvalidNumber => write!(f, "Invalid number: {}", self.segment),
            &ParserErrorType::UnexpectedToken { position, expected } => write!(
                f,
                "Unexpected token at column {}, expected {}: '{}'",
                position, expected, self.segment
            ),
            &ParserErrorType::UnexpectedEOS { expected } =>
                write!(f, "Reached end of input but expected {}", expected),
            &ParserErrorType::TrailingComma { position } => write!(
                f,
                "Trailing comma at column {}: '{}'",
                position, self.segment
            ),
            &ParserErrorType::UnmatchedBrace { position } => write!(
                f,
                "Unmatched brace at column {} near '{}'",
                position, self.segment
            ),
            &ParserErrorType::NonHomogenousList { position } => write!(
                f,
                "Non-homogenous typed list at column {} near '{}'",
                position, self.segment
            ),
        }
    }
}

impl Debug for ParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.error, f)
    }
}

impl Error for ParserError {}

#[derive(Clone, Debug)]
enum ParserErrorType {
    UnmatchedQuote {
        position: usize,
    },
    UnexpectedQuote {
        position: usize,
    },
    UnknownEscapeSequence,
    InvalidNumber,
    UnexpectedToken {
        position: usize,
        expected: &'static str,
    },
    UnexpectedEOS {
        expected: &'static str,
    },
    TrailingComma {
        position: usize,
    },
    UnmatchedBrace {
        position: usize,
    },
    NonHomogenousList {
        position: usize,
    },
}
