use std::error::Error;

pub struct TestCase {
    pub source: String,
    pub path: std::path::PathBuf,
}

pub fn collect_test_cases() -> std::io::Result<Vec<TestCase>> {
    let mut results = vec![];

    for entry in std::fs::read_dir(format!("{}/../cases", env!("CARGO_MANIFEST_DIR")))? {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "60" {
                let source = std::fs::read_to_string(&path)?;
                results.push(TestCase { source, path });
            }
        }
    }

    Ok(results)
}

mod cpp;

fn main() -> Result<(), Box<dyn Error>> {
    let cpp_driver = cpp::Driver::new()?;

    for testcase in collect_test_cases()? {
        cpp_driver.test(&testcase)?;
    }

    Ok(())
}
