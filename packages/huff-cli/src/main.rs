use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "huffc", about = "Huff → TypeScript transpiler (v0)")]
struct Args {
    /// Input .huff file
    input: PathBuf,
    /// Output path (defaults to <input>.ts)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Emit to stdout instead of a file
    #[arg(long)]
    stdout: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let src = match std::fs::read_to_string(&args.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {:?}: {}", args.input, e);
            return ExitCode::from(2);
        }
    };
    let file = match huff_parser::parse_source(&src) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {}", args.input.display(), format_error(&src, &e));
            return ExitCode::from(1);
        }
    };
    let ts = huff_emit_ts::emit(&file);
    if args.stdout {
        print!("{}", ts);
        return ExitCode::SUCCESS;
    }
    let out_path = args.output.unwrap_or_else(|| {
        let mut p = args.input.clone();
        p.set_extension("ts");
        p
    });
    if let Err(e) = std::fs::write(&out_path, &ts) {
        eprintln!("error writing {:?}: {}", out_path, e);
        return ExitCode::from(2);
    }
    eprintln!("wrote {}", out_path.display());
    ExitCode::SUCCESS
}

fn format_error(src: &str, err: &huff_parser::ParseError) -> String {
    use huff_parser::ParseError;
    match err {
        ParseError::Lex(e) => format!("lex error: {}", e),
        ParseError::NotYetSupported(msg) => format!("not yet supported: {}", msg),
        ParseError::Generic { offset, msg } => {
            let (line, col) = line_col(src, *offset);
            format!("error at line {}, col {}: {}", line, col, msg)
        }
    }
}

fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in src.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
