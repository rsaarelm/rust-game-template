use std::{
    error::Error,
    fs::{self, File},
    io::{self, prelude::*},
    path::Path,
};

/// Dump a directory tree into a single IDM expression.
pub fn directory_to_idm(
    path: impl AsRef<Path>,
) -> Result<String, Box<dyn Error>> {
    use std::fmt::Write;

    // If pointed at a file, just read the file.
    if path.as_ref().is_file() {
        return Ok(fs::read_to_string(path)?);
    }

    let mut ret = String::new();
    for e in walkdir::WalkDir::new(path) {
        let e = e.expect("read_path failed");
        let depth = e.depth();
        if depth == 0 {
            // The root element, do not print out.
            continue;
        }
        for _ in 1..depth {
            write!(ret, "  ")?;
        }
        let is_dir = e.file_type().is_dir();
        if is_dir {
            writeln!(ret, "{}", e.file_name().to_string_lossy())?;
        } else {
            let path = Path::new(e.file_name());

            if !matches!(
                path.extension()
                    .map(|a| a.to_str().unwrap_or(""))
                    .unwrap_or(""),
                "idm"
            ) {
                // Only read IDM files.
                continue;
            }

            let name = path
                .file_stem()
                .expect("read_path failed")
                .to_string_lossy();
            writeln!(ret, "{}", name)?;

            // Print lines
            let file =
                File::open(e.path()).expect("read_path: Open file failed");
            for line in io::BufReader::new(file).lines() {
                let line = line.expect("read_path failed");
                let mut ln = &line[..];
                let mut depth = depth;
                // Turn tab indentation into spaces.
                while ln.starts_with('\t') {
                    depth += 1;
                    ln = &ln[1..];
                }
                for _ in 1..(depth + 1) {
                    write!(ret, "  ")?;
                }
                writeln!(ret, "{ln}")?;
            }
        }
    }

    Ok(ret)
}
