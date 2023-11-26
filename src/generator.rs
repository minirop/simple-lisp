#![allow(unused)]

use std::io::Write;
use pest::Parser;
use crate::parser::parse_block;
use crate::*;
use std::fs;
use std::process::Command;

struct Generator {
    depth: isize,
    functions_names: Vec<String>,
    paths: Vec<String>,
    headers: Vec<String>,
    functions: Vec<String>,
    main: Vec<String>,
    inside_expression: isize,
}

pub fn generate(filename: &str) {
    let mut gen = Generator {
        depth: 0,
        functions_names: vec![],
        paths: vec![],
        headers: vec![],
        functions: vec![],
        main: vec![String::new()],
        inside_expression: 0,
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

        self.generate_file(filename);

        let mut output = format!("#include \"simplelisp.h\"\n\n");
        for header in &self.headers {
            output.push_str(&header);
            output.push_str("\n");
        }
        output.push_str("\n");

        for function in &self.functions {
            output.push_str(&function);
            output.push_str("\n");
        }

        output.push_str("int main() {\n");
        output.push_str(&self.main.last().unwrap());
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
                    let mut header = String::new();
                    let converted_name = Self::convert_name(&name);

                    header.push_str(&format!("Value func_{}(std::vector<Value>);", converted_name));
                    self.headers.push(header);
                    self.functions.push(res);

                    self.functions_names.push(converted_name);
                },
                Node::Call { name, args } => {
                    if name == "let" {
                        let varname = match args[0].clone() {
                            Node::Identifier(s) => s,
                            _ => panic!("{}", args[0]),
                        };
                        self.headers.push(format!("Value {};\n", Self::convert_name(&varname)));
                        self.depth += 1;
                        self.inside_expression = 1;
                        let n = self.generate_node(args[1].clone());
                        self.inside_expression = 0;
                        self.depth -= 1;
                        self.main.last_mut().unwrap().push_str(&format!("{} = {};\n", Self::convert_name(&varname), n));
                    } else {
                        self.main.last_mut().unwrap().push_str(&format!("{};\n", res));
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
                let mut output = String::new();

                let converted_name = Self::convert_name(&name);

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
                    output.push_str(" {\n");
                }
                self.functions_names.push(converted_name);

                self.inside_expression = 0;

                for (i, p) in params.iter().enumerate() {
                    output.push_str(&format!("Value {} = ", p.name));

                    if let Some(default_value) = &p.default_value {
                        output.push_str(&format!("{};\n", self.generate_node(default_value.clone())));
                        output.push_str(&format!("if ({} < args1.size()) {{\n", i));
                        output.push_str(&format!("{} = args1[{}];\n", p.name, i));
                        output.push_str("}\n");
                    } else {
                        output.push_str(&format!("args1[{}];", i));
                    }

                    output.push_str("\n");
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
                            if name == "test-two" {
                                panic!("arg = {:?}", arg);
                            }
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
                        ret.push_str(&format!("Value {} = {}", Self::convert_name(&varname), self.generate_node(args[1].clone())));
                        self.inside_expression = 0;
                        self.depth -= 1;
                    },
                    "set" => {
                        let varname = match args[0].clone() {
                            Node::Identifier(s) => s,
                            _ => panic!("{}", args[0]),
                        };
                        ret.push_str(&format!("{} = {}", Self::convert_name(&varname), self.generate_node(args[1].clone())));
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

                        ret.push_str("[=]() mutable -> Value {\n");
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
                        ret.push_str("[=]() mutable -> Value {\n");
                        ret.push_str("std::vector<Value> ret1;\n");
                        for arg in args {
                            ret.push_str(&format!("ret1.emplace_back({});\n", self.generate_node(arg.clone())));
                        }
                        ret.push_str("return ret1;\n");
                        ret.push_str("}()");
                    },
                    "block" => {
                        ret.push_str("[=]() mutable -> Value {\n");
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
                        ret.push_str("[=]() mutable -> Value {\n");
                        let mut prev = String::new();
                        let mut cout = String::from("std::cout");
                        for (i, arg) in args.iter().enumerate() {
                            let output = match &arg {
                                Node::Identifier(id) => Self::convert_name(&id),
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
                        ret.push_str(&format!("{}(std::vector<Value>{{", self.generate_node(args[0].clone())));
                        for (i, arg) in args.iter().skip(1).enumerate() {
                            if i > 0 { ret.push_str(", "); }

                            ret.push_str(&format!("{}", self.generate_node(arg.clone())));
                        }
                        ret.push_str("})");
                    },
                    "switch" => {
                        ret.push_str("[=]() mutable -> Value {\n");
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

                        ret.push_str("[=]() mutable -> Value {\n");
                        ret.push_str(&format!("{}", load_output));
                        ret.push_str("}()");
                    },
                    _ => {
                        let converted_name = Self::convert_name(&name);
                        if self.functions_names.contains(&converted_name) {
                            ret.push_str(&format!("func_{}", converted_name));
                        } else {
                            ret.push_str(&format!("{}.as_func()", converted_name));
                        }

                        ret.push_str("(std::vector<Value>{");
                        for (i, p) in args.iter().enumerate() {
                            if i > 0 { ret.push_str(", "); }

                            ret.push_str(&format!("{}", self.generate_node(p.clone())));
                        }
                        ret.push_str("})");
                    },
                };
            },
            Node::Integer(i) => ret.push_str(&format!("{i}")),
            Node::Float(f) => ret.push_str(&format!("{f}")),
            Node::String(s) => ret.push_str(&format!("\"{s}\"s")),
            Node::Identifier(id) => ret.push_str(&format!("{id}")),
            _ => panic!("{:?}", node),
        };

        ret
    }

    fn convert_name(name: &str) -> String {
        let name = str::replace(name, "-", "_");
        let name = str::replace(&name, "+", "plus_");
        let name = str::replace(&name, "*", "times_");
        let name = str::replace(&name, "/", "divide_");
        name
    }
}
