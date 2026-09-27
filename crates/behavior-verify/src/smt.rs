//! SMT-LIB 2 text: a small s-expression reader for solver output, value conversion, and
//! literal helpers for writing queries (research R1).

use behavior_core::decimal::Dec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sexp {
    Atom(String),
    Str(String),
    List(Vec<Sexp>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SmtError {
    #[error("malformed solver output: {0}")]
    Parse(String),
    #[error("unsupported value: {0}")]
    Value(String),
}

/// Parses every s-expression in `text`.
pub fn parse(text: &str) -> Result<Vec<Sexp>, SmtError> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while let Some(s) = next(&chars, &mut i)? {
        out.push(s);
    }
    Ok(out)
}

fn skip_ws(c: &[char], i: &mut usize) {
    while *i < c.len() {
        if c[*i].is_whitespace() {
            *i += 1;
        } else if c[*i] == ';' {
            while *i < c.len() && c[*i] != '\n' {
                *i += 1;
            }
        } else {
            break;
        }
    }
}

fn next(c: &[char], i: &mut usize) -> Result<Option<Sexp>, SmtError> {
    skip_ws(c, i);
    if *i >= c.len() {
        return Ok(None);
    }
    match c[*i] {
        '(' => {
            *i += 1;
            let mut items = Vec::new();
            loop {
                skip_ws(c, i);
                if *i >= c.len() {
                    return Err(SmtError::Parse("unclosed list".into()));
                }
                if c[*i] == ')' {
                    *i += 1;
                    return Ok(Some(Sexp::List(items)));
                }
                match next(c, i)? {
                    Some(s) => items.push(s),
                    None => return Err(SmtError::Parse("unclosed list".into())),
                }
            }
        }
        ')' => Err(SmtError::Parse("unexpected `)`".into())),
        '"' => {
            *i += 1;
            let mut s = String::new();
            loop {
                if *i >= c.len() {
                    return Err(SmtError::Parse("unclosed string".into()));
                }
                if c[*i] == '"' {
                    if *i + 1 < c.len() && c[*i + 1] == '"' {
                        s.push('"');
                        *i += 2;
                        continue;
                    }
                    *i += 1;
                    return Ok(Some(Sexp::Str(unescape(&s))));
                }
                s.push(c[*i]);
                *i += 1;
            }
        }
        '|' => {
            *i += 1;
            let start = *i;
            while *i < c.len() && c[*i] != '|' {
                *i += 1;
            }
            let atom: String = c[start..*i].iter().collect();
            *i += 1;
            Ok(Some(Sexp::Atom(atom)))
        }
        _ => {
            let start = *i;
            while *i < c.len() && !c[*i].is_whitespace() && c[*i] != '(' && c[*i] != ')' {
                *i += 1;
            }
            Ok(Some(Sexp::Atom(c[start..*i].iter().collect())))
        }
    }
}

/// Decodes Z3's `\u{XX}` escapes in string literals.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(pos) = rest.find("\\u{") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 3..];
        match after.find('}') {
            Some(end) => {
                match u32::from_str_radix(&after[..end], 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    Some(ch) => out.push(ch),
                    None => out.push_str(&rest[pos..pos + 3 + end + 1]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[pos..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// A value from a solver model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtValue {
    Bool(bool),
    Int(i128),
    /// Numerator and positive denominator, in lowest terms.
    Rational(i128, i128),
    Str(String),
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

fn rational(num: i128, den: i128) -> Result<SmtValue, SmtError> {
    if den == 0 {
        return Err(SmtError::Value("zero denominator".into()));
    }
    let g = gcd(num, den);
    let (n, d) = (num / g, den / g);
    Ok(if d < 0 {
        SmtValue::Rational(-n, -d)
    } else {
        SmtValue::Rational(n, d)
    })
}

/// Parses a decimal atom such as `3.0` or `12.5` into a rational.
fn decimal_atom(a: &str) -> Result<SmtValue, SmtError> {
    let (int, frac) = a.split_once('.').unwrap_or((a, ""));
    let digits = format!("{int}{frac}");
    let num: i128 = digits.parse().map_err(|_| SmtError::Value(a.into()))?;
    let den = 10i128
        .checked_pow(u32::try_from(frac.len()).map_err(|_| SmtError::Value(a.into()))?)
        .ok_or_else(|| SmtError::Value(a.into()))?;
    rational(num, den)
}

fn negate(v: SmtValue) -> Result<SmtValue, SmtError> {
    match v {
        SmtValue::Int(i) => Ok(SmtValue::Int(-i)),
        SmtValue::Rational(n, d) => Ok(SmtValue::Rational(-n, d)),
        other => Err(SmtError::Value(format!("cannot negate {other:?}"))),
    }
}

/// Converts a model value expression to a value.
pub fn value(s: &Sexp) -> Result<SmtValue, SmtError> {
    match s {
        Sexp::Str(t) => Ok(SmtValue::Str(t.clone())),
        Sexp::Atom(a) if a == "true" => Ok(SmtValue::Bool(true)),
        Sexp::Atom(a) if a == "false" => Ok(SmtValue::Bool(false)),
        Sexp::Atom(a) if a.contains('.') => decimal_atom(a),
        Sexp::Atom(a) => a
            .parse::<i128>()
            .map(SmtValue::Int)
            .map_err(|_| SmtError::Value(a.clone())),
        Sexp::List(items) => match items.as_slice() {
            [Sexp::Atom(op), x] if op == "-" => negate(value(x)?),
            [Sexp::Atom(op), n, d] if op == "/" => {
                let to_rat = |v: SmtValue| match v {
                    SmtValue::Int(i) => Ok((i, 1)),
                    SmtValue::Rational(n, d) => Ok((n, d)),
                    other => Err(SmtError::Value(format!("{other:?}"))),
                };
                let (n1, d1) = to_rat(value(n)?)?;
                let (n2, d2) = to_rat(value(d)?)?;
                let num = n1
                    .checked_mul(d2)
                    .ok_or_else(|| SmtError::Value("overflow".into()))?;
                let den = d1
                    .checked_mul(n2)
                    .ok_or_else(|| SmtError::Value("overflow".into()))?;
                rational(num, den)
            }
            _ => Err(SmtError::Value(format!("{s:?}"))),
        },
    }
}

impl SmtValue {
    /// The value as a normalized engine decimal, if it is exactly representable.
    pub fn to_decimal_string(&self) -> Option<String> {
        let (num, den) = match self {
            SmtValue::Int(i) => (*i, 1),
            SmtValue::Rational(n, d) => (*n, *d),
            _ => return None,
        };
        // A finite decimal expansion exists only if the denominator is 2^a · 5^b.
        let (mut d, mut twos, mut fives) = (den, 0u32, 0u32);
        while d % 2 == 0 {
            d /= 2;
            twos += 1;
        }
        while d % 5 == 0 {
            d /= 5;
            fives += 1;
        }
        if d != 1 {
            return None;
        }
        let scale = twos.max(fives);
        let factor = 10i128.checked_pow(scale)? / den;
        let scaled = num.checked_mul(factor)?;
        let negative = scaled < 0;
        let digits = scaled.unsigned_abs().to_string();
        let scale = scale as usize;
        let text = if scale == 0 {
            digits
        } else {
            let padded = format!("{digits:0>width$}", width = scale + 1);
            let (int, frac) = padded.split_at(padded.len() - scale);
            format!("{int}.{frac}")
        };
        let text = if negative { format!("-{text}") } else { text };
        Dec::parse_str(&text).ok().map(|d| d.to_normalized_string())
    }
}

/// An SMT-LIB string literal.
pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\"\""),
            c if (' '..='~').contains(&c) && c != '\\' => out.push(c),
            c => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
        }
    }
    out.push('"');
    out
}

/// An SMT-LIB integer literal.
pub fn int_lit(i: i128) -> String {
    if i < 0 {
        format!("(- {})", i.unsigned_abs())
    } else {
        i.to_string()
    }
}

/// An SMT-LIB real literal from a normalized decimal string (`"-12.5"` → `(- 12.5)`).
pub fn real_lit(decimal: &str) -> String {
    let (negative, body) = match decimal.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, decimal),
    };
    let body = if body.contains('.') {
        body.to_string()
    } else {
        format!("{body}.0")
    };
    if negative {
        format!("(- {body})")
    } else {
        body
    }
}
