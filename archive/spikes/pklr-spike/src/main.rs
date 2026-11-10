// Evaluates a pkl file with pklr and prints the JSON value, or the error.
fn main() {
    let path = std::env::args().nth(1).expect("usage: pklr-spike <file.pkl>");
    match pklr::eval_to_json(std::path::Path::new(&path)) {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("ERROR: {e:?}");
            std::process::exit(1);
        }
    }
}
