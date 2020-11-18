use colored::*;
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "nt", about = "The note-taking app")]
enum Nt {
    /// Start writing a new note
    #[structopt(alias = "n")]
    New { filename: Option<String> },
    /// Search existing notes and open in file for editing
    #[structopt(alias = "s")]
    Search { needle: String },
}

enum Action {
    Created,
    Edited,
}

fn print_preview(action: Action, fullname: &std::path::PathBuf) {
    let file = std::fs::File::open(&fullname).expect("File should exist");
    let reader = std::io::BufReader::new(file);
    for line in reader.lines() {
        let action = match action {
            Action::Created => "Created",
            Action::Edited => "Edited",
        };

        println!(
            "{} {} [{}]",
            action,
            fullname
                .file_name()
                .unwrap()
                .to_os_string()
                .into_string()
                .unwrap()
                .bold(),
            line.expect("Line should at least exist").bright_black()
        );
        return;
    }
}

fn main() {
    // Ensure that the dir we're working with exists
    let dir = Path::new(&std::env::var("XDG_CONFIG_HOME").unwrap()).join("nt");
    std::fs::create_dir_all(&dir).unwrap();

    match Nt::from_args() {
        Nt::New { filename } => {
            let now = chrono::Local::now().format("%Y%m%d%H%M%S");
            let filename = match filename {
                Some(x) => format!("{}-{}.md", now, x),
                None => format!("{}.md", now),
            };
            let fullname = dir.join(&filename);
            edit::edit_file(&fullname).unwrap();
            print_preview(Action::Created, &fullname);
        }

        Nt::Search { needle } => {
            // Use rg to search the nt directory
            let rg_bytes = Command::new("rg")
                .arg("--line-number")
                .arg("--no-heading")
                .arg("--fixed-strings")
                .arg(needle)
                .arg(dir.as_path().display().to_string())
                .output()
                .expect("rg to work")
                .stdout;

            // Pipe the results of rg to skim, the fzf library written in Rust
            let bat_cmd = "bat --style=numbers --color=always --highlight-line {2} --line-range";
            let awk_cmd = "(echo {2} | awk '{a=$1-5;if(a<0)a=0;print a}')";
            let preview_cmd = format!("{} {}: {{1}} | head -n10", bat_cmd, awk_cmd);

            let options = skim::prelude::SkimOptionsBuilder::default()
                .height(Some("50%"))
                .delimiter(Some(":"))
                .multi(true)
                .preview(Some(&preview_cmd))
                .build()
                .unwrap();

            // `SkimItemReader` is a helper to turn any `BufRead` into a stream of `SkimItem`
            // `SkimItem` was implemented for `AsRef<str>` by default
            let item_reader = skim::prelude::SkimItemReader::default();
            let items = item_reader.of_bufread(std::io::Cursor::new(rg_bytes));

            // `run_with` would read and show items from the stream
            let selected_items = skim::Skim::run_with(&options, Some(items))
                .map(|out| out.selected_items)
                .unwrap_or_else(|| Vec::new());

            // For each item selected in Skim, open in editor
            for item in selected_items.iter() {
                let output = item.output();
                let split: Vec<&str> = output.splitn(3, ':').collect();
                let fullname = split[0];
                let lineno = split[1];

                let editor = edit::get_editor().expect("Editor should exist");

                // We can open to the correct line in vim
                match editor.as_path().display().to_string().as_ref() {
                    "vim" => {
                        Command::new(&editor)
                            .args(&[format!("+{}", lineno)])
                            .arg(&fullname)
                            .stdin(Stdio::inherit())
                            .stdout(Stdio::inherit())
                            .stderr(Stdio::inherit())
                            .output()
                            .expect("vim should have worked")
                            .status;
                    }
                    x => {
                        edit::edit_file(Path::new(x)).expect("Edit should have worked");
                    }
                }
                let mut path = std::path::PathBuf::new();
                path.push(fullname);
                print_preview(Action::Edited, &path);
            }
        }
    }
}
