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
    analyze: Vec<String>,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let loader = Loader::new();

    loader.read_libs(&args.lib_paths)?;

    let (mut interner, modules, app_modules, app_deps) = loader.finish();

    println!("\ntotal apps: {}", app_modules.len());
    println!("total app dependencies: {}", app_deps.edge_count());
    println!("total modules: {}", modules.len());
    println!("total atoms: {}", interner.len());

    let analyzer = Analyzer::new(modules, app_modules.clone());

    let analyze: Vec<_> = args
        .analyze
        .iter()
        .map(|app| Atom::intern(&mut interner, app))
        .collect();

    println!("\n");
    for &app in &analyze {
        println!(
            "{}: {:?}",
            app.resolve(&interner).unwrap(),
            app_deps
                .neighbors_directed(app, petgraph::EdgeDirection::Outgoing)
                .flat_map(|name| name.resolve(&interner))
                .collect::<Vec<_>>()
        )
    }

    let results = analyzer.run(&analyze);

    println!("\n");
    for (module, result) in results {
        println!(
            "{}: {}",
            module.resolve(&interner).unwrap(),
            result.fmt(&interner)
        )
    }

    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut args = pico_args::Arguments::from_env();

    let parsed = Args {
        lib_paths: args.values_from_str("--lib-path")?,
        analyze: args.values_from_str("--analyze")?,
    };

    args.finish()?;

    Ok(parsed)
}
