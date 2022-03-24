use std::{env::args, error::Error, result::Result};

use crate::analysis::SemanticAnalyzer;

pub mod analysis;
mod cidl;
mod filters;
mod viewmodels;

fn render(input: &str, template: &str, output: &str) -> Result<(), Box<dyn Error>> {
    let content = std::fs::read_to_string(input)?;
    let result = cidl::parser::parse_idl(content.as_str());

    let mut idl = match result {
        Some(r) => r,
        None => {
            println!("Cannot parse idl");
            return Ok(());
        }
    };

    let mut analyzer = SemanticAnalyzer::new();
    analyzer.analyze(&mut idl);
    let symbols = analyzer.symbols();
    let viewmodel = viewmodels::rust::RustViewModel::from_symbols(symbols);

    let template_content = std::fs::read_to_string(template)?;
    let template = liquid::ParserBuilder::with_stdlib()
        .filter(filters::UuidHexArray)
        .build()
        .unwrap()
        .parse(&template_content)
        .unwrap();

    let globals = liquid::object!({
        "model": viewmodel,
        "symbols": symbols,
        "ns_prefix": "crate::crosscom_gen",
    });

    // println!("Globals: {:?}\n", globals);

    let result = template.render(&globals).unwrap();
    std::fs::write(output, result)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() < 4 {
        println!("idlc file.idl template.tera output_folder");
    } else {
        render(&args[1], &args[2], &args[3]).unwrap();
    }
}
