use std::error::Error;

pub fn test(testcase: &super::TestCase) -> Result<(), Box<dyn Error>> {
    use sixtyfps_compilerlib::*;

    let (syntax_node, mut diag) = parser::parse(&testcase.source);
    diag.current_path = testcase.path.clone();
    let mut tr = typeregister::TypeRegister::builtin();
    let doc = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    run_passes(&doc, &mut diag, &mut tr);

    let (mut diag, source) = diag.check_and_exit_on_error(testcase.source.clone());

    let mut generated_cpp: Vec<u8> = Vec::new();

    generator::generate(&mut generated_cpp, &doc.root_component, &mut diag)?;
    diag.check_and_exit_on_error(source);

    Ok(())
}
