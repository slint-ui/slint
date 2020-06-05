#[cfg(test)]
mod cpp;
#[cfg(test)]
mod interpreter;

include!(env!("TEST_FUNCTIONS"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
