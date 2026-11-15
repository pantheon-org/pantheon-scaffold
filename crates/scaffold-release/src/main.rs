fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::env::current_dir().expect("the current directory exists");
    let code = scaffold_release::run(&args, &root, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(code);
}
