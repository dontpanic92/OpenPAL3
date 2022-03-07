use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, multispace0, multispace1},
    combinator::recognize,
    multi::many0,
    sequence::{pair, delimited},
    IResult, error::ParseError,
};

#[derive(Debug)]
pub struct Idl {
    interfaces: Vec<Interface>,
}

#[derive(Debug)]
pub struct Interface {
    name: String,
}

fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
  where
  F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
  delimited(
    multispace0,
    inner,
    multispace0
  )
}

fn parse_identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn parse_interface(input: &str) -> IResult<&str, Interface> {
    let (input, _) = tag("interface")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = parse_identifier(input)?;
    Ok((
        input,
        Interface {
            name: name.to_string(),
        },
    ))
}

pub fn parse_idl(input: &str) -> IResult<&str, Idl> {
    let (input, interfaces) = many0(ws(parse_interface))(input)?;
    Ok((input, Idl { interfaces }))
}
