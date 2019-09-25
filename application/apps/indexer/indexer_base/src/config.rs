// Copyright (c) 2019 E.S.R.Labs. All rights reserved.
//
// NOTICE:  All information contained herein is, and remains
// the property of E.S.R.Labs and its suppliers, if any.
// The intellectual and technical concepts contained herein are
// proprietary to E.S.R.Labs and its suppliers and may be covered
// by German and Foreign Patents, patents in process, and are protected
// by trade secret or copyright law.
// Dissemination of this information or reproduction of this material
// is strictly forbidden unless prior written permission is obtained
// from E.S.R.Labs.
use std::path;
use std::fs;

#[derive(Debug)]
pub struct IndexingConfig<'a> {
    pub tag: &'a str,
    pub chunk_size: usize,
    pub in_file: fs::File,
    pub out_path: &'a path::PathBuf,
    pub append: bool,
    pub to_stdout: bool,
}
