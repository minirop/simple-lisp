use std::collections::HashMap;
use std::io::Write;
use std::fs::File;
use pest::Parser;
use crate::parser::parse_block;
use crate::*;
use std::fs;
use byteorder::{WriteBytesExt, LittleEndian};

pub fn emit(filename: &str) {
    let data = fs::read_to_string(filename).unwrap();
    let res = SimpleListParser::parse(Rule::file, &data);

    let nodes = match res {
        Ok(pairs) => parse_block(pairs).unwrap(),
        Err(e) => panic!("Can't parse {}:\n{:?}", filename, e),
    };

    let mut emitter = Emitter {
        strings: vec![],
        classes: HashMap::new(),
        closure_id: 0,
    };

    emitter.parse_root(&nodes);
}

#[derive(Debug)]
struct Function {
    name: String,
    arity: u8,
    code: Vec<u8>,
    args_names: Vec<String>,
}

#[derive(Debug)]
struct Field {
    name: String,
    default: Vec<u8>,
}

#[derive(Debug)]
struct Class {
    parent: String,
    fields: Vec<Field>,
    functions: Vec<Function>,
}

#[derive(Debug)]
struct Emitter {
    strings: Vec<String>,
    classes: HashMap<String, Class>,
    closure_id: u32,
}

impl Emitter {
    fn parse_root(&mut self, nodes: &Vec<Node>) {

        self.str_push("$self");

        self.classes.insert("$self".into(), Class {
            parent: "Object".into(),
            fields: vec![],
            functions: vec![],
        });

        let mut main_bytes = vec![];
        for node in nodes {
            match node {
                Node::Call { name, args } => {
                    main_bytes.extend(self.parse_call(name, args, &vec![], &vec![]));

                    if name != "class" {
                        main_bytes.push(OP_POP);
                    }
                },
                Node::Function { name, params, body } => {
                    let name_only = name.clone();
                    let args_ph = if params.len() > 0 { format!("_{}", ",_".repeat(params.len() - 1)) } else { "".to_string() };
                    let name = format!("{name}({args_ph})");

                    let mut args_names = vec!["this".to_string()];
                    for p in params {
                        args_names.push(p.name.clone());
                    }

                    let f = Function {
                        name: name.clone(), arity: params.len() as u8,
                        code: vec![], args_names: args_names.clone(),
                    };

                    self.classes.get_mut("$self").unwrap().functions.push(f);

                    let mut code = vec![];
                    let body_size = body.len();
                    for (i, o) in body.iter().enumerate() {
                        code.extend(self.parse_node(o, &args_names, &vec![]));
                        if i + 1 < body_size {
                            code.write_u8(OP_POP).unwrap();
                        }
                    }
                    code.write_u8(OP_RETURN).unwrap();

                    self.classes.get_mut("$self").unwrap().functions.last_mut().unwrap().code = code;

                    let count_required_args = params.iter().filter(|&p| p.default_value.is_none()).count();

                    for i in count_required_args..(args_names.len() - 1) {
                        let args_ph = if i > 0 { format!("_{}", ",_".repeat(i - 1)) } else { "".to_string() };
                        let vname = format!("{name_only}({args_ph})");

                        let mut args = vec![];
                        for idx in 1..(i + 1) {
                            args.push(Node::Identifier(args_names[idx].clone()));
                        }

                        for idx in i..params.len() {
                            let Some(param) = params.get(idx) else {
                                panic!("INVALID PARAM");
                            };

                            let Some(default) = &param.default_value else {
                                panic!("EMPTY PARAM");
                            };

                            args.push(default.clone());
                        }

                        let mut code = self.parse_call(&name_only, &args, &args_names, &vec![]);
                        code.write_u8(OP_RETURN).unwrap();

                        let f = Function {
                            name: vname, arity: i as u8,
                            code, args_names: args_names[0..(i + 1)].to_vec(),
                        };

                        self.classes.get_mut("$self").unwrap().functions.push(f);
                    }
                },
                _ => panic!("{:?} not handled.", node),
            };
        }
        main_bytes.write_u8(OP_NULL).unwrap();
        main_bytes.write_u8(OP_RETURN).unwrap();
        self.classes.get_mut("$self").unwrap().functions.push(Function {
            name: "main".into(), arity: 0, code: main_bytes,
            args_names: vec![],
        });

        let mut f = File::create("test.bin").unwrap();

        f.write(b"ROCK").unwrap();
        f.write_u8(1).unwrap();
        f.write_u32::<LittleEndian>(self.strings.len() as u32).unwrap();
        for string in &self.strings {
            Self::write_string(&mut f, &string);
        }

        f.write_u32::<LittleEndian>(self.classes.len() as u32).unwrap();
        for (name, c) in &self.classes {
            Self::write_string(&mut f, &name);
            Self::write_string(&mut f, &c.parent);
            f.write_u8(c.fields.len() as u8).unwrap();
            for field in &c.fields {
                Self::write_string(&mut f, &field.name);
                f.write_u8(field.default.len() as u8).unwrap();
                f.write(&field.default).unwrap();
            }

            f.write_u8(c.functions.len() as u8).unwrap();

            for fun in &c.functions {
                Self::write_string(&mut f, &fun.name);
                f.write_u8(fun.arity).unwrap();
                f.write_u8(0).unwrap(); // locals
                f.write_u16::<LittleEndian>(fun.code.len() as u16).unwrap();
                f.write(&fun.code).unwrap();
            }
        }
    }

    fn write_string(f: &mut File, string: &str) {
        f.write_u16::<LittleEndian>(string.len() as u16).unwrap();
        f.write(string.as_bytes()).unwrap();
    }

    fn parse_call(&mut self, name: &str, args: &Vec<Node>, args_names: &Vec<String>, fields_names: &Vec<String>) -> Vec<u8> {
        let mut bytes = vec![];

        match name {
            "print" => {
                let args_count = args.len();
                let args_ph = if args_count > 0 { format!("_{}", ",_".repeat(args_count - 1)) } else { "".to_string() };
                let name = format!("{name}({args_ph})");

                self.str_push("System");
                self.str_push(&name);

                bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("System")).unwrap();

                for a in args {
                    bytes.extend(self.parse_node(&a, args_names, fields_names));
                }

                bytes.write_u8(OP_CALL).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index(&name)).unwrap();
                bytes.write_u8(args_count as u8).unwrap();
            },
            "add" | "sub" | "mul" | "div" => {
                for a in args {
                    bytes.extend(self.parse_node(&a, args_names, fields_names));
                }

                bytes.write_u8(match name {
                    "add" => OP_ADD,
                    "sub" => OP_SUB,
                    "mul" => OP_MUL,
                    "div" => OP_DIV,
                    &_ => panic!("???"),
                }).unwrap();
            },
            "lt" | "gt" | "eq" | "neq" => {
                for a in args {
                    bytes.extend(self.parse_node(&a, args_names, fields_names));
                }

                bytes.write_u8(match name {
                    "lt" => OP_LOWER_THAN,
                    "gt" => OP_GREATER_THAN,
                    "eq" => OP_EQUAL,
                    "neq" => OP_EQUAL,
                    &_ => panic!("???"),
                }).unwrap();

                if name == "neq" {
                    bytes.write_u8(OP_NOT).unwrap();
                }
            },
            "inc" => {
                let (write_op, index) = match &args[0] {
                    Node::Identifier(name) => {
                        if args_names.contains(name) {
                            bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                            let index = args_names.iter().position(|r| r == name).unwrap();
                            bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                            (OP_STORE_LOCAL_VAR, index as u16)
                        } else if fields_names.contains(name) {
                            bytes.write_u8(OP_LOAD_FIELD_THIS).unwrap();
                            let index = self.str_index(name);
                            bytes.write_u16::<LittleEndian>(index).unwrap();
                            (OP_STORE_FIELD_THIS, index)
                        } else {
                            bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                            self.str_push(name);
                            let index = self.str_index(name);
                            bytes.write_u16::<LittleEndian>(index).unwrap();
                            (OP_STORE_MODULE_VAR, index)
                        }
                    },
                    _ =>{
                        panic!("'inc' only accepts identifiers. Got {:?}.", args[0]);
                    } ,
                };

                bytes.extend(self.parse_constant(&Node::Integer(1)));
                bytes.write_u8(OP_ADD).unwrap();
                bytes.write_u8(write_op).unwrap();
                bytes.write_u16::<LittleEndian>(index).unwrap();
            },
            "list" => {
                self.str_push("List");
                self.str_push("new()");
                self.str_push("add(_)");

                bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("List")).unwrap();
                bytes.write_u8(OP_CALL).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("new()")).unwrap();
                bytes.write_u8(0).unwrap();

                for a in args {
                    bytes.extend(self.parse_node(&a, args_names, fields_names));
                    bytes.write_u8(OP_CALL).unwrap();
                    bytes.write_u16::<LittleEndian>(self.str_index("add(_)")).unwrap();
                    bytes.write_u8(1).unwrap();
                }
            },
            "nth" => {
                self.str_push("[_]");
                self.str_push("[_]=(_)");

                bytes.extend(self.load_variable(&args[0], args_names, fields_names));
                bytes.extend(self.parse_node(&args[1], args_names, fields_names));
                if args.len() == 2 {
                    bytes.write_u8(OP_CALL).unwrap();
                    bytes.write_u16::<LittleEndian>(self.str_index("[_]")).unwrap();
                    bytes.write_u8(1).unwrap();
                } else if args.len() == 3 {
                    bytes.extend(self.parse_node(&args[2], args_names, fields_names));
                    bytes.write_u8(OP_CALL).unwrap();
                    bytes.write_u16::<LittleEndian>(self.str_index("[_]=(_)")).unwrap();
                    bytes.write_u8(2).unwrap();
                } else {
                    panic!("nth");
                }
            },
            "if" => {
                bytes.extend(self.parse_node(&args[0], args_names, fields_names));
                bytes.write_u8(OP_JUMP_IF).unwrap();
                let pos1 = bytes.len();
                bytes.write_u8(0).unwrap();
                bytes.extend(self.parse_node(&args[2], args_names, fields_names));
                bytes.write_u8(OP_JUMP).unwrap();

                let pos2 = bytes.len();
                bytes.write_u8(0).unwrap();

                let pos1end = bytes.len();
                bytes[pos1] = self.count_opcodes(&bytes[(pos1 + 1)..pos1end]);

                bytes.extend(self.parse_node(&args[1], args_names, fields_names));

                let pos2end = bytes.len();
                bytes[pos2] = self.count_opcodes(&bytes[(pos2 + 1)..pos2end]);
            },
            "while" => {
                let loop1 = bytes.len();

                bytes.extend(self.parse_node(&args[0], args_names, fields_names));
                bytes.write_u8(OP_NOT).unwrap();
                bytes.write_u8(OP_JUMP_IF).unwrap();
                let jmp1 = bytes.len();
                bytes.write_u8(0).unwrap();

                bytes.extend(self.parse_node(&args[1], args_names, fields_names));

                for i in 2..args.len() {
                    bytes.extend(self.parse_node(&args[i], args_names, fields_names));
                    bytes.write_u8(OP_POP).unwrap();
                }

                let loop2 = bytes.len();
                let loop_count = self.count_opcodes(&bytes[loop1..loop2]);
                bytes.write_u8(OP_LOOP).unwrap();
                bytes.write_u8(loop_count).unwrap();

                let jmp2 = bytes.len();
                bytes[jmp1] = self.count_opcodes(&bytes[(jmp1 + 1)..jmp2]);
                bytes.write_u8(OP_NULL).unwrap();
            },
            "class" => {
                let name = match &args[0] {
                    Node::Identifier(id) => id,
                    _ => panic!("'class' only accept identifiers. Got {:?}", args[0]),
                };

                let mut skipped = 1;
                let mut parent = "Object".to_string();
                if args.len() > 1 {
                    if let Node::Identifier(parent_class) = &args[1] {
                        parent = parent_class.clone();
                        skipped += 1;
                    }
                }

                let mut fields: Vec<Field> = vec![];
                let mut functions: Vec<Function> = vec![];

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

                            let default = self.parse_node(&args[1], &vec![], fields_names);
                            fields.push(Field {
                                name: field.clone(), default,
                            });
                        },
                        Node::Function { .. } => {},
                        _ => {
                            panic!("{:?}", elem);
                        },
                    }
                }

                for elem in args.iter().skip(skipped) {
                    match elem {
                        Node::Call { .. } => {},
                        Node::Function { name, params, body } => {
                            let mut args_names = vec!["this".to_string()];
                            for a in params {
                                args_names.push(a.name.clone());
                            }

                            let mut fields_names = vec![];
                            for field in &fields {
                                fields_names.push(field.name.clone());
                            }

                            let mut parent_ptr = parent.clone();
                            while let Some(parent_class) = self.classes.get(&parent_ptr) {
                                for field in &parent_class.fields {
                                    fields_names.push(field.name.clone());
                                }
                                parent_ptr = parent_class.parent.clone();
                            }

                            let mut code = vec![];
                            for o in body {
                                code.extend(self.parse_node(o, &args_names, &fields_names));
                            }
                            code.write_u8(OP_RETURN).unwrap();

                            let args_ph = if params.len() > 0 { format!("_{}", ",_".repeat(params.len() - 1)) } else { "".to_string() };
                            let name = format!("{name}({args_ph})");

                            functions.push(Function {
                                name: name.clone(),
                                arity: params.len() as u8,
                                args_names: args_names,
                                code: code,
                            });
                        },
                        _ => {
                            panic!("{:?}", elem);
                        },
                    };
                }

                self.classes.insert(name.into(), Class {
                    parent, fields, functions,
                });
            },
            "let" => {
                let name = match &args[0] {
                    Node::Identifier(id) => id,
                    _ => panic!("'let' expects an identifier. Got {:?}", args[0]),
                };

                self.str_push(&name);
                bytes.extend(self.parse_node(&args[1], args_names, fields_names));
                bytes.write_u8(OP_STORE_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index(name)).unwrap();
            },
            "set" => {
                let name = match &args[0] {
                    Node::Identifier(id) => id,
                    _ => panic!("'set' expects an identifier. Got {:?}", args[0]),
                };

                self.str_push(&name);
                bytes.extend(self.parse_node(&args[1], args_names, fields_names));

                if args_names.contains(name) {
                    bytes.write_u8(OP_STORE_LOCAL_VAR).unwrap();
                    let index = args_names.iter().position(|r| r == name).unwrap();
                    bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                } else if fields_names.contains(name) {
                    bytes.write_u8(OP_STORE_FIELD_THIS).unwrap();
                    let index = self.str_index(name);
                    bytes.write_u16::<LittleEndian>(index).unwrap();
                } else {
                    bytes.write_u8(OP_STORE_MODULE_VAR).unwrap();
                    self.str_push(name);
                    let index = self.str_index(name);
                    bytes.write_u16::<LittleEndian>(index).unwrap();
                }
            },
            "new" => {
                let name = match &args[0] {
                    Node::Identifier(id) => id,
                    _ => panic!("'new' expects an identifier. Got {:?}", args[0]),
                };

                if self.classes.contains_key(name) {
                    self.str_push(name);
                    bytes.write_u8(OP_ALLOCATE_VAR).unwrap();
                    bytes.write_u16::<LittleEndian>(self.str_index(name)).unwrap();
                } else {
                    let mut c = name.chars();
                    let var = match c.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().to_string(),
                    };
                    let name = &format!("{}{}", var, c.as_str());
                    let new = &format!("new({})", vec!["_"; args.len() - 1].join(","));
                    self.str_push(new);
                    self.str_push(name);

                    bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                    bytes.write_u16::<LittleEndian>(self.str_index(name)).unwrap();
                    for a in args.iter().skip(1) {
                        bytes.extend(self.parse_node(&a, args_names, fields_names));
                    }
                    bytes.write_u8(OP_CALL).unwrap();
                    bytes.write_u16::<LittleEndian>(self.str_index(new)).unwrap();
                    bytes.write_u8((args.len() - 1) as u8).unwrap();
                }
            },
            "abort" => {
                self.str_push("Fiber");
                self.str_push("abort(_)");

                bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("Fiber")).unwrap();

                bytes.extend(self.load_variable(&args[0], args_names, fields_names));

                bytes.write_u8(OP_CALL).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("abort(_)")).unwrap();
                bytes.write_u8(1).unwrap();
            },
            "fiber" => {
                self.str_push("Fiber");
                self.str_push("new(_)");

                let (name, params, body) = match &args[0] {
                    Node::Function { name, params, body } => (name, params, body),
                    _ => panic!("'fiber' expects an function. Got {:?}", args[0]),
                };

                let mut name = name.clone();
                if name.len() == 0 {
                    name = format!("closure#{}", self.closure_id);
                    self.str_push(&name);
                    self.closure_id += 1;
                } else {
                    panic!("Only unnamed functions can be passed to 'fiber'. Got {name}");
                }

                let mut args_names = vec!["this".to_string()];
                for p in params {
                    args_names.push(p.name.clone());
                }

                let mut code = vec![];
                let body_size = body.len();
                for (i, o) in body.iter().enumerate() {
                    code.extend(self.parse_node(o, &args_names, &vec![]));
                    if i + 1 < body_size {
                        code.write_u8(OP_POP).unwrap();
                    }
                }
                code.write_u8(OP_RETURN).unwrap();

                self.classes.get_mut("$self").unwrap().functions.push(Function {
                        name: name.clone(), arity: params.len() as u8,
                        code, args_names: args_names,
                });

                bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("Fiber")).unwrap();

                bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("$self")).unwrap();
                bytes.write_u8(OP_CLOSURE).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index(&name)).unwrap();

                bytes.write_u8(OP_CALL).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("new(_)")).unwrap();
                bytes.write_u8(1).unwrap();
            },
            "yield" => {
                self.str_push("Fiber");
                self.str_push("yield(_)");

                bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("Fiber")).unwrap();

                if args.len() == 1 {
                    bytes.extend(self.load_variable(&args[0], args_names, fields_names));
                } else {
                    bytes.write_u8(OP_NULL).unwrap();
                }

                bytes.write_u8(OP_CALL).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("yield(_)")).unwrap();
                bytes.write_u8(1).unwrap();
            },
            "return" => {
                bytes.write_u8(OP_RETURN).unwrap();
            },
            "load" => {
                bytes.write_u8(OP_IMPORT_MODULE).unwrap();
                let name = match &args[0] {
                    Node::String(s) => s,
                    _ => panic!("'load' expects a string. Got {:?}", args[0]),
                };
                self.str_push(name);
                let index = self.str_index(name);
                bytes.write_u16::<LittleEndian>(index as u16).unwrap();
            },
            _ => {
                let getters = vec!["count", "isdone"];

                let mut args_names = args_names.clone();
                let mut minus = 1;

                let self_class = self.classes.get("$self").unwrap();
                for fun in &self_class.functions {
                    if fun.name.starts_with(name) {
                        let occurence = fun.name.chars().fold(0, |acc, c| acc + if c == '_' { 1 } else { 0 } );
                        if occurence == args.len() {
                            bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                            bytes.write_u16::<LittleEndian>(self.str_index("$self")).unwrap();

                            args_names = fun.args_names.clone();
                            minus = 0;
                            break;
                        }
                    }
                }

                let args_count = args.len() - minus;

                let args_ph = if args_count > 0 { format!("_{}", ",_".repeat(args_count - 1)) } else { "".to_string() };
                let name = if args_count == 0 && getters.contains(&name) { format!("{name}") } else { format!("{name}({args_ph})") };
                let name = if name == "isdone" {
                    "isDone".to_string()
                } else if name == "not()" {
                    "!".to_string()
                } else {
                    name
                };
                self.str_push(&name);

                for a in args {
                    bytes.extend(self.parse_node(&a, &args_names, fields_names));
                }

                bytes.write_u8(OP_CALL).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index(&name)).unwrap();
                bytes.write_u8(args_count as u8).unwrap();
            },
        };

        bytes
    }

    fn load_variable(&mut self, node: &Node, args_names: &Vec<String>, fields_names: &Vec<String>) -> Vec<u8> {
        let mut bytes = vec![];

        match node {
            Node::Identifier(name) => {
                if args_names.contains(name) {
                    bytes.write_u8(OP_LOAD_LOCAL_VAR).unwrap();
                    let index = args_names.iter().position(|r| r == name).unwrap();
                    bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                } else if fields_names.contains(name) {
                    bytes.write_u8(OP_LOAD_FIELD_THIS).unwrap();
                    let index = self.str_index(name);
                    bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                } else {
                    bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                    self.str_push(name);
                    let index = self.str_index(name);
                    bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                }
            },
            _ => {
                bytes.extend(self.parse_constant(node));
            },
        };

        bytes
    }

    fn parse_node(&mut self, node: &Node, args_names: &Vec<String>, fields_names: &Vec<String>) -> Vec<u8> {
        let mut bytes = vec![];

        let other = match node {
            Node::String(_) => self.parse_constant(node),
            Node::Integer(_) => self.parse_constant(node),
            Node::Float(_) => self.parse_constant(node),
            Node::Call { name, args } => self.parse_call(name, args, args_names, fields_names),
            Node::Identifier(name) => {
                if args_names.contains(name) {
                    bytes.write_u8(OP_LOAD_LOCAL_VAR).unwrap();
                    let index = args_names.iter().position(|r| r == name).unwrap();
                    bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                } else if fields_names.contains(name) {
                    bytes.write_u8(OP_LOAD_FIELD_THIS).unwrap();
                    self.str_push(name);
                    let index = self.str_index(name);
                    bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                } else {
                    match name.as_str() {
                        "null" => {
                            bytes.write_u8(OP_NULL).unwrap();
                        },
                        "true" => {
                            bytes.write_u8(OP_TRUE).unwrap();
                        },
                        "false" => {
                            bytes.write_u8(OP_FALSE).unwrap();
                        },
                        _ => {
                            bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                            self.str_push(name);
                            let index = self.str_index(name);
                            bytes.write_u16::<LittleEndian>(index as u16).unwrap();
                        }
                    };
                }

                vec![]
            },
            Node::Function { name, params, body } => {
                let mut name = name.clone();
                if name.len() == 0 {
                    name = format!("closure#{}", self.closure_id);
                    self.str_push(&name);
                    self.closure_id += 1;
                }

                let mut args_names = vec!["this".to_string()];
                for p in params {
                    args_names.push(p.name.clone());
                }

                let mut code = vec![];
                let body_size = body.len();
                for (i, o) in body.iter().enumerate() {
                    code.extend(self.parse_node(o, &args_names, &vec![]));
                    if i + 1 < body_size {
                        code.write_u8(OP_POP).unwrap();
                    }
                }
                code.write_u8(OP_RETURN).unwrap();

                self.classes.get_mut("$self").unwrap().functions.push(Function {
                        name: name.clone(), arity: params.len() as u8,
                        code, args_names: args_names,
                });

                bytes.write_u8(OP_LOAD_MODULE_VAR).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index("$self")).unwrap();
                bytes.write_u8(OP_CLOSURE).unwrap();
                bytes.write_u16::<LittleEndian>(self.str_index(&name)).unwrap();

                vec![]
            },
            _ => panic!("{:?}", node),
        };
        bytes.extend(other);

        bytes
    }

    fn count_opcodes(&self, buf: &[u8]) -> u8 {
        let mut total = 0;
        let mut i = 0;
        while i < buf.len() {
            i += 1 + match buf[i] {
                OP_CONSTANT => match buf[i+1] {
                    VAL_NULL => 1,
                    VAL_BOOL => 2,
                    VAL_INTEGER => 5,
                    VAL_FLOAT => 5,
                    VAL_STRING => 3,
                    _ => panic!("{:?}", buf[i+1]),
                },
                OP_LOAD_MODULE_VAR | OP_STORE_MODULE_VAR | OP_LOAD_LOCAL_VAR | OP_STORE_LOCAL_VAR => 2,
                OP_JUMP | OP_JUMP_IF | OP_LOOP | OP_LOOP_IF => 1,
                OP_CALL => 3,
                OP_ADD | OP_SUB | OP_MUL | OP_DIV | OP_EQUAL | OP_LOWER_THAN | OP_GREATER_THAN
                | OP_NEGATE | OP_RETURN | OP_POP | OP_NOT => 0,
                _ => panic!("count_opcodes: {} at index {i}", buf[i]),
            };
            total += 1;
        }

        total as u8
    }

    fn parse_constant(&mut self, node: &Node) -> Vec<u8> {
        let mut bytes = vec![];

        bytes.write_u8(OP_CONSTANT).unwrap();

        match node {
            Node::String(string) => {
                self.str_push(string);
                bytes.write_u8(VAL_STRING).unwrap();
                let index = self.str_index(string);
                bytes.write_u16::<LittleEndian>(index as u16).unwrap();
            },
            Node::Integer(i) => {
                bytes.write_u8(VAL_INTEGER).unwrap();
                bytes.write_i32::<LittleEndian>(*i).unwrap();
            },
            Node::Float(i) => {
                bytes.write_u8(VAL_FLOAT).unwrap();
                bytes.write_f32::<LittleEndian>(*i).unwrap();
            },
            Node::Null => {
                bytes.write_u8(VAL_NULL).unwrap();
            },
            Node::Bool(b) => {
                bytes.write_u8(VAL_BOOL).unwrap();
                bytes.write_u8(if *b { 1 } else { 0 }).unwrap();
            },
            _ => panic!("parse_constant: {:?}", node),
        };        

        bytes
    }

    fn str_push(&mut self, s: &str) {
        if s.starts_with("set(") {
            panic!("{s}");
        }
        if self.strings.iter().position(|r| r == s).is_none() {
            self.strings.push(s.into());
        }
    }

    fn str_index(&self, s: &str) -> u16 {
        self.strings.iter().position(|r| r == s).unwrap() as u16
    }
}

const OP_RETURN: u8 = 1;
const OP_CONSTANT: u8 = 2;
const OP_NEGATE: u8 = 3;
const OP_ADD: u8 = 4;
const OP_SUB: u8 = 5;
const OP_MUL: u8 = 6;
const OP_DIV: u8 = 7;
const OP_TRUE: u8 = 8;
const OP_FALSE: u8 = 9;
const OP_NULL: u8 = 10;
const OP_NOT: u8 = 11;
const OP_EQUAL: u8 = 12;
const OP_LOWER_THAN: u8 = 13;
const OP_GREATER_THAN: u8 = 14;
const OP_CLOSURE: u8 = 15;
const OP_POP: u8 = 16;
const OP_LOAD_MODULE_VAR: u8 = 17;
const OP_STORE_MODULE_VAR: u8 = 18;
const OP_CALL: u8 = 19;
const OP_LOOP: u8 = 20;
const OP_LOAD_LOCAL_VAR: u8 = 21;
const OP_STORE_LOCAL_VAR: u8 = 22;
const OP_ALLOCATE_VAR: u8 = 23;
const OP_LOAD_FIELD_THIS: u8 = 24;
const OP_STORE_FIELD_THIS: u8 = 25;
const OP_JUMP_IF: u8 = 26;
const OP_JUMP: u8 = 27;
//const OP_DUP: u8 = 28;
const OP_LOOP_IF: u8 = 29;
const OP_IMPORT_MODULE: u8 = 30;
//const OP_DUMP_STACK: u8 = 255;

const VAL_NULL: u8 = 1;
const VAL_BOOL: u8 = 2;
const VAL_INTEGER: u8 = 3;
const VAL_FLOAT: u8 = 4;
const VAL_STRING: u8 = 5;
