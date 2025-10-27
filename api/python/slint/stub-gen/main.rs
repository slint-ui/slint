use pyo3_stub_gen::Result;

fn main() -> Result<()> {
    let stub = core::stub_info()?;
    stub.generate()?;
    Ok(())
}
