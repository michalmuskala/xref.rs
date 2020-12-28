use std::path::PathBuf;

use anyhow::Result;

mod analyzer;
mod loader;
mod types;

use analyzer::Analyzer;
use loader::Loader;
use types::Atom;

struct Args {
    lib_paths: Vec<PathBuf>,
    analyze: Vec<String>
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let loader = Loader::new();

    loader.read_libs(&args.lib_paths)?;

    let (mut interner, modules, apps) = loader.finish();

    println!("\ntotal apps: {}", apps.len());
    println!("total modules: {}", modules.len());
    println!("total atoms: {}", interner.len());

    let analyzer = Analyzer::new(modules, apps);

    let analyze: Vec<_> = args.analyze.iter().map(|app| Atom::intern(&mut interner, app)).collect();

    let results = analyzer.run(&analyze);

    println!("\n");
    for (module, result) in results {
        println!("{}: {}", module.resolve(&interner).unwrap(), result.fmt(&interner))
    }

    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut args = pico_args::Arguments::from_env();

    let parsed = Args {
        lib_paths: args.values_from_str("--lib-path")?,
        analyze: args.values_from_str("--analyze")?
    };

    args.finish()?;

    Ok(parsed)
}
