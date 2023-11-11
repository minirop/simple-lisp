use std::fmt;

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub default_value: Option<Node>,
}

#[derive(Debug, Clone)]
pub enum Node {
    Function {
        name: String,
        params: Vec<Param>,
        body: Vec<Node>,
    },
    Call {
        name: String,
        args: Vec<Node>,
    },
    Integer(i32),
    Float(f32),
    String(String),
    Identifier(String),
    Bool(bool),
    List(Vec<Node>),
    Null,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Integer(i) => write!(f, "Value({i})"),
            Node::Float(f2) => write!(f, "Value({f2})"),
            Node::Bool(b) => write!(f, "Value({b})"),
            Node::String(s) => write!(f, "Value(\"{s}\")"),
            Node::Null => write!(f, "Value()"),
            Node::Identifier(id) => write!(f, "Value({id})"),
            _ => panic!("switch: {:?}", self),
        }
    }
}
/*
impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Function { name, .. } => write!(f, "function({})", name),
            Node::Call { name, .. } => write!(f, "call({})", name),
            Node::Integer(i) => write!(f, "integer({i})"),
            Node::Float(f2) => write!(f, "float({f2})"),
            Node::String(s) => write!(f, "string(\"{s}\")"),
            Node::Identifier(s) => write!(f, "identifier({s})"),
            Node::Bool(b) => write!(f, "bool({b})"),
            Node::List(list) => write!(f, "list({:?})", list),
            Node::Null => write!(f, "null"),
        }
    }
}*/
