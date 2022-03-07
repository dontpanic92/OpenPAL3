use std::env::args;

mod cidl;

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() < 2 {
        println!("cidl file.idl");
    } else {
        let content = std::fs::read_to_string(args[1].as_str()).unwrap();
        let result = cidl::parse_idl(content.as_str());
        println!("{:?}", result);
    }
}
