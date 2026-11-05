//! Mermaid's token types for a use case diagram, in the order that makes its token categories
//! ranges: `WORD` is `Usecase` to `Identifier`, and `LABEL_TEXT` is `Usecase` to `LabelSymbol`.

/// The token kinds of Mermaid's use case lexer. The order is the one of Mermaid's token list,
/// which its categories are ranges of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub(crate) enum T {
    Eof,
    Usecase,
    Actor,
    SystemBoundary,
    End,
    Direction,
    Td,
    Tb,
    Bt,
    Lr,
    Rl,
    Note,
    For,
    Json,
    ClassDef,
    Class,
    Style,
    Include,
    Extend,
    True,
    False,
    Identifier,
    Number,
    At,
    Comma,
    Colon,
    HashColor,
    CssIdentifier,
    CssEscapedComma,
    Dash,
    Dot,
    Percent,
    CssPunctuation,
    LabelPunctuation,
    LabelSymbol,
    PlainString,
    MarkdownString,
    UnclosedMarkdownString,
    Comment,
    Newline,
    AccDescrBlock,
    AccTitleLine,
    AccDescrLine,
    JsonDeclarationStart,
    JsonObjectLiteral,
    UnclosedJsonObjectLiteral,
    Generalization,
    DependencyArrow,
    StereotypeStart,
    StereotypeEnd,
    StereotypeText,
    UnclosedStereotypeText,
    ClassSeparator,
    ForwardSolid,
    BackwardSolid,
    ForwardCircle,
    BackwardCircle,
    ForwardCross,
    BackwardCross,
    MarkerlessSolid,
    MetadataStart,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
}

impl T {
    /// The token's name, as the parser's error messages name it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            T::Eof => "EOF",
            T::Usecase => "USECASE",
            T::Actor => "ACTOR",
            T::SystemBoundary => "SYSTEM_BOUNDARY",
            T::End => "END",
            T::Direction => "DIRECTION",
            T::Td => "TD",
            T::Tb => "TB",
            T::Bt => "BT",
            T::Lr => "LR",
            T::Rl => "RL",
            T::Note => "NOTE",
            T::For => "FOR",
            T::Json => "JSON",
            T::ClassDef => "CLASS_DEF",
            T::Class => "CLASS",
            T::Style => "STYLE",
            T::Include => "INCLUDE",
            T::Extend => "EXTEND",
            T::True => "TRUE",
            T::False => "FALSE",
            T::Identifier => "IDENTIFIER",
            T::Number => "NUMBER",
            T::At => "AT",
            T::Comma => "COMMA",
            T::Colon => "COLON",
            T::HashColor => "HASH_COLOR",
            T::CssIdentifier => "CSS_IDENTIFIER",
            T::CssEscapedComma => "CSS_ESCAPED_COMMA",
            T::Dash => "DASH",
            T::Dot => "DOT",
            T::Percent => "PERCENT",
            T::CssPunctuation => "CSS_PUNCTUATION",
            T::LabelPunctuation => "LABEL_PUNCTUATION",
            T::LabelSymbol => "LABEL_SYMBOL",
            T::PlainString => "PLAIN_STRING",
            T::MarkdownString => "MARKDOWN_STRING",
            T::UnclosedMarkdownString => "UNCLOSED_MARKDOWN_STRING",
            T::Comment => "COMMENT",
            T::Newline => "NEWLINE",
            T::AccDescrBlock => "ACC_DESCR_BLOCK",
            T::AccTitleLine => "ACC_TITLE_LINE",
            T::AccDescrLine => "ACC_DESCR_LINE",
            T::JsonDeclarationStart => "JSON_DECLARATION_START",
            T::JsonObjectLiteral => "JSON_OBJECT_LITERAL",
            T::UnclosedJsonObjectLiteral => "UNCLOSED_JSON_OBJECT_LITERAL",
            T::Generalization => "GENERALIZATION",
            T::DependencyArrow => "DEPENDENCY_ARROW",
            T::StereotypeStart => "STEREOTYPE_START",
            T::StereotypeEnd => "STEREOTYPE_END",
            T::StereotypeText => "STEREOTYPE_TEXT",
            T::UnclosedStereotypeText => "UNCLOSED_STEREOTYPE_TEXT",
            T::ClassSeparator => "CLASS_SEPARATOR",
            T::ForwardSolid => "FORWARD_SOLID",
            T::BackwardSolid => "BACKWARD_SOLID",
            T::ForwardCircle => "FORWARD_CIRCLE",
            T::BackwardCircle => "BACKWARD_CIRCLE",
            T::ForwardCross => "FORWARD_CROSS",
            T::BackwardCross => "BACKWARD_CROSS",
            T::MarkerlessSolid => "MARKERLESS_SOLID",
            T::MetadataStart => "METADATA_START",
            T::LBrace => "LBRACE",
            T::RBrace => "RBRACE",
            T::LBracket => "LBRACKET",
            T::RBracket => "RBRACKET",
            T::LParen => "LPAREN",
            T::RParen => "RPAREN",
        }
    }

    /// Whether the token is in Mermaid's `WORD` category: every keyword and `IDENTIFIER`.
    pub(crate) fn is_word(self) -> bool {
        matches!(
            self,
            T::Usecase
                | T::Actor
                | T::SystemBoundary
                | T::End
                | T::Direction
                | T::Td
                | T::Tb
                | T::Bt
                | T::Lr
                | T::Rl
                | T::Note
                | T::For
                | T::Json
                | T::ClassDef
                | T::Class
                | T::Style
                | T::Include
                | T::Extend
                | T::True
                | T::False
                | T::Identifier
        )
    }

    /// Whether the token is in Mermaid's `LABEL_TEXT` category.
    pub(crate) fn is_label_text(self) -> bool {
        self.is_word()
            || matches!(
                self,
                T::Number
                    | T::At
                    | T::Comma
                    | T::Colon
                    | T::HashColor
                    | T::CssIdentifier
                    | T::CssEscapedComma
                    | T::Dash
                    | T::Dot
                    | T::Percent
                    | T::CssPunctuation
                    | T::LabelPunctuation
                    | T::LabelSymbol
            )
    }
}

/// The token kinds that can start a statement of the diagram.
pub(crate) const STATEMENTS: &[T] = &[
    T::AccTitleLine,
    T::AccDescrLine,
    T::AccDescrBlock,
    T::Direction,
    T::Actor,
    T::SystemBoundary,
    T::Note,
    T::JsonDeclarationStart,
    T::ClassDef,
    T::Class,
    T::Style,
];

/// The token kinds that name an element.
pub(crate) const NAMES: &[T] = &[T::Identifier, T::PlainString, T::MarkdownString];

/// The token kinds that are an arrow of a relationship.
pub(crate) const ARROWS: &[T] = &[
    T::DependencyArrow,
    T::Generalization,
    T::ForwardSolid,
    T::BackwardSolid,
    T::MarkerlessSolid,
    T::ForwardCircle,
    T::BackwardCircle,
    T::ForwardCross,
    T::BackwardCross,
];

/// The token kinds that may be part of a style value.
pub(crate) const STYLE_PARTS: &[T] = &[
    T::PlainString,
    T::Number,
    T::HashColor,
    T::CssIdentifier,
    T::CssEscapedComma,
    T::Dash,
    T::Dot,
    T::Percent,
    T::CssPunctuation,
    T::Colon,
    T::LParen,
    T::RParen,
    T::LBracket,
    T::RBracket,
    T::LBrace,
    T::RBrace,
    T::At,
    T::MarkerlessSolid,
];

/// A token may start a line of the diagram.
pub(crate) const LINE: u8 = 1;
/// A token may be the arrow of a relationship.
pub(crate) const RELATION: u8 = 2;
/// A token may be part of a style value.
pub(crate) const STYLE_PART: u8 = 4;
/// A token may follow the name of an element.
pub(crate) const FOLLOWS_NAME: u8 = 8;
/// A token may be a line of a system boundary.
pub(crate) const BOUNDARY_LINE: u8 = 16;
/// A token may close the label of a relationship.
pub(crate) const CLOSES_LABEL: u8 = 32;
/// A token may be the first of a metadata block.
pub(crate) const METADATA_FIRST: u8 = 64;

/// The flags Mermaid's grammar gives a token, as pele's parser reads them.
pub(crate) fn flags(t: T) -> u8 {
    let mut bits = 0;
    if t == T::Newline || t == T::Comment || STATEMENTS.contains(&t) || NAMES.contains(&t) {
        bits |= LINE;
    }
    if t == T::Identifier || ARROWS.contains(&t) {
        bits |= RELATION;
    }
    if t.is_word() || STYLE_PARTS.contains(&t) {
        bits |= STYLE_PART;
    }
    if t == T::StereotypeStart
        || t == T::ClassSeparator
        || t == T::Identifier
        || ARROWS.contains(&t)
        || t == T::Newline
        || t == T::Eof
    {
        bits |= FOLLOWS_NAME;
    }
    if t == T::Newline || t == T::Comment || t == T::Actor || NAMES.contains(&t) {
        bits |= BOUNDARY_LINE;
    }
    if matches!(
        t,
        T::ForwardSolid | T::MarkerlessSolid | T::ForwardCircle | T::ForwardCross
    ) {
        bits |= CLOSES_LABEL;
    }
    if matches!(t, T::Newline | T::Identifier | T::PlainString | T::RBrace) {
        bits |= METADATA_FIRST;
    }
    bits
}
