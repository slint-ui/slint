use std::error::Error;

mod cpp;

fn main() -> Result<(), Box<dyn Error>> {
    let cpp_driver = cpp::Driver::new()?;

    for testcase in test_driver_lib::collect_test_cases()? {
        cpp_driver.test(&testcase)?;
    }

    Ok(())
}
