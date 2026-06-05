//! Huff → TypeScript emitter (v0 walking skeleton).
//!
//! Mappings follow `docs/plans/ts-transpiler-walking-skeleton.md` §Phase 4.
//! Errors are emitted as thrown exceptions for v0; this is a known shortcut
//! to revisit when Result codegen lands.

#![forbid(unsafe_code)]

use huff_ast::*;
use std::fmt::Write;

pub fn emit(file: &File) -> String {
    let mut e = Emitter::new();
    e.emit_file(file);
    e.out
}

struct Emitter {
    out: String,
    indent: usize,
}

impl Emitter {
    fn new() -> Self {
        Self {
            out: String::new(),
            indent: 0,
        }
    }

    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }
    fn blank(&mut self) {
        self.out.push('\n');
    }

    fn emit_file(&mut self, f: &File) {
        match f.kind {
            ProgKind::Prog => {
                self.emit_items(&f.items, /*inside_namespace=*/ false);
                let main = f.items.iter().find_map(|it| match it {
                    Item::Op(op) if op.name == "Main" => Some(op),
                    _ => None,
                });
                let (arity, is_async) =
                    main.map(|m| (m.params.len(), m.is_async)).unwrap_or((0, false));
                let args = if arity == 0 { "" } else { "process.argv.slice(2)" };
                if is_async {
                    self.line(&format!("Main({}).catch((e) => {{ console.error(e); process.exit(1); }});", args));
                } else {
                    self.line(&format!("Main({});", args));
                }
            }
            ProgKind::Mod => {
                self.line(&format!("export namespace {} {{", f.name));
                self.indent += 1;
                self.emit_items(&f.items, /*inside_namespace=*/ true);
                self.indent -= 1;
                self.line("}");
            }
        }
    }

    fn emit_items(&mut self, items: &[Item], inside_namespace: bool) {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                // Single blank line between top-level items (ops/types), none before first
                match item {
                    Item::Op(_) | Item::Type(_) => self.blank(),
                    _ => {}
                }
            }
            match item {
                Item::Use(u) => {
                    self.line(&format!(
                        "import * as {0} from './{0}';",
                        u.name
                    ));
                }
                Item::Err(e) => self.emit_err(e, inside_namespace),
                Item::Type(t) => self.emit_type_decl(t, inside_namespace),
                Item::State(s) => self.emit_state(s, inside_namespace),
                Item::Op(o) => self.emit_op(o, inside_namespace),
            }
        }
    }

    fn export_kw(&self, inside_namespace: bool) -> &'static str {
        if inside_namespace {
            "export "
        } else {
            "export "
        }
    }

    fn emit_err(&mut self, e: &ErrDecl, inside_namespace: bool) {
        let exp = self.export_kw(inside_namespace);
        if e.fields.is_empty() {
            self.line(&format!(
                "{exp}class {name} extends Error {{ constructor() {{ super({lit}); this.name = {lit}; }} }}",
                exp = exp,
                name = e.name,
                lit = format!("\"{}\"", e.name),
            ));
        } else {
            let params: Vec<String> = e
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, ts_type(&f.ty)))
                .collect();
            let assignments: Vec<String> = e
                .fields
                .iter()
                .map(|f| format!("this.{0} = {0};", f.name))
                .collect();
            let field_decls: Vec<String> = e
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, ts_type(&f.ty)))
                .collect();
            let msg_arg = e
                .fields
                .iter()
                .find(|f| f.name == "msg")
                .map(|_| "msg".to_string())
                .unwrap_or_else(|| format!("\"{}\"", e.name));
            self.line(&format!(
                "{exp}class {name} extends Error {{ {field_decls}; constructor({params}) {{ super({msg}); this.name = \"{name}\"; {assignments} }} }}",
                exp = exp,
                name = e.name,
                field_decls = field_decls.join("; "),
                params = params.join(", "),
                msg = msg_arg,
                assignments = assignments.join(" "),
            ));
        }
    }

    fn emit_type_decl(&mut self, t: &TypeDecl, inside_namespace: bool) {
        let exp = self.export_kw(inside_namespace);
        match t {
            TypeDecl::Alias { name, target, .. } => {
                self.line(&format!("{exp}type {} = {};", name, ts_type(target)));
            }
            TypeDecl::Product { name, fields, .. } => {
                self.line(&format!("{exp}interface {} {{", name));
                self.indent += 1;
                for f in fields {
                    self.line(&format!("{}: {};", f.name, ts_type(&f.ty)));
                }
                self.indent -= 1;
                self.line("}");
                // Convenience constructor — `Type(a, b)` in Huff becomes a
                // function call in TS that builds the object positionally.
                let params: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, ts_type(&f.ty)))
                    .collect();
                let inits: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
                self.line(&format!(
                    "{exp}function {name}({params}): {name} {{ return {{ {inits} }}; }}",
                    exp = exp,
                    name = name,
                    params = params.join(", "),
                    inits = inits.join(", "),
                ));
            }
        }
    }

    fn emit_state(&mut self, s: &StateDecl, _inside_namespace: bool) {
        for f in &s.fields {
            let val = self.expr_to_string(&f.init);
            match &f.ty {
                Some(t) => self.line(&format!("let {}: {} = {};", f.name, ts_type(t), val)),
                None => self.line(&format!("let {} = {};", f.name, val)),
            }
        }
    }

    fn emit_op(&mut self, op: &OpDecl, inside_namespace: bool) {
        let exp = self.export_kw(inside_namespace);
        let params: Vec<String> = op
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, ts_type(&p.ty)))
            .collect();
        let inner_ret = match &op.return_type {
            Some(t) => ts_type(t),
            None => "void".to_string(),
        };
        let ret = if op.is_async {
            format!("Promise<{}>", inner_ret)
        } else {
            inner_ret
        };
        let async_kw = if op.is_async { "async " } else { "" };
        self.line(&format!(
            "{exp}{async_kw}function {}({}): {} {{",
            op.name,
            params.join(", "),
            ret
        ));
        self.indent += 1;
        self.emit_body(&op.body, op.return_type.is_some());
        self.indent -= 1;
        self.line("}");
    }

    fn emit_body(&mut self, stmts: &[Stmt], has_return: bool) {
        let last = stmts.len().saturating_sub(1);
        for (i, s) in stmts.iter().enumerate() {
            let is_last = i == last;
            match s {
                Stmt::Let { name, ty, value, .. } => {
                    let v = self.expr_to_string(value);
                    match ty {
                        Some(t) => self.line(&format!("const {}: {} = {};", name, ts_type(t), v)),
                        None => self.line(&format!("const {} = {};", name, v)),
                    }
                }
                Stmt::Effect { target, .. } => self.emit_effect(target),
                Stmt::Pre { cond, err, .. } => {
                    let c = self.expr_to_string(cond);
                    let throw = match err {
                        Some(ec) => {
                            let args: Vec<String> = ec
                                .args
                                .iter()
                                .map(|a| self.expr_to_string(a))
                                .collect();
                            format!("throw new {}({})", ec.name, args.join(", "))
                        }
                        None => "throw new Error(\"precondition failed\")".to_string(),
                    };
                    self.line(&format!("if (!({})) {{ {}; }}", c, throw));
                }
                Stmt::Expr { expr, .. } => {
                    let v = self.expr_to_string(expr);
                    if is_last && has_return {
                        self.line(&format!("return {};", v));
                    } else {
                        self.line(&format!("{};", v));
                    }
                }
            }
        }
    }

    fn emit_effect(&mut self, t: &EffectTarget) {
        match t {
            EffectTarget::Call(expr) => {
                let s = self.expr_to_string(expr);
                // Special-case io.writeln / io.write / io.err -> console.{log,*}.
                let mapped = map_effect_call(&s).unwrap_or(s);
                self.line(&format!("{};", mapped));
            }
            EffectTarget::Assign { name, value } => {
                let v = self.expr_to_string(value);
                self.line(&format!("{} = {};", name, v));
            }
            EffectTarget::AddAssign { name, value } => {
                let v = self.expr_to_string(value);
                self.line(&format!("{} += {};", name, v));
            }
            EffectTarget::SubAssign { name, value } => {
                let v = self.expr_to_string(value);
                self.line(&format!("{} -= {};", name, v));
            }
        }
    }

    fn expr_to_string(&self, e: &Expr) -> String {
        let mut s = String::new();
        write_expr(&mut s, e);
        s
    }
}

fn map_effect_call(rendered: &str) -> Option<String> {
    // New short forms (preferred)
    if let Some(rest) = rendered.strip_prefix("log(") {
        Some(format!("console.log({}", rest))
    } else if let Some(rest) = rendered.strip_prefix("print(") {
        Some(format!("process.stdout.write({}", rest))
    } else if let Some(rest) = rendered.strip_prefix("err(") {
        Some(format!("console.error({}", rest))
    // Legacy io.* forms (still accepted)
    } else if let Some(rest) = rendered.strip_prefix("io.writeln(") {
        Some(format!("console.log({}", rest))
    } else if let Some(rest) = rendered.strip_prefix("io.write(") {
        Some(format!("process.stdout.write({}", rest))
    } else if let Some(rest) = rendered.strip_prefix("io.err(") {
        Some(format!("console.error({}", rest))
    } else {
        None
    }
}

fn write_expr(out: &mut String, e: &Expr) {
    match e {
        Expr::Lit(Lit::Int(n), _) => write!(out, "{}", n).unwrap(),
        Expr::Lit(Lit::Bool(b), _) => write!(out, "{}", b).unwrap(),
        Expr::Lit(Lit::Str(s), _) => {
            write!(out, "\"").unwrap();
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    c => out.push(c),
                }
            }
            write!(out, "\"").unwrap();
        }
        Expr::Name(s, _) => out.push_str(s),
        Expr::Call { callee, args, .. } => {
            write_expr(out, callee);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_expr(out, a);
            }
            out.push(')');
        }
        Expr::Member { target, field, .. } => {
            write_expr(out, target);
            out.push('.');
            // .len → .length on TS strings/arrays.
            if field == "len" {
                out.push_str("length");
            } else {
                out.push_str(field);
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let prec = bin_op_prec(*op);
            let need_lhs_parens = matches!(lhs.as_ref(), Expr::Binary { op: inner_op, .. } if bin_op_prec(*inner_op) < prec);
            let need_rhs_parens = matches!(rhs.as_ref(), Expr::Binary { op: inner_op, .. } if bin_op_prec(*inner_op) <= prec);
            if need_lhs_parens { out.push('('); }
            write_expr(out, lhs);
            if need_lhs_parens { out.push(')'); }
            out.push(' ');
            out.push_str(bin_op_str(*op));
            out.push(' ');
            if need_rhs_parens { out.push('('); }
            write_expr(out, rhs);
            if need_rhs_parens { out.push(')'); }
        }
        Expr::Unary { op, expr, .. } => {
            out.push_str(match op {
                UnOp::Neg => "-",
                UnOp::Not => "!",
            });
            // Parens needed only if inner expr is binary
            let need_parens = matches!(expr.as_ref(), Expr::Binary { .. });
            if need_parens { out.push('('); }
            write_expr(out, expr);
            if need_parens { out.push(')'); }
        }
        Expr::Pipeline { source, stages, .. } => {
            // Render `xs->map(f)->each(g)` as `xs.map(f).forEach(g)`.
            write_expr(out, source);
            for s in stages {
                out.push('.');
                let method = match s.name.as_str() {
                    "each" => "forEach",
                    other => other,
                };
                out.push_str(method);
                out.push('(');
                for (i, a) in s.args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write_expr(out, a);
                }
                out.push(')');
            }
        }
        Expr::Closure { param, body, .. } => {
            out.push('(');
            out.push_str(param);
            out.push_str(") => ");
            write_expr(out, body);
        }
        Expr::Propagate { inner, .. } => {
            // v0: propagation is a no-op; exceptions bubble naturally.
            write_expr(out, inner);
        }
        Expr::Await { inner, .. } => {
            out.push_str("await ");
            write_expr(out, inner);
        }
        Expr::Interpolation { parts, .. } => {
            out.push('`');
            for part in parts {
                match part {
                    InterpPart::Lit(s) => {
                        // Escape backticks and ${} in literal segments
                        for c in s.chars() {
                            match c {
                                '`' => out.push_str("\\`"),
                                '$' => out.push_str("\\$"),
                                '\\' => out.push_str("\\\\"),
                                '\n' => out.push_str("\\n"),
                                '\t' => out.push_str("\\t"),
                                c => out.push(c),
                            }
                        }
                    }
                    InterpPart::Expr(e) => {
                        out.push_str("${");
                        write_expr(out, e);
                        out.push('}');
                    }
                }
            }
            out.push('`');
        }
    }
}

fn bin_op_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "===",
        BinOp::Ne => "!==",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn bin_op_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne => 3,
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 4,
        BinOp::Add | BinOp::Sub => 5,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 6,
    }
}

fn ts_type(t: &Type) -> String {
    match t {
        Type::Prim(p) => match p {
            PrimType::Str => "string".into(),
            PrimType::Bool => "boolean".into(),
            PrimType::I32
            | PrimType::U32
            | PrimType::I64
            | PrimType::U64
            | PrimType::F32
            | PrimType::F64 => "number".into(),
            PrimType::Bytes => "Uint8Array".into(),
        },
        Type::Named(s) => s.clone(),
        Type::List(inner) => format!("{}[]", ts_type(inner)),
        Type::Optional(inner) => format!("({} | undefined)", ts_type(inner)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use huff_parser::parse_source;

    fn emit_str(src: &str) -> String {
        let f = parse_source(src).expect("parse");
        emit(&f)
    }

    #[test]
    fn hello_minimal_emits() {
        let src = "prog HelloWorld\n  op Main()\n    !log(\"Hello World\")\n";
        let ts = emit_str(src);
        assert!(ts.contains("function Main()"), "ts:\n{}", ts);
        assert!(ts.contains("console.log(\"Hello World\")"), "ts:\n{}", ts);
        // Main has no params → entry call passes no args.
        assert!(ts.contains("Main();"), "ts:\n{}", ts);
    }

    #[test]
    fn main_with_args_gets_argv() {
        let src = "prog X\n  op Main(args: []str)\n    !log(\"hi\")\n";
        let ts = emit_str(src);
        assert!(ts.contains("Main(process.argv.slice(2));"), "ts:\n{}", ts);
    }

    #[test]
    fn product_type_gets_constructor() {
        let src = "mod Greetings\n  type Greeting\n    to: str\n    msg: str\n  op Make(name: str) Greeting\n    Greeting(name, \"Hi \" + name)\n";
        let ts = emit_str(src);
        assert!(ts.contains("interface Greeting"), "{}", ts);
        assert!(ts.contains("function Greeting"), "{}", ts);
        assert!(ts.contains("function Make"), "{}", ts);
        assert!(ts.contains("return Greeting(name,"), "{}", ts);
    }

    #[test]
    fn pre_becomes_throw() {
        let src = "prog X\n  err Bad\n  op M(n: u32)\n    pre n > 0 : Bad\n    !io.writeln(\"ok\")\n";
        let ts = emit_str(src);
        assert!(ts.contains("if (!(n > 0))"), "{}", ts);
        assert!(ts.contains("throw new Bad()"), "{}", ts);
    }

    #[test]
    fn state_compound_assign() {
        let src = "prog X\n  state n: u32 = 0\n  op M()\n    !n += 1\n    !log(\"done\")\n";
        let ts = emit_str(src);
        assert!(ts.contains("let n: number = 0;"), "{}", ts);
        assert!(ts.contains("n += 1;"), "{}", ts);
    }

    #[test]
    fn string_interpolation_emits_template_literal() {
        let src = "prog X\n  op Greet(name: str) str\n    \"hello {name}\"\n";
        let ts = emit_str(src);
        assert!(ts.contains("`hello ${name}`"), "ts:\n{}", ts);
    }

    #[test]
    fn plain_string_stays_quoted() {
        let src = "prog X\n  op M()\n    !log(\"no interp here\")\n";
        let ts = emit_str(src);
        assert!(ts.contains("\"no interp here\""), "ts:\n{}", ts);
    }
}
