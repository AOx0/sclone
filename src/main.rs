extern crate execute;
extern crate fs_extra;

use fs_extra::dir::move_dir;

use execute::{command as c, Execute};
use std::process::Command;

use clap::{ColorChoice, Parser};
use fs_extra::file::move_file;
use std::env::{current_dir, set_current_dir};
use std::fs::OpenOptions;
use std::io::Write;
use std::ops::Add;

#[derive(Parser, Debug)]
#[clap(about, version, author, color(ColorChoice::Never))]
struct Args {
    //Url of the github repository
    #[clap(required = true)]
    url: String,

    // Folders to clone from main/
    #[clap(required = true, multiple_values = true)]
    folders: Vec<String>,

    // Name of the branch to clone
    #[clap(short, long, required = false, default_value = "main")]
    branch: String,

    // If you want to store in place cloned files
    #[clap(short, long, takes_value = false)]
    in_place: bool,
}

pub fn write_contents_to(path: &str, contents: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .append(true)
        .open(path)?;

    file.write(contents)?;
    Ok(())
}

trait Handle {
    fn handle(&mut self, on_error: &str) -> bool;
}

impl Handle for Command {
    fn handle(&mut self, on_error: &str) -> bool {
        if let Some(exit_code) = self.execute().unwrap() {
            if !(exit_code == 0) {
                eprintln!("{}", on_error);
                return false;
            }
        } else {
            eprintln!("Interrupted!");
            return false;
        }

        return true;
    }
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let name: String = args.url.clone();
    let name = name.split("/").collect::<Vec<&str>>()[4];
    let name = if args.in_place {
        format!("temp_{}", name)
    } else {
        name.to_string()
    };

    let original_wk_directory: String = format!("{}", current_dir()?.display());
    let wk_directory = format!("{}/{}", current_dir()?.display(), name);

    fs_extra::dir::create(name, args.in_place).expect("Failed to create directory");
    set_current_dir(wk_directory.clone()).expect("Failed to set working directory");

    let mut command = c("git init");
    command.handle("Failed to initialize empty github repo");

    let mut command = c(format!("git remote add -f origin {}", args.url));
    command.handle("Failed to add remote repo url");

    let mut command = c("git config core.sparseCheckout true");
    command.handle("Failed to write to .git/config");

    for folder in args.folders {
        write_contents_to(
            format!("{}/.git/info/sparse-checkout", wk_directory.clone()).as_str(),
            folder.add("\n").as_bytes(),
        )?
    }

    let mut command = c(format!("git pull origin {}", args.branch));
    if command.handle("") == false {
        let mut command = c(format!("git pull origin master"));
        command.handle("Failed to get folders from repo. Check the branch name is correct.");
    }

    if args.in_place {
        let options_dir = fs_extra::dir::CopyOptions::new();
        let options_file = fs_extra::file::CopyOptions::new();

        for entry in std::fs::read_dir(".")? {
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

    Ok(())
}
