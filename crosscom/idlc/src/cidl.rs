use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, multispace0, multispace1},
    combinator::recognize,
    error::ParseError,
    multi::many0,
    sequence::{delimited, pair},
    IResult,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Idl {
    pub interfaces: Vec<Interface>,
}

#[derive(Debug, Serialize)]
pub struct Interface {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct InterfaceBody {
    pub methods: Vec<Method>,
}

#[derive(Debug, Serialize)]
pub struct Method {
    pub name: String,
    pub return_type: String,
    pub parameters: Vec<MethodParameter>,
}

#[derive(Debug, Serialize)]
pub struct MethodParameter {
    pub name: String,
    pub ty: String,
}

fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn parse_identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn parse_interface_body(input: &str) -> IResult<&str, ()> {
    let (input, _) = multispace0(input)?;
    Ok((input, ()))
}

fn parse_interface(input: &str) -> IResult<&str, Interface> {
    let (input, _) = tag("interface")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = parse_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = delimited(tag("{"), parse_interface_body, tag("}"))(input)?;
    Ok((
        input,
        Interface {
            name: name.to_string(),
        },
    ))
}

pub fn parse_idl(input: &str) -> Option<Idl> {
    let result = many0(ws(parse_interface))(input);
    println!("{:?}", result);
    
    if let Ok((input, interfaces)) = result {
        if input.len() == 0 {
            return Some(Idl { interfaces });
        }
    }

    None
}
