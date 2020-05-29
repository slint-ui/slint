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

impl TestCase {
    fn cpp(&self) -> Result<(), Box<dyn Error>> {
        use sixtyfps_compiler::*;

        let (syntax_node, mut diag) = parser::parse(&self.source);
        diag.current_path = self.path.clone();
        let mut tr = typeregister::TypeRegister::builtin();
        let doc = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
        run_passes(&doc, &mut diag, &mut tr);

        let (mut diag, source) = diag.check_and_exit_on_error(self.source.clone());

        let mut generated_cpp: Vec<u8> = Vec::new();

        generator::generate(&mut generated_cpp, &doc.root_component, &mut diag)?;
        diag.check_and_exit_on_error(source);

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    for test in collect_test_cases()? {
        test.cpp()?;
    }

    Ok(())
}
