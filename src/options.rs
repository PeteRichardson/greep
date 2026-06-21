use std::path::PathBuf;

use clap::Parser;

use crate::filelist::{expand_paths, read_filelist};
use crate::search::{find_algorithm, list_algorithms};

#[derive(Parser, Debug)]
#[command(name = "greep", about = "A simple grep")]
pub struct Args {
    #[arg(short, long)]
    pub verbose: bool,

    #[arg(short, long)]
    pub timing: bool,

    #[arg(short, long, default_value = "bf")]
    pub algorithm: String,

    #[arg(short, long)]
    pub list: bool,

    #[arg(short, long)]
    pub filelist: Option<PathBuf>,

    #[arg(value_name = "STRING")]
    pub search_word: Option<String>,

    #[arg(value_name = "FILES")]
    pub files: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unknown algorithm '{0}'. Run 'greep -l' to see available algorithms.")]
    UnknownAlgorithm(String),
    #[error("cannot combine -f/--filelist with positional file arguments")]
    ConflictingFileArgs,
    #[error("unable to open filelist '{0}': {1}")]
    FilelistOpen(PathBuf, std::io::Error),
}

#[derive(Debug)]
pub struct ResolvedArgs {
    pub verbose: bool,
    pub timing: bool,
    pub algorithm_code: String,
    pub search_word: String,
    pub files: Vec<String>,
}

pub fn print_algorithm_list() {
    for (code, name) in list_algorithms() {
        println!("{code:<6} {name}");
    }
}

pub fn resolve(args: Args, search_word: String) -> Result<ResolvedArgs, AppError> {
    if find_algorithm(&args.algorithm).is_none() {
        return Err(AppError::UnknownAlgorithm(args.algorithm));
    }

    if args.filelist.is_some() && !args.files.is_empty() {
        return Err(AppError::ConflictingFileArgs);
    }

    let initial_files = if let Some(path) = &args.filelist {
        read_filelist(path).map_err(|e| AppError::FilelistOpen(path.clone(), e))?
    } else if !args.files.is_empty() {
        args.files
    } else {
        vec!["/dev/stdin".to_string()]
    };

    Ok(ResolvedArgs {
        verbose: args.verbose,
        timing: args.timing,
        algorithm_code: args.algorithm,
        search_word,
        files: expand_paths(initial_files),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> Args {
        Args {
            verbose: false,
            timing: false,
            algorithm: "bf".to_string(),
            list: false,
            filelist: None,
            search_word: None,
            files: vec![],
        }
    }

    #[test]
    fn unknown_algorithm_errors() {
        let mut args = base_args();
        args.algorithm = "nope".to_string();
        let err = resolve(args, "word".to_string()).unwrap_err();
        assert!(matches!(err, AppError::UnknownAlgorithm(code) if code == "nope"));
    }

    #[test]
    fn filelist_and_positional_files_conflict() {
        let mut args = base_args();
        args.filelist = Some(PathBuf::from("/tmp/somelist"));
        args.files = vec!["a.txt".to_string()];
        let err = resolve(args, "word".to_string()).unwrap_err();
        assert!(matches!(err, AppError::ConflictingFileArgs));
    }

    #[test]
    fn defaults_to_stdin_when_no_files_given() {
        let args = base_args();
        let resolved = resolve(args, "word".to_string()).unwrap();
        assert_eq!(resolved.files, vec!["/dev/stdin".to_string()]);
    }

    #[test]
    fn uses_positional_files_when_given() {
        let mut args = base_args();
        args.files = vec!["a.txt".to_string(), "b.txt".to_string()];
        let resolved = resolve(args, "word".to_string()).unwrap();
        assert_eq!(resolved.files, vec!["a.txt".to_string(), "b.txt".to_string()]);
    }
}
