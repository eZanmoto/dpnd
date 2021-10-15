// Copyright 2021 Sean Kelleher. All rights reserved.
// Use of this source code is governed by an MIT
// licence that can be found in the LICENCE file.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;

pub enum Node<'a> {
    AnyDir,
    AnyFile,
    Dir(HashMap<&'a str, Node<'a>>),
    File(&'a str),
}

pub fn assert_contents<'a>(path: &str, exp: &Node<'a>) {
    match exp {
        Node::File(exp_conts) => {
            let act_conts =
                fs::read(&path)
                    .unwrap_or_else(|_| panic!(
                        "couldn't open '{}' as a file",
                        &path,
                    ));

            assert!(
                exp_conts.as_bytes().to_vec() == act_conts,
                "'{}' contained unexpected data, expected:\n{}",
                &path,
                exp_conts,
            );
        }
        Node::AnyDir => {
            let md = fs::metadata(&path)
                .unwrap_or_else(|_| panic!(
                    "couldn't get metadata for '{}'",
                    path,
                ));
            assert!(md.is_dir());
        }
        Node::AnyFile => {
            let md = fs::metadata(&path)
                .unwrap_or_else(|_| panic!(
                    "couldn't get metadata for '{}'",
                    path,
                ));
            assert!(md.is_file());
        }
        Node::Dir(exp_entries) => {
            let act_entries =
                fs::read_dir(&path)
                    .unwrap_or_else(|_| panic!(
                        "couldn't open '{}' as a directory",
                        &path,
                    ));

            let mut act_entry_names: HashSet<String> = HashSet::new();
            for act_entry in act_entries {
                let entry =
                    act_entry
                        .unwrap_or_else(|_| panic!(
                            "couldn't get entry from '{}'",
                            &path,
                        ));

                let entry_name: String =
                    entry
                        .file_name()
                        .into_string()
                        .expect("couldn't read entry's file name as Unicode");

                let exp_entry =
                    exp_entries
                        .get::<str>(&entry_name)
                        .unwrap_or_else(|| panic!(
                            "found unexpected entry '{}/{}' in filesystem",
                            path,
                            entry_name,
                        ));

                assert_contents(
                    &format!("{}/{}", path, entry_name),
                    exp_entry,
                );

                act_entry_names.insert(entry_name);
            }

            for exp_entry_name in exp_entries.keys() {
                assert!(
                    act_entry_names.contains::<str>(exp_entry_name),
                    "couldn't find expected entry '{}/{}' in filesystem",
                    path,
                    exp_entry_name,
                );
            }
        }
    }
}
