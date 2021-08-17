// DIMACS Model Checker in Rust
#![allow(unused_imports)]
use {
    splr::{Config, SatSolverIF, Solver, ValidateIF},
    std::{
        env,
        fs::File,
        io::{stdin, BufRead, BufReader, Result},
        path::{Path, PathBuf},
    },
};

const ABOUT: &str = "DIMACS-format Model Checker in Rust";
const RED: &str = "\x1B[001m\x1B[031m";
const GREEN: &str = "\x1B[001m\x1B[032m";
const BLUE: &str = "\x1B[001m\x1B[034m";
const RESET: &str = "\x1B[000m";

struct TargetOpts {
    /// an assign file generated by slpr
    assign: Option<std::path::PathBuf>,
    /// a CNF file
    problem: std::path::PathBuf,
    /// disable colorized output
    no_color: bool,
}

impl Default for TargetOpts {
    fn default() -> Self {
        TargetOpts {
            assign: None,
            problem: PathBuf::new(),
            no_color: false,
        }
    }
}

impl TargetOpts {
    pub fn inject_from_args(&mut self) {
        let mut help = false;
        let mut version = false;
        if let Some(ref cnf) = std::env::args().last() {
            let path = PathBuf::from(cnf.clone());
            if path.exists() {
                self.problem = path;
            }
        }
        let mut iter = std::env::args().skip(1);
        while let Some(arg) = iter.next() {
            if let Some(name) = arg.strip_prefix("--") {
                let flags = ["no-color", "help", "version"];
                let options_path = ["assign"];
                if flags.contains(&name) {
                    match name {
                        "no-color" => self.no_color = true,
                        "help" => help = true,
                        "version" => version = true,
                        _ => panic!("invalid flag: {}", name),
                    }
                } else if options_path.contains(&name) {
                    if options_path.contains(&name) {
                        if let Some(val) = iter.next() {
                            match name {
                                "assign" => self.assign = Some(PathBuf::from(val)),
                                _ => panic!("not"),
                            }
                        } else {
                            panic!("no argument for {}", name);
                        }
                    } else {
                        panic!("invalid argument: {}", name);
                    }
                }
            } else if let Some(name) = arg.strip_prefix('-') {
                let flags = ["C", "h", "V"];
                let options = ["a"];
                if flags.contains(&name) {
                    match name {
                        "C" => self.no_color = true,
                        "h" => help = true,
                        "V" => version = true,
                        _ => panic!(),
                    }
                } else if options.contains(&name) {
                    if let Some(val) = iter.next() {
                        match name {
                            "a" => self.assign = Some(PathBuf::from(val)),
                            _ => panic!("invalid option: {}", name),
                        }
                    } else {
                        panic!("no argument for {}", name);
                    }
                }
            } else if !self.problem.exists() || self.problem.to_string_lossy() != arg {
                panic!("invalid argument: {}", arg);
            }
        }
        if help {
            println!("{}\n{}", ABOUT, HELP_MESSAGE);
            std::process::exit(0);
        }
        if version {
            println!("{}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
    }
}

const HELP_MESSAGE: &str = "
USAGE:
    dmcr [FLAGS] [OPTIONS] <problem>
FLAGS:
    -h, --help        Prints help information
    -C, --no-color    disable colorized output
    -V, --version     Prints version information
OPTIONS:
    -a, --assign <assign>    an assign file generated by slpr
ARGS:
    <problem>    a CNF file
";

#[allow(clippy::field_reassign_with_default)]
fn main() {
    let mut from_file = true;
    let mut found = false;
    let mut args = TargetOpts::default();
    args.inject_from_args();
    let cnf = args
        .problem
        .to_str()
        .unwrap_or_else(|| panic!("{} does not exist.", args.problem.to_str().unwrap()));
    let mut config = Config::default();
    config.cnf_file = args.problem.clone();
    config.quiet_mode = true;
    let (red, green, blue) = if args.no_color {
        (RESET, RESET, RESET)
    } else {
        (RED, GREEN, BLUE)
    };
    let mut s = Solver::build(&config).expect("failed to load");
    if args.assign == None {
        args.assign = Some(PathBuf::from(format!(
            "ans_{}",
            Path::new(&args.problem)
                .file_name()
                .unwrap()
                .to_string_lossy()
        )));
    }
    if let Some(f) = &args.assign {
        if let Ok(d) = File::open(f.as_path()) {
            if let Some(vec) = read_assignment(&mut BufReader::new(d), cnf, &args.assign) {
                if s.inject_assignment(&vec).is_err() {
                    println!(
                        "{}{} seems an unsat problem but no proof.{}",
                        blue,
                        args.problem.to_str().unwrap(),
                        RESET
                    );
                    return;
                }
            } else {
                return;
            }
            found = true;
        }
    }
    if !found {
        if let Some(vec) = read_assignment(&mut BufReader::new(stdin()), cnf, &args.assign) {
            if s.inject_assignment(&vec).is_err() {
                println!(
                    "{}{} seems an unsat problem but no proof.{}",
                    blue,
                    args.problem.to_str().unwrap(),
                    RESET,
                );
                return;
            }
            found = true;
            from_file = false;
        } else {
            return;
        }
    }
    if !found {
        println!("There's no assign file.");
        return;
    }
    match s.validate() {
        Some(v) => println!(
            "{}An invalid assignment set for {}{} due to {:?}.",
            red,
            args.problem.to_str().unwrap(),
            RESET,
            v,
        ),
        None if from_file => println!(
            "{}A valid assignment set for {}{} is found in {}",
            green,
            &args.problem.to_str().unwrap(),
            RESET,
            &args.assign.unwrap().to_str().unwrap(),
        ),
        None => println!(
            "{}A valid assignment set for {}.{}",
            green,
            &args.problem.to_str().unwrap(),
            RESET,
        ),
    }
}

fn read_assignment(rs: &mut dyn BufRead, cnf: &str, assign: &Option<PathBuf>) -> Option<Vec<i32>> {
    let mut buf = String::new();
    loop {
        match rs.read_line(&mut buf) {
            Ok(0) => return Some(Vec::new()),
            Ok(_) => {
                if buf.starts_with('c') {
                    buf.clear();
                    continue;
                }
                if buf.starts_with("s ") {
                    if buf.starts_with("s SATISFIABLE") {
                        buf.clear();
                        continue;
                    } else if buf.starts_with("s UNSATISFIABLE") {
                        println!("{} seems an unsatisfiable problem. I can't handle it.", cnf);
                        return None;
                    } else if let Some(asg) = assign {
                        println!("{} seems an illegal format file.", asg.to_str().unwrap(),);
                        return None;
                    } else {
                        buf.clear();
                    }
                }
                if let Some(stripped) = buf.strip_prefix("v ") {
                    let mut v: Vec<i32> = Vec::new();
                    for s in stripped.split_whitespace() {
                        match s.parse::<i32>() {
                            Ok(0) => break,
                            Ok(x) => v.push(x),
                            Err(e) => panic!("{} by {}", e, s),
                        }
                    }
                    return Some(v);
                }
                panic!("Failed to parse here: {}", buf);
            }
            Err(e) => panic!("{}", e),
        }
    }
}
