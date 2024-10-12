#![allow(unused)]

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use pest::Parser;
use crate::parser::parse_block;
use crate::*;
use std::fs;
use std::process::Command;

struct Generator {
    depth: isize,
    functions_names: HashSet<String>,
    class_functions_names: HashSet<String>,
    paths: Vec<String>,
    headers: Vec<String>,
    functions: Vec<String>,
    main: Vec<String>,
    inside_expression: isize,
    current_class: Option<String>,
    current_method: Option<String>,
    current_method_args: Vec<String>,
    classes: HashMap::<String, String>,
    converted_names: HashMap::<String, String>,
}

pub fn generate(filename: &str) {
    let mut gen = Generator {
        depth: 0,
        functions_names: HashSet::new(),
        class_functions_names: HashSet::new(),
        paths: vec![],
        headers: vec![],
        functions: vec![],
        main: vec![String::new()],
        inside_expression: 0,
        current_class: None,
        current_method: None,
        current_method_args: vec![],
        classes: HashMap::new(),
        converted_names: HashMap::new(),
    };
    let source = gen.generate(filename);

    std::fs::create_dir_all("tmp").unwrap();
    fs::write("tmp/main.cpp", source).unwrap();
    let output = Command::new("g++")
                    .arg("-std=c++23")
                    .arg("-O2")
                    .arg("-Iinclude")
                    .arg("tmp/main.cpp")
                    .output()
                    .expect("failed to compile the code");
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();
}

impl Generator {
    fn generate(&mut self, filename: &str) -> String {
        let path = Path::new(filename);
        self.paths.push(path.parent().unwrap().to_str().unwrap().to_string());

        self.class_functions_names.insert("read".into());
        for i in 1..16 {
            self.functions_names.insert("print".into());
            self.functions_names.insert("write".into());
        }

        self.generate_file(filename);

        let mut output = format!("#include \"simplelisp.h\"\n\n");
        output.push_str("class SimpleListObject : public std::enable_shared_from_this<SimpleListObject> {\n");
        output.push_str("public:\n");
        output.push_str("    virtual ~SimpleListObject() = default;\n");
        output.push_str("    virtual const char* name() const = 0;\n");
        for f in &self.class_functions_names {
            let old_name = if let Some(n) = self.converted_names.get(f) {
                n
            } else {
                f
            };
            output.push_str(&format!("    virtual Value func_{f}(std::vector<Value> args1) {{ throw std::string(\"class '\") + name() + \"' does not implement '{old_name}'\"; }}\n"));
        }
        output.push_str("};\n\n");
        output.push_str("#include \"simplelisp-api.h\"\n\n");

        let mut all_functions_names: HashSet<String> = self.functions_names.iter().cloned().collect();
        for f in &self.class_functions_names {
            all_functions_names.insert(f.clone());
        }

        for function in all_functions_names {
            output.push_str(&format!("Value func_{function}(std::vector<Value> args1);\n"));
        }
        output.push_str("\n");

        for header in &self.headers {
            output.push_str(&header);
        }
        output.push_str("\n");

        for function in &self.functions {
            output.push_str(&function);
            output.push_str("\n");
        }

        for cf in &self.class_functions_names {
            if !self.functions_names.contains(cf) {
                output.push_str(&format!("Value func_{}(std::vector<Value> args1) {{\n", cf));
                output.push_str("if (args1[0].is_instance()) {\n");
                output.push_str("auto obj = args1[0].as_instance();\n");
                output.push_str("auto new_args = std::vector<Value> { args1.begin() + 1, args1.end() };\n");
                output.push_str(&format!("return obj->func_{cf}(new_args);\n"));
                output.push_str("}\n\n");
                output.push_str(&format!("std::cerr << \"function '{cf}' exists only as a class function.\";\n"));
                output.push_str("std::exit(1);\n");
                output.push_str("}\n");
            }
        }

        output.push_str("int main() {\n");
        output.push_str(&self.main.last().unwrap());
        output.push_str("return 0;\n");
        output.push_str("}\n");

        output
    }

    fn generate_file(&mut self, filename: &str) {
        let data = fs::read_to_string(filename).unwrap();
        let res = SimpleListParser::parse(Rule::file, &data);

        let nodes = match res {
            Ok(pairs) => parse_block(pairs).unwrap(),
            Err(e) => panic!("Can't parse {}:\n{:?}", filename, e),
        };

        for node in nodes {
            let res = self.generate_node(node.clone());
            match node {
                Node::Function { name, params, .. } => {
                    let converted_name = self.convert_name(&name);

                    self.functions.push(res);
                    self.functions_names.insert(converted_name);
                },
                Node::Call { name, args } => {
                    if name == "let" {
                        let varname = match args[0].clone() {
                            Node::Identifier(s) => s,
                            _ => panic!("{}", args[0]),
                        };
                        let new_name = self.convert_name(&varname);
                        self.headers.push(format!("Value {};\n", new_name));
                        self.depth += 1;
                        self.inside_expression = 1;
                        let n = self.generate_node(args[1].clone());
                        self.inside_expression = 0;
                        self.depth -= 1;
                        self.main.last_mut().unwrap().push_str(&format!("{} = {};\n", new_name, n));
                    } else {
                        if res.len() > 0 {
                            self.main.last_mut().unwrap().push_str(&format!("{};\n", res));
                        }
                    }
                },
                _ => {
                    self.main.last_mut().unwrap().push_str(&format!("{};\n", res));
                }
            }
        }
    }

    fn generate_node(&mut self, node: Node) -> String {
        let mut ret = String::new();

        match node {
            Node::Function { name, params, body } => {
                if self.current_class.is_some() {
                    self.current_method = Some(name.clone());

                    for p in &params {
                        self.current_method_args.push(p.name.clone());
                    }
                }

                let mut output = String::new();

                let converted_name = self.convert_name(&name);

                if self.depth > 0 {
                    if name.len() > 0 && self.inside_expression == 0 {
                        output.push_str(&format!("auto func_{} = ", converted_name));
                    }
                    output.push_str("(Value::Function([=]");
                    if name.len() > 0 && self.inside_expression > 0 {
                        panic!("Not supported until gcc/clang support C++23's deducing this.");
                        output.push_str(&format!("<typename Self>(this Self & {}, ", converted_name));
                    } else {
                        output.push_str("(");
                    }
                    output.push_str("std::vector<Value> args1) mutable -> Value {\n");
                } else {
                    if name.len() > 0 {
                        output.push_str(&format!("Value func_{}", converted_name));
                    } else {
                        output.push_str("(Value::Function([=]");
                    }
                    output.push_str("(std::vector<Value> args1)");
                    if name.len() == 0 {
                        output.push_str(" mutable -> Value");
                    }
                    if self.current_class.is_some() {
                        output.push_str(" override");
                    }
                    output.push_str(" {\n");
                }

                self.inside_expression = 0;

                self.class_functions_names.insert(converted_name.clone(),);
                if self.current_class.is_none() {
                    self.functions_names.insert(converted_name.clone());
                    output.push_str("if (args1.size() > 0 && args1[0].is_instance()) {\n");
                    output.push_str("auto obj = args1[0].as_instance();\n");
                    output.push_str("auto new_args = std::vector<Value> { args1.begin() + 1, args1.end() };\n");
                    output.push_str(&format!("return obj->func_{converted_name}(new_args);\n"));
                    output.push_str("}\n\n");
                }

                for (id, p) in params.iter().enumerate() {
                    let pname = &p.name;
                    if let Some(default_value) = &p.default_value {
                        output.push_str(&format!("Value {pname};\n"));
                        output.push_str(&format!("if ({id} < args1.size()) {{\n"));
                        output.push_str(&format!("{pname} = args1[{id}];\n"));
                        output.push_str("} else {\n");
                        output.push_str(&format!("{pname} = {default_value};\n"));
                        output.push_str("}\n");
                    } else {
                        output.push_str(&format!("Value {pname} = args1[{id}];\n"));
                    }
                }
                output.push_str("Value ret1;\n");
                if body.len() > 0 {
                    self.depth += 1;
                    let last = body.len() - 1;
                    for (i, arg) in body.iter().enumerate() {
                        if i == last {
                            if let Node::Call{ name, args } = arg {
                                if name != "let" {
                                    output.push_str(&format!("ret1 = {};\n", self.generate_node(arg.clone())));
                                } else {
                                    output.push_str(&format!("{};\nret1 = {};\n", self.generate_node(arg.clone()), args[1]));
                                }
                            } else {
                                    output.push_str(&format!("ret1 = {};\n", self.generate_node(arg.clone())));
                            }
                        } else {
                            output.push_str(&format!("{};\n", self.generate_node(arg.clone())));
                        }
                    }
                    self.depth -= 1;
                }
                output.push_str("return ret1;\n");
                if name.len() > 0 && self.depth == 0 {
                    output.push_str("}\n");
                } else {
                    output.push_str("}))\n");
                }

                ret.push_str(&output);

                self.current_method = None;
                self.current_method_args.clear();
            },
            Node::Call { name, args } => {
                match name.as_str() {
                    "let" => {
                        let varname = match args[0].clone() {
                            Node::Identifier(s) => s,
                            _ => panic!("{}", args[0]),
                        };
                        self.depth += 1;
                        self.inside_expression = 1;
                        ret.push_str(&format!("Value {} = {}", self.convert_name(&varname), self.generate_node(args[1].clone())));
                        self.inside_expression = 0;
                        self.depth -= 1;

                        if self.current_class.is_some() {
                            ret.push_str(";\n");
                        }
                    },
                    "set" => {
                        let varname = match args[0].clone() {
                            Node::Identifier(s) => s,
                            _ => panic!("{}", args[0]),
                        };
                        ret.push_str(&format!("{} = {}", self.convert_name(&varname), self.generate_node(args[1].clone())));
                    },
                    "while" => {
                        ret.push_str("[=]() mutable {\nValue ret1;\nwhile (");
                        ret.push_str(&format!("{}", self.generate_node(args[0].clone())));
                        ret.push_str(") {\n");
                        let last = args.len() - 2;
                        for (i, arg) in args.iter().skip(1).enumerate() {
                            if i == last {
                                if let Node::Call{ name, args } = arg {
                                    if name != "let" {
                                        ret.push_str(&format!("ret1 = {};\n", self.generate_node(arg.clone())));
                                    } else {
                                        ret.push_str(&format!("{};\nret1 = {};\n", self.generate_node(arg.clone()), args[1]));
                                    }
                                }
                            } else {
                                ret.push_str(&format!("{};\n", self.generate_node(arg.clone())));
                            }
                        }
                        ret.push_str("}\nreturn ret1;\n}()");
                    },
                    "<" | ">" | "+" | "<=" | ">=" | "*" | "-" | "/" | "=" => {
                        let first = self.generate_node(args[0].clone());
                        ret.push_str(&format!("({}", first));

                        let name = if name == "=" { "==".to_owned() } else { name };

                        for arg in args.iter().skip(1) {
                            ret.push_str(&format!(" {} {}", name, self.generate_node(arg.clone())));
                        }
                        ret.push_str(")");
                    },
                    "if" => {
                        let cond = self.generate_node(args[0].clone());
                        let truthy = self.generate_node(args[1].clone());
                        let falsy = self.generate_node(args[2].clone());

                        ret.push_str(&format!("[={}]() mutable -> Value {{\n", if self.current_class.is_some() { ", this"} else { "" }));
                        ret.push_str(&format!("if ({}) {{\n", cond));
                        if let Node::Call{ name, .. } = &args[1] {
                            if name == "return" {
                                ret.push_str(&format!("{truthy}"));
                            } else {
                                ret.push_str(&format!("return {truthy};\n"));
                            }
                        } else {
                            ret.push_str(&format!("return {truthy};\n"));
                        }
                        ret.push_str("} else {\n");
                        if let Node::Call{ name, .. } = &args[2] {
                            if name == "return" {
                                ret.push_str(&format!("{falsy}"));
                            } else {
                                ret.push_str(&format!("return {falsy};\n"));
                            }
                        } else {
                            ret.push_str(&format!("return {falsy};\n"));
                        }
                        ret.push_str("}\n");
                        ret.push_str("}()");
                    },
                    "return" => {
                        let val = self.generate_node(args[0].clone());
                        ret.push_str(&format!("return {}", val));
                    },
                    "list" => {
                        ret.push_str(&format!("[={}]() mutable -> Value {{\n", if self.current_class.is_some() { ", this"} else { "" }));
                        ret.push_str("std::vector<Value> ret1;\n");
                        for arg in args {
                            ret.push_str(&format!("ret1.emplace_back({});\n", self.generate_node(arg.clone())));
                        }
                        ret.push_str("return ret1;\n");
                        ret.push_str("}()");
                    },
                    "block" => {
                        ret.push_str(&format!("[={}]() mutable -> Value {{\n", if self.current_class.is_some() { ", this"} else { "" }));
                        ret.push_str("Value ret1;\n");
                        let last = args.len() - 1;
                        for (i, arg) in args.iter().enumerate() {
                            if i == last {
                                if let Node::Call{ name, args } = arg {
                                    if name != "let" {
                                        ret.push_str(&format!("ret1 = {};\n", self.generate_node(arg.clone())));
                                    } else {
                                        ret.push_str(&format!("{};\nret1 = {};\n", self.generate_node(arg.clone()), args[1]));
                                    }
                                }
                            } else {
                                ret.push_str(&format!("{};\n", self.generate_node(arg.clone())));
                            }
                        }
                        ret.push_str("return ret1;\n");
                        ret.push_str("}()");
                    },
                    "dump" => {
                        ret.push_str(&format!("[={}]() mutable -> Value {{\n", if self.current_class.is_some() { ", this"} else { "" }));
                        let mut prev = String::new();
                        let mut cout = String::from("std::cout");
                        for (i, arg) in args.iter().enumerate() {
                            let output = match &arg {
                                Node::Identifier(id) => self.convert_name(&id),
                                _ => self.generate_node(arg.clone()),
                            };

                            prev.push_str(&format!("Value arg_tmp_{} = {};\n", i, output));
                            cout.push_str(&format!(" << Value(arg_tmp_{0}).get_type() << \": \" << arg_tmp_{0}", i));
                        }
                        ret.push_str(&prev);
                        ret.push_str(&cout);
                        ret.push_str(" << \"\\n\";\n");
                        ret.push_str("return Value();\n");
                        ret.push_str("}()");
                    },
                    "call" => {
                        ret.push_str(&format!("{}(", self.generate_node(args[0].clone())));
                        for (i, arg) in args.iter().skip(1).enumerate() {
                            if i > 0 { ret.push_str(", "); }

                            ret.push_str(&format!("{}", self.generate_node(arg.clone())));
                        }
                        ret.push_str(")");
                    },
                    "switch" => {
                        ret.push_str(&format!("[={}]() mutable -> Value {{\n", if self.current_class.is_some() { ", this"} else { "" }));
                        let val = self.generate_node(args[0].clone());
                        ret.push_str(&format!("Value test1 = {};\n", val));

                        let count = args.len() - 1;
                        for i in (1..count).step_by(2) {
                            ret.push_str(&format!("if (test1 == {}) {{\n", args[i]));
                            ret.push_str(&format!("return {};\n", args[i+1]));
                            ret.push_str("}\n");
                        }
                        ret.push_str(&format!("return {};\n", args[count]));
                        ret.push_str("}()");
                    },
                    "load" => {
                        let filename = match &args[0] {
                            Node::String(s) => s.clone(),
                            _ => panic!("load expects a string."),
                        };

                        let filename = if filename.ends_with(".sl") {
                            filename
                        } else {
                            format!("{}.sl", filename)
                        };

                        let path = Path::new(self.paths.last().unwrap());
                        let filepath = path.join(&filename);
                        self.main.push(String::new());
                        self.generate_file(filepath.to_str().unwrap());
                        let load_output = self.main.last().unwrap().clone();
                        self.main.pop();

                        ret.push_str(&format!("[={}]() mutable -> Value {{\n", if self.current_class.is_some() { ", this"} else { "" }));
                        ret.push_str(&format!("{}", load_output));
                        ret.push_str("}()");
                    },
                    "class" => {
                        let mut header = String::new();

                        let name = match &args[0] {
                            Node::Identifier(id) => id,
                            _ => panic!("'class' only accept identifiers. Got {:?}", args[0]),
                        };

                        let (parent_name, skip) = match &args[1] {
                            Node::Identifier(id) => (id.clone(), 2),
                            _ => ("SimpleListObject".to_string(), 1),
                        };

                        self.classes.insert(name.clone(), parent_name.clone());

                        header.push_str(&format!("class {name} : public {parent_name} {{\n"));
                        header.push_str("public:\n");
                        header.push_str(&format!("   virtual const char* name() const override {{ return \"{name}\"; }};\n"));

                        self.current_class = Some(name.clone());
                        for elem in args.iter().skip(skip) {
                            header.push_str(&self.generate_node(elem.clone()));
                            header.push_str("\n");
                        }
                        self.current_class = None;

                        header.push_str("};\n\n");
                        self.headers.push(header);
                    },
                    "new" => {
                        let classname = match &args[0] {
                            Node::Identifier(id) => id,
                            _ => panic!("'new' only accept identifiers. Got {:?}.", args[0]),
                        };

                        ret.push_str(&format!("Value(new {}(", classname));
                        let mut is_first_arg = true;
                        for elem in args.iter().skip(1) {
                            if !is_first_arg { ret.push_str(", "); }
                            is_first_arg = false;
                            
                            ret.push_str(&self.generate_node(elem.clone()));
                        }
                        ret.push_str("))");
                    },
                    "lt" => {
                        ret.push_str(&format!("{} < {}", self.generate_node(args[0].clone()), self.generate_node(args[1].clone())));
                    },
                    "eq" => {
                        ret.push_str(&format!("{} == {}", self.generate_node(args[0].clone()), self.generate_node(args[1].clone())));
                    },
                    "add" => {
                        ret.push_str(&format!("{} + {}", self.generate_node(args[0].clone()), self.generate_node(args[1].clone())));
                    },
                    "sub" => {
                        ret.push_str(&format!("{} - {}", self.generate_node(args[0].clone()), self.generate_node(args[1].clone())));
                    },
                    _ => {
                        if name == "super" {
                            let cur_meth = self.current_method.clone().unwrap();
                            let cur_cls = self.current_class.clone().unwrap();
                            let converted_name = self.convert_name(&cur_meth);
                            let parent_class_name = self.classes.get(&cur_cls).unwrap();
                            ret.push_str(&format!("{parent_class_name}::func_{converted_name}"));
                        } else {
                            let converted_name = self.convert_name(&name);
                            if self.functions_names.contains(&converted_name) || self.class_functions_names.contains(&converted_name) {
                                if self.current_class.is_some() {
                                    ret.push_str("::");
                                }
                                ret.push_str(&format!("func_{converted_name}"));
                            } else {
                                ret.push_str(&format!("{converted_name}.as_func()"));
                            }
                        }

                        ret.push_str("({");

                        let mut is_first_arg = true;
                        for p in &args {
                            if !is_first_arg { ret.push_str(", "); }
                            is_first_arg = false;

                            ret.push_str(&format!("Value({})", self.generate_node(p.clone())));
                        }

                        if args.len() < self.current_method_args.len() {
                            for p in self.current_method_args.iter().skip(args.len()) {
                                if !is_first_arg { ret.push_str(", "); }
                                is_first_arg = false;

                                ret.push_str(&p);
                            }
                        }

                        ret.push_str("})");
                    },
                };
            },
            Node::Integer(i) => ret.push_str(&format!("{i}")),
            Node::Float(f) => ret.push_str(&format!("{f}")),
            Node::String(s) => ret.push_str(&format!("\"{s}\"s")),
            Node::Identifier(id) => {
                if id == "this" {
                    ret.push_str("shared_from_this()");
                } else {
                    ret.push_str(&format!("{id}"));
                }
            }
            _ => panic!("{:?}", node),
        };

        ret
    }

    fn convert_name(&mut self, name: &str) -> String {
        let prev = name;
        let name = str::replace(name, "-", "_");
        let name = str::replace(&name, "+", "plus_");
        let name = str::replace(&name, "*", "times_");
        let name = str::replace(&name, "/", "divide_");
        self.converted_names.insert(name.to_string(), prev.to_string());
        name
    }
}
