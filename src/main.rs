use std::path::Path;
use clap::Parser;

mod node;
use node::*;

mod parser;

mod interpreter;
use interpreter::*;
mod generator;
mod compiler;
mod emitter;
use emitter::*;

/// Compiles simple list .sl files
#[derive(Parser, Debug)]
#[command(author = None, version = None, about = None, long_about = None)]
struct Args {
    /// Input file
    input: String,

    /// Compile
    #[arg(short, long)]
    compile: bool,

    /// Emit VM bytecode
    #[arg(short, long)]
    emit: bool,

    /// Emit QBE SSA (barely working)
    #[arg(short, long)]
    qbe: bool,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.compile {
        generator::generate(&args.input);
    } else if args.qbe {
        compiler::generate(&args.input);
    } else if args.emit {
        emit(&args.input);
    } else {
        let mut visitor = Visitor::new();
        let path = Path::new(&args.input).canonicalize().unwrap();
        visitor.interpret(path.to_str().unwrap());
    }

    Ok(())
}
