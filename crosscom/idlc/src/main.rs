use std::{env::args, error::Error, result::Result};
mod cidl;

fn render(input: &str, template: &str, output: &str) -> Result<(), Box<dyn Error>> {
    let content = std::fs::read_to_string(input)?;
    let result = cidl::parse_idl(content.as_str());
    println!("{:?}", result);

    let idl = match result {
        Some(r) => r,
        None => {
            println!("Cannot parse idl");
            return Ok(());
        }
    };

    let template_content = std::fs::read_to_string(template)?;
    let template = liquid::ParserBuilder::with_stdlib()
        .build()
        .unwrap()
        .parse(&template_content)
        .unwrap();

    let globals = liquid::object!({
        "interfaces": idl.interfaces,
    });

    let result = template.render(&globals).unwrap();
    std::fs::write(output, result)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() < 4 {
        println!("cidl file.idl template.tera output_folder");
    } else {
        render(&args[1], &args[2], &args[3]).unwrap();
    }
}
