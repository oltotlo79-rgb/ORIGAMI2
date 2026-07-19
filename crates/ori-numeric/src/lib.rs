//! Resource-bounded scalar expressions for exact origami construction input.
//!
//! Decimal literals and rational arithmetic remain exact. Irrational
//! operations (`pi` and square root) return a certified closed rational
//! interval at the requested binary precision instead of silently rounding to
//! `f64`. The original expression text is retained for project persistence.

use num_bigint::{BigInt, BigUint, Sign};
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

pub const MIN_PRECISION_BITS: usize = 32;
pub const MAX_PRECISION_BITS: usize = 512;
pub const HARD_MAX_SOURCE_BYTES: usize = 4_096;
pub const HARD_MAX_TOKENS: usize = 1_024;
pub const HARD_MAX_AST_NODES: usize = 1_024;
pub const HARD_MAX_NESTING_DEPTH: usize = 64;
pub const HARD_MAX_LITERAL_DIGITS: usize = 1_024;
pub const HARD_MAX_DECIMAL_EXPONENT: usize = 4_096;
pub const HARD_MAX_OPERATIONS: usize = 20_000;
pub const HARD_MAX_VALUE_BITS: usize = 32_768;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpressionLimits {
    pub max_source_bytes: usize,
    pub max_tokens: usize,
    pub max_ast_nodes: usize,
    pub max_nesting_depth: usize,
    pub max_literal_digits: usize,
    pub max_decimal_exponent: usize,
    pub precision_bits: usize,
    pub max_operations: usize,
    pub max_value_bits: usize,
}

impl Default for ExpressionLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: HARD_MAX_SOURCE_BYTES,
            max_tokens: HARD_MAX_TOKENS,
            max_ast_nodes: HARD_MAX_AST_NODES,
            max_nesting_depth: HARD_MAX_NESTING_DEPTH,
            max_literal_digits: HARD_MAX_LITERAL_DIGITS,
            max_decimal_exponent: HARD_MAX_DECIMAL_EXPONENT,
            precision_bits: 192,
            max_operations: HARD_MAX_OPERATIONS,
            max_value_bits: HARD_MAX_VALUE_BITS,
        }
    }
}

impl ExpressionLimits {
    fn validate(self) -> Result<Self, ExpressionError> {
        if !(MIN_PRECISION_BITS..=MAX_PRECISION_BITS).contains(&self.precision_bits) {
            return Err(ExpressionError::PrecisionOutOfRange);
        }
        if self.max_source_bytes == 0
            || self.max_tokens == 0
            || self.max_ast_nodes == 0
            || self.max_nesting_depth == 0
            || self.max_literal_digits == 0
            || self.max_decimal_exponent == 0
            || self.max_operations == 0
            || self.max_value_bits == 0
            || self.max_source_bytes > HARD_MAX_SOURCE_BYTES
            || self.max_tokens > HARD_MAX_TOKENS
            || self.max_ast_nodes > HARD_MAX_AST_NODES
            || self.max_nesting_depth > HARD_MAX_NESTING_DEPTH
            || self.max_literal_digits > HARD_MAX_LITERAL_DIGITS
            || self.max_decimal_exponent > HARD_MAX_DECIMAL_EXPONENT
            || self.max_operations > HARD_MAX_OPERATIONS
            || self.max_value_bits > HARD_MAX_VALUE_BITS
        {
            return Err(ExpressionError::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpressionResource {
    SourceBytes,
    Tokens,
    AstNodes,
    NestingDepth,
    LiteralDigits,
    DecimalExponent,
    Operations,
    ValueBits,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExpressionError {
    #[error("expression is empty")]
    Empty,
    #[error("expression limits are invalid")]
    InvalidLimits,
    #[error("requested precision is outside the supported range")]
    PrecisionOutOfRange,
    #[error("expression resource limit exceeded: {0:?}")]
    ResourceLimit(ExpressionResource),
    #[error("invalid token at scalar position {position}")]
    InvalidToken { position: usize },
    #[error("invalid number at scalar position {position}")]
    InvalidNumber { position: usize },
    #[error("unexpected token at scalar position {position}")]
    UnexpectedToken { position: usize },
    #[error("expression ended unexpectedly")]
    UnexpectedEnd,
    #[error("division by zero")]
    DivisionByZero,
    #[error("square root operand is negative")]
    NegativeSquareRoot,
    #[error("expression state is inconsistent")]
    InconsistentState,
}

/// A finite binary64 enclosure of a certified high-precision interval.
///
/// Each endpoint is rounded outwards after comparing the candidate binary64
/// value with the exact rational endpoint.  This type is therefore suitable
/// for bounded IPC and geometry previews; a nearest binary64 conversion alone
/// must not be presented as a certified enclosure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CertifiedF64Interval {
    lower: f64,
    upper: f64,
}

impl CertifiedF64Interval {
    #[must_use]
    pub fn lower(self) -> f64 {
        self.lower
    }

    #[must_use]
    pub fn upper(self) -> f64 {
        self.upper
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum F64IntervalError {
    #[error("certified interval cannot be represented by finite binary64 endpoints")]
    NonFinite,
}

#[derive(Clone, Debug)]
pub struct ScalarExpression {
    source: String,
    nodes: Vec<Node>,
    root: NodeId,
    footprint: ParseFootprint,
}

impl ScalarExpression {
    pub fn parse(
        source: impl AsRef<str>,
        limits: ExpressionLimits,
    ) -> Result<Self, ExpressionError> {
        let limits = limits.validate()?;
        let source = source.as_ref();
        if source.len() > limits.max_source_bytes {
            return Err(ExpressionError::ResourceLimit(
                ExpressionResource::SourceBytes,
            ));
        }
        if source.is_empty() || source.chars().all(char::is_whitespace) {
            return Err(ExpressionError::Empty);
        }
        let tokenized = tokenize(source, &limits)?;
        let parsed = Parser::new(&tokenized.tokens, &limits).parse()?;
        let footprint = ParseFootprint {
            source_bytes: source.len(),
            tokens: tokenized.tokens.len(),
            ast_nodes: parsed.nodes.len(),
            nesting_depth: parsed.nesting_depth,
            literal_digits: tokenized.literal_digits,
            decimal_exponent: tokenized.decimal_exponent,
        };
        Ok(Self {
            source: source.to_owned(),
            nodes: parsed.nodes,
            root: parsed.root,
            footprint,
        })
    }

    pub fn parse_default(source: impl AsRef<str>) -> Result<Self, ExpressionError> {
        Self::parse(source, ExpressionLimits::default())
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn evaluate(
        &self,
        limits: ExpressionLimits,
    ) -> Result<HighPrecisionValue, ExpressionError> {
        let limits = limits.validate()?;
        self.footprint.ensure_within(&limits)?;
        let mut meter = EvaluationMeter::new(limits);
        let interval =
            evaluate_node_iterative(&self.nodes, self.root, &mut meter, self.footprint.ast_nodes)?;
        Ok(HighPrecisionValue {
            lower: interval.lower,
            upper: interval.upper,
            precision_bits: limits.precision_bits,
            operations: meter.operations,
        })
    }

    pub fn evaluate_default(&self) -> Result<HighPrecisionValue, ExpressionError> {
        self.evaluate(ExpressionLimits::default())
    }
}

impl PartialEq for ScalarExpression {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for ScalarExpression {}

impl Serialize for ScalarExpression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.source)
    }
}

impl<'de> Deserialize<'de> for ScalarExpression {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let source = String::deserialize(deserializer)?;
        Self::parse_default(&source).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParseFootprint {
    source_bytes: usize,
    tokens: usize,
    ast_nodes: usize,
    nesting_depth: usize,
    literal_digits: usize,
    decimal_exponent: usize,
}

impl ParseFootprint {
    fn ensure_within(self, limits: &ExpressionLimits) -> Result<(), ExpressionError> {
        for (observed, allowed, resource) in [
            (
                self.source_bytes,
                limits.max_source_bytes,
                ExpressionResource::SourceBytes,
            ),
            (self.tokens, limits.max_tokens, ExpressionResource::Tokens),
            (
                self.ast_nodes,
                limits.max_ast_nodes,
                ExpressionResource::AstNodes,
            ),
            (
                self.nesting_depth,
                limits.max_nesting_depth,
                ExpressionResource::NestingDepth,
            ),
            (
                self.literal_digits,
                limits.max_literal_digits,
                ExpressionResource::LiteralDigits,
            ),
            (
                self.decimal_exponent,
                limits.max_decimal_exponent,
                ExpressionResource::DecimalExponent,
            ),
        ] {
            if observed > allowed {
                return Err(ExpressionError::ResourceLimit(resource));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighPrecisionValue {
    lower: BigRational,
    upper: BigRational,
    precision_bits: usize,
    operations: usize,
}

impl HighPrecisionValue {
    #[must_use]
    pub fn lower(&self) -> &BigRational {
        &self.lower
    }

    #[must_use]
    pub fn upper(&self) -> &BigRational {
        &self.upper
    }

    #[must_use]
    pub fn is_exact(&self) -> bool {
        self.lower == self.upper
    }

    #[must_use]
    pub fn precision_bits(&self) -> usize {
        self.precision_bits
    }

    #[must_use]
    pub fn operations(&self) -> usize {
        self.operations
    }

    /// Converts the exact rational endpoints to finite binary64 values while
    /// preserving enclosure by directed one-ULP correction where required.
    pub fn certified_f64_interval(&self) -> Result<CertifiedF64Interval, F64IntervalError> {
        let lower = rational_to_f64_outward(&self.lower, F64Direction::Down)?;
        let upper = rational_to_f64_outward(&self.upper, F64Direction::Up)?;
        if !lower.is_finite() || !upper.is_finite() || lower > upper {
            return Err(F64IntervalError::NonFinite);
        }
        Ok(CertifiedF64Interval {
            lower: normalize_f64_zero(lower),
            upper: normalize_f64_zero(upper),
        })
    }
}

#[derive(Clone, Copy)]
enum F64Direction {
    Down,
    Up,
}

fn rational_to_f64_outward(
    value: &BigRational,
    direction: F64Direction,
) -> Result<f64, F64IntervalError> {
    let candidate = value.to_f64().ok_or(F64IntervalError::NonFinite)?;
    if !candidate.is_finite() {
        return Err(F64IntervalError::NonFinite);
    }
    let candidate_exact = BigRational::from_float(candidate).ok_or(F64IntervalError::NonFinite)?;
    let rounded = match direction {
        F64Direction::Down if candidate_exact > *value => next_f64_down(candidate),
        F64Direction::Up if candidate_exact < *value => next_f64_up(candidate),
        _ => candidate,
    };
    rounded
        .is_finite()
        .then_some(rounded)
        .ok_or(F64IntervalError::NonFinite)
}

fn next_f64_down(value: f64) -> f64 {
    if value == f64::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value > 0.0 { bits - 1 } else { bits + 1 })
}

fn next_f64_up(value: f64) -> f64 {
    if value == f64::INFINITY {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value > 0.0 { bits + 1 } else { bits - 1 })
}

fn normalize_f64_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

type NodeId = usize;

#[derive(Clone, Debug)]
enum Node {
    Rational(BigRational),
    Pi,
    Neg(NodeId),
    Sqrt(NodeId),
    Add(NodeId, NodeId),
    Subtract(NodeId, NodeId),
    Multiply(NodeId, NodeId),
    Divide(NodeId, NodeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TokenKind {
    Number(BigRational),
    Pi,
    Sqrt,
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Token {
    kind: TokenKind,
    position: usize,
}

struct Tokenization {
    tokens: Vec<Token>,
    literal_digits: usize,
    decimal_exponent: usize,
}

fn tokenize(source: &str, limits: &ExpressionLimits) -> Result<Tokenization, ExpressionError> {
    let characters: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut literal_digits = 0_usize;
    let mut decimal_exponent = 0_usize;
    let mut index = 0_usize;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }
        let position = index;
        let kind = match character {
            '+' => {
                index += 1;
                TokenKind::Plus
            }
            '-' => {
                index += 1;
                TokenKind::Minus
            }
            '*' => {
                index += 1;
                TokenKind::Star
            }
            '/' => {
                index += 1;
                TokenKind::Slash
            }
            '(' => {
                index += 1;
                TokenKind::LeftParen
            }
            ')' => {
                index += 1;
                TokenKind::RightParen
            }
            'π' => {
                index += 1;
                TokenKind::Pi
            }
            '√' => {
                index += 1;
                TokenKind::Sqrt
            }
            value if value.is_ascii_digit() || value == '.' => {
                let start = index;
                scan_number(&characters, &mut index);
                let text: String = characters[start..index].iter().collect();
                let parsed = parse_number(&text, position, limits)?;
                literal_digits = literal_digits.max(parsed.literal_digits);
                decimal_exponent = decimal_exponent.max(parsed.decimal_exponent);
                TokenKind::Number(parsed.value)
            }
            value if value.is_ascii_alphabetic() => {
                let start = index;
                while index < characters.len() && characters[index].is_ascii_alphabetic() {
                    index += 1;
                }
                let identifier: String = characters[start..index].iter().collect();
                match identifier.to_ascii_lowercase().as_str() {
                    "pi" => TokenKind::Pi,
                    "sqrt" => TokenKind::Sqrt,
                    _ => return Err(ExpressionError::InvalidToken { position }),
                }
            }
            _ => return Err(ExpressionError::InvalidToken { position }),
        };
        if tokens.len() >= limits.max_tokens {
            return Err(ExpressionError::ResourceLimit(ExpressionResource::Tokens));
        }
        tokens.push(Token { kind, position });
    }
    if tokens.is_empty() {
        return Err(ExpressionError::Empty);
    }
    Ok(Tokenization {
        tokens,
        literal_digits,
        decimal_exponent,
    })
}

fn scan_number(characters: &[char], index: &mut usize) {
    let mut saw_dot = false;
    while *index < characters.len() {
        match characters[*index] {
            value if value.is_ascii_digit() => *index += 1,
            '.' if !saw_dot => {
                saw_dot = true;
                *index += 1;
            }
            _ => break,
        }
    }
    if *index < characters.len() && matches!(characters[*index], 'e' | 'E') {
        *index += 1;
        if *index < characters.len() && matches!(characters[*index], '+' | '-') {
            *index += 1;
        }
        while *index < characters.len() && characters[*index].is_ascii_digit() {
            *index += 1;
        }
    }
}

struct ParsedNumber {
    value: BigRational,
    literal_digits: usize,
    decimal_exponent: usize,
}

fn parse_number(
    text: &str,
    position: usize,
    limits: &ExpressionLimits,
) -> Result<ParsedNumber, ExpressionError> {
    let (mantissa, exponent_text) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], Some(&text[index + 1..])),
        None => (text, None),
    };
    let exponent = match exponent_text {
        Some("") | Some("+") | Some("-") => {
            return Err(ExpressionError::InvalidNumber { position });
        }
        Some(value) => value
            .parse::<i64>()
            .map_err(|_| ExpressionError::ResourceLimit(ExpressionResource::DecimalExponent))?,
        None => 0,
    };
    let (whole, fraction) = match mantissa.split_once('.') {
        Some(parts) => parts,
        None => (mantissa, ""),
    };
    if whole.is_empty() && fraction.is_empty() {
        return Err(ExpressionError::InvalidNumber { position });
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|value| value.is_ascii_digit())
    {
        return Err(ExpressionError::InvalidNumber { position });
    }
    let digit_count = whole.len().saturating_add(fraction.len());
    if digit_count == 0 {
        return Err(ExpressionError::InvalidNumber { position });
    }
    if digit_count > limits.max_literal_digits {
        return Err(ExpressionError::ResourceLimit(
            ExpressionResource::LiteralDigits,
        ));
    }
    let exponent_magnitude = usize::try_from(exponent.unsigned_abs())
        .map_err(|_| ExpressionError::ResourceLimit(ExpressionResource::DecimalExponent))?;
    if exponent_magnitude > limits.max_decimal_exponent {
        return Err(ExpressionError::ResourceLimit(
            ExpressionResource::DecimalExponent,
        ));
    }
    let scale = i64::try_from(fraction.len())
        .map_err(|_| ExpressionError::ResourceLimit(ExpressionResource::DecimalExponent))?
        .checked_sub(exponent)
        .ok_or(ExpressionError::ResourceLimit(
            ExpressionResource::DecimalExponent,
        ))?;
    let scale_magnitude = usize::try_from(scale.unsigned_abs())
        .map_err(|_| ExpressionError::ResourceLimit(ExpressionResource::DecimalExponent))?;
    if scale_magnitude > limits.max_decimal_exponent {
        return Err(ExpressionError::ResourceLimit(
            ExpressionResource::DecimalExponent,
        ));
    }
    let estimated_bits = estimated_decimal_literal_bits(digit_count, scale_magnitude)?;
    if estimated_bits > limits.max_value_bits {
        return Err(ExpressionError::ResourceLimit(
            ExpressionResource::ValueBits,
        ));
    }
    let mut digits = String::with_capacity(digit_count);
    digits.push_str(whole);
    digits.push_str(fraction);
    let numerator = BigInt::parse_bytes(digits.as_bytes(), 10)
        .ok_or(ExpressionError::InvalidNumber { position })?;
    let power = u32::try_from(scale_magnitude)
        .map_err(|_| ExpressionError::ResourceLimit(ExpressionResource::DecimalExponent))?;
    let factor = BigInt::from(10_u8).pow(power);
    let value = if scale >= 0 {
        BigRational::new(numerator, factor)
    } else {
        BigRational::from_integer(numerator * factor)
    };
    ensure_value_bits(&value, limits.max_value_bits)?;
    Ok(ParsedNumber {
        value,
        literal_digits: digit_count,
        decimal_exponent: exponent_magnitude.max(scale_magnitude),
    })
}

fn estimated_decimal_literal_bits(
    digit_count: usize,
    scale_magnitude: usize,
) -> Result<usize, ExpressionError> {
    // `n >= 1` decimal digits need at most `4*n` magnitude bits.  A non-zero
    // positive scale places a `10^scale` factor in the denominator; a
    // negative scale multiplies it into the mantissa.  Because `digit_count`
    // is always positive here, the sum is a conservative bound including the
    // scale-zero case and either sign of the decimal exponent.  A leading
    // unary sign is an AST operation and does not increase BigInt magnitude.
    digit_count
        .checked_add(scale_magnitude)
        .and_then(|digits| digits.checked_mul(4))
        .ok_or(ExpressionError::ResourceLimit(
            ExpressionResource::ValueBits,
        ))
}

struct Parser<'a> {
    tokens: &'a [Token],
    limits: &'a ExpressionLimits,
    cursor: usize,
    nodes: Vec<Node>,
    nesting_depth: usize,
}

struct ParsedAst {
    root: NodeId,
    nodes: Vec<Node>,
    nesting_depth: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], limits: &'a ExpressionLimits) -> Self {
        Self {
            tokens,
            limits,
            cursor: 0,
            nodes: Vec::new(),
            nesting_depth: 0,
        }
    }

    fn parse(mut self) -> Result<ParsedAst, ExpressionError> {
        let root = self.parse_sum(0)?;
        if let Some(token) = self.tokens.get(self.cursor) {
            return Err(ExpressionError::UnexpectedToken {
                position: token.position,
            });
        }
        Ok(ParsedAst {
            root,
            nodes: self.nodes,
            nesting_depth: self.nesting_depth,
        })
    }

    fn parse_sum(&mut self, depth: usize) -> Result<NodeId, ExpressionError> {
        let mut node = self.parse_product(depth)?;
        loop {
            if self.consume(TokenDiscriminant::Plus) {
                let right = self.parse_product(depth)?;
                node = self.node(Node::Add(node, right))?;
            } else if self.consume(TokenDiscriminant::Minus) {
                let right = self.parse_product(depth)?;
                node = self.node(Node::Subtract(node, right))?;
            } else {
                return Ok(node);
            }
        }
    }

    fn parse_product(&mut self, depth: usize) -> Result<NodeId, ExpressionError> {
        let mut node = self.parse_unary(depth)?;
        loop {
            if self.consume(TokenDiscriminant::Star) {
                let right = self.parse_unary(depth)?;
                node = self.node(Node::Multiply(node, right))?;
            } else if self.consume(TokenDiscriminant::Slash) {
                let right = self.parse_unary(depth)?;
                node = self.node(Node::Divide(node, right))?;
            } else {
                return Ok(node);
            }
        }
    }

    fn parse_unary(&mut self, depth: usize) -> Result<NodeId, ExpressionError> {
        self.check_depth(depth)?;
        if self.consume(TokenDiscriminant::Plus) {
            return self.parse_unary(depth + 1);
        }
        if self.consume(TokenDiscriminant::Minus) {
            let child = self.parse_unary(depth + 1)?;
            return self.node(Node::Neg(child));
        }
        if self.consume(TokenDiscriminant::Sqrt) {
            let child = self.parse_unary(depth + 1)?;
            return self.node(Node::Sqrt(child));
        }
        self.parse_primary(depth)
    }

    fn parse_primary(&mut self, depth: usize) -> Result<NodeId, ExpressionError> {
        self.check_depth(depth)?;
        let Some(token) = self.tokens.get(self.cursor) else {
            return Err(ExpressionError::UnexpectedEnd);
        };
        match &token.kind {
            TokenKind::Number(value) => {
                self.cursor += 1;
                self.node(Node::Rational(value.clone()))
            }
            TokenKind::Pi => {
                self.cursor += 1;
                self.node(Node::Pi)
            }
            TokenKind::LeftParen => {
                self.cursor += 1;
                let node = self.parse_sum(depth + 1)?;
                let Some(closing) = self.tokens.get(self.cursor) else {
                    return Err(ExpressionError::UnexpectedEnd);
                };
                if !matches!(closing.kind, TokenKind::RightParen) {
                    return Err(ExpressionError::UnexpectedToken {
                        position: closing.position,
                    });
                }
                self.cursor += 1;
                Ok(node)
            }
            _ => Err(ExpressionError::UnexpectedToken {
                position: token.position,
            }),
        }
    }

    fn node(&mut self, node: Node) -> Result<NodeId, ExpressionError> {
        let node_id = self.nodes.len();
        let node_count = node_id
            .checked_add(1)
            .ok_or(ExpressionError::ResourceLimit(ExpressionResource::AstNodes))?;
        if node_count > self.limits.max_ast_nodes {
            return Err(ExpressionError::ResourceLimit(ExpressionResource::AstNodes));
        }
        self.nodes.push(node);
        Ok(node_id)
    }

    fn check_depth(&mut self, depth: usize) -> Result<(), ExpressionError> {
        if depth > self.limits.max_nesting_depth {
            return Err(ExpressionError::ResourceLimit(
                ExpressionResource::NestingDepth,
            ));
        }
        self.nesting_depth = self.nesting_depth.max(depth);
        Ok(())
    }

    fn consume(&mut self, expected: TokenDiscriminant) -> bool {
        if self
            .tokens
            .get(self.cursor)
            .is_some_and(|token| expected.matches(&token.kind))
        {
            self.cursor += 1;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy)]
enum TokenDiscriminant {
    Plus,
    Minus,
    Star,
    Slash,
    Sqrt,
}

impl TokenDiscriminant {
    fn matches(self, token: &TokenKind) -> bool {
        matches!(
            (self, token),
            (Self::Plus, TokenKind::Plus)
                | (Self::Minus, TokenKind::Minus)
                | (Self::Star, TokenKind::Star)
                | (Self::Slash, TokenKind::Slash)
                | (Self::Sqrt, TokenKind::Sqrt)
        )
    }
}

#[derive(Clone, Debug)]
struct Interval {
    lower: BigRational,
    upper: BigRational,
}

impl Interval {
    fn exact(value: BigRational) -> Self {
        Self {
            lower: value.clone(),
            upper: value,
        }
    }
}

#[derive(Clone, Copy)]
enum EvaluationFrame {
    Visit(NodeId),
    Apply(EvaluationOperation),
}

#[derive(Clone, Copy)]
enum EvaluationOperation {
    Negate,
    SquareRoot,
    Add,
    Subtract,
    Multiply,
    Divide,
}

fn evaluate_node_iterative(
    nodes: &[Node],
    root: NodeId,
    meter: &mut EvaluationMeter,
    ast_nodes: usize,
) -> Result<Interval, ExpressionError> {
    let frame_capacity = ast_nodes
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(ExpressionError::ResourceLimit(ExpressionResource::AstNodes))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    let mut values = Vec::with_capacity(ast_nodes);
    frames.push(EvaluationFrame::Visit(root));

    while let Some(frame) = frames.pop() {
        match frame {
            EvaluationFrame::Visit(node_id) => match nodes
                .get(node_id)
                .ok_or(ExpressionError::InconsistentState)?
            {
                Node::Rational(value) => {
                    meter.observe(value)?;
                    values.push(Interval::exact(value.clone()));
                }
                Node::Pi => values.push(pi_interval(meter)?),
                Node::Neg(child) => {
                    frames.push(EvaluationFrame::Apply(EvaluationOperation::Negate));
                    frames.push(EvaluationFrame::Visit(*child));
                }
                Node::Sqrt(child) => {
                    frames.push(EvaluationFrame::Apply(EvaluationOperation::SquareRoot));
                    frames.push(EvaluationFrame::Visit(*child));
                }
                Node::Add(left, right) => {
                    push_binary_frames(&mut frames, *left, *right, EvaluationOperation::Add);
                }
                Node::Subtract(left, right) => {
                    push_binary_frames(&mut frames, *left, *right, EvaluationOperation::Subtract);
                }
                Node::Multiply(left, right) => {
                    push_binary_frames(&mut frames, *left, *right, EvaluationOperation::Multiply);
                }
                Node::Divide(left, right) => {
                    push_binary_frames(&mut frames, *left, *right, EvaluationOperation::Divide);
                }
            },
            EvaluationFrame::Apply(operation) => match operation {
                EvaluationOperation::Negate => {
                    let value = pop_interval(&mut values)?;
                    values.push(Interval {
                        lower: meter.negate(&value.upper)?,
                        upper: meter.negate(&value.lower)?,
                    });
                }
                EvaluationOperation::SquareRoot => {
                    let value = pop_interval(&mut values)?;
                    values.push(sqrt_interval(&value, meter)?);
                }
                EvaluationOperation::Add => {
                    let (left, right) = pop_binary_intervals(&mut values)?;
                    values.push(Interval {
                        lower: meter.add(&left.lower, &right.lower)?,
                        upper: meter.add(&left.upper, &right.upper)?,
                    });
                }
                EvaluationOperation::Subtract => {
                    let (left, right) = pop_binary_intervals(&mut values)?;
                    values.push(Interval {
                        lower: meter.subtract(&left.lower, &right.upper)?,
                        upper: meter.subtract(&left.upper, &right.lower)?,
                    });
                }
                EvaluationOperation::Multiply => {
                    let (left, right) = pop_binary_intervals(&mut values)?;
                    values.push(multiply_intervals(&left, &right, meter)?);
                }
                EvaluationOperation::Divide => {
                    let (left, right) = pop_binary_intervals(&mut values)?;
                    values.push(divide_intervals(&left, &right, meter)?);
                }
            },
        }
    }

    if values.len() != 1 {
        return Err(ExpressionError::InconsistentState);
    }
    values.pop().ok_or(ExpressionError::InconsistentState)
}

fn push_binary_frames(
    frames: &mut Vec<EvaluationFrame>,
    left: NodeId,
    right: NodeId,
    operation: EvaluationOperation,
) {
    frames.push(EvaluationFrame::Apply(operation));
    frames.push(EvaluationFrame::Visit(right));
    frames.push(EvaluationFrame::Visit(left));
}

fn pop_interval(values: &mut Vec<Interval>) -> Result<Interval, ExpressionError> {
    values.pop().ok_or(ExpressionError::InconsistentState)
}

fn pop_binary_intervals(
    values: &mut Vec<Interval>,
) -> Result<(Interval, Interval), ExpressionError> {
    let right = pop_interval(values)?;
    let left = pop_interval(values)?;
    Ok((left, right))
}

fn multiply_intervals(
    left: &Interval,
    right: &Interval,
    meter: &mut EvaluationMeter,
) -> Result<Interval, ExpressionError> {
    let products = [
        meter.multiply(&left.lower, &right.lower)?,
        meter.multiply(&left.lower, &right.upper)?,
        meter.multiply(&left.upper, &right.lower)?,
        meter.multiply(&left.upper, &right.upper)?,
    ];
    let mut lower = products[0].clone();
    let mut upper = products[0].clone();
    for value in &products[1..] {
        if value < &lower {
            lower = value.clone();
        }
        if value > &upper {
            upper = value.clone();
        }
    }
    Ok(Interval { lower, upper })
}

fn divide_intervals(
    numerator: &Interval,
    denominator: &Interval,
    meter: &mut EvaluationMeter,
) -> Result<Interval, ExpressionError> {
    if denominator.lower <= BigRational::zero() && denominator.upper >= BigRational::zero() {
        return Err(ExpressionError::DivisionByZero);
    }
    let one = BigRational::one();
    let first = meter.divide(&one, &denominator.lower)?;
    let second = meter.divide(&one, &denominator.upper)?;
    let reciprocal = if first <= second {
        Interval {
            lower: first,
            upper: second,
        }
    } else {
        Interval {
            lower: second,
            upper: first,
        }
    };
    multiply_intervals(numerator, &reciprocal, meter)
}

fn sqrt_interval(
    value: &Interval,
    meter: &mut EvaluationMeter,
) -> Result<Interval, ExpressionError> {
    if value.lower.is_negative() {
        return Err(ExpressionError::NegativeSquareRoot);
    }
    let (lower, _) = sqrt_bounds(&value.lower, meter)?;
    let (_, upper) = sqrt_bounds(&value.upper, meter)?;
    Ok(Interval { lower, upper })
}

fn sqrt_bounds(
    value: &BigRational,
    meter: &mut EvaluationMeter,
) -> Result<(BigRational, BigRational), ExpressionError> {
    meter.consume_operation()?;
    if value.is_negative() {
        return Err(ExpressionError::NegativeSquareRoot);
    }
    if value.is_zero() {
        return Ok((BigRational::zero(), BigRational::zero()));
    }
    meter.observe(value)?;
    let numerator = nonnegative_biguint(value.numer())?;
    let denominator = nonnegative_biguint(value.denom())?;
    let shift =
        meter
            .limits
            .precision_bits
            .checked_mul(2)
            .ok_or(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ))?;
    let shifted_bits =
        bit_length_biguint(&numerator)
            .checked_add(shift)
            .ok_or(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ))?;
    if shifted_bits > meter.limits.max_value_bits {
        return Err(ExpressionError::ResourceLimit(
            ExpressionResource::ValueBits,
        ));
    }
    let scaled = numerator << shift;
    let quotient = &scaled / &denominator;
    let root = quotient.sqrt();
    let scale = BigInt::one() << meter.limits.precision_bits;
    let root_integer = BigInt::from_biguint(Sign::Plus, root.clone());
    let lower = BigRational::new(root_integer.clone(), scale.clone());
    let exact = &root * &root * &denominator == scaled;
    let upper = if exact {
        lower.clone()
    } else {
        BigRational::new(root_integer + BigInt::one(), scale)
    };
    meter.observe(&lower)?;
    meter.observe(&upper)?;
    Ok((lower, upper))
}

fn pi_interval(meter: &mut EvaluationMeter) -> Result<Interval, ExpressionError> {
    let guard_precision =
        meter
            .limits
            .precision_bits
            .checked_add(8)
            .ok_or(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ))?;
    let tolerance_denominator = checked_power_of_two(guard_precision, meter.limits.max_value_bits)?;
    let tolerance = BigRational::new(BigInt::one(), tolerance_denominator);
    meter.observe(&tolerance)?;
    let atan_fifth = arctangent_reciprocal_interval(5, &tolerance, meter)?;
    let atan_two_hundred_thirty_ninth = arctangent_reciprocal_interval(239, &tolerance, meter)?;
    let sixteen = BigRational::from_integer(BigInt::from(16_u8));
    let four = BigRational::from_integer(BigInt::from(4_u8));
    let lower_fifth = meter.multiply(&atan_fifth.lower, &sixteen)?;
    let upper_two_hundred_thirty_ninth =
        meter.multiply(&atan_two_hundred_thirty_ninth.upper, &four)?;
    let lower = meter.subtract(&lower_fifth, &upper_two_hundred_thirty_ninth)?;
    let upper_fifth = meter.multiply(&atan_fifth.upper, &sixteen)?;
    let lower_two_hundred_thirty_ninth =
        meter.multiply(&atan_two_hundred_thirty_ninth.lower, &four)?;
    let upper = meter.subtract(&upper_fifth, &lower_two_hundred_thirty_ninth)?;
    Ok(Interval { lower, upper })
}

fn checked_power_of_two(exponent: usize, maximum_bits: usize) -> Result<BigInt, ExpressionError> {
    let required_bits = exponent
        .checked_add(1)
        .ok_or(ExpressionError::ResourceLimit(
            ExpressionResource::ValueBits,
        ))?;
    if required_bits > maximum_bits {
        return Err(ExpressionError::ResourceLimit(
            ExpressionResource::ValueBits,
        ));
    }
    Ok(BigInt::one() << exponent)
}

fn arctangent_reciprocal_interval(
    reciprocal: u16,
    tolerance: &BigRational,
    meter: &mut EvaluationMeter,
) -> Result<Interval, ExpressionError> {
    let x = BigRational::new(BigInt::one(), BigInt::from(reciprocal));
    let x_squared = meter.multiply(&x, &x)?;
    let mut magnitude = x;
    let mut sum = BigRational::zero();
    let mut index = 0_u32;
    loop {
        sum = if index.is_multiple_of(2) {
            meter.add(&sum, &magnitude)?
        } else {
            meter.subtract(&sum, &magnitude)?
        };
        let numerator_factor = index
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(ExpressionError::ResourceLimit(
                ExpressionResource::Operations,
            ))?;
        let denominator_factor =
            numerator_factor
                .checked_add(2)
                .ok_or(ExpressionError::ResourceLimit(
                    ExpressionResource::Operations,
                ))?;
        let powered = meter.multiply(&magnitude, &x_squared)?;
        let factor = BigRational::new(
            BigInt::from(numerator_factor),
            BigInt::from(denominator_factor),
        );
        let next = meter.multiply(&powered, &factor)?;
        if next <= *tolerance {
            let adjacent = if index.is_multiple_of(2) {
                meter.subtract(&sum, &next)?
            } else {
                meter.add(&sum, &next)?
            };
            return if adjacent <= sum {
                Ok(Interval {
                    lower: adjacent,
                    upper: sum,
                })
            } else {
                Ok(Interval {
                    lower: sum,
                    upper: adjacent,
                })
            };
        }
        magnitude = next;
        index = index.checked_add(1).ok_or(ExpressionError::ResourceLimit(
            ExpressionResource::Operations,
        ))?;
    }
}

struct EvaluationMeter {
    limits: ExpressionLimits,
    operations: usize,
}

impl EvaluationMeter {
    fn new(limits: ExpressionLimits) -> Self {
        Self {
            limits,
            operations: 0,
        }
    }

    fn consume_operation(&mut self) -> Result<(), ExpressionError> {
        self.operations = self
            .operations
            .checked_add(1)
            .ok_or(ExpressionError::ResourceLimit(
                ExpressionResource::Operations,
            ))?;
        if self.operations > self.limits.max_operations {
            return Err(ExpressionError::ResourceLimit(
                ExpressionResource::Operations,
            ));
        }
        Ok(())
    }

    fn observe(&self, value: &BigRational) -> Result<(), ExpressionError> {
        ensure_value_bits(value, self.limits.max_value_bits)
    }

    fn add(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ExpressionError> {
        self.binary_preflight(left, right, BinaryOperation::Add)?;
        let result = left + right;
        self.observe(&result)?;
        Ok(result)
    }

    fn subtract(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ExpressionError> {
        self.binary_preflight(left, right, BinaryOperation::Add)?;
        let result = left - right;
        self.observe(&result)?;
        Ok(result)
    }

    fn multiply(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ExpressionError> {
        self.binary_preflight(left, right, BinaryOperation::Multiply)?;
        let result = left * right;
        self.observe(&result)?;
        Ok(result)
    }

    fn divide(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ExpressionError> {
        if right.is_zero() {
            return Err(ExpressionError::DivisionByZero);
        }
        self.binary_preflight(left, right, BinaryOperation::Divide)?;
        let result = left / right;
        self.observe(&result)?;
        Ok(result)
    }

    fn negate(&mut self, value: &BigRational) -> Result<BigRational, ExpressionError> {
        self.consume_operation()?;
        self.observe(value)?;
        let result = -value;
        self.observe(&result)?;
        Ok(result)
    }

    fn binary_preflight(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        operation: BinaryOperation,
    ) -> Result<(), ExpressionError> {
        self.consume_operation()?;
        self.observe(left)?;
        self.observe(right)?;
        let left_numerator = bit_length_bigint(left.numer());
        let left_denominator = bit_length_bigint(left.denom());
        let right_numerator = bit_length_bigint(right.numer());
        let right_denominator = bit_length_bigint(right.denom());
        let (numerator_bound, denominator_bound) = match operation {
            BinaryOperation::Add => (
                left_numerator
                    .checked_add(right_denominator)
                    .and_then(|first| {
                        right_numerator
                            .checked_add(left_denominator)
                            .map(|second| first.max(second))
                    })
                    .and_then(|value| value.checked_add(1)),
                left_denominator.checked_add(right_denominator),
            ),
            BinaryOperation::Multiply => (
                left_numerator.checked_add(right_numerator),
                left_denominator.checked_add(right_denominator),
            ),
            BinaryOperation::Divide => (
                left_numerator.checked_add(right_denominator),
                left_denominator.checked_add(right_numerator),
            ),
        };
        if numerator_bound.is_none_or(|value| value > self.limits.max_value_bits)
            || denominator_bound.is_none_or(|value| value > self.limits.max_value_bits)
        {
            return Err(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum BinaryOperation {
    Add,
    Multiply,
    Divide,
}

fn ensure_value_bits(value: &BigRational, maximum: usize) -> Result<(), ExpressionError> {
    if bit_length_bigint(value.numer()) > maximum || bit_length_bigint(value.denom()) > maximum {
        return Err(ExpressionError::ResourceLimit(
            ExpressionResource::ValueBits,
        ));
    }
    Ok(())
}

fn bit_length_bigint(value: &BigInt) -> usize {
    usize::try_from(value.bits()).unwrap_or(usize::MAX).max(1)
}

fn bit_length_biguint(value: &BigUint) -> usize {
    usize::try_from(value.bits()).unwrap_or(usize::MAX).max(1)
}

fn nonnegative_biguint(value: &BigInt) -> Result<BigUint, ExpressionError> {
    value
        .to_biguint()
        .ok_or(ExpressionError::NegativeSquareRoot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rational(numerator: i64, denominator: i64) -> BigRational {
        BigRational::new(numerator.into(), denominator.into())
    }

    fn decimal_bracket_endpoint(numerator: &[u8], decimal_places: u32) -> BigRational {
        BigRational::new(
            BigInt::parse_bytes(numerator, 10).unwrap(),
            BigInt::from(10_u8).pow(decimal_places),
        )
    }

    #[test]
    fn decimal_fraction_and_precedence_remain_exact() {
        let expression = ScalarExpression::parse_default("1/3 + 0.25 * 2").unwrap();
        let value = expression.evaluate_default().unwrap();
        assert!(value.is_exact());
        assert_eq!(value.lower(), &rational(5, 6));
    }

    #[test]
    fn unary_parentheses_and_scientific_decimal_are_exact() {
        let expression = ScalarExpression::parse_default("-(1.5e2 - +25) / 5").unwrap();
        let value = expression.evaluate_default().unwrap();
        assert_eq!(value.lower(), &rational(-25, 1));
        assert_eq!(value.upper(), &rational(-25, 1));
    }

    #[test]
    fn square_root_supports_ascii_and_symbol_forms() {
        for source in ["sqrt(4)", "√4", "√(9/4)"] {
            let expression = ScalarExpression::parse_default(source).unwrap();
            let value = expression.evaluate_default().unwrap();
            assert!(value.is_exact(), "{source}");
        }
        assert_eq!(
            ScalarExpression::parse_default("sqrt(4)")
                .unwrap()
                .evaluate_default()
                .unwrap()
                .lower(),
            &rational(2, 1),
        );
        assert_eq!(
            ScalarExpression::parse_default("√(9/4)")
                .unwrap()
                .evaluate_default()
                .unwrap()
                .lower(),
            &rational(3, 2),
        );
    }

    #[test]
    fn irrational_square_root_returns_a_certified_narrow_interval() {
        let limits = ExpressionLimits {
            precision_bits: 128,
            ..ExpressionLimits::default()
        };
        let value = ScalarExpression::parse("sqrt(2)", limits)
            .unwrap()
            .evaluate(limits)
            .unwrap();
        assert!(value.lower() * value.lower() <= rational(2, 1));
        assert!(value.upper() * value.upper() >= rational(2, 1));
        assert!(value.upper() - value.lower() <= rational(1, 1 << 30));
    }

    #[test]
    fn square_root_is_outward_at_both_supported_precision_boundaries() {
        let tiny = BigRational::new(BigInt::one(), BigInt::from(10_u8).pow(20));
        for precision_bits in [MIN_PRECISION_BITS, MAX_PRECISION_BITS] {
            let limits = ExpressionLimits {
                precision_bits,
                ..ExpressionLimits::default()
            };
            let grid_width = BigRational::new(BigInt::one(), BigInt::one() << precision_bits);
            for (source, radicand) in [
                ("sqrt(2)", rational(2, 1)),
                ("sqrt(1/3)", rational(1, 3)),
                ("sqrt(1e-20)", tiny.clone()),
            ] {
                let value = ScalarExpression::parse(source, limits)
                    .unwrap()
                    .evaluate(limits)
                    .unwrap();
                assert!(
                    value.lower() * value.lower() <= radicand.clone(),
                    "{source}"
                );
                assert!(value.upper() * value.upper() >= radicand, "{source}");
                assert!(
                    value.upper() - value.lower() <= grid_width.clone(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn pi_and_unicode_pi_enclose_the_known_prefix() {
        let limits = ExpressionLimits::default();
        let ascii = ScalarExpression::parse("pi", limits)
            .unwrap()
            .evaluate(limits)
            .unwrap();
        let unicode = ScalarExpression::parse("π", limits)
            .unwrap()
            .evaluate(limits)
            .unwrap();
        assert_eq!(ascii, unicode);
        let prefix_lower = decimal_bracket_endpoint(b"3141592653589793238462643383279", 30);
        let prefix_upper = decimal_bracket_endpoint(b"3141592653589793238462643383280", 30);
        assert!(ascii.lower() > &prefix_lower);
        assert!(ascii.upper() < &prefix_upper);
        assert!(ascii.upper() - ascii.lower() < rational(1, 1 << 60));
    }

    #[test]
    fn machin_interval_is_narrow_at_both_supported_precision_boundaries() {
        for precision_bits in [MIN_PRECISION_BITS, MAX_PRECISION_BITS] {
            let limits = ExpressionLimits {
                precision_bits,
                ..ExpressionLimits::default()
            };
            let value = ScalarExpression::parse("pi", limits)
                .unwrap()
                .evaluate(limits)
                .unwrap();
            let requested_width = BigRational::new(BigInt::one(), BigInt::one() << precision_bits);
            assert!(value.lower() < value.upper());
            assert!(value.upper() - value.lower() < requested_width);
            assert!(value.lower() > &rational(3, 1));
            assert!(value.upper() < &rational(22, 7));
            let (known_lower, known_upper) = if precision_bits == MIN_PRECISION_BITS {
                (
                    decimal_bracket_endpoint(b"3141592653", 9),
                    decimal_bracket_endpoint(b"3141592654", 9),
                )
            } else {
                (
                    decimal_bracket_endpoint(b"3141592653589793238462643383279", 30),
                    decimal_bracket_endpoint(b"3141592653589793238462643383280", 30),
                )
            };
            assert!(value.lower() > &known_lower);
            assert!(value.upper() < &known_upper);
        }
    }

    #[test]
    fn source_is_preserved_and_serde_revalidates_it() {
        let source = " ( 1 / 3 ) + √2 ";
        let expression = ScalarExpression::parse_default(source).unwrap();
        assert_eq!(expression.source(), source);
        let json = serde_json::to_string(&expression).unwrap();
        assert_eq!(json, format!("{source:?}"));
        let decoded: ScalarExpression = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, expression);
        assert_eq!(decoded.footprint, expression.footprint);
        assert!(serde_json::from_str::<ScalarExpression>("\"1 + @\"").is_err());
        let oversized_json = format!("\"{}\"", "1".repeat(HARD_MAX_SOURCE_BYTES + 1));
        assert!(serde_json::from_str::<ScalarExpression>(&oversized_json).is_err());
        assert!(serde_json::from_str::<ScalarExpression>("123").is_err());
    }

    #[test]
    fn syntax_errors_are_explicit_and_implicit_multiplication_is_rejected() {
        assert!(matches!(
            ScalarExpression::parse_default(""),
            Err(ExpressionError::Empty)
        ));
        assert!(matches!(
            ScalarExpression::parse_default("2pi"),
            Err(ExpressionError::UnexpectedToken { .. })
        ));
        assert!(matches!(
            ScalarExpression::parse_default("(1+2"),
            Err(ExpressionError::UnexpectedEnd)
        ));
        assert!(matches!(
            ScalarExpression::parse_default("1 + @"),
            Err(ExpressionError::InvalidToken { .. })
        ));
    }

    #[test]
    fn undefined_arithmetic_never_produces_a_silent_number() {
        assert_eq!(
            ScalarExpression::parse_default("1/0")
                .unwrap()
                .evaluate_default(),
            Err(ExpressionError::DivisionByZero),
        );
        assert_eq!(
            ScalarExpression::parse_default("sqrt(-1)")
                .unwrap()
                .evaluate_default(),
            Err(ExpressionError::NegativeSquareRoot),
        );
        assert_eq!(
            ScalarExpression::parse_default("1/(sqrt(2)-sqrt(2))")
                .unwrap()
                .evaluate_default(),
            Err(ExpressionError::DivisionByZero),
        );
    }

    #[test]
    fn all_parser_and_evaluator_resources_fail_closed() {
        let defaults = ExpressionLimits::default();

        let mut source = defaults;
        source.max_source_bytes = 3;
        assert_eq!(
            ScalarExpression::parse("1234", source),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::SourceBytes,
            )),
        );

        let mut tokens = defaults;
        tokens.max_tokens = 2;
        assert_eq!(
            ScalarExpression::parse("1+2", tokens),
            Err(ExpressionError::ResourceLimit(ExpressionResource::Tokens)),
        );

        let mut nodes = defaults;
        nodes.max_ast_nodes = 2;
        assert_eq!(
            ScalarExpression::parse("1+2", nodes),
            Err(ExpressionError::ResourceLimit(ExpressionResource::AstNodes)),
        );

        let mut depth = defaults;
        depth.max_nesting_depth = 2;
        assert_eq!(
            ScalarExpression::parse("(((1)))", depth),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::NestingDepth,
            )),
        );

        let mut digits = defaults;
        digits.max_literal_digits = 3;
        assert_eq!(
            ScalarExpression::parse("1234", digits),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::LiteralDigits,
            )),
        );

        let mut exponent = defaults;
        exponent.max_decimal_exponent = 3;
        assert_eq!(
            ScalarExpression::parse("1e4", exponent),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::DecimalExponent,
            )),
        );

        let expression = ScalarExpression::parse_default("pi").unwrap();
        let mut operations = defaults;
        operations.max_operations = 1;
        assert_eq!(
            expression.evaluate(operations),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::Operations,
            )),
        );
    }

    #[test]
    fn evaluation_is_deterministic_at_each_precision() {
        let expression = ScalarExpression::parse_default("pi * sqrt(2) - 1/7").unwrap();
        let limits = ExpressionLimits {
            precision_bits: 96,
            ..ExpressionLimits::default()
        };
        let first = expression.evaluate(limits).unwrap();
        let second = expression.evaluate(limits).unwrap();
        assert_eq!(first, second);
        assert!(first.operations() > 0);
        assert_eq!(first.precision_bits(), 96);
    }

    #[test]
    fn malformed_limits_and_extreme_exponents_are_rejected_before_work() {
        let limits = ExpressionLimits {
            precision_bits: 8,
            ..ExpressionLimits::default()
        };
        assert_eq!(
            ScalarExpression::parse("1", limits),
            Err(ExpressionError::PrecisionOutOfRange),
        );
        assert!(matches!(
            ScalarExpression::parse_default("1e999999999999999999999999"),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::DecimalExponent,
            ))
        ));
    }

    #[test]
    fn decimal_bit_estimate_has_an_allocation_preflight_boundary() {
        for (source, digit_count, scale_magnitude, estimated_bits) in [
            ("1e2", 1, 2, 12),
            ("-1e2", 1, 2, 12),
            ("1.25e2", 3, 0, 12),
            ("-1.25e-2", 3, 4, 28),
            ("0.001", 4, 3, 28),
        ] {
            assert_eq!(
                estimated_decimal_literal_bits(digit_count, scale_magnitude),
                Ok(estimated_bits),
                "{source}"
            );
            let at_boundary = ExpressionLimits {
                max_value_bits: estimated_bits,
                ..ExpressionLimits::default()
            };
            assert!(
                ScalarExpression::parse(source, at_boundary).is_ok(),
                "{source}"
            );

            let one_bit_short = ExpressionLimits {
                max_value_bits: estimated_bits - 1,
                ..ExpressionLimits::default()
            };
            assert_eq!(
                ScalarExpression::parse(source, one_bit_short),
                Err(ExpressionError::ResourceLimit(
                    ExpressionResource::ValueBits,
                )),
                "{source}"
            );
        }
        assert_eq!(
            estimated_decimal_literal_bits(usize::MAX, 1),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ))
        );
    }

    #[test]
    fn pi_tolerance_denominator_checks_its_high_bit_before_shifting() {
        let exponent = MIN_PRECISION_BITS + 8;
        let required_bits = exponent + 1;
        let denominator = checked_power_of_two(exponent, required_bits).unwrap();
        assert_eq!(bit_length_bigint(&denominator), required_bits);
        assert_eq!(
            checked_power_of_two(exponent, required_bits - 1),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ))
        );
        assert_eq!(
            checked_power_of_two(usize::MAX, HARD_MAX_VALUE_BITS),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ))
        );

        let expression = ScalarExpression::parse_default("pi").unwrap();
        let one_bit_short = ExpressionLimits {
            precision_bits: MIN_PRECISION_BITS,
            max_value_bits: required_bits - 1,
            ..ExpressionLimits::default()
        };
        assert_eq!(
            expression.evaluate(one_bit_short),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::ValueBits,
            ))
        );
    }

    #[test]
    fn ordering_of_interval_products_and_negative_divisors_is_correct() {
        let expression = ScalarExpression::parse_default("-sqrt(2) / -2").unwrap();
        let value = expression.evaluate_default().unwrap();
        assert!(value.lower().is_positive());
        assert!(value.upper().is_positive());
        assert!(value.lower() < value.upper());
        assert!(value.lower() * value.lower() <= rational(1, 2));
        assert!(value.upper() * value.upper() >= rational(1, 2));
    }

    #[test]
    fn interval_arithmetic_encloses_every_sign_quadrant() {
        let intervals = [
            Interval {
                lower: rational(-5, 1),
                upper: rational(-2, 1),
            },
            Interval {
                lower: rational(-3, 1),
                upper: rational(4, 1),
            },
            Interval {
                lower: rational(0, 1),
                upper: rational(5, 1),
            },
            Interval {
                lower: rational(2, 1),
                upper: rational(7, 1),
            },
        ];

        for left in &intervals {
            for right in &intervals {
                let mut meter = EvaluationMeter::new(ExpressionLimits::default());
                let product = multiply_intervals(left, right, &mut meter).unwrap();
                for left_sample in interval_samples(left) {
                    for right_sample in interval_samples(right) {
                        assert_interval_contains(
                            &product,
                            &(left_sample.clone() * right_sample.clone()),
                        );
                        let sum = Interval {
                            lower: &left.lower + &right.lower,
                            upper: &left.upper + &right.upper,
                        };
                        assert_interval_contains(
                            &sum,
                            &(left_sample.clone() + right_sample.clone()),
                        );
                        let difference = Interval {
                            lower: &left.lower - &right.upper,
                            upper: &left.upper - &right.lower,
                        };
                        assert_interval_contains(
                            &difference,
                            &(left_sample.clone() - right_sample.clone()),
                        );
                        if right.lower > BigRational::zero() || right.upper < BigRational::zero() {
                            let quotient = divide_intervals(left, right, &mut meter).unwrap();
                            assert_interval_contains(
                                &quotient,
                                &(left_sample.clone() / right_sample),
                            );
                        }
                    }
                }
            }
        }
    }

    fn interval_samples(interval: &Interval) -> [BigRational; 3] {
        [
            interval.lower.clone(),
            (&interval.lower + &interval.upper) / BigInt::from(2_u8),
            interval.upper.clone(),
        ]
    }

    fn assert_interval_contains(interval: &Interval, value: &BigRational) {
        assert!(&interval.lower <= value);
        assert!(value <= &interval.upper);
    }

    #[test]
    fn every_public_hard_ceiling_accepts_its_boundary_and_rejects_one_more() {
        let defaults = ExpressionLimits::default();
        assert_eq!(defaults.max_source_bytes, HARD_MAX_SOURCE_BYTES);
        assert_eq!(defaults.max_tokens, HARD_MAX_TOKENS);
        assert_eq!(defaults.max_ast_nodes, HARD_MAX_AST_NODES);
        assert_eq!(defaults.max_nesting_depth, HARD_MAX_NESTING_DEPTH);
        assert_eq!(defaults.max_literal_digits, HARD_MAX_LITERAL_DIGITS);
        assert_eq!(defaults.max_decimal_exponent, HARD_MAX_DECIMAL_EXPONENT);
        assert_eq!(defaults.max_operations, HARD_MAX_OPERATIONS);
        assert_eq!(defaults.max_value_bits, HARD_MAX_VALUE_BITS);

        macro_rules! check_hard_ceiling {
            ($field:ident, $maximum:ident) => {{
                let mut at_boundary = defaults;
                at_boundary.$field = $maximum;
                assert!(ScalarExpression::parse("1", at_boundary).is_ok());

                let mut above_boundary = defaults;
                above_boundary.$field = $maximum + 1;
                assert_eq!(
                    ScalarExpression::parse("1", above_boundary),
                    Err(ExpressionError::InvalidLimits),
                    stringify!($field),
                );
            }};
        }

        check_hard_ceiling!(max_source_bytes, HARD_MAX_SOURCE_BYTES);
        check_hard_ceiling!(max_tokens, HARD_MAX_TOKENS);
        check_hard_ceiling!(max_ast_nodes, HARD_MAX_AST_NODES);
        check_hard_ceiling!(max_nesting_depth, HARD_MAX_NESTING_DEPTH);
        check_hard_ceiling!(max_literal_digits, HARD_MAX_LITERAL_DIGITS);
        check_hard_ceiling!(max_decimal_exponent, HARD_MAX_DECIMAL_EXPONENT);
        check_hard_ceiling!(max_operations, HARD_MAX_OPERATIONS);
        check_hard_ceiling!(max_value_bits, HARD_MAX_VALUE_BITS);

        for precision_bits in [MIN_PRECISION_BITS, MAX_PRECISION_BITS] {
            let limits = ExpressionLimits {
                precision_bits,
                ..defaults
            };
            assert!(ScalarExpression::parse("1", limits).is_ok());
        }
        for precision_bits in [MIN_PRECISION_BITS - 1, MAX_PRECISION_BITS + 1] {
            let limits = ExpressionLimits {
                precision_bits,
                ..defaults
            };
            assert_eq!(
                ScalarExpression::parse("1", limits),
                Err(ExpressionError::PrecisionOutOfRange),
            );
        }
    }

    #[test]
    fn evaluation_reapplies_every_parse_footprint_limit() {
        let expression = ScalarExpression::parse_default("1.25e2 + sqrt(4)").unwrap();
        let footprint = expression.footprint;
        assert!(footprint.source_bytes > 1);
        assert!(footprint.tokens > 1);
        assert!(footprint.ast_nodes > 1);
        assert!(footprint.nesting_depth > 1);
        assert!(footprint.literal_digits > 1);
        assert!(footprint.decimal_exponent > 1);

        macro_rules! check_smaller_limit {
            ($field:ident, $observed:ident, $resource:ident) => {{
                let mut limits = ExpressionLimits::default();
                limits.$field = footprint.$observed - 1;
                assert_eq!(
                    expression.evaluate(limits),
                    Err(ExpressionError::ResourceLimit(
                        ExpressionResource::$resource,
                    )),
                    stringify!($field),
                );
            }};
        }

        check_smaller_limit!(max_source_bytes, source_bytes, SourceBytes);
        check_smaller_limit!(max_tokens, tokens, Tokens);
        check_smaller_limit!(max_ast_nodes, ast_nodes, AstNodes);
        check_smaller_limit!(max_nesting_depth, nesting_depth, NestingDepth);
        check_smaller_limit!(max_literal_digits, literal_digits, LiteralDigits);
        check_smaller_limit!(max_decimal_exponent, decimal_exponent, DecimalExponent);

        let fractional = ScalarExpression::parse_default("0.001").unwrap();
        let fractional_limits = ExpressionLimits {
            max_decimal_exponent: 2,
            ..ExpressionLimits::default()
        };
        assert_eq!(
            fractional.evaluate(fractional_limits),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::DecimalExponent,
            )),
        );

        let exact_footprint_limits = ExpressionLimits {
            max_source_bytes: footprint.source_bytes,
            max_tokens: footprint.tokens,
            max_ast_nodes: footprint.ast_nodes,
            max_nesting_depth: footprint.nesting_depth,
            max_literal_digits: footprint.literal_digits,
            max_decimal_exponent: footprint.decimal_exponent,
            ..ExpressionLimits::default()
        };
        assert_eq!(
            expression.evaluate(exact_footprint_limits).unwrap().lower(),
            &rational(127, 1),
        );
    }

    #[test]
    fn oversized_whitespace_is_rejected_before_empty_scanning_or_ownership() {
        let source = " ".repeat(HARD_MAX_SOURCE_BYTES + 1);
        assert_eq!(
            ScalarExpression::parse_default(&source),
            Err(ExpressionError::ResourceLimit(
                ExpressionResource::SourceBytes,
            )),
        );
    }

    #[test]
    fn the_largest_left_associative_ast_has_a_stack_safe_full_lifecycle() {
        let source = std::iter::repeat_n("1", HARD_MAX_TOKENS.div_ceil(2))
            .collect::<Vec<_>>()
            .join("+");
        let expected = rational(HARD_MAX_TOKENS.div_ceil(2) as i64, 1);
        let worker = std::thread::Builder::new()
            .stack_size(32 * 1_024)
            .spawn(move || {
                let expression = ScalarExpression::parse_default(source)?;
                assert_eq!(expression.footprint.tokens, HARD_MAX_TOKENS - 1);
                assert_eq!(expression.footprint.ast_nodes, HARD_MAX_AST_NODES - 1);

                let cloned = expression.clone();
                let debug = format!("{cloned:?}");
                assert!(debug.contains("ScalarExpression"));
                assert!(debug.contains("nodes"));
                let result = cloned.evaluate_default();
                drop(debug);
                drop(cloned);
                drop(expression);
                result
            })
            .unwrap();
        let value = worker.join().unwrap().unwrap();
        assert_eq!(value.lower(), &expected);
        assert_eq!(value.upper(), &expected);
    }

    #[test]
    fn binary64_conversion_is_exact_for_representable_values() {
        for (source, expected) in [("0", 0.0), ("-0", 0.0), ("3 / 2", 1.5), ("-3 / 2", -1.5)] {
            let expression = ScalarExpression::parse_default(source).unwrap();
            let value = expression.evaluate_default().unwrap();
            let interval = value.certified_f64_interval().unwrap();

            assert_eq!(interval.lower(), expected, "{source}");
            assert_eq!(interval.upper(), expected, "{source}");
            assert!(!interval.lower().is_sign_negative() || interval.lower() != 0.0);
            assert!(!interval.upper().is_sign_negative() || interval.upper() != 0.0);
        }
    }

    #[test]
    fn binary64_conversion_rounds_each_rational_endpoint_outwards() {
        for source in [
            "1 / 10", "-1 / 10", "sqrt(2)", "-sqrt(2)", "1e-400", "-1e-400",
        ] {
            let expression = ScalarExpression::parse_default(source).unwrap();
            let value = expression.evaluate_default().unwrap();
            let interval = value.certified_f64_interval().unwrap();
            let lower = BigRational::from_float(interval.lower()).unwrap();
            let upper = BigRational::from_float(interval.upper()).unwrap();

            assert!(lower <= *value.lower(), "{source}");
            assert!(*value.upper() <= upper, "{source}");
            assert!(interval.lower() <= interval.upper(), "{source}");
            assert!(interval.lower().is_finite(), "{source}");
            assert!(interval.upper().is_finite(), "{source}");
            assert!(!interval.lower().is_sign_negative() || interval.lower() != 0.0);
            assert!(!interval.upper().is_sign_negative() || interval.upper() != 0.0);
        }
    }

    #[test]
    fn binary64_conversion_rejects_an_endpoint_outside_the_finite_range() {
        let expression = ScalarExpression::parse_default("1e400").unwrap();
        let value = expression.evaluate_default().unwrap();

        assert_eq!(
            value.certified_f64_interval(),
            Err(F64IntervalError::NonFinite),
        );
    }

    #[test]
    fn binary64_conversion_covers_maximum_finite_boundaries_in_both_signs() {
        let maximum = BigRational::from_float(f64::MAX).unwrap();
        for value in [
            maximum.clone(),
            &maximum - BigInt::one(),
            -maximum.clone(),
            -maximum.clone() + BigInt::one(),
        ] {
            assert_exact_rational_has_f64_enclosure(value);
        }
        for outside in [&maximum + BigInt::one(), -maximum - BigInt::one()] {
            let value = exact_high_precision_value(outside);
            assert_eq!(
                value.certified_f64_interval(),
                Err(F64IntervalError::NonFinite)
            );
        }
    }

    #[test]
    fn binary64_conversion_covers_subnormal_and_signed_underflow_boundaries() {
        let minimum_subnormal = BigRational::from_float(f64::from_bits(1)).unwrap();
        for value in [
            minimum_subnormal.clone(),
            &minimum_subnormal / BigInt::from(2_u8),
            &minimum_subnormal * BigInt::from(3_u8) / BigInt::from(2_u8),
            -minimum_subnormal.clone(),
            -minimum_subnormal.clone() / BigInt::from(2_u8),
            -minimum_subnormal * BigInt::from(3_u8) / BigInt::from(2_u8),
        ] {
            assert_exact_rational_has_f64_enclosure(value);
        }
    }

    #[test]
    fn binary64_conversion_handles_high_bit_rationals_without_decimal_fallback() {
        let denominator = BigInt::one() << 32_700_usize;
        let positive = BigRational::new(&denominator + BigInt::one(), denominator);
        assert!(bit_length_bigint(positive.numer()) > 32_000);
        assert!(bit_length_bigint(positive.denom()) > 32_000);

        assert_exact_rational_has_f64_enclosure(positive.clone());
        assert_exact_rational_has_f64_enclosure(-positive);
    }

    fn exact_high_precision_value(value: BigRational) -> HighPrecisionValue {
        HighPrecisionValue {
            lower: value.clone(),
            upper: value,
            precision_bits: 192,
            operations: 0,
        }
    }

    fn assert_exact_rational_has_f64_enclosure(value: BigRational) {
        let high_precision = exact_high_precision_value(value.clone());
        let interval = high_precision.certified_f64_interval().unwrap();
        let lower = BigRational::from_float(interval.lower()).unwrap();
        let upper = BigRational::from_float(interval.upper()).unwrap();
        assert!(lower <= value);
        assert!(value <= upper);
        assert!(interval.lower().is_finite());
        assert!(interval.upper().is_finite());
        assert!(interval.lower() <= interval.upper());
    }
}
