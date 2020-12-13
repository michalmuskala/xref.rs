use std::path::PathBuf;

use anyhow::Result;

mod loader;

use loader::Loader;

struct Args {
    lib_paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let loader = Loader::new();

    loader.read_libs(&args.lib_paths)?;

    let (interner, modules, apps) = loader.finish();

    println!("\ntotal apps: {}", apps.len());
    println!("total modules: {}", modules.len());
    println!("total atoms: {}", interner.len());

    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut args = pico_args::Arguments::from_env();

    let parsed = Args {
        lib_paths: args.values_from_str("--lib-path")?,
    };

    args.finish()?;

    Ok(parsed)
}
