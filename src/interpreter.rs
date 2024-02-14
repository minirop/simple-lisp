use std::io::Write;
use std::path::Path;
use crate::parser::*;
use crate::Node;
use std::fs;
use std::collections::HashMap;
use std::path::PathBuf;
use pest::Parser;
use pest_derive::Parser;
use rand::Rng;

#[derive(Parser)]
#[grammar = "simple-lisp.pest"]
pub struct SimpleListParser;

#[derive(Clone)]
struct Class {
    parent: Option<String>,
    fields: HashMap<String, Node>,
    functions: HashMap<String, Node>,
}

#[derive(Debug)]
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
    classes: HashMap<String, Class>,
    return_value: Option<Node>,
    path: String,
    libs: Vec<libloading::Library>,
}

impl Visitor {
    pub fn new() -> Self {
        let mut natives = HashMap::<String, Box::<dyn Fn(Vec<Node>) -> Node>>::new();

        load_io_module(&mut natives);
        load_maths_module(&mut natives);
        load_list_module(&mut natives);
        load_type_module(&mut natives);

        let mut root = Scope::new();
        root.variables.insert("null".to_string(), Node::Null);
        root.variables.insert("true".to_string(), Node::Bool(true));
        root.variables.insert("false".to_string(), Node::Bool(false));

        Self {
            scopes: vec![root],
            natives: natives,
            classes: HashMap::new(),
            return_value: None,
            path: String::new(),
            libs: vec![],
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
            ret = self.evaluate_node(&node);

            if let Some(val) = &self.return_value {
                ret = val.clone();
                break;
            }
        }

        ret
    }

    fn evaluate_node(&mut self, node: &Node) -> Node {
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
                } else if self.natives.contains_key(s) {
                    Node::Function { name: "<native#1>".to_string(), params: vec![], body: vec![], }
                } else {
                    if let Some(func) = self.find_function(&s) {
                        func
                    } else {
                        panic!("Unknown variable or function: {s}");
                    }
                }
            },
            Node::Integer(i) => Node::Integer(*i),
            Node::String(s) => Node::String(s.clone()),
            Node::Float(f) => Node::Float(*f),
            _ => panic!("{node}"),
        }
    }

    fn evaluate_call(&mut self, name: &str, args: &Vec<Node>) -> Node {
        match name {
            "let" => {
                let varname = match args[0].clone() {
                    Node::Identifier(s) => s,
                    _ => panic!("{}", args[0]),
                };
                let last_scope = self.scopes.last().unwrap();
                if last_scope.variables.contains_key(&varname) {
                    panic!("Variable '{varname}' already exists in that scope.");
                }

                let ret = self.evaluate_node(&args[1]);

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
                    ret = self.evaluate_node(&arg);
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

                let ret = self.evaluate_node(&args[1]);

                self.update_variable(&varname, ret.clone());

                ret
            },
            "if" => {
                let cond = self.evaluate_node(&args[0]);

                match cond {
                    Node::Bool(b) => {
                        let ret;

                        self.scopes.push(Scope::new());
                        if b {
                            ret = self.evaluate_node(&args[1]);
                        } else if args.len() > 2 {
                            ret = self.evaluate_node(&args[2]);
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
                    let cond = self.evaluate_node(&args[0]);
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
                let var = self.evaluate_node(&args[0]);

                for i in 1..(args.len() - 1) {
                    let Node::Call { name, args: list } = &args[i] else {
                        panic!("switch expects case statements. Got {:?}.", args[i]);
                    };

                    if name != "case" {
                        panic!("switch expects case statements. Got {:?}.", name);
                    }

                    if list.len() != 2 {
                        panic!("switch expects lists of 2 elements. Got {} elements.", list.len());
                    }

                    let value = self.evaluate_node(&list[0]);

                    let are_equals = self.check_equality(&var, &value);

                    if are_equals {
                        ret = self.evaluate_node(&list[1]);
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
                let ret = self.evaluate_node(&args[0]);
                self.return_value = Some(ret.clone());
                ret
            },
            "dump" => {
                let ret = self.evaluate_node(&args[0]);
                match ret {
                    Node::String(s) => println!("string: {s}"),
                    Node::Integer(i) => println!("int: {i}"),
                    Node::Float(f) => println!("float: {f}"),
                    Node::Bool(b) => println!("bool: {b}"),
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
                        self.execute_function(id, &args)
                    },
                    Node::Function { .. } => {
                        self.scopes.last_mut().unwrap().functions.insert("lambda#1".to_string(), args[0].clone());
                        let mut args = args.clone();
                        args.remove(0);
                        let ret = self.execute_function("lambda#1", &args);
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
                

                if Path::new(&format!("{}.sl", filename)).exists() {
                    self.interpret(filename)
                } else if Path::new(&format!("{}.so", filename)).exists() {
                    self.load_library(filename)
                } else {
                    eprintln!("Could not load module {:?}", filename);
                    Node::Null
                }
            },
            "class" => {
                if self.scopes.len() > 2 {
                    panic!("Class definition can only be done in the main scope.");
                }

                let mut skipped = 1;
                let name = match &args[0] {
                    Node::Identifier(id) => id,
                    _ => panic!("'class' only accept identifiers. Got {:?}", args[0]),
                };

                let mut parent = None;
                if args.len() > 1 {
                    if let Node::Identifier(parent_class) = &args[1] {
                        skipped += 1;
                        parent = Some(parent_class.clone());
                    }
                }
                let mut fields: HashMap<String, Node> = HashMap::new();
                let mut functions: HashMap<String, Node> = HashMap::new();

                for elem in args.iter().skip(skipped) {
                    match elem {
                        Node::Call { name, args } => {
                            if name != "let" {
                                panic!("Can't call '{}' inside the body of a class, only 'let' and 'fun' are available.", name);
                            }

                            let field = match &args[0] {
                                Node::Identifier(id) => id,
                                _ => panic!("'let' expects an identifier. Got {:?}", args[0]),
                            };

                            fields.insert(field.clone(), args[1].clone());
                        },
                        Node::Function { name, .. } => {
                            functions.insert(name.clone(), elem.clone());
                            self.insert_functions(&name, elem.clone());
                        },
                        _ => {
                            panic!("{:?}", elem);
                        },
                    };
                }

                self.classes.insert(name.clone(), Class {
                    parent, fields, functions,
                });

                Node::Null
            },
            "new" => {
                let classname = match &args[0] {
                    Node::Identifier(id) => id,
                    _ => panic!("'new' only accept identifiers. Got {:?}.", args[0]),
                };
                let mut class = &self.classes[classname];
                let mut fields = class.fields.clone();

                while let Some(parent) = &class.parent {
                    class = &self.classes[parent];
                    fields.extend(class.fields.clone());
                }

                Node::Instance {
                    class: classname.to_string(),
                    fields,
                }
            },
            "inc" => {
                let mut ret = Node::Null;
                for a in args {
                    match a {
                        Node::Identifier(id) => {
                            let variable = self.find_variable(&id);
                            let Some(variable) = variable else {
                                panic!("Can't increment non-existing variable '{}'.", id);
                            };
                            let new_val = match variable {
                                Node::Integer(i) => Node::Integer(i + 1),
                                Node::Float(f) => Node::Float(f + 1.0),
                                _ => panic!("'inc' accepts only integers and floats variables. Got {:?}.", variable),
                            };

                            ret = new_val.clone();
                            self.update_variable(&id, new_val);
                        },
                        Node::Integer(i) => {
                            ret = Node::Integer(i + 1);
                        },
                        _ => panic!("'inc' only accept identifiers, integers or float. Got {:?}.", args[0]),
                    };
                }

                ret
            },
            "dec" => {
                let mut ret = Node::Null;
                for a in args {
                    match a {
                        Node::Identifier(id) => {
                            let variable = self.find_variable(&id);
                            let Some(variable) = variable else {
                                panic!("Can't decrement non-existing variable '{}'.", id);
                            };
                            let new_val = match variable {
                                Node::Integer(i) => Node::Integer(i - 1),
                                Node::Float(f) => Node::Float(f - 1.0),
                                _ => panic!("'dec' accepts only integers and floats variables. Got {:?}.", variable),
                            };

                            ret = new_val.clone();
                            self.update_variable(&id, new_val);
                        },
                        Node::Integer(i) => {
                            ret = Node::Integer(i - 1);
                        },
                        _ => panic!("'dec' only accept identifiers, integers or float. Got {:?}.", args[0]),
                    };
                }

                ret
            },
            _ => {
                if self.scopes.last_mut().unwrap().functions.contains_key(name) {
                    self.execute_function(name, args)
                } else if self.natives.contains_key(name) {
                    self.execute_native_function(name, args)
                } else {
                    if let Some(func) = &self.find_function(&name) {
                        self.scopes.last_mut().unwrap().functions.insert("lambda#1".to_string(), func.clone());
                        let ret = self.execute_function("lambda#1", args);
                        self.scopes.last_mut().unwrap().functions.remove("lambda#1");

                        ret
                    } else if let Some(var) = &self.find_variable(&name) {
                        self.scopes.last_mut().unwrap().functions.insert("lambda#1".to_string(), var.clone());
                        let ret = self.execute_function("lambda#1", args);
                        self.scopes.last_mut().unwrap().functions.remove("lambda#1");

                        ret
                    } else {
                        panic!("Unknown function: {}", name);
                    }
                }
            },
        }
    }

    fn load_library(&mut self, filename: &str) -> Node {
        unsafe {
            let lib = libloading::Library::new(format!("{}.so", filename)).unwrap();
            let func: libloading::Symbol<unsafe extern fn(&mut HashMap<String, Box<dyn Fn(Vec<Node>) -> Node>>) -> Node> = lib.get(b"module_init").unwrap();
            println!("load_library: {}", filename);
            let ret = func(&mut self.natives);
            self.libs.push(lib);
            ret
        }
    }

    fn check_equality(&self, left: &Node, right: &Node) -> bool {
        match (left, right) {
            (Node::Integer(i), Node::Integer(j)) => i == j,
            _ => false, 
        }
    }

    fn evaluate_list(&mut self, args: &Vec<Node>) -> Vec<Node> {
        let mut values = vec![];

        for node in args {
            values.push(self.evaluate_node(&node));
        }

        values
    }

    fn execute_native_function(&mut self, name: &str, args: &Vec<Node>) -> Node {
        let args = self.evaluate_list(args);

        self.natives[name](args)
    }

    fn execute_function(&mut self, name: &str, args: &Vec<Node>) -> Node {
        let func = self.find_function(&name).unwrap();
        let mut instance_var = None;
        let mut instance_class = String::new();
        let mut instance_fields: HashMap<String, Node> = HashMap::new();
        let mut all_instance_fields = HashMap::new();

        match func {
            Node::Function { name, params, body } => {
                let mut scope = Scope::new();
                let mut f_params = params;
                let mut f_body = body;

                if args.len() > 0 {
                    if let Node::Identifier(varname) = args[0].clone() {
                        if let Some(var) = &self.find_variable(&varname) {
                            if let Node::Instance { class, fields } = &var {
                                instance_var = Some(varname);
                                instance_class = class.clone();
                                all_instance_fields = fields.clone();

                                let mut classtype = &self.classes[class];
                                while !classtype.functions.contains_key(&name) {
                                    if classtype.parent.is_none() {
                                        panic!("Class '{class}' doesn't have a function named '{name}'.");
                                    } else {
                                        classtype = &self.classes[&classtype.parent.clone().unwrap()];
                                    }
                                }

                                if let Node::Function { name: _, params, body } = &classtype.functions[&name] {
                                    f_params = params.clone();
                                    f_body = body.clone();
                                }

                                loop {
                                    for (name, _) in &classtype.fields {
                                        instance_fields.insert(name.clone(), all_instance_fields[name].clone());
                                    }

                                    if classtype.parent.is_none() {
                                        break;
                                    }

                                    classtype = &self.classes[&classtype.parent.clone().unwrap()];
                                }
                            }
                        }
                    }
                }

                if instance_var.is_none() && self.natives.contains_key(&name) {
                    return self.execute_native_function(&name, args);
                }

                let offset = if instance_var.is_some() { 1 } else { 0 };
                if args.len() > f_params.len() + offset {
                    panic!("Too much arguments given to '{name}'.");
                }

                if instance_var.is_some() {
                    for (name, value) in &instance_fields {
                        scope.variables.insert(name.clone(), self.evaluate_node(&value));
                    }
                }

                for (i, param) in f_params.iter().enumerate() {
                    if i < args.len() {
                        scope.variables.insert(param.name.clone(), self.evaluate_node(&args[i + offset]));
                    } else {
                        if let Some(def_val) = &param.default_value {
                            scope.variables.insert(param.name.clone(), self.evaluate_node(&def_val));
                        } else {
                            panic!("Parameter '{name}' isn't set and has no default value.");
                        }
                    }
                }

                self.scopes.push(scope);

                let ret = self.evaluate_block(f_body);

                if let Some(varname) = instance_var {
                    let mut new_fields = all_instance_fields;

                    instance_fields.iter().for_each(|(key, val)| {
                        if let Some(f) = self.scopes.last().unwrap().variables.get(key) {
                            new_fields.insert(key.clone(), f.clone());
                        } else {
                            new_fields.insert(key.clone(), val.clone());
                        }
                    });

                    let new_var = Node::Instance { class: instance_class.clone(), fields: new_fields };
                    self.update_variable(&varname, new_var);
                }

                self.scopes.pop();
                self.return_value = None;

                ret
            },
            Node::Call { name, args } => {
                self.evaluate_call(&name, &args)
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
                    _ => panic!("'add' doesn't accept {n} as operand"),
                }
            },
            Node::Float(f) => {
                match n {
                    Node::Integer(i) => Node::Float(f + (*i as f32)),
                    Node::Float(g) => Node::Float(f + g),
                    Node::String(s) => Node::String(format!("{f}{s}")),
                    _ => panic!("'add' doesn't accept {n} as operand"),
                }
            },
            Node::String(s) => {
                match n {
                    Node::Integer(i) => Node::String(format!("{s}{i}")),
                    Node::Float(f) => Node::String(format!("{s}{f}")),
                    Node::String(t) => Node::String(format!("{s}{t}")),
                    _ => panic!("'add' doesn't accept {n} as operand"),
                }
            },
            _ => panic!("'add' doesn't accept {n} as operand"),
        };
    }

    ret
}

fn minus_operator(args: Vec<Node>) -> Node {

    if args.len() == 1 {
        match args[0] {
            Node::Integer(i) => Node::Integer(-i),
            Node::Float(f) => Node::Float(-f),
            _ => panic!("'sub' doesn't accept {} as operand", args[0]),
        }
    } else {
        let mut ret = args[0].clone();

        for n in args.iter().skip(1) {
            ret = match ret {
                Node::Integer(i) => {
                    match n {
                        Node::Integer(j) => Node::Integer(i - j),
                        Node::Float(f) => Node::Float((i as f32) - f),
                        Node::String(..) => panic!("Can't substract a string from an int"),
                        _ => panic!("'sub' doesn't accept {n} as operand"),
                    }
                },
                Node::Float(f) => {
                    match n {
                        Node::Integer(i) => Node::Float(f - (*i as f32)),
                        Node::Float(g) => Node::Float(f - g),
                        Node::String(..) => panic!("Can't substract a string from a float"),
                        _ => panic!("'sub' doesn't accept {n} as operand"),
                    }
                },
                _ => panic!("'sub' doesn't accept {n} as operand"),
            };
        }

        ret
    }
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
                    _ => panic!("'mul' doesn't accept {n} as operand"),
                }
            },
            Node::Float(f) => {
                match n {
                    Node::Integer(i) => Node::Float(f * (*i as f32)),
                    Node::Float(g) => Node::Float(f * g),
                    Node::String(..) => panic!("Can't multiply a float and a string"),
                    _ => panic!("'mul' doesn't accept {n} as operand"),
                }
            },
            Node::String(s) => {
                match n {
                    Node::Integer(i) => Node::String(s.repeat(*i as usize)),
                    Node::Float(..) => panic!("Can't multiply a string and a float"),
                    Node::String(..) => panic!("Can't multiply two strings together"),
                    _ => panic!("'mul' doesn't accept {n} as operand"),
                }
            },
            _ => panic!("'mul' doesn't accept {n} as operand"),
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
                    _ => panic!("'div' doesn't accept {n} as operand"),
                }
            },
            Node::Float(f) => {
                match n {
                    Node::Integer(i) => Node::Float(f / (*i as f32)),
                    Node::Float(g) => Node::Float(f / g),
                    Node::String(..) => panic!("Can't divide a float and a string"),
                    _ => panic!("'div' doesn't accept {n} as operand"),
                }
            },
            Node::String(..) => {
                match n {
                    Node::Integer(..) => panic!("Can't divide a string and an int"),
                    Node::Float(..) => panic!("Can't divide a string and a float"),
                    Node::String(..) => panic!("Can't divide two strings together"),
                    _ => panic!("'div' doesn't accept {n} as operand"),
                }
            },
            _ => panic!("'div' doesn't accept {n} as operand"),
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

    if name == "eq" {
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
    cmp_binary_operator("lt",
                        |x, y| x < y,
                        |x, y| x < y,
                        |x, y| x < y,
                        args)
}

fn le_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("le",
                        |x, y| x <= y,
                        |x, y| x <= y,
                        |x, y| x <= y,
                        args)
}

fn gt_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("gt",
                        |x, y| x > y,
                        |x, y| x > y,
                        |x, y| x > y,
                        args)
}

fn ge_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("ge",
                        |x, y| x >= y,
                        |x, y| x >= y,
                        |x, y| x >= y,
                        args)
}

fn eq_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("eq",
                        |x, y| x == y,
                        |x, y| x == y,
                        |x, y| x == y,
                        args)
}

fn neq_operator(args: Vec<Node>) -> Node {
    cmp_binary_operator("neq",
                        |x, y| x != y,
                        |x, y| x != y,
                        |x, y| x != y,
                        args)
}

fn random(args: Vec<Node>) -> Node {
    let mut rng = rand::thread_rng();

    match args.len() {
        0 => Node::Float(rng.gen::<f32>()),
        1 => {
            let Node::Integer(max) = args[0] else {
                panic!("'random' expects only integer arguments. Got {:?}.", args[0]);
            };

            let val: i32 = rng.gen_range(0..max);
            Node::Integer(val)
        },
        2 => {
            let Node::Integer(min) = args[0] else {
                panic!("'random' expects only integer arguments. Got {:?}.", args[0]);
            };
            let Node::Integer(max) = args[1] else {
                panic!("'random' expects only integer arguments. Got {:?}.", args[0]);
            };

            let val: i32 = rng.gen_range(min..max);
            Node::Integer(val)
        },
        _ => panic!("'random' expects between 0 and 2 arguments. Got {}.", args.len()),
    }
}

fn load_maths_module(natives: &mut HashMap<String, Box<dyn Fn(Vec<Node>) -> Node>>) {
    natives.insert("add".to_string(), Box::new(plus_operator));
    natives.insert("sub".to_string(), Box::new(minus_operator));
    natives.insert("mul".to_string(), Box::new(mult_operator));
    natives.insert("div".to_string(), Box::new(div_operator));
    natives.insert("lt".to_string(), Box::new(lt_operator));
    natives.insert("gt".to_string(), Box::new(gt_operator));
    natives.insert("le".to_string(), Box::new(le_operator));
    natives.insert("ge".to_string(), Box::new(ge_operator));
    natives.insert("eq".to_string(), Box::new(eq_operator));
    natives.insert("neq".to_string(), Box::new(neq_operator));

    natives.insert("random".to_string(), Box::new(random));
}

fn list_size(args: Vec<Node>) -> Node {
    if args.len() != 1 {
        panic!("size expects 1 argument. Got {}.", args.len());
    }

    match &args[0] {
        Node::List(l) => Node::Integer(l.len() as i32),
        Node::Null => Node::Integer(0),
        _ => Node::Integer(1),
    }
}

fn list_get(args: Vec<Node>) -> Node {
    if args.len() != 2 {
        panic!("nth expects 2 argument. Got {}.", args.len());
    }

    let Node::List(list) = &args[0] else {
        panic!("nth only accepts list.");
    };

    let Node::Integer(index) = &args[1] else {
        panic!("nth only accepts integer indices.");
    };
    let index = *index as usize;

    if list.len() > index {
        list[index].clone()
    } else {
        panic!("nth out-of-bound access. list size: {}, index provided: {}", list.len(), index);
    }
}

fn load_list_module(natives: &mut HashMap<String, Box<dyn Fn(Vec<Node>) -> Node>>) {
    natives.insert("size".to_string(), Box::new(list_size));
    natives.insert("nth".to_string(), Box::new(list_get));
}

fn is_null(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::Null = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn is_int(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::Integer(_) = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn is_float(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::Float(_) = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn is_string(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::String(_) = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn is_bool(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::Bool(_) = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn is_list(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::List(_) = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn is_instance(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::Instance { .. } = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn is_function(args: Vec<Node>) -> Node {
    for a in &args {
        let Node::Function { .. } = a else {
            return Node::Bool(false);
        };
    }
    Node::Bool(args.len() > 0)
}

fn load_type_module(natives: &mut HashMap<String, Box<dyn Fn(Vec<Node>) -> Node>>) {
    natives.insert("is-null".to_string(), Box::new(is_null));
    natives.insert("is-int".to_string(), Box::new(is_int));
    natives.insert("is-float".to_string(), Box::new(is_float));
    natives.insert("is-string".to_string(), Box::new(is_string));
    natives.insert("is-bool".to_string(), Box::new(is_bool));
    natives.insert("is-list".to_string(), Box::new(is_list));
    natives.insert("is-instance".to_string(), Box::new(is_instance));
    natives.insert("is-function".to_string(), Box::new(is_function));
}

fn println(args: Vec<Node>) -> Node {
    print(args);
    println!("");
    Node::Null
}

fn print(args: Vec<Node>) -> Node {
    for a in &args {
        match a {
            Node::Integer(i) => print!("{i}"),
            Node::Float(f) => print!("{f}"),
            Node::Bool(b) => print!("{b}"),
            Node::String(s) => print!("{}", unescaper::unescape(s).unwrap()),
            Node::Null => print!("null"),
            _ => print!("<error>"),
        };
    }
    std::io::stdout().flush().unwrap();

    Node::Null
}

fn read(_args: Vec<Node>) -> Node {
    let mut ret = String::new();

    std::io::stdin()
        .read_line(&mut ret)
        .expect("Failed to read line");

    Node::String(ret.trim().to_string())
}

fn read_int(_args: Vec<Node>) -> Node {
    let mut ret = String::new();

    std::io::stdin()
        .read_line(&mut ret)
        .expect("Failed to read line");

    match ret.trim().parse::<i32>() {
        Ok(i) => Node::Integer(i),
        Err(e) => {
            println!("{:?}", e);
            Node::Null
        },
    }
}

fn read_float(_args: Vec<Node>) -> Node {
    let mut ret = String::new();

    std::io::stdin()
        .read_line(&mut ret)
        .expect("Failed to read line");

    match ret.parse::<f32>() {
        Ok(i) => Node::Float(i),
        Err(_) => Node::Null,
    }
}

fn load_io_module(natives: &mut HashMap<String, Box<dyn Fn(Vec<Node>) -> Node>>) {
    natives.insert("print".to_string(), Box::new(print));
    natives.insert("println".to_string(), Box::new(println));
    natives.insert("read".to_string(), Box::new(read));
    natives.insert("read-int".to_string(), Box::new(read_int));
    natives.insert("read-float".to_string(), Box::new(read_float));
}
