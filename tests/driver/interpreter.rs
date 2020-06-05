use std::error::Error;

pub fn test(testcase: &test_driver_lib::TestCase) -> Result<(), Box<dyn Error>> {
    let source = std::fs::read_to_string(&testcase.absolute_path)?;

    let component = match sixtyfps_interpreter::load(source.as_str(), &testcase.absolute_path) {
        Ok(c) => c,
        Err(diag) => {
            let vec = diag.inner.iter().map(|d| d.message.clone()).collect::<Vec<String>>();
            diag.print(source);
            return Err(vec.join("\n").into());
        }
    };

    component.create();

    Ok(())
}
