extern crate execute;
extern crate fs_extra;

use lazy_static::*;

use fs_extra::dir::move_dir;

use execute::{command as c, Execute};
use std::process::{Command, exit, Stdio};
use std::sync::Mutex;


use clap::{ColorChoice, Parser};
use fs_extra::file::move_file;
use std::env::{current_dir, set_current_dir};
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::ops::Add;


lazy_static! {
    static ref VERBOSE: Mutex<bool> = Mutex::from(false);
    static ref ERRORS: Mutex<bool> = Mutex::from(false);
    static ref NAME:  Mutex<String> = Mutex::new(String::new());
}

macro_rules! v {
    ($msg: expr) => {
        let _ = io::stdout().flush();
        if *VERBOSE.lock().unwrap() {  print!("{}", $msg) }
        let _ = io::stdout().flush();
    };
}

macro_rules! ev {
    ($msg: expr) => {
        let _ = io::stdout().flush();
        if *ERRORS.lock().unwrap() {  eprint!("{}", $msg) }
        let _ = io::stdout().flush();
    };
}

#[derive(Parser, Debug)]
#[clap(about, version, author, color(ColorChoice::Never))]
struct Args {
    /// Url of the github repository
    #[clap(required = true)]
    url: String,

    /// Folders to clone from main/
    #[clap(required = true, multiple_values = true)]
    folders: Vec<String>,

    /// Name of the branch to clone
    #[clap(short, long, required = false, default_value = "main")]
    branch: String,

    /// Store in place fetched files
    #[clap(short, long, takes_value = false)]
    in_place: bool,

    /// Show current step
    #[clap(short, long, takes_value = false)]
    verbose: bool,

    /// Show stdout and stderr from failed commands
    #[clap(short, long, takes_value = false)]
    errors: bool,
}

pub fn write_contents_to(path: &str, contents: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().read(true).write(true).create(true).append(true).open(path)?;
    file.write(contents)?;
    Ok(())
}

trait Handle {
    fn handle(&mut self, on_error: &str) -> bool;
    fn handle_or_exit(&mut self, on_error: &str);
}

impl Handle for Command {
    fn handle(&mut self, on_error: &str) -> bool {
        self.stdout(Stdio::piped());
        self.stderr(Stdio::piped());

        let output = self.execute_output().unwrap();

        if let Some(exit_code) = output.status.code() {
            if !(exit_code == 0) {
                let _ = io::stdout().flush();
                eprint!("{}", on_error);
                ev!(format!("{}\n", String::from_utf8(output.stdout).unwrap()));
                ev!(format!("{}\n", String::from_utf8(output.stderr).unwrap()));
                return false;
            }
        }
        return true;
    }

    fn handle_or_exit(&mut self, on_error: &str) {
        if self.handle(on_error) == false {
            fs_extra::dir::remove(&*NAME.lock().unwrap()).expect("Failed to remove dir");
            print!("\n");
            exit(1);
        }
    }
}

fn main() {
    let args = Args::parse();

    *VERBOSE.lock().unwrap() = args.verbose;
    *ERRORS.lock().unwrap() = args.errors;



    v!("Reading arguments...\n");
    let name: String = args.url.clone();

    if args.verbose { println!("{:?}", args) }

    v!("Generating name...");
    let name = name.split("/").collect::<Vec<&str>>()[4];
    let name = if args.in_place { format!("temp_{}", name) } else { name.to_string() };
    v!( format!("{}\n", name) );

    *NAME.lock().unwrap() = name.clone();

    let c_dir  = current_dir();
    let c_dir = match c_dir {
        Ok(dir) => dir,
        Err(err) => {
            eprint!("Failed to get current directory.\n{}\n", err);
            exit(1);
        }
    };




    let original_wk_directory: String = format!("{}", c_dir.display());
    let wk_directory = format!("{}/{}", c_dir.display(), name);


    v!( format!("Creating folder '{}'...\n", wk_directory) );
    fs_extra::dir::create(name, args.in_place).expect("Failed to create directory");

    v!( format!("Switching from {} to {}...\n", original_wk_directory, wk_directory) );
    set_current_dir(wk_directory.clone()).expect("Failed to set working directory");

    v!("Running git init...\n");
    let mut command = c("git init");
    command.handle_or_exit("Failed to initialize empty github repo");

    v!( format!("Adding remote origin with url '{}'\n", args.url) );
    let mut command = c(format!("git remote add -f origin {}", args.url));
    command.handle_or_exit("Failed to add remote repo url");

    v!("Enabling sparse checkout...\n");
    let mut command = c("git config core.sparseCheckout true");
    command.handle_or_exit("Failed to write to .git/config");

    v!("Writing files to checkout...\n");
    for folder in args.folders {
        v!( format!("    - {}\n", folder) );
        if let Err(e) =  write_contents_to(
            format!("{}/.git/info/sparse-checkout", wk_directory.clone()).as_str(),
            folder.add("\n").as_bytes(),
        ) {
            eprint!("Error: {}\n", e);
            exit(1);
        }
    }

    v!( format!("Pulling from branch {}... ",  args.branch) );
    let mut command = c(format!("git pull origin {}", args.branch));
    let to_try = if args.branch == "main" { "master" } else { "main" };
    let error_msg = if args.branch == "main" ||args.branch == "master" {
        format!("\nFailed to get {}. Trying with {}... ", args.branch, to_try)
    } else {
        format!("Failed to pull from branch. Is the branch name correct?")
    };
    if command.handle(&error_msg) == false && (args.branch == "main" || args.branch == "master") {
        let mut command = c(format!("git pull origin {}", to_try));
        command.handle_or_exit("Failed to get folders from repo. Check the branch name is correct.");
    }

    v!("Success!\n");

    if args.in_place {
        let options_dir = fs_extra::dir::CopyOptions::new();
        let options_file = fs_extra::file::CopyOptions::new();

        let files = match std::fs::read_dir(".") {
            Ok(c) => c,
            Err(err) => {
                eprintln!("{}", err);
                exit(1);
            }
        };


        for entry in files {
            let e = entry.expect("Failed to read dir files");
            if e.file_name() == ".git" {
                continue;
            };

            if e.file_type().unwrap().is_dir() {
                move_dir(e.path(), original_wk_directory.clone(), &options_dir)
                    .expect("Failed to copy dir");
            } else {
                move_file(
                    e.path(),
                    format!(
                        "{}/{}",
                        original_wk_directory.clone(),
                        e.file_name().into_string().unwrap()
                    ),
                    &options_file,
                )
                .expect("Failed to copy dir");
            }
        }
        fs_extra::dir::remove(wk_directory.clone()).expect("Failed to remove dir");
    }
}
