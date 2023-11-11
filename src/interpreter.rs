use crate::parser::*;
use crate::Node;
use std::fs;
use std::collections::HashMap;
use std::path::PathBuf;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "simple-lisp.pest"]
pub struct SimpleListParser;

struct Scope {
    functions: HashMap<String, Node>,
    variables: HashMap<String, Node>,
}

impl Scope {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            variables: HashMap::new(),
        }
    }
}

pub struct Visitor {
    scopes: Vec<Scope>,
    natives: HashMap<String, Box<dyn Fn(Vec<Node>) -> Node>>,
    return_value: Option<Node>,
    path: String,
}

impl Visitor {
    pub fn new() -> Self {
        let mut nat = HashMap::<String, Box::<dyn Fn(Vec<Node>) -> Node>>::new();
        nat.insert("+".to_string(), Box::new(plus_operator));
        nat.insert("-".to_string(), Box::new(minus_operator));
        nat.insert("*".to_string(), Box::new(mult_operator));
        nat.insert("/".to_string(), Box::new(div_operator));
        nat.insert("<".to_string(), Box::new(lt_operator));
        nat.insert(">".to_string(), Box::new(gt_operator));
        nat.insert("<=".to_string(), Box::new(le_operator));
        nat.insert(">=".to_string(), Box::new(ge_operator));
        nat.insert("=".to_string(), Box::new(eq_operator));

        Self {
            scopes: vec![Scope::new()],
            natives: nat,
            return_value: None,
            path: String::new(),
        }
    }

    pub fn interpret(&mut self, filename: &str) -> Node {
        let cur_path = self.path.clone();
        let filename = if filename.starts_with("/") {
            PathBuf::from(&filename)
        } else {
            PathBuf::from(&format!("{}/{}", cur_path, filename))
        };
        let path = filename.parent().unwrap().to_str().unwrap().to_string();
        self.path = path.clone();

        let filename = filename.to_str().unwrap().to_string();
        let filename = if filename.ends_with(".sl") {
            filename
        } else {
            format!("{}.sl", filename)
        };

        let data = fs::read_to_string(&filename).unwrap();
        let res = SimpleListParser::parse(Rule::file, &data);

        let ast = match res {
            Ok(pairs) => parse_block(pairs).unwrap(),
            Err(e) => panic!("Can't parse {}:\n{:?}", filename, e),
        };

        self.scopes.push(Scope::new());
        let ret = self.evaluate_block(ast);
        self.scopes.pop();
        self.return_value = None;

        ret
    }

    fn evaluate_block(&mut self, nodes: Vec<Node>) -> Node {
        let mut ret = Node::Null;

        for node in nodes {
            ret = self.evaluate_node(node);

            if let Some(val) = &self.return_value {
                ret = val.clone();
                break;
            }
        }

        ret
    }

    fn evaluate_node(&mut self, node: Node) -> Node {
        let cpy = node.clone();

        match node {
            Node::Function { name, .. } => {
                let ret = cpy.clone();

                if name.len() > 0 {
                    self.insert_functions(&name, cpy);
                }

                ret
            },
            Node::Call { name, args } => {
                self.evaluate_call(name, args)
            },
            Node::Identifier(s) => {
                let variable = self.find_variable(&s);
                if let Some(v) = variable {
                    v
                } else if self.natives.contains_key(&s) {
                    Node::Function { name: "<native#1>".to_string(), params: vec![], body: vec![], }
                } else {
                    if let Some(func) = self.find_function(&s) {
                        func
                    } else {
                        panic!("Unknown variable or function: {s}");
                    }
                }
            },
            Node::Integer(i) => Node::Integer(i),
            Node::String(s) => Node::String(s),
            Node::Float(f) => Node::Float(f),
            _ => panic!("{node}"),
        }
    }

    fn evaluate_call(&mut self, name: String, args: Vec<Node>) -> Node {
        match name.as_str() {
            "let" => {
                let varname = match args[0].clone() {
                    Node::Identifier(s) => s,
                    _ => panic!("{}", args[0]),
                };
                let last_scope = self.scopes.last().unwrap();
                if last_scope.variables.contains_key(&varname) {
                    panic!("Variable '{varname}' already exists in that scope.");
                }

                let ret = self.evaluate_node(args[1].clone());

                let last_scope = self.scopes.last_mut().unwrap();
                last_scope.variables.insert(varname, ret.clone());

                ret
            },
            "list" => {
                let args = self.evaluate_list(args);

                Node::List(args)
            },
            "block" => {
                let mut ret = Node::Null;

                for arg in args {
                    ret = self.evaluate_node(arg.clone());
                }

                ret
            },
            "set" => {
                let varname = match args[0].clone() {
                    Node::Identifier(s) => s,
                    _ => panic!("{}", args[0]),
                };
                
                if self.find_variable(&varname).is_none() {
                    panic!("Variable '{varname}' does not exist in that scope.");
                }

                let ret = self.evaluate_node(args[1].clone());

                self.update_variable(&varname, ret.clone());

                ret
            },
            "if" => {
                let cond = self.evaluate_node(args[0].clone());

                match cond {
                    Node::Bool(b) => {
                        let ret;

                        self.scopes.push(Scope::new());
                        if b {
                            ret = self.evaluate_node(args[1].clone());
                        } else if args.len() > 2 {
                            ret = self.evaluate_node(args[2].clone());
                        } else {
                            ret = Node::Null;
                        }
                        self.scopes.pop();

                        ret
                    },
                    _ => panic!("if condition isn't a bool. Got '{}'", args[0]),
                }
            },
            "while" => {
                let mut ret = Node::Null;
                let mut continue_loop = true;

                while continue_loop {
                    let cond = self.evaluate_node(args[0].clone());
                    match cond {
                        Node::Bool(b) => {
                            if b {
                                self.scopes.push(Scope::new());
                                
                                ret = self.evaluate_block(args.clone());

                                self.scopes.pop();
                            }

                            if self.return_value.is_some() {
                                continue_loop = false;
                            } else {
                                continue_loop = b;
                            }
                        },
                        _ => panic!("if condition isn't a bool. Got '{}'", args[0]),
                    };
                }

                ret
            },
            "switch" => {
                let mut ret = Node::Null;
                let mut continue_loop = true;
                let var = self.evaluate_node(args[0].clone());

                for i in 1..(args.len() - 1) {
                    let value = self.evaluate_node(args[i].clone());

                    let are_equals = self.check_equality(&var, &value);

                    if are_equals {
                        ret = self.evaluate_node(args[i + 1].clone());
                        continue_loop = false;
                    }

                    if !continue_loop {
                        break;
                    }
                }

                if continue_loop {
                    ret = args.last().unwrap().clone();
                }

                ret
            },
            "return" => {
                let ret = self.evaluate_node(args[0].clone());
                self.return_value = Some(ret.clone());
                ret
            },
            "dump" => {
                let ret = self.evaluate_node(args[0].clone());
                match ret {
                    Node::String(s) => println!("string: {s}"),
                    Node::Integer(i) => println!("int: {i}"),
                    Node::Float(f) => println!("float: {f}"),
                    Node::List(list) => println!("list: {:?}", list),
                    Node::Null => println!("null: NULL"),
                    Node::Function { name, .. } => {
                        if name.len() > 0 {
                            println!("function: {name}");
                        } else {
                            println!("function: <lambda#1>");
                        }
                    },
                    _ => println!("{:?}", ret),
                };

                Node::Null
            },
            "call" => {
                match &args[0] {
                    Node::Identifier(id) => {
                        let mut args = args.clone();
                        args.remove(0);
                        self.execute_function(id.clone(), args)
                    },
                    Node::Function { .. } => {
                        self.scopes.last_mut().unwrap().functions.insert("lambda#1".to_string(), args[0].clone());
                        let mut args = args;
                        args.remove(0);
                        let ret = self.execute_function("lambda#1".to_string(), args);
                        self.scopes.last_mut().unwrap().functions.remove("lambda#1");

                        ret
                    },
                    _ => panic!("{:?}", args),
                }
            },
            "load" => {
                let filename = match &args[0] {
                    Node::String(s) => s,
                    _ => panic!("load only accept strings. Got {:?}", args[0]),
                };
                
                self.interpret(filename)
            },
            _ => {
                if self.scopes.last_mut().unwrap().functions.contains_key(&name) {
                    self.execute_function(name, args)
                } else if self.natives.contains_key(&name) {
                    self.execute_native_function(name, args)
                } else {
                    if let Some(func) = &self.find_function(&name) {
                        self.scopes.last_mut().unwrap().functions.insert("lambda#1".to_string(), func.clone());
                        let ret = self.execute_function("lambda#1".to_string(), args);
                        self.scopes.last_mut().unwrap().functions.remove("lambda#1");

                        ret
                    } else if let Some(var) = &self.find_variable(&name) {
                        self.scopes.last_mut().unwrap().functions.insert("lambda#1".to_string(), var.clone());
                        let ret = self.execute_function("lambda#1".to_string(), args);
                        self.scopes.last_mut().unwrap().functions.remove("lambda#1");

                        ret
                    } else {
                        panic!("Unknown function: {}", name);
                    }
                }
            },
        }
    }

    fn check_equality(&self, left: &Node, right: &Node) -> bool {
        match (left, right) {
            (Node::Integer(i), Node::Integer(j)) => i == j,
            _ => false, 
        }
    }

    fn evaluate_list(&mut self, args: Vec<Node>) -> Vec<Node> {
        let mut values = vec![];

        for node in args {
            values.push(self.evaluate_node(node));
        }

        values
    }

    fn execute_native_function(&mut self, name: String, args: Vec<Node>) -> Node {
        let args = self.evaluate_list(args);

        self.natives[&name](args)
    }

    fn execute_function(&mut self, name: String, args: Vec<Node>) -> Node {
        let func = self.find_function(&name).unwrap();
        match func {
            Node::Function { name, params, body } => {
                let mut scope = Scope::new();

                if args.len() > params.len() {
                    panic!("Too much arguments given to {name}");
                }

                for (i, param) in params.iter().enumerate() {
                    if i < args.len() {
                        scope.variables.insert(param.name.clone(), self.evaluate_node(args[i].clone()));
                    } else {
                        if let Some(def_val) = &param.default_value {
                            scope.variables.insert(param.name.clone(), self.evaluate_node(def_val.clone()));
                        } else {
                            panic!("Parameter {name} isn't set and has no default value.");
                        }
                    }
                }

                self.scopes.push(scope);

                let ret = self.evaluate_block(body);

                self.scopes.pop();
                self.return_value = None;

                ret
            },
            Node::Call { name, args } => {
                self.evaluate_call(name, args)
            },
            _ => panic!("{name} is not a function."),
        }
    }

    fn find_variable(&self, name: &str) -> Option<Node> {
        for scope in self.scopes.iter().rev() {
            if scope.variables.contains_key(name) {
                return Some(scope.variables[name].clone());
            }
        }

        None
    }

    fn update_variable(&mut self, name: &str, value: Node) {
        for scope in self.scopes.iter_mut().rev() {
            if scope.variables.contains_key(name) {
                scope.variables.insert(name.to_string(), value);
                return;
            }
        }
    }

    fn find_function(&self, name: &str) -> Option<Node> {
        for scope in self.scopes.iter().rev() {
            if scope.functions.contains_key(name) {
                return Some(scope.functions[name].clone());
            }
        }

        None
    }

    fn insert_functions(&mut self, name: &str, value: Node) {
        self.scopes.last_mut().unwrap().functions.insert(name.to_string(), value);
    }
}

fn plus_operator(args: Vec<Node>) -> Node {
    let mut ret = args[0].clone();

    for n in args.iter().skip(1) {
        ret = match ret.clone() {
            Node::Integer(i) => {
                match n {
                    Node::Integer(j) => Node::Integer(i + j),
                    Node::Float(f) => Node::Float((i as f32) + f),
                    Node::String(s) => Node::String(format!("{i}{s}")),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            Node::Float(f) => {
                match n {
                    Node::Integer(i) => Node::Float(f + (*i as f32)),
                    Node::Float(g) => Node::Float(f + g),
                    Node::String(s) => Node::String(format!("{f}{s}")),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            Node::String(s) => {
                match n {
                    Node::Integer(i) => Node::String(format!("{s}{i}")),
                    Node::Float(f) => Node::String(format!("{s}{f}")),
                    Node::String(t) => Node::String(format!("{s}{t}")),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            _ => panic!("operator '+' doesn't accept {n} as operand"),
        };
    }

    ret
}

fn minus_operator(args: Vec<Node>) -> Node {
    let mut ret = args[0].clone();

    for n in args.iter().skip(1) {
        ret = match ret {
            Node::Integer(i) => {
                match n {
                    Node::Integer(j) => Node::Integer(i - j),
                    Node::Float(f) => Node::Float((i as f32) - f),
                    Node::String(..) => panic!("Can't substract a string from an int"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            Node::Float(f) => {
                match n {
                    Node::Integer(i) => Node::Float(f - (*i as f32)),
                    Node::Float(g) => Node::Float(f - g),
                    Node::String(..) => panic!("Can't substract a string from a float"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            _ => panic!("operator '+' doesn't accept {n} as operand"),
        };
    }

    ret
}

fn mult_operator(args: Vec<Node>) -> Node {
    let mut ret = args[0].clone();

    for n in args.iter().skip(1) {
        ret = match ret.clone() {
            Node::Integer(i) => {
                match n {
                    Node::Integer(j) => Node::Integer(i * j),
                    Node::Float(f) => Node::Float((i as f32) * f),
                    Node::String(..) => panic!("Can't multiply an int and a string"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            Node::Float(f) => {
                match n {
                    Node::Integer(i) => Node::Float(f * (*i as f32)),
                    Node::Float(g) => Node::Float(f * g),
                    Node::String(..) => panic!("Can't multiply a float and a string"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            Node::String(s) => {
                match n {
                    Node::Integer(i) => Node::String(s.repeat(*i as usize)),
                    Node::Float(..) => panic!("Can't multiply a string and a float"),
                    Node::String(..) => panic!("Can't multiply two strings together"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            _ => panic!("operator '+' doesn't accept {n} as operand"),
        };
    }

    ret
}

fn div_operator(args: Vec<Node>) -> Node {
    let mut ret = args[0].clone();

    for n in args.iter().skip(1) {
        ret = match ret.clone() {
            Node::Integer(i) => {
                match n {
                    Node::Integer(j) => Node::Integer(i / j),
                    Node::Float(f) => Node::Float((i as f32) / f),
                    Node::String(..) => panic!("Can't divide an int and a string"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            Node::Float(f) => {
                match n {
                    Node::Integer(i) => Node::Float(f / (*i as f32)),
                    Node::Float(g) => Node::Float(f / g),
                    Node::String(..) => panic!("Can't divide a float and a string"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            Node::String(..) => {
                match n {
                    Node::Integer(..) => panic!("Can't divide a string and an int"),
                    Node::Float(..) => panic!("Can't divide a string and a float"),
                    Node::String(..) => panic!("Can't divide two strings together"),
                    _ => panic!("operator '+' doesn't accept {n} as operand"),
                }
            },
            _ => panic!("operator '+' doesn't accept {n} as operand"),
        };
    }

    ret
}

fn cmp_binary_operator<I, F, S>(name: &str,
                                op_int: I,
                                op_float: F,
                                op_str: S,
                                args: Vec<Node>) -> Node
    where
        I: Fn(i32, i32) -> bool,
        F: Fn(f32, f32) -> bool,
        S: Fn(String, String) -> bool,
{
    let left = args[0].clone();
    let right = args[1].clone();

    if name == "=" {
        if std::mem::discriminant(&left) != std::mem::discriminant(&right) {
            return Node::Bool(false);
        }
    }

    match left {
        Node::Integer(i) => {
            match right {
                Node::Integer(j) => Node::Bool(op_int(i, j)),
                Node::Float(f) => Node::Bool(op_float(i as f32, f)),
                Node::String(..) => panic!("Can't apply '{name}' to an int and a string"),
                _ => panic!("operator '{name}' doesn't accept {right} as operand"),
            }
        },
        Node::Float(f) => {
            match right {
                Node::Integer(i) => Node::Bool(op_float(f, i as f32)),
                Node::Float(g) => Node::Bool(op_float(f, g)),
                Node::String(..) => panic!("Can't apply '{name}' to a float and a string"),
                _ => panic!("operator '{name}' doesn't accept {right} as operand"),
            }
        },
        Node::String(s) => {
            match right {
                Node::Integer(..) => panic!("Can't apply '{name}' to a string and an int"),
                Node::Float(..) => panic!("Can't apply '{name}' to a string and a float"),
                Node::String(t) => Node::Bool(op_str(s, t)),
                _ => panic!("operator '{name}' doesn't accept {right} as operand"),
            }
        },
        _ => panic!("operator '{name}' doesn't accept {left} as operand"),
    }
}

fn lt_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("<",
                        |x, y| x < y,
                        |x, y| x < y,
                        |x, y| x < y,
                        args)
}

fn le_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("<=",
                        |x, y| x <= y,
                        |x, y| x <= y,
                        |x, y| x <= y,
                        args)
}

fn gt_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator(">",
                        |x, y| x > y,
                        |x, y| x > y,
                        |x, y| x > y,
                        args)
}

fn ge_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator(">=",
                        |x, y| x >= y,
                        |x, y| x >= y,
                        |x, y| x >= y,
                        args)
}

fn eq_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("=",
                        |x, y| x == y,
                        |x, y| x == y,
                        |x, y| x == y,
                        args)
}
