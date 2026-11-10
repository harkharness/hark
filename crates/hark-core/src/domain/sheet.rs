//! A spreadsheet page: what the agent writes as json, what Hark shows.
//!
//! The file is deliberately small so a weak model writes it right:
//!
//! ```json
//! { "title": "Custos", "tabs": [
//!   { "name": "Mensal", "columns": ["mês", "infra"],
//!     "rows": [["set/26", 412], ["out/26", "=B2*1.03"], ["média", "=MÉDIA(B2:B3)"]] } ] }
//! ```
//!
//! Row 1 is the header (`columns`); data rows start at 2, so `B2` is the
//! first value under the second column. A cell is a number, a text, a
//! bool, null, or a formula: a text starting with `=`. Hark computes the
//! formulas here, with a small engine (no token spent): arithmetic, cell
//! refs and ranges, comparisons, and a dozen functions in Portuguese or
//! English. What the engine does not know shows as an error in the cell,
//! named so the agent can fix it; the file is never refused for it.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sheet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Tab {
    pub name: String,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<serde_json::Value>>,
}

/// One cell as the viewer shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    /// What the file holds: the formula text, or the literal as text.
    pub raw: String,
    /// What the cell shows: the computed value as text ("" when empty).
    pub text: String,
    pub formula: bool,
    /// The numeric value when the cell is a number (the viewer formats it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    /// An engine error, named ("#NOME?", "#CICLO!", …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputedTab {
    pub name: String,
    pub columns: Vec<String>,
    pub cells: Vec<Vec<Shown>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Computed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub tabs: Vec<ComputedTab>,
}

/// A cell by position: tab, data row (0 = row 2 of the grid), column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct CellRef {
    pub tab: usize,
    pub row: usize,
    pub col: usize,
}

/// The file as a sheet, or why it is not one. A sheet without tabs is a
/// sheet (empty); malformed json is the error the agent needs to see.
pub fn parse(json: &str) -> Result<Sheet, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("não é json: {e}"))?;
    if !v.is_object() {
        return Err("a planilha é um objeto com \"tabs\"".into());
    }
    let sheet: Sheet = serde_json::from_value(v).map_err(|e| format!("planilha inválida: {e}"))?;
    for (i, tab) in sheet.tabs.iter().enumerate() {
        if tab.name.trim().is_empty() {
            return Err(format!("aba {}: sem \"name\"", i + 1));
        }
    }
    Ok(sheet)
}

/// A column index as a letter name: 0 → A, 25 → Z, 26 → AA.
pub fn column_name(mut i: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push((b'A' + (i % 26) as u8) as char);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    out.iter().rev().collect()
}

/// A1 → (row 1, col 0); the row is the grid row (1 = header).
fn parse_ref(s: &str) -> Option<(usize, usize)> {
    let letters: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &s[letters.len()..];
    if letters.is_empty() || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut col = 0usize;
    for c in letters.chars() {
        col = col * 26 + (c.to_ascii_uppercase() as u8 - b'A') as usize + 1;
    }
    let row: usize = digits.parse().ok()?;
    if row == 0 {
        return None;
    }
    Some((row, col - 1))
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Number(f64),
    Text(String),
    Bool(bool),
    Empty,
    Error(String),
}

impl Value {
    fn as_number(&self) -> Result<f64, String> {
        match self {
            Value::Number(n) => Ok(*n),
            Value::Empty => Ok(0.0),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Text(t) => t.trim().replace(',', ".").parse::<f64>().map_err(|_| "#VALOR!".to_string()),
            Value::Error(e) => Err(e.clone()),
        }
    }
    fn as_text(&self) -> String {
        match self {
            Value::Number(n) => format_number(*n),
            Value::Text(t) => t.clone(),
            Value::Bool(b) => if *b { "VERDADEIRO".into() } else { "FALSO".into() },
            Value::Empty => String::new(),
            Value::Error(e) => e.clone(),
        }
    }
    fn truthy(&self) -> Result<bool, String> {
        match self {
            Value::Bool(b) => Ok(*b),
            Value::Number(n) => Ok(*n != 0.0),
            Value::Empty => Ok(false),
            Value::Text(t) => Ok(!t.is_empty()),
            Value::Error(e) => Err(e.clone()),
        }
    }
}

/// Up to two decimals, none when whole; always a dot (the viewer localises).
fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

// ---------- the formula language ----------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(String),
    LParen,
    RParen,
    Sep,
    Colon,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            out.push(Tok::Num(s.parse().map_err(|_| "#ERRO".to_string())?));
        } else if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return Err("#ERRO".into()),
                    Some('"') if chars.get(i + 1) == Some(&'"') => {
                        s.push('"');
                        i += 2;
                    }
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            out.push(Tok::Str(s));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.' || chars[i] == '$') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().filter(|c| **c != '$').collect()));
        } else {
            i += 1;
            match c {
                '(' => out.push(Tok::LParen),
                ')' => out.push(Tok::RParen),
                ';' | ',' => out.push(Tok::Sep),
                ':' => out.push(Tok::Colon),
                '<' if chars.get(i) == Some(&'>') => {
                    i += 1;
                    out.push(Tok::Op("<>".into()));
                }
                '<' | '>' if chars.get(i) == Some(&'=') => {
                    i += 1;
                    out.push(Tok::Op(format!("{c}=")));
                }
                '+' | '-' | '*' | '/' | '^' | '&' | '=' | '<' | '>' => out.push(Tok::Op(c.to_string())),
                '%' => out.push(Tok::Op("%".into())),
                _ => return Err("#ERRO".into()),
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Lit(Value),
    Ref(usize, usize),
    Range((usize, usize), (usize, usize)),
    Neg(Box<Expr>),
    Bin(String, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.at).cloned();
        self.at += 1;
        t
    }
    fn expr(&mut self) -> Result<Expr, String> {
        self.compare()
    }
    fn compare(&mut self) -> Result<Expr, String> {
        let mut left = self.concat()?;
        while let Some(Tok::Op(op)) = self.peek() {
            if !matches!(op.as_str(), "=" | "<>" | "<" | ">" | "<=" | ">=") {
                break;
            }
            let op = op.clone();
            self.next();
            let right = self.concat()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn concat(&mut self) -> Result<Expr, String> {
        let mut left = self.add()?;
        while let Some(Tok::Op(op)) = self.peek() {
            if op != "&" {
                break;
            }
            self.next();
            let right = self.add()?;
            left = Expr::Bin("&".into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn add(&mut self) -> Result<Expr, String> {
        let mut left = self.mul()?;
        while let Some(Tok::Op(op)) = self.peek() {
            if op != "+" && op != "-" {
                break;
            }
            let op = op.clone();
            self.next();
            let right = self.mul()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn mul(&mut self) -> Result<Expr, String> {
        let mut left = self.pow()?;
        while let Some(Tok::Op(op)) = self.peek() {
            if op != "*" && op != "/" {
                break;
            }
            let op = op.clone();
            self.next();
            let right = self.pow()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn pow(&mut self) -> Result<Expr, String> {
        let base = self.unary()?;
        if let Some(Tok::Op(op)) = self.peek() {
            if op == "^" {
                self.next();
                let exp = self.pow()?;
                return Ok(Expr::Bin("^".into(), Box::new(base), Box::new(exp)));
            }
        }
        Ok(base)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Tok::Op(op)) if op == "-" => {
                self.next();
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Some(Tok::Op(op)) if op == "+" => {
                self.next();
                self.unary()
            }
            _ => self.postfix(),
        }
    }
    fn postfix(&mut self) -> Result<Expr, String> {
        let e = self.atom()?;
        if let Some(Tok::Op(op)) = self.peek() {
            if op == "%" {
                self.next();
                return Ok(Expr::Bin("/".into(), Box::new(e), Box::new(Expr::Lit(Value::Number(100.0)))));
            }
        }
        Ok(e)
    }
    fn atom(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Expr::Lit(Value::Number(n))),
            Some(Tok::Str(s)) => Ok(Expr::Lit(Value::Text(s))),
            Some(Tok::LParen) => {
                let e = self.expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    _ => Err("#ERRO".into()),
                }
            }
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() == Some(&Tok::RParen) {
                        self.next();
                        return Ok(Expr::Call(name, args));
                    }
                    loop {
                        args.push(self.expr()?);
                        match self.next() {
                            Some(Tok::Sep) => continue,
                            Some(Tok::RParen) => break,
                            _ => return Err("#ERRO".into()),
                        }
                    }
                    return Ok(Expr::Call(name, args));
                }
                let upper = name.to_uppercase();
                match upper.as_str() {
                    "VERDADEIRO" | "TRUE" => return Ok(Expr::Lit(Value::Bool(true))),
                    "FALSO" | "FALSE" => return Ok(Expr::Lit(Value::Bool(false))),
                    _ => {}
                }
                let Some(a) = parse_ref(&upper) else { return Err("#NOME?".into()) };
                if self.peek() == Some(&Tok::Colon) {
                    self.next();
                    match self.next() {
                        Some(Tok::Ident(b)) => {
                            let b = parse_ref(&b.to_uppercase()).ok_or("#REF!")?;
                            return Ok(Expr::Range(a, b));
                        }
                        _ => return Err("#REF!".into()),
                    }
                }
                Ok(Expr::Ref(a.0, a.1))
            }
            _ => Err("#ERRO".into()),
        }
    }
}

fn parse_formula(src: &str) -> Result<Expr, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, at: 0 };
    let e = p.expr()?;
    if p.at != p.toks.len() {
        return Err("#ERRO".into());
    }
    Ok(e)
}

/// Accents folded and case raised: MÉDIA, média and MEDIA are one name.
fn canonical(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'Á' | 'À' | 'Â' | 'Ã' => 'A',
            'é' | 'ê' | 'É' | 'Ê' => 'E',
            'í' | 'Í' => 'I',
            'ó' | 'ô' | 'õ' | 'Ó' | 'Ô' | 'Õ' => 'O',
            'ú' | 'Ú' => 'U',
            'ç' | 'Ç' => 'C',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

struct Engine<'a> {
    tab: &'a Tab,
    today: &'a str,
    memo: HashMap<(usize, usize), Value>,
    visiting: Vec<(usize, usize)>,
}

impl Engine<'_> {
    /// The literal of a grid cell (row 1 = header), before any formula.
    fn literal(&self, row: usize, col: usize) -> Option<&serde_json::Value> {
        if row == 1 {
            return None;
        }
        self.tab.rows.get(row - 2)?.get(col)
    }

    fn cell(&mut self, row: usize, col: usize) -> Value {
        if row == 1 {
            return self.tab.columns.get(col).map(|c| Value::Text(c.clone())).unwrap_or(Value::Empty);
        }
        if let Some(v) = self.memo.get(&(row, col)) {
            return v.clone();
        }
        if self.visiting.contains(&(row, col)) {
            return Value::Error("#CICLO!".into());
        }
        let Some(lit) = self.literal(row, col).cloned() else { return Value::Empty };
        let value = match &lit {
            serde_json::Value::String(s) if s.starts_with('=') => {
                self.visiting.push((row, col));
                let v = match parse_formula(&s[1..]) {
                    Ok(e) => self.eval(&e),
                    Err(e) => Value::Error(e),
                };
                self.visiting.pop();
                v
            }
            other => literal_value(other),
        };
        self.memo.insert((row, col), value.clone());
        value
    }

    fn range(&mut self, a: (usize, usize), b: (usize, usize)) -> Vec<Value> {
        let (r1, r2) = (a.0.min(b.0), a.0.max(b.0));
        let (c1, c2) = (a.1.min(b.1), a.1.max(b.1));
        let mut out = Vec::new();
        for r in r1..=r2 {
            for c in c1..=c2 {
                out.push(self.cell(r, c));
            }
        }
        out
    }

    /// Every value an argument yields: a range spreads, a scalar is one.
    fn flat(&mut self, args: &[Expr]) -> Vec<Value> {
        let mut out = Vec::new();
        for a in args {
            match a {
                Expr::Range(x, y) => out.extend(self.range(*x, *y)),
                e => out.push(self.eval(e)),
            }
        }
        out
    }

    fn numbers(&mut self, args: &[Expr]) -> Result<Vec<f64>, String> {
        let mut out = Vec::new();
        for v in self.flat(args) {
            match v {
                Value::Number(n) => out.push(n),
                Value::Error(e) => return Err(e),
                // Text and empties in a range are skipped, as a spreadsheet does.
                _ => {}
            }
        }
        Ok(out)
    }

    fn eval(&mut self, e: &Expr) -> Value {
        match self.try_eval(e) {
            Ok(v) => v,
            Err(msg) => Value::Error(msg),
        }
    }

    fn try_eval(&mut self, e: &Expr) -> Result<Value, String> {
        Ok(match e {
            Expr::Lit(v) => v.clone(),
            Expr::Ref(r, c) => self.cell(*r, *c),
            Expr::Range(..) => return Err("#VALOR!".into()),
            Expr::Neg(x) => Value::Number(-self.try_eval(x)?.as_number()?),
            Expr::Bin(op, a, b) => {
                let va = self.try_eval(a)?;
                let vb = self.try_eval(b)?;
                match op.as_str() {
                    "&" => Value::Text(format!("{}{}", va.as_text(), vb.as_text())),
                    "=" | "<>" | "<" | ">" | "<=" | ">=" => Value::Bool(compare(op, &va, &vb)?),
                    _ => {
                        let (x, y) = (va.as_number()?, vb.as_number()?);
                        Value::Number(match op.as_str() {
                            "+" => x + y,
                            "-" => x - y,
                            "*" => x * y,
                            "/" => {
                                if y == 0.0 {
                                    return Err("#DIV/0!".into());
                                }
                                x / y
                            }
                            "^" => x.powf(y),
                            _ => return Err("#ERRO".into()),
                        })
                    }
                }
            }
            Expr::Call(name, args) => self.call(&canonical(name), args)?,
        })
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> Result<Value, String> {
        Ok(match name {
            "SUM" | "SOMA" => Value::Number(self.numbers(args)?.iter().sum()),
            "AVERAGE" | "MEDIA" => {
                let ns = self.numbers(args)?;
                if ns.is_empty() {
                    return Err("#DIV/0!".into());
                }
                Value::Number(ns.iter().sum::<f64>() / ns.len() as f64)
            }
            "MIN" => self.numbers(args)?.into_iter().reduce(f64::min).map(Value::Number).unwrap_or(Value::Number(0.0)),
            "MAX" => self.numbers(args)?.into_iter().reduce(f64::max).map(Value::Number).unwrap_or(Value::Number(0.0)),
            "COUNT" | "CONT.NUM" => Value::Number(self.numbers(args)?.len() as f64),
            "COUNTA" | "CONT.VALORES" => {
                Value::Number(self.flat(args).iter().filter(|v| !matches!(v, Value::Empty)).count() as f64)
            }
            "COUNTIF" | "CONT.SE" => {
                let [range, crit] = args else { return Err("#ERRO".into()) };
                let crit = self.try_eval(crit)?;
                let vals = self.flat(std::slice::from_ref(range));
                let mut n = 0;
                for v in vals {
                    if matches_criteria(&v, &crit)? {
                        n += 1;
                    }
                }
                Value::Number(n as f64)
            }
            "SUMIF" | "SOMA.SE" => {
                let (range, crit, sum_range) = match args {
                    [r, c] => (r, c, r),
                    [r, c, s] => (r, c, s),
                    _ => return Err("#ERRO".into()),
                };
                let crit = self.try_eval(crit)?;
                let tests = self.flat(std::slice::from_ref(range));
                let sums = self.flat(std::slice::from_ref(sum_range));
                let mut total = 0.0;
                for (t, s) in tests.iter().zip(sums.iter()) {
                    if matches_criteria(t, &crit)? {
                        total += s.as_number().unwrap_or(0.0);
                    }
                }
                Value::Number(total)
            }
            "IF" | "SE" => {
                let (cond, yes, no) = match args {
                    [c, y] => (c, Some(y), None),
                    [c, y, n] => (c, Some(y), Some(n)),
                    _ => return Err("#ERRO".into()),
                };
                if self.try_eval(cond)?.truthy()? {
                    yes.map(|e| self.try_eval(e)).transpose()?.unwrap_or(Value::Bool(true))
                } else {
                    no.map(|e| self.try_eval(e)).transpose()?.unwrap_or(Value::Bool(false))
                }
            }
            "AND" | "E" => {
                for v in self.flat(args) {
                    if !v.truthy()? {
                        return Ok(Value::Bool(false));
                    }
                }
                Value::Bool(true)
            }
            "OR" | "OU" => {
                for v in self.flat(args) {
                    if v.truthy()? {
                        return Ok(Value::Bool(true));
                    }
                }
                Value::Bool(false)
            }
            "NOT" | "NAO" => {
                let [x] = args else { return Err("#ERRO".into()) };
                Value::Bool(!self.try_eval(x)?.truthy()?)
            }
            "ROUND" | "ARRED" => {
                let (x, places) = match args {
                    [x] => (self.try_eval(x)?.as_number()?, 0.0),
                    [x, p] => (self.try_eval(x)?.as_number()?, self.try_eval(p)?.as_number()?),
                    _ => return Err("#ERRO".into()),
                };
                let f = 10f64.powi(places as i32);
                Value::Number((x * f).round() / f)
            }
            "ABS" => {
                let [x] = args else { return Err("#ERRO".into()) };
                Value::Number(self.try_eval(x)?.as_number()?.abs())
            }
            "TODAY" | "HOJE" => Value::Text(self.today.to_string()),
            "CONCAT" | "CONCATENAR" => {
                Value::Text(self.flat(args).iter().map(Value::as_text).collect::<Vec<_>>().join(""))
            }
            "LEN" | "NUM.CARACT" => {
                let [x] = args else { return Err("#ERRO".into()) };
                Value::Number(self.try_eval(x)?.as_text().chars().count() as f64)
            }
            _ => return Err("#NOME?".into()),
        })
    }
}

fn literal_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Empty,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => n.as_f64().map(Value::Number).unwrap_or(Value::Empty),
        serde_json::Value::String(s) if s.is_empty() => Value::Empty,
        serde_json::Value::String(s) => Value::Text(s.clone()),
        other => Value::Text(other.to_string()),
    }
}

fn compare(op: &str, a: &Value, b: &Value) -> Result<bool, String> {
    if let (Value::Error(e), _) | (_, Value::Error(e)) = (a, b) {
        return Err(e.clone());
    }
    let ord = match (a.as_number(), b.as_number()) {
        (Ok(x), Ok(y)) if !matches!((a, b), (Value::Text(_), Value::Text(_))) => x.partial_cmp(&y),
        _ => Some(a.as_text().to_lowercase().cmp(&b.as_text().to_lowercase())),
    };
    let Some(ord) = ord else { return Ok(false) };
    Ok(match op {
        "=" => ord.is_eq(),
        "<>" => !ord.is_eq(),
        "<" => ord.is_lt(),
        ">" => ord.is_gt(),
        "<=" => ord.is_le(),
        ">=" => ord.is_ge(),
        _ => false,
    })
}

/// A COUNTIF/SUMIF criterion: a value to equal, or ">3", "<=10", "<>x".
fn matches_criteria(v: &Value, crit: &Value) -> Result<bool, String> {
    if let Value::Text(s) = crit {
        for op in ["<>", "<=", ">=", "<", ">", "="] {
            if let Some(rest) = s.strip_prefix(op) {
                let rhs = match rest.trim().parse::<f64>() {
                    Ok(n) => Value::Number(n),
                    Err(_) => Value::Text(rest.trim().to_string()),
                };
                return compare(op, v, &rhs);
            }
        }
    }
    compare("=", v, crit)
}

/// Every cell of every tab, computed. `today` is the date HOJE() gives,
/// as an ISO day, because the domain keeps no clock.
pub fn compute(sheet: &Sheet, today: &str) -> Computed {
    let tabs = sheet
        .tabs
        .iter()
        .map(|tab| {
            let mut engine = Engine { tab, today, memo: HashMap::new(), visiting: vec![] };
            let width = tab.columns.len().max(tab.rows.iter().map(Vec::len).max().unwrap_or(0));
            let cells = (0..tab.rows.len())
                .map(|r| {
                    (0..width)
                        .map(|c| {
                            let lit = engine.literal(r + 2, c).cloned().unwrap_or(serde_json::Value::Null);
                            let raw = match &lit {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Null => String::new(),
                                other => literal_value(other).as_text(),
                            };
                            let formula = raw.starts_with('=');
                            let v = engine.cell(r + 2, c);
                            let (error, number) = match &v {
                                Value::Error(e) => (Some(e.clone()), None),
                                Value::Number(n) => (None, Some(*n)),
                                _ => (None, None),
                            };
                            Shown { raw, text: v.as_text(), formula, number, error }
                        })
                        .collect()
                })
                .collect();
            let mut columns = tab.columns.clone();
            while columns.len() < width {
                columns.push(column_name(columns.len()));
            }
            ComputedTab { name: tab.name.clone(), columns, cells }
        })
        .collect();
    Computed { title: sheet.title.clone(), tabs }
}

/// One tab as CSV: the header, then the computed values; commas, quotes
/// and newlines quoted the usual way.
pub fn csv(tab: &ComputedTab) -> String {
    let quote = |s: &str| {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = tab.columns.iter().map(|c| quote(c)).collect::<Vec<_>>().join(",");
    out.push('\n');
    for row in &tab.cells {
        out.push_str(&row.iter().map(|c| quote(&c.text)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

/// The cells whose raw content differs between two versions of a sheet:
/// a value changed, a row or column added. Tabs match by name; a tab new
/// in `after` has every cell changed; a tab gone is not a cell.
pub fn changed(before: &Sheet, after: &Sheet) -> Vec<CellRef> {
    let mut out = Vec::new();
    for (ti, tab) in after.tabs.iter().enumerate() {
        let old = before.tabs.iter().find(|t| t.name == tab.name);
        for (ri, row) in tab.rows.iter().enumerate() {
            for (ci, cell) in row.iter().enumerate() {
                let was = old.and_then(|t| t.rows.get(ri)).and_then(|r| r.get(ci));
                if was != Some(cell) && !(was.is_none() && cell.is_null()) {
                    out.push(CellRef { tab: ti, row: ri, col: ci });
                }
            }
        }
    }
    out
}

/// A cell replaced by what the person typed: a number when it reads as
/// one, a formula when it starts with `=`, else text; empty clears it.
/// Rows and columns grow to reach the cell.
pub fn set_cell(sheet: &mut Sheet, at: CellRef, typed: &str) -> Result<(), String> {
    let tab = sheet.tabs.get_mut(at.tab).ok_or("aba inexistente")?;
    while tab.rows.len() <= at.row {
        tab.rows.push(vec![]);
    }
    let row = &mut tab.rows[at.row];
    while row.len() <= at.col {
        row.push(serde_json::Value::Null);
    }
    while tab.columns.len() <= at.col {
        tab.columns.push(column_name(tab.columns.len()));
    }
    let typed = typed.trim();
    row[at.col] = if typed.is_empty() {
        serde_json::Value::Null
    } else if typed.starts_with('=') {
        serde_json::Value::String(typed.to_string())
    } else if let Ok(n) = typed.replace(',', ".").parse::<f64>() {
        serde_json::json!(n)
    } else {
        serde_json::Value::String(typed.to_string())
    };
    Ok(())
}

/// The sheet back as the file: two-space json, a newline at the end.
pub fn to_json(sheet: &Sheet) -> String {
    let mut s = serde_json::to_string_pretty(sheet).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: &str = "2026-11-10";

    fn sample() -> Sheet {
        parse(
            r#"{ "title": "Custos de infra", "tabs": [
              { "name": "Mensal", "columns": ["mês", "infra", "banco", "total"],
                "rows": [
                  ["set/26", 412, 96, "=B2+C2"],
                  ["out/26", 438, 101, "=SOMA(B3:C3)"],
                  ["nov/26", "=B3*1.03", "=ARRED(C3*1.03; 0)", "=B4+C4"],
                  ["média", "=MÉDIA(B2:B4)", "=average(C2:C4)", "=SE(D4>D3; \"subiu\"; \"caiu\")"]
                ] } ] }"#,
        )
        .unwrap()
    }

    fn cell(c: &Computed, row: usize, col: usize) -> &Shown {
        &c.tabs[0].cells[row][col]
    }

    #[test]
    fn a_sheet_is_parsed_from_the_small_json_the_skill_describes_and_bad_json_says_why() {
        let s = sample();
        assert_eq!(s.title.as_deref(), Some("Custos de infra"));
        assert_eq!(s.tabs[0].columns, vec!["mês", "infra", "banco", "total"]);
        assert_eq!(s.tabs[0].rows.len(), 4);
        assert!(parse("not json").unwrap_err().contains("json"));
        assert!(parse("[]").unwrap_err().contains("tabs"));
        assert!(parse(r#"{"tabs":[{"name":""}]}"#).unwrap_err().contains("name"));
        assert_eq!(parse("{}").unwrap().tabs.len(), 0, "no tabs is an empty sheet, not an error");
    }

    #[test]
    fn literals_show_as_they_are_and_formulas_show_their_value() {
        let c = compute(&sample(), TODAY);
        assert_eq!(cell(&c, 0, 0).text, "set/26");
        assert_eq!(cell(&c, 0, 1).text, "412");
        assert_eq!(cell(&c, 0, 1).number, Some(412.0));
        assert!(!cell(&c, 0, 1).formula);
        // D2 = B2 + C2
        assert_eq!(cell(&c, 0, 3).text, "508");
        assert!(cell(&c, 0, 3).formula);
        assert_eq!(cell(&c, 0, 3).raw, "=B2+C2");
        // SOMA over a range, in Portuguese.
        assert_eq!(cell(&c, 1, 3).text, "539");
        // A formula over formulas: B4 = B3 * 1.03 = 451.14; C4 rounded.
        assert_eq!(cell(&c, 2, 1).text, "451.14");
        assert_eq!(cell(&c, 2, 2).text, "104");
        assert_eq!(cell(&c, 2, 3).text, "555.14");
        // MÉDIA with an accent, average in English; SE with text results.
        assert_eq!(cell(&c, 3, 1).text, "433.71");
        assert_eq!(cell(&c, 3, 2).text, "100.33");
        assert_eq!(cell(&c, 3, 3).text, "subiu");
    }

    #[test]
    fn arithmetic_follows_precedence_and_the_header_row_is_row_one() {
        let s = parse(r#"{"tabs":[{"name":"t","columns":["a","b"],"rows":[[2, 3], ["=A2+B2*2^2", "=(A2+B2)*2"], ["=-A2 & \" un\"", "=A1"], ["=10%", "=B2>A2"]]}]}"#).unwrap();
        let c = compute(&s, TODAY);
        assert_eq!(cell(&c, 1, 0).text, "14");
        assert_eq!(cell(&c, 1, 1).text, "10");
        assert_eq!(cell(&c, 2, 0).text, "-2 un");
        assert_eq!(cell(&c, 2, 1).text, "a", "A1 is the header");
        assert_eq!(cell(&c, 3, 0).text, "0.1");
        assert_eq!(cell(&c, 3, 1).text, "VERDADEIRO");
    }

    #[test]
    fn errors_are_named_in_the_cell_and_never_refuse_the_sheet() {
        let s = parse(r#"{"tabs":[{"name":"t","columns":["a"],"rows":[["=1/0"], ["=FOO(1)"], ["=A5"], ["=A4"], ["=\"x\"+1"], ["=1+"], ["=Z99"], ["=HOJE()"]]}]}"#).unwrap();
        let c = compute(&s, TODAY);
        assert_eq!(cell(&c, 0, 0).error.as_deref(), Some("#DIV/0!"));
        assert_eq!(cell(&c, 1, 0).error.as_deref(), Some("#NOME?"));
        assert_eq!(cell(&c, 2, 0).error.as_deref(), Some("#CICLO!"), "A4 → A5 → A4");
        assert_eq!(cell(&c, 3, 0).error.as_deref(), Some("#CICLO!"));
        assert_eq!(cell(&c, 4, 0).error.as_deref(), Some("#VALOR!"));
        assert_eq!(cell(&c, 5, 0).error.as_deref(), Some("#ERRO"));
        assert_eq!(cell(&c, 6, 0).text, "", "a reference past the data is empty, not an error");
        assert_eq!(cell(&c, 7, 0).text, TODAY);
        assert_eq!(cell(&c, 0, 0).text, "#DIV/0!", "the error is what the cell shows");
    }

    #[test]
    fn counting_and_summing_with_criteria_and_ranges_that_mix_text() {
        let s = parse(r#"{"tabs":[{"name":"t","columns":["item","valor","ok"],"rows":[["a", 10, "sim"], ["b", 20, "não"], ["c", 30, "sim"], ["total", "=SOMA(B2:B4)", "=CONT.SE(C2:C4; \"sim\")"], ["acima de 15", "=COUNTIF(B2:B4; \">15\")", "=SOMA.SE(C2:C4; \"sim\"; B2:B4)"], ["mín/máx", "=MÍN(B2:B4)", "=MAX(B2:B4)"], ["texto", "=SOMA(A2:A4)", "=CONT.VALORES(A2:A4)"]]}]}"#).unwrap();
        let c = compute(&s, TODAY);
        assert_eq!(cell(&c, 3, 1).text, "60");
        assert_eq!(cell(&c, 3, 2).text, "2");
        assert_eq!(cell(&c, 4, 1).text, "2");
        assert_eq!(cell(&c, 4, 2).text, "40");
        assert_eq!(cell(&c, 5, 1).text, "10");
        assert_eq!(cell(&c, 5, 2).text, "30");
        assert_eq!(cell(&c, 6, 1).text, "0", "text in a summed range is skipped");
        assert_eq!(cell(&c, 6, 2).text, "3");
    }

    #[test]
    fn a_tab_exports_as_csv_with_values_not_formulas() {
        let c = compute(&sample(), TODAY);
        let out = csv(&c.tabs[0]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "mês,infra,banco,total");
        assert_eq!(lines[1], "set/26,412,96,508");
        assert_eq!(lines[4], "média,433.71,100.33,subiu");
        let s = parse(r#"{"tabs":[{"name":"t","columns":["a, b","c"],"rows":[["diz \"oi\"", 1]]}]}"#).unwrap();
        assert_eq!(csv(&compute(&s, TODAY).tabs[0]), "\"a, b\",c\n\"diz \"\"oi\"\"\",1\n");
    }

    #[test]
    fn the_cells_that_changed_between_two_versions_are_named() {
        let before = sample();
        let mut after = sample();
        set_cell(&mut after, CellRef { tab: 0, row: 2, col: 1 }, "460").unwrap();
        set_cell(&mut after, CellRef { tab: 0, row: 4, col: 0 }, "dez/26").unwrap();
        let diff = changed(&before, &after);
        assert_eq!(diff, vec![CellRef { tab: 0, row: 2, col: 1 }, CellRef { tab: 0, row: 4, col: 0 }]);
        assert!(changed(&before, &before).is_empty());
        // A new tab: every cell of it changed.
        after.tabs.push(Tab { name: "Nova".into(), columns: vec!["x".into()], rows: vec![vec![serde_json::json!(1)]] });
        assert!(changed(&before, &after).contains(&CellRef { tab: 1, row: 0, col: 0 }));
    }

    #[test]
    fn typing_into_a_cell_stores_a_number_a_formula_or_text_and_grows_the_grid() {
        let mut s = parse(r#"{"tabs":[{"name":"t","columns":["a"],"rows":[]}]}"#).unwrap();
        set_cell(&mut s, CellRef { tab: 0, row: 1, col: 2 }, "12,5").unwrap();
        set_cell(&mut s, CellRef { tab: 0, row: 0, col: 0 }, "=C3*2").unwrap();
        set_cell(&mut s, CellRef { tab: 0, row: 0, col: 1 }, "texto").unwrap();
        assert_eq!(s.tabs[0].rows[1][2], serde_json::json!(12.5));
        assert_eq!(s.tabs[0].rows[0][0], serde_json::json!("=C3*2"));
        assert_eq!(s.tabs[0].rows[0][1], serde_json::json!("texto"));
        assert_eq!(s.tabs[0].columns, vec!["a", "B", "C"], "columns grow with a letter name");
        assert_eq!(compute(&s, TODAY).tabs[0].cells[0][0].text, "25");
        set_cell(&mut s, CellRef { tab: 0, row: 0, col: 1 }, "").unwrap();
        assert!(s.tabs[0].rows[0][1].is_null());
        assert!(set_cell(&mut s, CellRef { tab: 3, row: 0, col: 0 }, "x").is_err());
        let json = to_json(&s);
        assert_eq!(parse(&json).unwrap(), s, "round-trips through the file");
    }

    #[test]
    fn column_names_count_in_letters() {
        assert_eq!(column_name(0), "A");
        assert_eq!(column_name(25), "Z");
        assert_eq!(column_name(26), "AA");
        assert_eq!(column_name(27), "AB");
        assert_eq!(parse_ref("AA3"), Some((3, 26)));
        assert_eq!(parse_ref("3A"), None);
    }
}
