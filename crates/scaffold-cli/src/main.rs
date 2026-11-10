fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().expect("the current directory exists");
    let code = scaffold_cli::run(&args, &cwd, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(code);
}
