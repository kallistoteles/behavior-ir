#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Rendered expressions preserve the expression tree (traces and check subjects are evidence):
//! `parse(pretty(e))` has the same structure as `e`. A small reader for the rendered syntax
//! parses `pretty::text(e)`; both sides are printed fully parenthesized and compared. Checked for
//! every sub-expression of every fixture module and for generated expressions.

mod common;

use behavior_core::admit;
use behavior_core::pretty::{text, value_text};
use behavior_core::semantic::expr::{Expr, ExprKind};
use behavior_core::semantic::module::Module;
use behavior_core::semantic::types::{ArithOp, CmpOp, Type, Unit};
use proptest::prelude::*;
use serde_json::{Value, json};

// --- the reader --------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Num(String),
    Str(String),
    Sym(&'static str),
}

const SYMS: [&str; 16] = [
    "==", "!=", "<=", ">=", "<", ">", "+", "-", "*", "/", "(", ")", "[", "]", ",", ".",
];

fn tokenize(s: &str) -> Vec<Tok> {
    let b = s.as_bytes();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c == ' ' {
            i += 1;
            continue;
        }
        // A `-` directly before a digit is a sign where an operand is expected.
        let operand_expected = match out.last() {
            None | Some(Tok::Sym(_)) => !matches!(out.last(), Some(Tok::Sym(")" | "]"))),
            Some(Tok::Ident(w)) => ["and", "or", "not", "in"].contains(&w.as_str()),
            _ => false,
        };
        if c.is_ascii_digit()
            || (c == '-' && operand_expected && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.' || b[i] == b'/') {
                i += 1;
            }
            out.push(Tok::Num(s[start..i].to_string()));
        } else if c == '"' {
            let start = i;
            i += 1;
            while b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push(Tok::Str(s[start..i].to_string()));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Ident(s[start..i].to_string()));
        } else {
            let sym = SYMS
                .iter()
                .find(|x| s[i..].starts_with(**x))
                .unwrap_or_else(|| panic!("unexpected {c:?} in {s}"));
            out.push(Tok::Sym(sym));
            i += sym.len();
        }
    }
    out
}

/// A parsed expression, rendered fully parenthesized by `full`.
enum Node {
    Atom(String),
    Call(String, Vec<Node>),
    Bin(&'static str, Box<Node>, Box<Node>),
    Nary(&'static str, Vec<Node>),
    Not(Box<Node>),
    In(Box<Node>, Vec<Node>),
    Post(Box<Node>, String, Option<Box<Node>>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Tok {
        self.pos += 1;
        self.toks[self.pos - 1].clone()
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if self.peek() == Some(&Tok::Sym(leak(s))) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn eat_word(&mut self, w: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(x)) if x == w) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) {
        assert!(
            self.eat_sym(s),
            "expected {s} at {} in {:?}",
            self.pos,
            self.toks
        );
    }

    fn nary(&mut self, word: &'static str, sub: fn(&mut Parser) -> Node) -> Node {
        let first = sub(self);
        let mut xs = vec![first];
        while self.eat_word(word) {
            xs.push(sub(self));
        }
        if xs.len() == 1 {
            xs.pop().unwrap()
        } else {
            Node::Nary(word, xs)
        }
    }
    fn or(&mut self) -> Node {
        self.nary("or", Parser::and)
    }
    fn and(&mut self) -> Node {
        self.nary("and", Parser::not)
    }
    fn not(&mut self) -> Node {
        if self.eat_word("not") {
            Node::Not(Box::new(self.not()))
        } else {
            self.cmp()
        }
    }
    fn cmp(&mut self) -> Node {
        let a = self.add();
        for op in ["==", "!=", "<=", ">=", "<", ">"] {
            if self.eat_sym(op) {
                let b = self.add();
                return Node::Bin(leak(op), Box::new(a), Box::new(b));
            }
        }
        if self.eat_word("in") {
            self.expect("[");
            let mut vs = Vec::new();
            while !self.eat_sym("]") {
                vs.push(self.or());
                self.eat_sym(",");
            }
            return Node::In(Box::new(a), vs);
        }
        a
    }
    fn binary(&mut self, ops: [&'static str; 2], sub: fn(&mut Parser) -> Node) -> Node {
        let mut a = sub(self);
        loop {
            let Some(op) = ops
                .into_iter()
                .find(|op| self.peek() == Some(&Tok::Sym(op)))
            else {
                return a;
            };
            self.pos += 1;
            let b = sub(self);
            a = Node::Bin(op, Box::new(a), Box::new(b));
        }
    }
    fn add(&mut self) -> Node {
        self.binary(["+", "-"], Parser::mul)
    }
    fn mul(&mut self) -> Node {
        self.binary(["*", "/"], Parser::postfix)
    }
    fn postfix(&mut self) -> Node {
        let mut a = self.primary();
        while self.peek() == Some(&Tok::Sym(".")) {
            self.pos += 1;
            let Tok::Ident(m) = self.next() else {
                panic!("method")
            };
            self.expect("(");
            let arg = if self.eat_sym(")") {
                None
            } else {
                let x = self.or();
                self.expect(")");
                Some(Box::new(x))
            };
            a = Node::Post(Box::new(a), m, arg);
        }
        a
    }
    fn primary(&mut self) -> Node {
        match self.next() {
            Tok::Sym("(") => {
                let x = self.or();
                self.expect(")");
                x
            }
            Tok::Num(n) => Node::Atom(n),
            Tok::Str(s) => Node::Atom(s),
            Tok::Ident(w) => {
                if self.eat_sym("(") {
                    let mut args = Vec::new();
                    while !self.eat_sym(")") {
                        args.push(self.or());
                        self.eat_sym(",");
                    }
                    return Node::Call(w, args);
                }
                // A path `a.b.c`, but not a postfix method call.
                let mut path = w;
                while self.peek() == Some(&Tok::Sym("."))
                    && matches!(self.toks.get(self.pos + 1), Some(Tok::Ident(_)))
                    && self.toks.get(self.pos + 2) != Some(&Tok::Sym("("))
                {
                    self.pos += 1;
                    let Tok::Ident(f) = self.next() else {
                        unreachable!()
                    };
                    path = format!("{path}.{f}");
                }
                Node::Atom(path)
            }
            t => panic!("unexpected {t:?} in {:?}", self.toks),
        }
    }
}

fn leak(s: &str) -> &'static str {
    SYMS.iter()
        .chain(["and", "or"].iter())
        .find(|x| **x == s)
        .copied()
        .unwrap_or_else(|| panic!("{s}"))
}

fn parse(s: &str) -> Node {
    let mut p = Parser {
        toks: tokenize(s),
        pos: 0,
    };
    let n = p.or();
    assert_eq!(p.pos, p.toks.len(), "trailing tokens in {s}");
    n
}

fn join(xs: &[String], sep: &str) -> String {
    xs.join(sep)
}

fn full(n: &Node) -> String {
    match n {
        Node::Atom(a) => a.clone(),
        Node::Call(f, args) => {
            let a: Vec<String> = args.iter().map(full).collect();
            format!("{f}({})", join(&a, ", "))
        }
        Node::Bin(op, a, b) => format!("({} {op} {})", full(a), full(b)),
        Node::Nary(w, xs) => {
            let a: Vec<String> = xs.iter().map(full).collect();
            format!("({})", join(&a, &format!(" {w} ")))
        }
        Node::Not(a) => format!("(not {})", full(a)),
        Node::In(a, vs) => {
            let v: Vec<String> = vs.iter().map(full).collect();
            format!("({} in [{}])", full(a), join(&v, ", "))
        }
        Node::Post(a, m, arg) => match arg {
            Some(x) => format!("[{}].{m}({})", full(a), full(x)),
            None => format!("[{}].{m}()", full(a)),
        },
    }
}

// --- the same form from the expression tree ----------------------------------------------------

fn cmp_sym(c: CmpOp) -> &'static str {
    match c {
        CmpOp::Eq => "==",
        CmpOp::Ne => "!=",
        CmpOp::Lt => "<",
        CmpOp::Le => "<=",
        CmpOp::Gt => ">",
        CmpOp::Ge => ">=",
    }
}

fn arith_sym(a: ArithOp) -> &'static str {
    match a {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
    }
}

/// A literal's text is itself rendered syntax (`Money(0.00)`, `some(1)`): normalize it the same
/// way the reader does.
fn lit(t: &Type, v: &behavior_core::semantic::value::Value) -> String {
    full(&parse(&value_text(t, v)))
}

/// The tree form of a query (feature 007).
fn qtree(q: &behavior_core::semantic::expr::QueryNode) -> String {
    use behavior_core::semantic::expr::QueryKind;
    match q.kind() {
        QueryKind::Select => format!("select({})", q.entity()),
        QueryKind::Where { base, param, body } => format!(
            "where({}, {param}, {})",
            qtree(base),
            tree(body).replace("$c", param)
        ),
        QueryKind::Set { op, a, b } => format!("{}({}, {})", op.as_str(), qtree(a), qtree(b)),
    }
}

fn tree(e: &Expr) -> String {
    match e.kind() {
        ExprKind::Lit(v) => lit(e.ty(), v),
        ExprKind::Field { param, field } => format!("{param}.{field}"),
        ExprKind::Param(n) => n.clone(),
        ExprKind::DerivedRef { name, args, .. } => format!("{name}({})", args.join(", ")),
        ExprKind::Cmp(c, a, b) => format!("({} {} {})", tree(a), cmp_sym(*c), tree(b)),
        ExprKind::Arith(op, a, b) => format!("({} {} {})", tree(a), arith_sym(*op), tree(b)),
        ExprKind::And(xs) | ExprKind::Or(xs) => {
            let w = if matches!(e.kind(), ExprKind::And(_)) {
                " and "
            } else {
                " or "
            };
            let a: Vec<String> = xs.iter().map(tree).collect();
            format!("({})", a.join(w))
        }
        ExprKind::Not(a) => format!("(not {})", tree(a)),
        ExprKind::In(a, vs) => {
            let v: Vec<String> = vs.iter().map(|v| lit(a.ty(), v)).collect();
            format!("({} in [{}])", tree(a), v.join(", "))
        }
        ExprKind::IsNone(a) => format!("[{}].is_none()", tree(a)),
        ExprKind::IsSome(a) => format!("[{}].is_some()", tree(a)),
        ExprKind::ValueOr(a, d) => format!("[{}].value_or({})", tree(a), tree(d)),
        ExprKind::Some(a) => format!("some({})", tree(a)),
        ExprKind::ToDecimal(a) => format!("decimal({})", tree(a)),
        ExprKind::Wrap(a) => {
            let n = match e.ty() {
                Type::Exact(Unit::Nominal(n)) => n.name.clone(),
                t => t.to_string(),
            };
            format!("{n}({})", tree(a))
        }
        ExprKind::Unwrap(a) => format!("underlying({})", tree(a)),
        ExprKind::Exists(a) => format!("exists({})", tree(a)),
        ExprKind::Referenced(a) => format!("referenced({})", tree(a)),
        ExprKind::Count(q) => format!("count({})", qtree(q)),
        ExprKind::Fold {
            op,
            query,
            param,
            body,
        } => format!(
            "{}({}, {param}, {})",
            op.as_str(),
            qtree(query),
            tree(body).replace("$c", param)
        ),
        ExprKind::Rescale { arg, rounding } => {
            format!("rescale({}, {}, {})", tree(arg), e.ty(), rounding.as_str())
        }
        ExprKind::StrictUnwrap(a) => format!("strict_unwrap({})", tree(a)),
        ExprKind::EnumMap {
            arg,
            mapping,
            strict,
        } => {
            let m: Vec<String> = mapping.iter().map(|(f, t)| format!("{f}: {t}")).collect();
            let name = if *strict {
                "strict_enum_map"
            } else {
                "enum_map"
            };
            format!("{name}({}, {{{}}})", tree(arg), m.join(", "))
        }
    }
}

fn children(e: &Expr) -> Vec<&Expr> {
    match e.kind() {
        ExprKind::Cmp(_, a, b) | ExprKind::Arith(_, a, b) | ExprKind::ValueOr(a, b) => {
            vec![a, b]
        }
        ExprKind::And(xs) | ExprKind::Or(xs) => xs.iter().collect(),
        ExprKind::Not(a)
        | ExprKind::In(a, _)
        | ExprKind::IsNone(a)
        | ExprKind::IsSome(a)
        | ExprKind::Some(a)
        | ExprKind::ToDecimal(a)
        | ExprKind::Wrap(a)
        | ExprKind::Unwrap(a)
        | ExprKind::Rescale { arg: a, .. }
        | ExprKind::Exists(a)
        | ExprKind::Referenced(a)
        | ExprKind::StrictUnwrap(a)
        | ExprKind::EnumMap { arg: a, .. } => vec![a],
        ExprKind::Count(_) | ExprKind::Fold { .. } => Vec::new(),
        _ => vec![],
    }
}

/// Problems for `e` and every sub-expression.
fn check(e: &Expr, out: &mut Vec<String>) {
    let rendered = text(e);
    let got = full(&parse(&rendered));
    let want = tree(e);
    if got != want {
        out.push(format!(
            "`{rendered}`\n    reads as {got}\n    tree is  {want}"
        ));
    }
    for c in children(e) {
        check(c, out);
    }
}

fn module_exprs(m: &Module) -> Vec<&Expr> {
    let mut xs: Vec<&Expr> = Vec::new();
    xs.extend(m.derived_items().values().map(|d| d.body()));
    xs.extend(m.invariants().values().map(|i| i.body()));
    xs.extend(m.constraints().values().map(|c| c.body()));
    for a in m.actions().values() {
        xs.extend(a.preconditions().iter().map(|c| c.expr()));
        xs.extend(a.postconditions().iter().map(|c| c.expr()));
        xs.extend(a.effects().iter().map(|e| e.value()));
        for c in a.creates() {
            xs.push(c.id());
            xs.extend(c.fields().iter().map(|(_, v)| v));
        }
    }
    xs
}

#[test]
fn every_fixture_expression_round_trips() {
    let f = common::fixtures();
    let mut files = Vec::new();
    for dir in ["wire/valid", "wire/python", "verify"] {
        files.extend(
            common::files(&f.join(dir), ".json")
                .into_iter()
                .filter(|p| !p.to_string_lossy().ends_with(".expected.json")),
        );
    }
    let mut problems = Vec::new();
    let mut count = 0;
    for file in &files {
        let m = admit(&common::read(file)).unwrap_or_else(|r| panic!("{file:?}: {:?}", r.errors));
        for e in module_exprs(&m) {
            count += 1;
            check(e, &mut problems);
        }
    }
    assert!(count > 100, "only {count} expressions");
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn known_shapes_keep_their_grouping() {
    // Integer fields: `/` lifts its operands to decimals, which the text shows.
    let cases = [
        ("mul", "div", "decimal(r.a) * (decimal(r.b) / decimal(r.c))"),
        ("sub", "add", "r.a - (r.b + r.c)"),
        ("add", "sub", "r.a + (r.b - r.c)"),
        ("mul", "mul", "r.a * (r.b * r.c)"),
        ("div", "mul", "decimal(r.a) / decimal(r.b * r.c)"),
        ("div", "div", "decimal(r.a) / (decimal(r.b) / decimal(r.c))"),
    ];
    for (outer, inner, want) in cases {
        let body = json!({"op": outer, "args": [fld("a"), {"op": inner, "args": [fld("b"), fld("c")], "loc": l()}], "loc": l()});
        let m = admit(&gen_module(body).to_string()).unwrap();
        assert_eq!(text(m.derived("d").unwrap().body()), want);
    }
    let nested = json!({"op": "and", "args": [cmp_lit("a"), {"op": "and", "args": [cmp_lit("b"), cmp_lit("c")], "loc": l()}], "loc": l()});
    let m = admit(&gen_module(nested).to_string()).unwrap();
    assert_eq!(
        text(m.derived("d").unwrap().body()),
        "r.a > 0 and (r.b > 0 and r.c > 0)"
    );
}

// --- generated expressions ---------------------------------------------------------------------

fn l() -> Value {
    json!({"file": "p.py", "line": 1})
}

fn fld(f: &str) -> Value {
    json!({"op": "field", "param": "r", "field": f, "loc": l()})
}

fn cmp_lit(f: &str) -> Value {
    json!({"op": "gt", "args": [fld(f), {"op": "lit", "type": {"t": "int"}, "value": 0, "loc": l()}], "loc": l()})
}

fn gen_module(body: Value) -> Value {
    let int = json!({"t": "int"});
    json!({
        "ir_version": "0.4", "enums": [], "nominals": [], "constraints": [], "invariants": [],
        "actions": [],
        "entities": [{"name": "Row", "loc": l(), "fields": [
            {"name": "a", "type": int, "loc": l()}, {"name": "b", "type": int, "loc": l()},
            {"name": "c", "type": int, "loc": l()}]}],
        "derived": [{"name": "d", "kind": "derived", "loc": l(), "body": body,
                     "params": [{"name": "r", "type": {"t": "entity", "name": "Row"}}]}],
    })
}

fn num() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        prop::sample::select(vec!["a", "b", "c"]).prop_map(fld),
        (-5i64..6).prop_map(|v| json!({"op": "lit", "type": {"t": "int"}, "value": v, "loc": l()})),
        prop::sample::select(vec!["0.5", "-1.25", "2"])
            .prop_map(|v| json!({"op": "lit", "type": {"t": "decimal"}, "value": v, "loc": l()})),
    ];
    leaf.prop_recursive(4, 16, 2, |inner| {
        (
            prop::sample::select(vec!["add", "sub", "mul", "div"]),
            inner.clone(),
            inner,
        )
            .prop_map(|(op, a, b)| json!({"op": op, "args": [a, b], "loc": l()}))
    })
}

fn boolean() -> impl Strategy<Value = Value> {
    let leaf = (
        prop::sample::select(vec!["lt", "le", "eq", "ne"]),
        num(),
        num(),
    )
        .prop_map(|(op, a, b)| json!({"op": op, "args": [a, b], "loc": l()}));
    leaf.prop_recursive(3, 12, 3, |inner| {
        prop_oneof![
            (
                prop::sample::select(vec!["and", "or"]),
                prop::collection::vec(inner.clone(), 2..4)
            )
                .prop_map(|(op, xs)| json!({"op": op, "args": xs, "loc": l()})),
            inner.prop_map(|x| json!({"op": "not", "args": [x], "loc": l()})),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn generated_expressions_round_trip(body in boolean()) {
        // Skip the rare expression whose exact bound exceeds the runtime's representation.
        let Ok(m) = admit(&gen_module(body).to_string()) else { return Ok(()); };
        let mut problems = Vec::new();
        check(m.derived("d").unwrap().body(), &mut problems);
        prop_assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    }
}
