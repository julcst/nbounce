use std::path::PathBuf;

pub fn search_files(path: &str, ext: &str) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|f| f.extension().map_or(false, |x| x == ext))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}