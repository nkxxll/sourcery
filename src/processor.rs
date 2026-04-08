use std::{fs::File, io::{BufReader, Read}, path::Path};

use anyhow::Result;

pub struct LinesOfCodeProcessor;

#[derive(Debug)]
pub struct FunctionLocResult {
    pub name: String,
    pub start_line: i32,
    pub end_line: i32,
    pub loc: usize,
}

const CHUNK_SIZE: usize = 1 << 16; // 64KB

impl LinesOfCodeProcessor {
    pub fn lines_of_code_file(path: &Path) -> Result<u64> {
        let mut file = File::open(path)?;
        count_lines_from_reader(&mut file)
    }

    pub fn lines_of_code_content(content: &str) -> Result<u64> {
        let mut reader = BufReader::new(content.as_bytes());
        count_lines_from_reader(&mut reader)
    }
}

fn count_lines_from_reader<R: Read>(reader: &mut R) -> Result<u64> {
    let mut buffer = [0u8; CHUNK_SIZE];
    let mut count = 0;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        count += bytecount::count(&buffer[..bytes_read], b'\n') as u64;
    }

    Ok(count)
}
