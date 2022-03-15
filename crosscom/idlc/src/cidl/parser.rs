use std::collections::HashMap;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_till},
    character::complete::{alpha1, alphanumeric1, multispace0, multispace1},
    combinator::{opt, recognize},
    error::ParseError,
    multi::many0,
    sequence::{delimited, pair},
    IResult,
};

use super::ast::*;


fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn parse_method_parameter(input: &str) -> IResult<&str, Option<MethodParameter>> {
    let result: Result<(&str, &str), nom::Err<nom::error::Error<&str>>> = ws(tag(")"))(input);
    match result {
        Ok((input, _)) => Ok((input, None)),
        Err(_) => {
            let (input, ty) = ws(parse_identifier)(input)?;
            let (input, name) = ws(parse_identifier)(input)?;
            let (input, _) = opt(tag(","))(input)?;
            Ok((
                input,
                Some(MethodParameter {
                    name: name.to_string(),
                    ty: ty.to_string(),
                    extra: Extra::new(),
                }),
            ))
        }
    }
}

fn parse_method_parameters(input: &str) -> IResult<&str, Vec<MethodParameter>> {
    let mut parameters = vec![];
    let (mut input, _) = ws(tag("("))(input)?;

    loop {
        match parse_method_parameter(input) {
            Ok((next_input, Some(parameter))) => {
                input = next_input;
                parameters.push(parameter)
            }
            Ok((next_input, _)) => {
                input = next_input;
            }
            Err(_) => {
                break;
            }
        }
    }

    let (input, _) = ws(tag(";"))(input)?;

    Ok((input, parameters))
}

fn parse_method(input: &str) -> IResult<&str, Method> {
    let (input, return_type) = ws(parse_identifier)(input)?;
    let (input, name) = ws(parse_identifier)(input)?;
    let (input, parameters) = ws(parse_method_parameters)(input)?;

    Ok((
        input,
        Method {
            name: name.to_string(),
            return_type: return_type.to_string(),
            parameters,
            extra: Extra::new(),
        },
    ))
}

fn parse_identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn parse_interface_body(input: &str) -> IResult<&str, InterfaceBody> {
    let (input, methods) = many0(ws(parse_method))(input)?;

    Ok((input, InterfaceBody { methods }))
}

fn parse_interface(input: &str) -> IResult<&str, TopLevelItemDefinition> {
    let (input, _) = tag("interface")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = parse_identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, body) = delimited(tag("{"), parse_interface_body, tag("}"))(input)?;
    Ok((
        input,
        TopLevelItemDefinition::Interface(Interface {
            name: name.to_string(),
            methods: body.methods,
        }),
    ))
}

fn parse_attribute(input: &str) -> IResult<&str, Option<(String, String)>> {
    let result: Result<(&str, &str), nom::Err<nom::error::Error<&str>>> = ws(tag("]"))(input);

    match result {
        Ok((input, _)) => Ok((input, None)),
        Err(_) => {
            let (input, name) = ws(parse_identifier)(input)?;
            let (input, _) = ws(tag("("))(input)?;
            let (input, content) = take_till(|c| c == ')')(input)?;
            let (input, _) = ws(tag(")"))(input)?;
            let (input, _) = opt(tag(","))(input)?;

            Ok((input, Some((name.to_string(), content.to_string()))))
        }
    }
}

fn parse_attributes(input: &str) -> IResult<&str, HashMap<String, String>> {
    let mut attributes = HashMap::new();
    let (mut input, _) = ws(tag("["))(input)?;

    loop {
        match parse_attribute(input) {
            Ok((next_input, Some(attribute))) => {
                input = next_input;
                attributes.insert(attribute.0, attribute.1);
            }
            Ok((next_input, _)) => {
                input = next_input;
            }
            Err(_) => {
                break;
            }
        }
    }

    Ok((input, attributes))
}

pub fn parse_implements(mut input: &str) -> IResult<&str, Vec<String>> {
    let mut implements = vec![];
    loop {
        let (next_input, name) = ws(parse_identifier)(input)?;
        implements.push(name.to_string());
        let result: Result<(&str, &str), nom::Err<nom::error::Error<&str>>> =
            ws(tag(","))(next_input);

        input = next_input;
        if result.is_err() {
            break;
        }
    }

    Ok((input, implements))
}

pub fn parse_class(input: &str) -> IResult<&str, TopLevelItemDefinition> {
    let (input, _) = tag("class")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = ws(parse_identifier)(input)?;

    let (input, colon) = opt(tag(":"))(input)?;

    let (input, implements) = if colon.is_some() {
        parse_implements(input)?
    } else {
        (input, vec![])
    };

    let (input, _) = delimited(tag("{"), parse_interface_body, tag("}"))(input)?;
    Ok((
        input,
        TopLevelItemDefinition::Class(Class {
            name: name.to_string(),
            implements,
        }),
    ))
}

pub fn parse_top_level(input: &str) -> IResult<&str, TopLevelItem> {
    let (input, attributes) = ws(parse_attributes)(input)?;
    let (input, definition) = alt((parse_interface, parse_class))(input)?;

    Ok((
        input,
        TopLevelItem {
            attributes,
            definition,
        },
    ))
}

pub fn parse_idl(input: &str) -> Option<Idl> {
    let result = many0(ws(parse_top_level))(input);
    println!("{:?}", result);

    if let Ok((input, items)) = result {
        if input.len() == 0 {
            return Some(Idl { items });
        }
    }

    None
}
