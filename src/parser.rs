use crate::*;
use pest::iterators::Pair;
use pest::iterators::Pairs;

pub fn parse_block(pairs: Pairs<Rule>) -> std::io::Result<Vec<Node>> {
    let mut nodes = vec![];

    for pair in pairs {
        if pair.as_rule() == Rule::sexp {
            nodes.push(parse_root_expression(pair)?);
        } else if pair.as_rule() != Rule::EOI {
            panic!("{:?} / {:?}", pair, pair.as_rule());
        }
    }

    Ok(nodes)
}

pub fn parse_root_expression(pair: Pair<Rule>) -> std::io::Result<Node> {
    let inner: Vec<_> = pair.into_inner().collect();

    let node = match inner[0].as_rule() {
        Rule::identifier => parse_identifier(inner)?,
        _ => panic!("{:?}", inner[0]),
    };

    Ok(node)
}

pub fn parse_identifier(elements: Vec<Pair<Rule>>) -> std::io::Result<Node> {
    let node = match elements[0].as_str() {
        "fun" => parse_function(elements)?,
        _ => parse_function_call(elements)?,
    };

    Ok(node)
}

pub fn parse_function(elements: Vec<Pair<Rule>>) -> std::io::Result<Node> {
    let name = match elements[1].as_rule() {
        Rule::identifier => elements[1].as_str().to_string(),
        _ => "".to_string(),
    };

    let param_start = if name.len() > 0 { 2 } else { 1 };

    let parameters: Vec<_> = elements[param_start].clone().into_inner().collect();
    let params = parse_parameters(parameters)?;

    let mut body = vec![];
    for elem in elements.iter().skip(param_start+1) {
        body.push(parse_expression(elem)?);
    }

    Ok(Node::Function {
        name,
        params,
        body,
    })
}

pub fn parse_function_call(elements: Vec<Pair<Rule>>) -> std::io::Result<Node> {
    let name = elements[0].as_str().to_string();

    let mut args = vec![];

    for elem in elements.iter().skip(1) {
        args.push(parse_expression(elem)?);
    }

    Ok(Node::Call {
        name,
        args,
    })
}

pub fn parse_parameters(parameters: Vec<Pair<Rule>>) -> std::io::Result<Vec<Param>> {
    let mut params = vec![];
    for elem in &parameters {
        match elem.as_rule() {
            Rule::sexp => {
                let data: Vec<_> = elem.clone().into_inner().collect();
                params.push(Param {
                    name: data[0].as_str().to_string(),
                    default_value: Some(parse_expression(&data[1])?),
                });
            },
            _ => {
                params.push(Param {
                    name: elem.as_str().to_string(),
                    default_value: None,
                });
            },
        };
    }

    Ok(params)
}

pub fn parse_expression(pair: &Pair<Rule>) -> std::io::Result<Node> {
    let node = match pair.as_rule() {
        Rule::number => {
            let i = pair.as_str().parse::<i32>().unwrap();
            Node::Integer(i)
        },
        Rule::float => {
            let f = pair.as_str().parse::<f32>().unwrap();
            Node::Float(f)
        },
        Rule::sexp => {
            let inner: Vec<_> = pair.clone().into_inner().collect();
            parse_identifier(inner)?
        },
        Rule::identifier => {
            Node::Identifier(pair.as_str().to_string())
        },
        Rule::string => {
            let mut s = pair.as_str()[1..].to_string();
            s.pop();
            Node::String(s)
        },
        _ => panic!("parse_expression: {:?}", pair),
    };

    Ok(node)
}
