// Copyright (C) 2017-2023 Smart code 203358507

use std::{env, fs, io::{self, BufRead}};

use chrono::{Utc, Datelike};
use regex::Regex;
use walkdir::WalkDir;

#[test]
fn copyright() {
    let include_dirs = vec!["src", "tests"];
    let project_root = env!("CARGO_MANIFEST_DIR");
    let current_year = Utc::now().year().to_string();
    let regex_pattern = format!(r"^\/\/ Copyright \(C\) 2017-{} Smart code 203358507", regex::escape(&current_year));
    let copyright_regex = Regex::new(&regex_pattern).unwrap();

    for entry in WalkDir::new(project_root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() {
            let parent_dir = path.parent();
            let parent_dir_included = parent_dir
                .and_then(|dir| dir.strip_prefix(project_root).ok())
                .and_then(|relative_dir| {
                    relative_dir.components().next().and_then(|comp| comp.as_os_str().to_str())
                })
                .map(|dir| include_dirs.contains(&dir))
                .unwrap_or(false);

            if parent_dir_included {
                if let Ok(file) = fs::File::open(&path) {
                    let reader = io::BufReader::new(file);
                    if let Some(first_line) = reader.lines().next() {
                        let line = first_line.unwrap();
                        assert_eq!(copyright_regex.is_match(&line), true);
                    }
                }
            }
        }
    }
}