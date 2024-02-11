use std::collections::HashMap;
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
    Instance {
        class: String,
        fields: HashMap<String, Node>,
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
