#![allow(unused)]

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use pest::Parser;
use crate::parser::parse_block;
use crate::*;
use std::fs;
use std::process::Command;

#[derive(Debug, Clone)]
enum Ast {
    Let { name: String, init: Box<Ast>, },
    Integer(i32),
    Float(f32),
    String(String),
    Identifier(String),
    Call { name: String, args: Vec<Box<Ast>>, },
    Switch {
        condition: Box<Ast>,
        cases: Vec<(Box<Ast>, Box<Ast>)>,
        default: Box<Ast>,
    },
    Class {
        name: String,
        fields: Vec<Variable>,
        functions: Vec<Function>,
    },
    Add {
        lhs: Box<Ast>,
        rhs: Box<Ast>,
    },
    New {
        class: String,
    }
}

#[derive(Debug, Clone)]
struct Param {
    name: String,
    default: Option<Ast>,
}

#[derive(Debug, Clone)]
struct Function {
    name: String,
    params: Vec<Param>,
    body: Vec<Ast>,
}

#[derive(Debug, Clone)]
struct Field {
    offset: u64,
    default: Ast,
}

#[derive(Debug, Clone)]
struct ClassLayout {
    size: u64,
    members: HashMap<String, Field>,
}

#[derive(Debug, Clone)]
struct Variable {
    name: String,
    value: Ast,
}

#[derive(Debug, Clone)]
enum VarType {
    Int,
    Float,
    String,
    Struct(String),
}

#[derive(Debug, Clone)]
struct CodegenVariable {
    name: String,
    kind: VarType,
}

struct Compiler {
    tmp_var_id: u64,
    strings: Vec<String>,
    file: std::fs::File,
    globals: Vec<CodegenVariable>,
    params: Vec<CodegenVariable>,
    classes: HashMap<String, ClassLayout>,
    current_class: Option<String>,
}

pub fn generate(filename: &str) {
    std::fs::create_dir_all("tmp").unwrap();
    let mut gen = Compiler {
        tmp_var_id: 0,
        strings: vec![],
        file: std::fs::File::create("tmp/main.qbe").unwrap(),
        globals: vec![],
        params: vec![],
        classes: HashMap::new(),
        current_class: None,
    };
    gen.generate(filename);
}

impl Compiler {
    fn generate(&mut self, filename: &str) {
        let asts = self.generate_file(filename);
        for ast in &asts {
            match ast {
                Ast::Let { name, init } => {
                    let kind = Self::infer_type(init);
                    let t = Self::get_type_str(&kind);
                    writeln!(self.file, "data ${name} = {{ {t} 0 }}");

                    self.globals.push(CodegenVariable {
                        name: format!("${name}"),
                        kind,
                    });
                }
                Ast::Class { name, fields, functions } => {
                    let mut fields_data = HashMap::new();

                    write!(self.file, "type :{name} = {{");

                    let mut current_offset = 0;
                    for field in fields {
                        let kind = Self::infer_type(&field.value);
                        let t = Self::get_type_str(&kind);
                        write!(self.file, " {t},");

                        fields_data.insert(field.name.clone(), Field {
                            offset: current_offset,
                            default: field.value.clone(),
                        });

                        current_offset += match kind {
                            VarType::Int => 4,
                            VarType::String => 8,
                            _ => panic!("current_offset: {kind:?}", ),
                        };
                    }
                    writeln!(self.file, " }}");

                    self.classes.insert(name.clone(), ClassLayout {
                        size: current_offset,
                        members: fields_data,
                    });
                    self.current_class = Some(name.clone());

                    for function in functions {
                        write!(self.file, "function w ${name}_{}(:{name} %self", function.name);
                        for (index, arg) in function.params.iter().enumerate() {
                            write!(self.file, ", w {}", arg.name);

                            self.params.push(CodegenVariable {
                                name: arg.name.clone(),
                                kind: VarType::Int,
                            })
                        }
                        writeln!(self.file, ") {{
@start");
                        for inst in &function.body {
                            self.emit_ast(inst);
                        }

                        self.params.clear();

                        writeln!(self.file, "    ret 0
}}");
                    }

                    self.current_class = None;
                }
                _ => {},
            }
        }

        writeln!(self.file, "export function w $main() {{
@start");

        for ast in &asts {
            self.emit_ast(ast);
        }

        writeln!(self.file, "    ret 0
}}");

        for (id, s) in self.strings.iter().enumerate() {
            writeln!(self.file, "data $str{id} = {{ b \"{s}\", b 0 }}");
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.tmp_var_id;
        self.tmp_var_id += 1;
        id
    }

    fn emit_ast(&mut self, ast: &Ast) -> Option<CodegenVariable> {
        match ast {
            Ast::Let { name, init } => {
                let id = self.next_id();
                let var_name = format!("%.{id}");

                let init_id = self.emit_ast(init).unwrap();
                let t = Self::get_type_str(&init_id.kind);
                writeln!(self.file, "{var_name} ={t} copy {}", init_id.name);

                writeln!(self.file, "store{t} {var_name}, ${name}");
                Some(CodegenVariable {
                    name: var_name,
                    kind: init_id.kind,
                })
            }
            Ast::Call { name, args } => {
                let mut typed_arged = vec![];

                for arg in args {
                    let var = self.emit_arg(arg);
                    typed_arged.push(var);
                }

                self.emit_call(name, &typed_arged)
            }
            Ast::String(string) => {
                let str_id = self.try_insert_string(string);

                Some(CodegenVariable {
                    name: format!("$str{str_id}"),
                    kind: VarType::String,
                })
            }
            Ast::Integer(i) => {
                Some(CodegenVariable {
                    name: format!("{i}"),
                    kind: VarType::Int,
                })
            }
            Ast::Identifier(ident) => {
                for v in &self.globals {
                    if v.name == format!("${ident}") {
                        return Some (v.clone());
                    }
                }
                for v in &self.params {
                    if v.name == format!("%{ident}") {
                        return Some (v.clone());
                    }
                }
                panic!("Unknown variable {ident}");
            }
            Ast::Class {..} => {
                None
            }
            Ast::New { class } => {
                let id = self.next_id();
                let name = format!("%.{id}");

                let layout = self.classes.get(class).unwrap();
                let size = layout.size;

                writeln!(self.file, "{name} =l alloc4 {size}");
                let members = layout.members.clone();
                for (field_name, field) in members {
                    let kind = Self::infer_type(&field.default);
                    let kind = Self::get_type_str(&kind);

                    writeln!(self.file, "%tmp =l add {name}, {}", field.offset);
                    let tmp = self.emit_ast(&field.default).unwrap();
                    writeln!(self.file, "store{kind} {}, %tmp", tmp.name);
                }

                Some(CodegenVariable {
                    name,
                    kind: VarType::Struct(class.clone()),
                })
            }
            _ => panic!("TODO: {ast:?}"),
        }
    }

    fn emit_arg(&mut self, arg: &Ast) -> CodegenVariable {
        match arg {
            Ast::Switch { condition, cases, default } => {
                let id = self.next_id();
                let name = format!("%.{id}");
                let condition = self.emit_ast(condition).unwrap();
                let cond_id = self.next_id();
                let cond_name = format!("%.{cond_id}");

                let t = Self::get_type_str(&condition.kind);
                writeln!(self.file, "{cond_name} ={t} load{t} {}", condition.name);

                let mut switch_labels = vec![];
                for i in 0..(cases.len() + 1) {
                    let sid = self.next_id();
                    switch_labels.push(format!("@switch.{sid}"));
                }

                for (index, case) in cases.iter().enumerate() {
                    let value = &case.0;
                    let expr = &case.1;
                    let val = self.emit_ast(&value).unwrap();
                    let kind = Self::get_type_str(&val.kind);
                    let expr_var = self.emit_ast(&expr).unwrap();
                    writeln!(self.file, "%cmp ={kind} ceq{kind} {cond_name}, {}", val.name);
                    writeln!(self.file, "jnz %cmp, {}, {}.next", switch_labels[index], switch_labels[index]);
                    writeln!(self.file, "{}", switch_labels[index]);
                    let kind = Self::get_type_str(&expr_var.kind);
                    writeln!(self.file, "{name} ={kind} copy {}", expr_var.name);
                    writeln!(self.file, "jmp {}", switch_labels.last().unwrap());
                    writeln!(self.file, "{}.next", switch_labels[index]);
                }

                let def_output = self.emit_ast(default).unwrap();
                let kind = Self::get_type_str(&def_output.kind);
                writeln!(self.file, "{name} ={kind} copy {}", def_output.name);
                writeln!(self.file, "{}", switch_labels.last().unwrap());

                CodegenVariable {
                    name,
                    kind: def_output.kind,
                }
            }
            Ast::String(string) => {
                let id = self.try_insert_string(string);
                let name = format!("$str{id}");

                CodegenVariable {
                    name,
                    kind: VarType::String,
                }
            }
            Ast::Add { lhs, rhs } => {
                let id = self.next_id();
                let name = format!("%.{id}");

                let l = self.emit_ast(lhs).unwrap().name;
                let r = self.emit_ast(rhs).unwrap().name;
                writeln!(self.file, "{name} =w add {l}, {r}");

                CodegenVariable {
                    name,
                    kind: VarType::Int,
                }
            }
            Ast::Identifier(ident) => {
                for v in &self.globals {
                    if v.name == format!("${ident}") {
                        return v.clone();
                    }
                }

                unimplemented!();
            }
            Ast::Integer(i) => {
                let id = self.next_id();
                let name = format!("%.{id}");
                writeln!(self.file, "{name} =w copy {i}");

                CodegenVariable {
                    name,
                    kind: VarType::Int,
                }
            }
            _ => panic!("{arg:?}", ),
        }
    }

    fn get_type_str(kind: &VarType) -> String {
        match kind {
            VarType::Int => "w".into(),
            VarType::String => "l".into(),
            VarType::Float => "s".into(),
            VarType::Struct(_) => "l".into(),
        }
    }

    fn emit_call(&mut self, name: &str, args: &Vec<CodegenVariable>) -> Option<CodegenVariable> {
        let id = self.next_id();
        let var_name = format!("%.{id}");

        write!(self.file, "{var_name} =l call ");
        if name == "print" || name == "write" {
            let mut fmt = String::new();
            for a in args {
                match a.kind {
                    VarType::Int => fmt.push_str("%d"),
                    VarType::String => fmt.push_str("%s"),
                    _ => panic!("{:?}", a.kind),
                }
            }

            if name == "print" {
                fmt.push_str("\\n");
            }

            let fmt_id = self.try_insert_string(&fmt);

            write!(self.file, "$printf(l $str{fmt_id}, ..., ");
        } else {
            let mut name = name.to_string();

            if args.len() > 0 {
                let kind = &args[0].kind;

                match kind {
                    VarType::Struct(class) => {
                        name = format!("{class}_{name}");
                    }
                    _ => {},
                }
            }

            write!(self.file, "${name}(");
        }

        for (index, var) in args.iter().enumerate() {
            if index > 0 { write!(self.file, ", "); }
            let kind = Self::get_type_str(&var.kind);
            write!(self.file, "{kind} {}", var.name);
        }

        writeln!(self.file, ")");

        Some(CodegenVariable {
            name: var_name,
            kind: VarType::Int
        })
    }

    fn try_insert_string(&mut self, string: &str) -> usize {
        if let Some(pos) = self.strings.iter().position(|r| r == string) {
            pos
        } else {
            let fmt_id = self.strings.len();
            self.strings.push(string.to_string());
            fmt_id
        }
    }

    fn infer_type(t: &Ast) -> VarType {
        match t {
            Ast::Integer(..) => VarType::Int,
            Ast::String(..) => VarType::String,
            Ast::Switch { condition, cases, default } => {
                Self::infer_type(&**default)
            }
            Ast::New { class } => {
                VarType::Struct(class.clone())
            }
            _ => panic!("{t:?}", ),
        }
    }

    fn generate_file(&mut self, filename: &str) -> Vec<Ast> {
        let data = fs::read_to_string(filename).unwrap();
        let res = SimpleLispParser::parse(Rule::file, &data);

        let nodes = match res {
            Ok(pairs) => parse_block(pairs).unwrap(),
            Err(e) => panic!("Can't parse {}:\n{:?}", filename, e),
        };

        let mut asts = vec![];

        for node in nodes {
            asts.push(self.node_to_ast(&node));
        }

        asts
    }

    fn node_to_ast(&self, node: &Node) -> Ast {
        match node {
            Node::Call { name, args } => {
                match name.as_str() {
                    "let" => {
                        assert_eq!(args.len(), 2);
                        let Node::Identifier(name) = &args[0] else {
                            panic!("{args:?}");
                        };
                        let name = name.clone();

                        Ast::Let {
                            name,
                            init: Box::new(self.node_to_ast(&args[1])),
                        }
                    }
                    "switch" => {
                        let mut iter = args.iter();
                        let condition = Box::new(self.node_to_ast(iter.next().unwrap()));
                        let mut cases = vec![];
                        let mut default = None;
                        while let Some(n) = iter.next() {
                            if let Node::Call { name, args } = n {
                                let first = Box::new(self.node_to_ast(&args[0]));
                                let second = Box::new(self.node_to_ast(&args[1]));
                                cases.push((first, second));
                            } else {
                                default = Some(n);
                                break;
                            }
                        }
                        assert!(iter.next().is_none());
                        let default = Box::new(self.node_to_ast(default.unwrap()));
                        Ast::Switch {
                            condition,
                            cases,
                            default,
                        }
                    }
                    "class" => {
                        let mut fields = vec![];
                        let mut functions = vec![];
                        let mut args_iter = args.iter();
                        let Node::Identifier(class_name) = args_iter.next().unwrap() else {
                            todo!();
                        };

                        for arg in args_iter {
                            match arg {
                                Node::Function { name, params, body } => {
                                    // println!("params! {params:?}", );
                                    let body = body.iter().map(|e| self.node_to_ast(e)).collect::<Vec<_>>();
                                    // println!("{name}: {body:?}", );

                                    let mut ast_params = vec![];
                                    for p in params {
                                        ast_params.push(Param {
                                            name: format!("%{}", p.name),
                                            default: p.default_value.clone().map(|node: node::Node| self.node_to_ast(&node)),
                                        });
                                    }

                                    functions.push(Function {
                                        name: name.to_string(),
                                        params: ast_params,
                                        body,
                                    });
                                }
                                Node::Call { name, args } => {
                                    assert_eq!(name, "let");

                                    let var_name = self.node_to_ast(&args[0]);
                                    let var_init = self.node_to_ast(&args[1]);

                                    let Ast::Identifier(var_name) = var_name else {
                                        todo!();
                                    };

                                    fields.push(Variable {
                                        name: var_name,
                                        value: var_init,
                                    });
                                }
                                _ => panic!("{arg:?}", ),
                            }
                        }

                        Ast::Class {
                            name: class_name.clone(),
                            fields,
                            functions,
                        }
                    }
                    "add" => {
                        let lhs = Box::new(self.node_to_ast(&args[0]));
                        let rhs = Box::new(self.node_to_ast(&args[1]));
                        Ast::Add { lhs, rhs }
                    }
                    "new" => {
                        let Node::Identifier(class) = &args[0] else {
                            todo!();
                        };
                        let class = class.clone();
                        Ast::New { class }
                    }
                    _ => {
                        println!("UNKNOWN: {name:?}: {args:?}", );
                        let mut vargs = vec![];
                        for arg in args {
                            vargs.push(Box::new(self.node_to_ast(arg)));
                        }

                        let name = name.clone();

                        Ast::Call {
                            name,
                            args: vargs,
                        }
                    }
                }
            }
            Node::Integer(i) => Ast::Integer(*i),
            Node::Float(f) => Ast::Float(*f),
            Node::String(id) => Ast::String(id.clone()),
            Node::Identifier(id) => Ast::Identifier(id.clone()),
            _ => panic!("{node:?}", ),
        }
    }
}
