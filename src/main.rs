use std::path::PathBuf;

use anyhow::Result;

mod loader;

use loader::Loader;

struct Args {
    lib_paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let mut loader = Loader::new();

    for lib in args.lib_paths {
        loader.read_libs(&lib)?;
    }

    println!("\ntotal modules: {}", loader.loaded_modules());
    println!("total atoms: {}", loader.loaded_atoms());

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
